use gbe_nexus::TransportError;

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
