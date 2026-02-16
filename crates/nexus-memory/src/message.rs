use async_trait::async_trait;
use bytes::Bytes;
use std::sync::atomic::{AtomicBool, Ordering};

use gbe_nexus::{Envelope, TransportError};

use crate::store::{SharedStore, extract_domain};

pub(crate) struct MemoryMessage {
    pub(crate) envelope: Envelope,
    pub(crate) subject: String,
    pub(crate) group: String,
    pub(crate) store: SharedStore,
    pub(crate) acked: AtomicBool,
}

#[async_trait]
impl gbe_nexus::Message for MemoryMessage {
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    fn payload(&self) -> &Bytes {
        &self.envelope.payload
    }

    async fn ack(&self) -> Result<(), TransportError> {
        if self.acked.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let mut store = self.store.lock().await;
        if let Some(stream) = store.streams.get_mut(&self.subject)
            && let Some(group) = stream.groups.get_mut(&self.group)
        {
            group.pending.remove(&self.envelope.message_id);
        }
        Ok(())
    }

    async fn nak(&self, _delay: Option<std::time::Duration>) -> Result<(), TransportError> {
        if self.acked.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let mut store = self.store.lock().await;
        if let Some(stream) = store.streams.get_mut(&self.subject)
            && let Some(group) = stream.groups.get_mut(&self.group)
        {
            group.redeliver.push_back(self.envelope.message_id.clone());
        }
        Ok(())
    }

    async fn dead_letter(&self, reason: &str) -> Result<(), TransportError> {
        if self.acked.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        let domain = extract_domain(&self.subject);
        let dl_subject = format!("gbe._deadletter.{domain}");

        let dl_envelope = Envelope::new(
            dl_subject.clone(),
            Bytes::from(
                serde_json::json!({
                    "original_envelope": serde_json::to_value(&self.envelope)
                        .map_err(TransportError::Serialization)?,
                    "reason": reason,
                })
                .to_string(),
            ),
            self.envelope.trace_id.clone(),
        );

        let mut store = self.store.lock().await;

        // Insert into dead-letter stream
        let dl_stream = store.get_or_create_stream(&dl_subject);
        let idx = dl_stream.messages.len();
        dl_stream
            .id_index
            .insert(dl_envelope.message_id.clone(), idx);
        dl_stream.messages.push(dl_envelope);
        dl_stream.notify.notify_waiters();

        // Remove from original group's pending
        if let Some(stream) = store.streams.get_mut(&self.subject)
            && let Some(group) = stream.groups.get_mut(&self.group)
        {
            group.pending.remove(&self.envelope.message_id);
        }

        Ok(())
    }
}
