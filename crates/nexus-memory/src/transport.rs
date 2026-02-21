use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use gbe_nexus::{
    Envelope, MessageHandler, PublishOpts, StartPosition, StreamConfig, SubscribeOpts,
    TransportError,
};

use crate::consumer::{ConsumerParams, run_consumer_loop};
use crate::store::{ConsumerGroup, SharedStore, StreamStore};
use crate::subscription::MemorySubscription;

#[derive(Debug, Clone)]
pub struct MemoryTransportConfig {
    pub max_payload_size: usize,
}

impl Default for MemoryTransportConfig {
    fn default() -> Self {
        Self {
            max_payload_size: 1_048_576, // 1MB
        }
    }
}

pub struct MemoryTransport {
    store: SharedStore,
    config: MemoryTransportConfig,
    closed: AtomicBool,
}

impl MemoryTransport {
    #[must_use]
    pub fn new(config: MemoryTransportConfig) -> Self {
        Self {
            store: Arc::new(Mutex::new(StreamStore::new())),
            config,
            closed: AtomicBool::new(false),
        }
    }

    fn check_closed(&self) -> Result<(), TransportError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(TransportError::Other("transport is closed".to_string()));
        }
        Ok(())
    }
}

#[async_trait]
impl gbe_nexus::Transport for MemoryTransport {
    async fn publish(
        &self,
        subject: &str,
        payload: Bytes,
        opts: Option<PublishOpts>,
    ) -> Result<String, TransportError> {
        self.check_closed()?;

        if payload.len() > self.config.max_payload_size {
            return Err(TransportError::PayloadTooLarge {
                size: payload.len(),
                max: self.config.max_payload_size,
            });
        }

        let trace_id = opts.and_then(|o| o.trace_id);
        let envelope = Envelope::new(subject.to_string(), payload, trace_id);
        let message_id = envelope.message_id.clone();

        let mut store = self.store.lock().await;
        let stream = store.get_or_create_stream(subject);
        let idx = stream.messages.len();
        stream.id_index.insert(message_id.clone(), idx);
        stream.messages.push(envelope);
        stream.notify.notify_waiters();

        Ok(message_id)
    }

    async fn subscribe(
        &self,
        subject: &str,
        group: &str,
        handler: Box<dyn MessageHandler>,
        opts: Option<SubscribeOpts>,
    ) -> Result<Box<dyn gbe_nexus::Subscription>, TransportError> {
        self.check_closed()?;

        let opts = opts.unwrap_or_default();
        let token = CancellationToken::new();
        let active = Arc::new(AtomicBool::new(true));

        let notify = {
            let mut store = self.store.lock().await;
            let stream = store.get_or_create_stream(subject);

            // Set cursor based on StartPosition
            let cursor = match &opts.start_from {
                StartPosition::Latest => {
                    if stream.messages.is_empty() {
                        None
                    } else {
                        Some(stream.messages.len() - 1)
                    }
                }
                StartPosition::Earliest => None,
                StartPosition::Timestamp(ts) => {
                    // Find the last message index before this timestamp
                    stream
                        .messages
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, env)| env.timestamp < *ts)
                        .map(|(i, _)| i)
                }
                StartPosition::Id(id) => stream.id_index.get(id).copied(),
            };

            stream
                .groups
                .entry(group.to_string())
                .or_insert_with(|| ConsumerGroup::new(cursor));

            stream.notify.clone()
        };

        tokio::spawn(run_consumer_loop(ConsumerParams {
            store: self.store.clone(),
            subject: subject.to_string(),
            group: group.to_string(),
            handler,
            opts,
            token: token.clone(),
            active: active.clone(),
            notify,
        }));

        Ok(Box::new(MemorySubscription { token, active }))
    }

    async fn ensure_stream(&self, config: StreamConfig) -> Result<(), TransportError> {
        self.check_closed()?;

        let mut store = self.store.lock().await;
        let stream = store.get_or_create_stream(&config.subject);
        stream.config = Some(config);
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)] // millis since epoch fits in u64 until year 584556
    async fn trim_stream(&self, subject: &str, max_age: Duration) -> Result<u64, TransportError> {
        self.check_closed()?;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let cutoff_ms = now_ms.saturating_sub(max_age.as_millis() as u64);

        let mut store = self.store.lock().await;
        let Some(stream) = store.streams.get_mut(subject) else {
            return Ok(0);
        };

        // Count messages at the front that are expired
        let expired_count = stream
            .messages
            .iter()
            .take_while(|env| env.timestamp <= cutoff_ms)
            .count();

        if expired_count == 0 {
            return Ok(0);
        }

        // Remove expired IDs from index and group pending sets
        let expired_ids: Vec<String> = stream.messages[..expired_count]
            .iter()
            .map(|env| env.message_id.clone())
            .collect();

        for id in &expired_ids {
            stream.id_index.remove(id);
            for group in stream.groups.values_mut() {
                group.pending.remove(id);
            }
        }

        // Remove the expired messages from the front
        stream.messages.drain(..expired_count);

        // Rebuild id_index since indices shifted
        stream.id_index.clear();
        for (i, env) in stream.messages.iter().enumerate() {
            stream.id_index.insert(env.message_id.clone(), i);
        }

        // Adjust group cursors
        for group in stream.groups.values_mut() {
            if let Some(cursor) = &mut group.cursor {
                if *cursor < expired_count {
                    group.cursor = None;
                } else {
                    *cursor -= expired_count;
                }
            }
        }

        Ok(expired_count as u64)
    }

    async fn ping(&self) -> Result<bool, TransportError> {
        Ok(true)
    }

    async fn close(&self) -> Result<(), TransportError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
}
