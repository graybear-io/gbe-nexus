use gbe_nexus::TransportError;

#[allow(clippy::needless_pass_by_value)] // signature required for use with .map_err()
pub(crate) fn map_redis_err(e: redis::RedisError) -> TransportError {
    use redis::ErrorKind;
    match e.kind() {
        ErrorKind::IoError | ErrorKind::AuthenticationFailed => {
            TransportError::Connection(e.to_string())
        }
        ErrorKind::TypeError => TransportError::Other(format!("redis type error: {e}")),
        _ => TransportError::Other(e.to_string()),
    }
}
