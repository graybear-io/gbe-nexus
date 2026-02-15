use crate::error::SweeperError;

const RELEASE_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('DEL', KEYS[1])
else
    return 0
end
"#;

pub struct DistributedLock {
    conn: redis::aio::ConnectionManager,
    key: String,
    instance_id: String,
    ttl_ms: u64,
}

impl DistributedLock {
    pub async fn new(
        redis_url: &str,
        key: String,
        ttl: std::time::Duration,
    ) -> Result<Self, SweeperError> {
        let client =
            redis::Client::open(redis_url).map_err(|e| SweeperError::Lock(e.to_string()))?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| SweeperError::Lock(e.to_string()))?;

        let host = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let instance_id = format!("{host}-{}", ulid::Ulid::new());

        Ok(Self {
            conn,
            key,
            instance_id,
            ttl_ms: ttl.as_millis() as u64,
        })
    }

    pub async fn acquire(&self) -> Result<bool, SweeperError> {
        let mut conn = self.conn.clone();
        let result: Option<String> = redis::cmd("SET")
            .arg(&self.key)
            .arg(&self.instance_id)
            .arg("NX")
            .arg("PX")
            .arg(self.ttl_ms)
            .query_async(&mut conn)
            .await
            .map_err(|e| SweeperError::Lock(e.to_string()))?;
        Ok(result.is_some())
    }

    pub async fn release(&self) -> Result<(), SweeperError> {
        let mut conn = self.conn.clone();
        let _: i32 = redis::Script::new(RELEASE_SCRIPT)
            .key(&self.key)
            .arg(&self.instance_id)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| SweeperError::Lock(e.to_string()))?;
        Ok(())
    }
}
