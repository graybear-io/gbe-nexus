use async_trait::async_trait;
use bytes::Bytes;
use std::sync::atomic::{AtomicBool, Ordering};

use gbe_transport::{Envelope, TransportError};

use crate::error::map_redis_err;
use crate::subject::extract_domain;

pub(crate) struct RedisMessage {
    pub(crate) envelope: Envelope,
    pub(crate) stream_key: String,
    pub(crate) group: String,
    pub(crate) entry_id: String,
    pub(crate) conn: redis::aio::ConnectionManager,
    pub(crate) acked: AtomicBool,
}

#[async_trait]
impl gbe_transport::Message for RedisMessage {
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    fn payload(&self) -> &Bytes {
        &self.envelope.payload
    }

    async fn ack(&self) -> Result<(), TransportError> {
        if self.acked.swap(true, Ordering::AcqRel) {
            return Ok(()); // already acked
        }
        let mut conn = self.conn.clone();
        redis::cmd("XACK")
            .arg(&self.stream_key)
            .arg(&self.group)
            .arg(&self.entry_id)
            .query_async::<i64>(&mut conn)
            .await
            .map_err(map_redis_err)?;
        Ok(())
    }

    async fn nak(&self, _delay: Option<std::time::Duration>) -> Result<(), TransportError> {
        // Claim-based nak: do nothing. Message stays in PEL and will be
        // reclaimed after ack_timeout via XAUTOCLAIM in the consumer loop.
        self.acked.store(true, Ordering::Release);
        Ok(())
    }

    async fn dead_letter(&self, reason: &str) -> Result<(), TransportError> {
        if self.acked.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let mut conn = self.conn.clone();

        let domain = extract_domain(&self.stream_key);
        let dl_key = format!("gbe:_deadletter:{domain}");

        let envelope_json =
            serde_json::to_string(&self.envelope).map_err(TransportError::Serialization)?;

        // Write to dead letter stream
        redis::cmd("XADD")
            .arg(&dl_key)
            .arg("*")
            .arg("envelope")
            .arg(&envelope_json)
            .arg("reason")
            .arg(reason)
            .query_async::<String>(&mut conn)
            .await
            .map_err(map_redis_err)?;

        // Ack the original message
        redis::cmd("XACK")
            .arg(&self.stream_key)
            .arg(&self.group)
            .arg(&self.entry_id)
            .query_async::<i64>(&mut conn)
            .await
            .map_err(map_redis_err)?;

        Ok(())
    }
}
