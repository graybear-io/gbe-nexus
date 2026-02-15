use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_util::sync::CancellationToken;

use gbe_transport::{
    Envelope, MessageHandler, PublishOpts, StreamConfig, SubscribeOpts, TransportError,
};

use crate::config::RedisTransportConfig;
use crate::consumer::{ConsumerParams, run_consumer_loop};
use crate::error::map_redis_err;
use crate::subject::subject_to_key;
use crate::subscription::RedisSubscription;

pub struct RedisTransport {
    conn: redis::aio::ConnectionManager,
    config: RedisTransportConfig,
    closed: AtomicBool,
}

impl RedisTransport {
    pub async fn connect(config: RedisTransportConfig) -> Result<Self, TransportError> {
        let client = redis::Client::open(config.url.as_str())
            .map_err(|e| TransportError::Connection(e.to_string()))?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))?;
        Ok(Self {
            conn,
            config,
            closed: AtomicBool::new(false),
        })
    }

    fn check_closed(&self) -> Result<(), TransportError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(TransportError::Other("transport is closed".to_string()));
        }
        Ok(())
    }

    fn consumer_id() -> String {
        let host = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        format!("{host}-{}", ulid::Ulid::new())
    }
}

#[async_trait]
impl gbe_transport::Transport for RedisTransport {
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

        let json = serde_json::to_string(&envelope).map_err(TransportError::Serialization)?;
        let key = subject_to_key(subject);

        let mut conn = self.conn.clone();
        redis::cmd("XADD")
            .arg(&key)
            .arg("*")
            .arg("envelope")
            .arg(&json)
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| TransportError::Publish(e.to_string()))?;

        Ok(message_id)
    }

    async fn subscribe(
        &self,
        subject: &str,
        group: &str,
        handler: Box<dyn MessageHandler>,
        opts: Option<SubscribeOpts>,
    ) -> Result<Box<dyn gbe_transport::Subscription>, TransportError> {
        self.check_closed()?;

        let opts = opts.unwrap_or_default();
        let stream_key = subject_to_key(subject);
        let consumer_id = Self::consumer_id();
        let token = CancellationToken::new();
        let active = Arc::new(AtomicBool::new(true));

        tokio::spawn(run_consumer_loop(ConsumerParams {
            conn: self.conn.clone(),
            stream_key,
            group: group.to_string(),
            consumer_id,
            handler,
            opts,
            token: token.clone(),
            active: active.clone(),
        }));

        Ok(Box::new(RedisSubscription { token, active }))
    }

    async fn ensure_stream(&self, config: StreamConfig) -> Result<(), TransportError> {
        self.check_closed()?;

        let key = subject_to_key(&config.subject);
        let mut conn = self.conn.clone();

        // XGROUP CREATE with MKSTREAM ensures the stream exists.
        // Use a sentinel group name; real groups are created by subscribe.
        let result: Result<String, redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&key)
            .arg("_init")
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
            Err(e) => Err(TransportError::Stream(e.to_string())),
        }
    }

    async fn ping(&self) -> Result<bool, TransportError> {
        let mut conn = self.conn.clone();
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(map_redis_err)?;
        Ok(pong == "PONG")
    }

    async fn close(&self) -> Result<(), TransportError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
}
