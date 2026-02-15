use gbe_state_store::StateStoreError;
use gbe_transport::TransportError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SweeperError {
    #[error("transport: {0}")]
    Transport(#[from] TransportError),

    #[error("state store: {0}")]
    StateStore(#[from] StateStoreError),

    #[error("lock: {0}")]
    Lock(String),

    #[error("{0}")]
    Other(String),
}
