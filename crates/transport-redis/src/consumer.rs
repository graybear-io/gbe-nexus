use redis::streams::StreamReadReply;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use gbe_transport::{Envelope, MessageHandler, SubscribeOpts, TransportError};

use crate::error::map_redis_err;
use crate::message::RedisMessage;

pub(crate) struct ConsumerParams {
    pub conn: redis::aio::ConnectionManager,
    pub stream_key: String,
    pub group: String,
    pub consumer_id: String,
    pub handler: Box<dyn MessageHandler>,
    pub opts: SubscribeOpts,
    pub token: CancellationToken,
    pub active: Arc<AtomicBool>,
}

pub(crate) async fn run_consumer_loop(mut p: ConsumerParams) {
    let start_id = match &p.opts.start_from {
        gbe_transport::StartPosition::Latest => "$".to_string(),
        gbe_transport::StartPosition::Earliest => "0".to_string(),
        gbe_transport::StartPosition::Id(id) => id.clone(),
        gbe_transport::StartPosition::Timestamp(ts) => format!("{ts}-0"),
    };

    if let Err(e) = create_group(&mut p.conn, &p.stream_key, &p.group, &start_id).await {
        tracing::error!(stream = %p.stream_key, group = %p.group, "failed to create consumer group: {e}");
        p.active.store(false, Ordering::Release);
        return;
    }

    let ack_timeout_ms = p.opts.ack_timeout.as_millis() as u64;
    let mut last_reclaim = Instant::now();
    let reclaim_interval = p.opts.ack_timeout / 2;

    loop {
        if p.token.is_cancelled() {
            break;
        }

        // Backpressure: check pending count
        if let Ok(pending) = get_pending_count(&mut p.conn, &p.stream_key, &p.group).await
            && pending >= p.opts.max_inflight as usize
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }

        // Phase 1: Reclaim timed-out messages periodically
        if last_reclaim.elapsed() >= reclaim_interval {
            process_reclaimed(
                &mut p.conn,
                &p.stream_key,
                &p.group,
                &p.consumer_id,
                ack_timeout_ms,
                p.opts.batch_size,
                &*p.handler,
            )
            .await;
            last_reclaim = Instant::now();
        }

        // Phase 2: Read new messages
        let result: Result<StreamReadReply, _> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&p.group)
            .arg(&p.consumer_id)
            .arg("COUNT")
            .arg(p.opts.batch_size)
            .arg("BLOCK")
            .arg(2000_u64)
            .arg("STREAMS")
            .arg(&p.stream_key)
            .arg(">")
            .query_async(&mut p.conn)
            .await;

        match result {
            Ok(reply) => {
                for key in &reply.keys {
                    for entry in &key.ids {
                        process_entry(
                            &p.conn,
                            &p.stream_key,
                            &p.group,
                            &entry.id,
                            entry,
                            &*p.handler,
                        )
                        .await;
                    }
                }
            }
            Err(e) => {
                // XREADGROUP returns nil (not an error) on timeout with no messages.
                // Actual errors get a backoff.
                if !is_timeout_nil(&e) {
                    tracing::warn!(stream = %p.stream_key, "XREADGROUP error: {e}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    p.active.store(false, Ordering::Release);
    tracing::debug!(stream = %p.stream_key, group = %p.group, "consumer loop exited");
}

async fn create_group(
    conn: &mut redis::aio::ConnectionManager,
    stream_key: &str,
    group: &str,
    start_id: &str,
) -> Result<(), TransportError> {
    let result: Result<String, redis::RedisError> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(stream_key)
        .arg(group)
        .arg(start_id)
        .arg("MKSTREAM")
        .query_async(conn)
        .await;

    match result {
        Ok(_) => Ok(()),
        Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
        Err(e) => Err(map_redis_err(e)),
    }
}

async fn get_pending_count(
    conn: &mut redis::aio::ConnectionManager,
    stream_key: &str,
    group: &str,
) -> Result<usize, redis::RedisError> {
    let result: redis::Value = redis::cmd("XPENDING")
        .arg(stream_key)
        .arg(group)
        .query_async(conn)
        .await?;

    if let redis::Value::Array(arr) = &result
        && let Some(redis::Value::Int(count)) = arr.first()
    {
        return Ok(*count as usize);
    }
    Ok(0)
}

async fn process_reclaimed(
    conn: &mut redis::aio::ConnectionManager,
    stream_key: &str,
    group: &str,
    consumer_id: &str,
    min_idle_ms: u64,
    count: u32,
    handler: &dyn MessageHandler,
) {
    let result: Result<redis::Value, _> = redis::cmd("XAUTOCLAIM")
        .arg(stream_key)
        .arg(group)
        .arg(consumer_id)
        .arg(min_idle_ms)
        .arg("0-0")
        .arg("COUNT")
        .arg(count)
        .query_async(conn)
        .await;

    // XAUTOCLAIM returns: [next-start-id, [[id, [field, value, ...]], ...], deleted-ids]
    if let Ok(redis::Value::Array(parts)) = &result
        && parts.len() >= 2
        && let redis::Value::Array(entries) = &parts[1]
    {
        for entry_val in entries {
            if let Some((entry_id, fields)) = parse_stream_entry(entry_val)
                && let Some(envelope_json) = fields.get("envelope")
            {
                match serde_json::from_str::<Envelope>(envelope_json) {
                    Ok(envelope) => {
                        let msg = RedisMessage {
                            envelope,
                            stream_key: stream_key.to_string(),
                            group: group.to_string(),
                            entry_id: entry_id.clone(),
                            conn: conn.clone(),
                            acked: AtomicBool::new(false),
                        };
                        let _ = handler.handle(&msg).await;
                    }
                    Err(e) => {
                        tracing::warn!(
                            entry_id = %entry_id,
                            "failed to deserialize reclaimed envelope: {e}"
                        );
                    }
                }
            }
        }
    }
}

async fn process_entry(
    conn: &redis::aio::ConnectionManager,
    stream_key: &str,
    group: &str,
    entry_id: &str,
    entry: &redis::streams::StreamId,
    handler: &dyn MessageHandler,
) {
    let envelope_json: Option<String> = entry.get("envelope");
    let Some(json) = envelope_json else {
        tracing::warn!(entry_id = %entry_id, "stream entry missing 'envelope' field");
        return;
    };

    match serde_json::from_str::<Envelope>(&json) {
        Ok(envelope) => {
            let msg = RedisMessage {
                envelope,
                stream_key: stream_key.to_string(),
                group: group.to_string(),
                entry_id: entry_id.to_string(),
                conn: conn.clone(),
                acked: AtomicBool::new(false),
            };
            if let Err(e) = handler.handle(&msg).await {
                tracing::debug!(
                    entry_id = %entry_id,
                    "handler returned error (claim-based nak): {e}"
                );
            }
        }
        Err(e) => {
            tracing::warn!(entry_id = %entry_id, "failed to deserialize envelope: {e}");
        }
    }
}

/// Parse a raw Redis Value into (entry_id, field_map) for XAUTOCLAIM results.
fn parse_stream_entry(
    val: &redis::Value,
) -> Option<(String, std::collections::HashMap<String, String>)> {
    if let redis::Value::Array(parts) = val
        && parts.len() >= 2
    {
        let id = match &parts[0] {
            redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
            _ => return None,
        };
        let mut fields = std::collections::HashMap::new();
        if let redis::Value::Array(kv_pairs) = &parts[1] {
            for chunk in kv_pairs.chunks(2) {
                if let (Some(redis::Value::BulkString(k)), Some(redis::Value::BulkString(v))) =
                    (chunk.first(), chunk.get(1))
                {
                    fields.insert(
                        String::from_utf8_lossy(k).to_string(),
                        String::from_utf8_lossy(v).to_string(),
                    );
                }
            }
        }
        return Some((id, fields));
    }
    None
}

/// Check if a redis error is the "nil" response from a blocking XREADGROUP timeout.
fn is_timeout_nil(e: &redis::RedisError) -> bool {
    matches!(e.kind(), redis::ErrorKind::TypeError)
}
