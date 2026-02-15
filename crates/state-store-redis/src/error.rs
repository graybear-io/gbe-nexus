use gbe_state_store::StateStoreError;

pub(crate) fn map_redis_err(e: redis::RedisError) -> StateStoreError {
    use redis::ErrorKind;
    match e.kind() {
        ErrorKind::IoError | ErrorKind::AuthenticationFailed => {
            StateStoreError::Connection(e.to_string())
        }
        _ => StateStoreError::Other(e.to_string()),
    }
}
