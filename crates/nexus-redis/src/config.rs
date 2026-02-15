/// Configuration for the Redis transport backend.
pub struct RedisTransportConfig {
    /// Redis connection URL (e.g. `redis://localhost:6379`).
    pub url: String,
    /// Maximum payload size in bytes. Publishes exceeding this are rejected.
    pub max_payload_size: usize,
}

impl Default for RedisTransportConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            max_payload_size: 1_048_576, // 1MB
        }
    }
}
