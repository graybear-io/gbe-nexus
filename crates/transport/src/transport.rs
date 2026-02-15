use async_trait::async_trait;
use bytes::Bytes;
use std::time::Duration;

use crate::envelope::Envelope;
use crate::error::TransportError;

/// Core transport trait. Created once, shared across the application.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn publish(
        &self,
        subject: &str,
        payload: Bytes,
        opts: Option<PublishOpts>,
    ) -> Result<String, TransportError>;

    async fn subscribe(
        &self,
        subject: &str,
        group: &str,
        handler: Box<dyn MessageHandler>,
        opts: Option<SubscribeOpts>,
    ) -> Result<Box<dyn Subscription>, TransportError>;

    async fn ensure_stream(&self, config: StreamConfig) -> Result<(), TransportError>;

    /// Trim entries older than `max_age` from the stream.
    /// Returns the number of entries removed. No-op for backends with native retention.
    async fn trim_stream(&self, subject: &str, max_age: Duration) -> Result<u64, TransportError>;

    async fn ping(&self) -> Result<bool, TransportError>;

    async fn close(&self) -> Result<(), TransportError>;
}

/// What the message handler receives.
#[async_trait]
pub trait Message: Send + Sync {
    fn envelope(&self) -> &Envelope;
    fn payload(&self) -> &Bytes;
    async fn ack(&self) -> Result<(), TransportError>;
    async fn nak(&self, delay: Option<Duration>) -> Result<(), TransportError>;
    async fn dead_letter(&self, reason: &str) -> Result<(), TransportError>;
}

/// Callback for processing messages.
#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle(&self, msg: &dyn Message) -> Result<(), TransportError>;
}

/// Handle returned by subscribe.
#[async_trait]
pub trait Subscription: Send + Sync {
    async fn unsubscribe(&self) -> Result<(), TransportError>;
    fn is_active(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub url: String,
    pub max_payload_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_payload_size: 1_048_576, // 1MB
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PublishOpts {
    pub trace_id: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubscribeOpts {
    pub batch_size: u32,
    pub max_inflight: u32,
    pub ack_timeout: Duration,
    pub start_from: StartPosition,
}

impl Default for SubscribeOpts {
    fn default() -> Self {
        Self {
            batch_size: 10,
            max_inflight: 100,
            ack_timeout: Duration::from_secs(30),
            start_from: StartPosition::Latest,
        }
    }
}

#[derive(Debug, Clone)]
pub enum StartPosition {
    Latest,
    Earliest,
    Timestamp(u64),
    Id(String),
}

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub subject: String,
    pub max_age: Duration,
    pub max_bytes: Option<u64>,
    pub max_msgs: Option<u64>,
}
