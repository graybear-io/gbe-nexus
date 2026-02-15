use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gbe_state_store::{Record, ScanFilter, ScanOp, StateStoreConfig, StateStoreError};

use crate::error::map_redis_err;

const CAS_SCRIPT: &str = r#"
local cur = redis.call('HGET', KEYS[1], ARGV[1])
if cur == ARGV[2] then
    redis.call('HSET', KEYS[1], ARGV[1], ARGV[3])
    return 1
else
    return 0
end
"#;

pub struct RedisStateStore {
    conn: redis::aio::ConnectionManager,
    closed: AtomicBool,
}

impl RedisStateStore {
    pub async fn connect(config: StateStoreConfig) -> Result<Self, StateStoreError> {
        let client = redis::Client::open(config.url.as_str())
            .map_err(|e| StateStoreError::Connection(e.to_string()))?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| StateStoreError::Connection(e.to_string()))?;
        Ok(Self {
            conn,
            closed: AtomicBool::new(false),
        })
    }

    fn check_closed(&self) -> Result<(), StateStoreError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(StateStoreError::Other("store is closed".to_string()));
        }
        Ok(())
    }
}

#[async_trait]
impl gbe_state_store::StateStore for RedisStateStore {
    async fn get(&self, key: &str) -> Result<Option<Record>, StateStoreError> {
        self.check_closed()?;
        let mut conn = self.conn.clone();
        let fields: HashMap<String, Vec<u8>> = redis::cmd("HGETALL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(map_redis_err)?;

        if fields.is_empty() {
            return Ok(None);
        }

        let fields = fields
            .into_iter()
            .map(|(k, v)| (k, Bytes::from(v)))
            .collect();

        Ok(Some(Record { fields, ttl: None }))
    }

    async fn put(
        &self,
        key: &str,
        record: Record,
        ttl: Option<Duration>,
    ) -> Result<(), StateStoreError> {
        self.check_closed()?;
        if record.fields.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn.clone();
        let pairs: Vec<(&str, &[u8])> = record
            .fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_ref()))
            .collect();

        redis::cmd("HSET")
            .arg(key)
            .arg(&pairs)
            .query_async::<()>(&mut conn)
            .await
            .map_err(map_redis_err)?;

        if let Some(ttl) = ttl {
            redis::cmd("EXPIRE")
                .arg(key)
                .arg(ttl.as_secs() as i64)
                .query_async::<()>(&mut conn)
                .await
                .map_err(map_redis_err)?;
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), StateStoreError> {
        self.check_closed()?;
        let mut conn = self.conn.clone();
        redis::cmd("DEL")
            .arg(key)
            .query_async::<()>(&mut conn)
            .await
            .map_err(map_redis_err)?;
        Ok(())
    }

    async fn get_field(&self, key: &str, field: &str) -> Result<Option<Bytes>, StateStoreError> {
        self.check_closed()?;
        let mut conn = self.conn.clone();
        let value: Option<Vec<u8>> = redis::cmd("HGET")
            .arg(key)
            .arg(field)
            .query_async(&mut conn)
            .await
            .map_err(map_redis_err)?;
        Ok(value.map(Bytes::from))
    }

    async fn set_field(&self, key: &str, field: &str, value: Bytes) -> Result<(), StateStoreError> {
        self.check_closed()?;
        let mut conn = self.conn.clone();
        redis::cmd("HSET")
            .arg(key)
            .arg(field)
            .arg(value.as_ref())
            .query_async::<()>(&mut conn)
            .await
            .map_err(map_redis_err)?;
        Ok(())
    }

    async fn set_fields(
        &self,
        key: &str,
        fields: HashMap<String, Bytes>,
    ) -> Result<(), StateStoreError> {
        self.check_closed()?;
        if fields.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn.clone();
        let pairs: Vec<(&str, &[u8])> = fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_ref()))
            .collect();

        redis::cmd("HSET")
            .arg(key)
            .arg(&pairs)
            .query_async::<()>(&mut conn)
            .await
            .map_err(map_redis_err)?;

        Ok(())
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        field: &str,
        expected: Bytes,
        new: Bytes,
    ) -> Result<bool, StateStoreError> {
        self.check_closed()?;
        let mut conn = self.conn.clone();
        let result: i32 = redis::Script::new(CAS_SCRIPT)
            .key(key)
            .arg(field)
            .arg(expected.as_ref())
            .arg(new.as_ref())
            .invoke_async(&mut conn)
            .await
            .map_err(map_redis_err)?;
        Ok(result == 1)
    }

    async fn scan(
        &self,
        prefix: &str,
        filter: Option<ScanFilter>,
    ) -> Result<Vec<(String, Record)>, StateStoreError> {
        self.check_closed()?;
        let mut conn = self.conn.clone();
        let pattern = format!("{prefix}*");
        let max_results = filter.as_ref().and_then(|f| f.max_results);

        let mut results: Vec<(String, Record)> = Vec::new();
        let mut cursor: u64 = 0;

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(map_redis_err)?;

            for key in keys {
                let fields: HashMap<String, Vec<u8>> = redis::cmd("HGETALL")
                    .arg(&key)
                    .query_async(&mut conn)
                    .await
                    .map_err(map_redis_err)?;

                if fields.is_empty() {
                    continue;
                }

                let record_fields: HashMap<String, Bytes> = fields
                    .into_iter()
                    .map(|(k, v)| (k, Bytes::from(v)))
                    .collect();

                if let Some(ref f) = filter {
                    if let Some(val) = record_fields.get(&f.field) {
                        let matches = match f.op {
                            ScanOp::Eq => val.as_ref() == f.value.as_ref(),
                            ScanOp::Lt => val.as_ref() < f.value.as_ref(),
                            ScanOp::Gt => val.as_ref() > f.value.as_ref(),
                        };
                        if !matches {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }

                results.push((
                    key,
                    Record {
                        fields: record_fields,
                        ttl: None,
                    },
                ));

                if let Some(max) = max_results
                    && results.len() >= max as usize
                {
                    return Ok(results);
                }
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(results)
    }

    async fn ping(&self) -> Result<bool, StateStoreError> {
        let mut conn = self.conn.clone();
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(map_redis_err)?;
        Ok(pong == "PONG")
    }

    async fn close(&self) -> Result<(), StateStoreError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
}
