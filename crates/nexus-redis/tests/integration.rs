//! Integration tests for the Redis Streams transport backend.
//!
//! Requires a running Redis instance. Set REDIS_URL to enable these tests.
//! Default: redis://127.0.0.1:6379
//!
//! Run with: REDIS_URL=redis://localhost:6379 cargo test --package gbe-nexus-redis

use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

use gbe_nexus::{
    Message, MessageHandler, StartPosition, StreamConfig, SubscribeOpts, Transport, TransportError,
};
use gbe_nexus_redis::{RedisTransport, RedisTransportConfig};

fn redis_url() -> Option<String> {
    std::env::var("REDIS_URL").ok()
}

async fn connect() -> RedisTransport {
    let url = redis_url().expect("REDIS_URL must be set for integration tests");
    RedisTransport::connect(RedisTransportConfig {
        url,
        ..Default::default()
    })
    .await
    .expect("failed to connect to Redis")
}

/// Generate a unique subject per test to avoid interference.
fn test_subject(name: &str) -> String {
    format!(
        "gbe.test.{}.{}",
        name,
        ulid::Ulid::new().to_string().to_lowercase()
    )
}

/// Clean up a Redis stream key after a test.
async fn cleanup_stream(subject: &str) {
    let url = redis_url().unwrap();
    let client = redis::Client::open(url.as_str()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let key = subject.replace('.', ":");
    let _: Result<(), _> = redis::cmd("DEL").arg(&key).query_async(&mut conn).await;
    // Also clean up dead letter stream
    let dl_key = "gbe:_deadletter:test".to_string();
    let _: Result<(), _> = redis::cmd("DEL").arg(&dl_key).query_async(&mut conn).await;
}

// --- Handlers ---

/// Handler that records received payloads and notifies a waiter.
struct CollectingHandler {
    payloads: Arc<tokio::sync::Mutex<Vec<Bytes>>>,
    notify: Arc<Notify>,
}

#[async_trait]
impl MessageHandler for CollectingHandler {
    async fn handle(&self, msg: &dyn Message) -> Result<(), TransportError> {
        self.payloads.lock().await.push(msg.payload().clone());
        msg.ack().await?;
        self.notify.notify_one();
        Ok(())
    }
}

/// Handler that dead-letters every message.
struct DeadLetterHandler {
    notify: Arc<Notify>,
}

#[async_trait]
impl MessageHandler for DeadLetterHandler {
    async fn handle(&self, msg: &dyn Message) -> Result<(), TransportError> {
        msg.dead_letter("test: forced dead letter").await?;
        self.notify.notify_one();
        Ok(())
    }
}

// --- Tests ---

#[tokio::test]
async fn test_ping() {
    if redis_url().is_none() {
        return;
    }
    let transport = connect().await;
    assert!(transport.ping().await.unwrap());
}

#[tokio::test]
async fn test_ensure_stream_idempotent() {
    if redis_url().is_none() {
        return;
    }
    let transport = connect().await;
    let subject = test_subject("ensure");

    let config = StreamConfig {
        subject: subject.clone(),
        max_age: Duration::from_secs(3600),
        max_bytes: None,
        max_msgs: None,
    };

    // Call twice — should succeed both times
    transport.ensure_stream(config.clone()).await.unwrap();
    transport.ensure_stream(config).await.unwrap();

    cleanup_stream(&subject).await;
}

#[tokio::test]
async fn test_publish_returns_message_id() {
    if redis_url().is_none() {
        return;
    }
    let transport = connect().await;
    let subject = test_subject("pubid");

    transport
        .ensure_stream(StreamConfig {
            subject: subject.clone(),
            max_age: Duration::from_secs(3600),
            max_bytes: None,
            max_msgs: None,
        })
        .await
        .unwrap();

    let id = transport
        .publish(&subject, Bytes::from("hello"), None)
        .await
        .unwrap();

    // Message ID should be a valid ULID (26 chars)
    assert_eq!(id.len(), 26);

    cleanup_stream(&subject).await;
}

#[tokio::test]
async fn test_payload_too_large() {
    if redis_url().is_none() {
        return;
    }
    let transport = RedisTransport::connect(RedisTransportConfig {
        url: redis_url().unwrap(),
        max_payload_size: 100,
    })
    .await
    .unwrap();

    let subject = test_subject("large");
    let big_payload = Bytes::from(vec![0u8; 200]);

    let result = transport.publish(&subject, big_payload, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(
            err,
            TransportError::PayloadTooLarge {
                size: 200,
                max: 100
            }
        ),
        "expected PayloadTooLarge, got: {err:?}"
    );
}

#[tokio::test]
async fn test_publish_subscribe_roundtrip() {
    if redis_url().is_none() {
        return;
    }
    let transport = connect().await;
    let subject = test_subject("roundtrip");

    transport
        .ensure_stream(StreamConfig {
            subject: subject.clone(),
            max_age: Duration::from_secs(3600),
            max_bytes: None,
            max_msgs: None,
        })
        .await
        .unwrap();

    let payloads: Arc<tokio::sync::Mutex<Vec<Bytes>>> = Arc::new(tokio::sync::Mutex::new(vec![]));
    let notify = Arc::new(Notify::new());

    let handler = CollectingHandler {
        payloads: payloads.clone(),
        notify: notify.clone(),
    };

    let sub = transport
        .subscribe(
            &subject,
            "test-group",
            Box::new(handler),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ack_timeout: Duration::from_secs(5),
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    // Give the consumer loop a moment to start
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Publish 3 messages
    for i in 0..3 {
        transport
            .publish(&subject, Bytes::from(format!("msg-{i}")), None)
            .await
            .unwrap();
    }

    // Wait for all 3 messages
    for _ in 0..3 {
        tokio::time::timeout(Duration::from_secs(5), notify.notified())
            .await
            .expect("timed out waiting for message");
    }

    let received = payloads.lock().await;
    assert_eq!(received.len(), 3);
    assert_eq!(received[0], Bytes::from("msg-0"));
    assert_eq!(received[1], Bytes::from("msg-1"));
    assert_eq!(received[2], Bytes::from("msg-2"));

    assert!(sub.is_active());
    sub.unsubscribe().await.unwrap();

    cleanup_stream(&subject).await;
}

#[tokio::test]
async fn test_dead_letter_routing() {
    if redis_url().is_none() {
        return;
    }
    let transport = connect().await;
    let subject = test_subject("deadletter");

    transport
        .ensure_stream(StreamConfig {
            subject: subject.clone(),
            max_age: Duration::from_secs(3600),
            max_bytes: None,
            max_msgs: None,
        })
        .await
        .unwrap();

    let notify = Arc::new(Notify::new());
    let handler = DeadLetterHandler {
        notify: notify.clone(),
    };

    let sub = transport
        .subscribe(
            &subject,
            "dl-group",
            Box::new(handler),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ack_timeout: Duration::from_secs(5),
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    transport
        .publish(&subject, Bytes::from("doomed"), None)
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(5), notify.notified())
        .await
        .expect("timed out waiting for dead letter handler");

    // Verify the dead letter stream has an entry
    let url = redis_url().unwrap();
    let client = redis::Client::open(url.as_str()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let dl_key = "gbe:_deadletter:test";
    let len: i64 = redis::cmd("XLEN")
        .arg(dl_key)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(len >= 1, "expected at least 1 dead letter entry, got {len}");

    sub.unsubscribe().await.unwrap();
    cleanup_stream(&subject).await;
}

#[tokio::test]
async fn test_close_prevents_operations() {
    if redis_url().is_none() {
        return;
    }
    let transport = connect().await;
    transport.close().await.unwrap();

    let result = transport
        .publish("gbe.test.closed", Bytes::from("nope"), None)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_trace_id_propagation() {
    if redis_url().is_none() {
        return;
    }
    let transport = connect().await;
    let subject = test_subject("trace");

    transport
        .ensure_stream(StreamConfig {
            subject: subject.clone(),
            max_age: Duration::from_secs(3600),
            max_bytes: None,
            max_msgs: None,
        })
        .await
        .unwrap();

    let trace_ids: Arc<tokio::sync::Mutex<Vec<Option<String>>>> =
        Arc::new(tokio::sync::Mutex::new(vec![]));
    let notify = Arc::new(Notify::new());

    let trace_ids_clone = trace_ids.clone();
    let notify_clone = notify.clone();

    struct TraceHandler {
        trace_ids: Arc<tokio::sync::Mutex<Vec<Option<String>>>>,
        notify: Arc<Notify>,
    }

    #[async_trait]
    impl MessageHandler for TraceHandler {
        async fn handle(&self, msg: &dyn Message) -> Result<(), TransportError> {
            self.trace_ids
                .lock()
                .await
                .push(msg.envelope().trace_id.clone());
            msg.ack().await?;
            self.notify.notify_one();
            Ok(())
        }
    }

    let sub = transport
        .subscribe(
            &subject,
            "trace-group",
            Box::new(TraceHandler {
                trace_ids: trace_ids_clone,
                notify: notify_clone,
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ack_timeout: Duration::from_secs(5),
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    transport
        .publish(
            &subject,
            Bytes::from("traced"),
            Some(gbe_nexus::PublishOpts {
                trace_id: Some("abc-123-trace".to_string()),
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(5), notify.notified())
        .await
        .expect("timed out waiting for traced message");

    let ids = trace_ids.lock().await;
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], Some("abc-123-trace".to_string()));

    sub.unsubscribe().await.unwrap();
    cleanup_stream(&subject).await;
}
