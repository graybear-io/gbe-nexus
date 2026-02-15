use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateStoreError {
    #[error("connection failed: {0}")]
    Connection(String),

    #[error("key not found: {0}")]
    NotFound(String),

    #[error("compare-and-swap failed: field {field} expected {expected:?}")]
    CasFailed { field: String, expected: Vec<u8> },

    #[error("{0}")]
    Other(String),
}
