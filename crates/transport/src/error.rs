use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("connection failed: {0}")]
    Connection(String),

    #[error("payload exceeds max size ({size} > {max})")]
    PayloadTooLarge { size: usize, max: usize },

    #[error("publish failed: {0}")]
    Publish(String),

    #[error("subscribe failed: {0}")]
    Subscribe(String),

    #[error("stream operation failed: {0}")]
    Stream(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}
