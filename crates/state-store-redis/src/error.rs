use gbe_state_store::StateStoreError;

#[allow(clippy::needless_pass_by_value)] // signature required for use with .map_err()
pub(crate) fn map_redis_err(e: redis::RedisError) -> StateStoreError {
    use redis::ErrorKind;
    match e.kind() {
        ErrorKind::IoError | ErrorKind::AuthenticationFailed => {
            StateStoreError::Connection(e.to_string())
        }
        _ => StateStoreError::Other(e.to_string()),
    }
}
