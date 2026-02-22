use std::sync::Arc;

use serde::Serialize;

use crate::error::TransportError;
use crate::payload::DomainPayload;
use crate::transport::{PublishOpts, Transport};

/// Convenience wrapper for publishing domain events to the transport.
///
/// Holds shared transport, component identity, and handles
/// `DomainPayload<T>` wrapping + serialization automatically.
pub struct EventEmitter {
    transport: Arc<dyn Transport>,
    component: String,
    instance_id: String,
}

impl EventEmitter {
    pub fn new(
        transport: Arc<dyn Transport>,
        component: impl Into<String>,
        instance_id: impl Into<String>,
    ) -> Self {
        Self {
            transport,
            component: component.into(),
            instance_id: instance_id.into(),
        }
    }

    pub fn component(&self) -> &str {
        &self.component
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// Publish a domain event wrapped in `DomainPayload<T>`.
    ///
    /// Builds the payload envelope with schema version, dedup ID, and
    /// auto-set timestamp, then serializes and publishes to the given subject.
    pub async fn emit<T: Serialize>(
        &self,
        subject: &str,
        schema_version: u32,
        dedup_id: impl Into<String>,
        data: T,
    ) -> Result<String, TransportError> {
        let payload = DomainPayload::new(schema_version, dedup_id, data);
        let bytes = payload.to_bytes()?;
        self.transport.publish(subject, bytes, None).await
    }

    /// Publish a domain event with an explicit trace ID for correlation.
    pub async fn emit_traced<T: Serialize>(
        &self,
        subject: &str,
        schema_version: u32,
        dedup_id: impl Into<String>,
        data: T,
        trace_id: impl Into<String>,
    ) -> Result<String, TransportError> {
        let payload = DomainPayload::new(schema_version, dedup_id, data);
        let bytes = payload.to_bytes()?;
        let opts = PublishOpts {
            trace_id: Some(trace_id.into()),
            idempotency_key: None,
        };
        self.transport.publish(subject, bytes, Some(opts)).await
    }

    /// Access the underlying transport for subscribe/stream operations.
    pub fn transport(&self) -> &Arc<dyn Transport> {
        &self.transport
    }
}

/// Helper to generate a dedup ID from component, instance, and event kind.
///
/// Format: `{component}-{instance_id}-{event}-{timestamp_millis}`
#[allow(clippy::cast_possible_truncation)]
pub fn dedup_id(component: &str, instance_id: &str, event: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_millis() as u64;
    format!("{component}-{instance_id}-{event}-{ts}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{MessageHandler, StreamConfig, SubscribeOpts, Subscription, Transport};
    use async_trait::async_trait;
    use bytes::Bytes;
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    struct TestEvent {
        msg: String,
    }

    #[derive(Default)]
    struct Published {
        subject: String,
        payload: Vec<u8>,
        opts: Option<PublishOpts>,
    }

    struct MockTransport {
        published: Mutex<Vec<Published>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                published: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn publish(
            &self,
            subject: &str,
            payload: Bytes,
            opts: Option<PublishOpts>,
        ) -> Result<String, TransportError> {
            self.published.lock().unwrap().push(Published {
                subject: subject.to_string(),
                payload: payload.to_vec(),
                opts,
            });
            Ok("msg-001".to_string())
        }

        async fn subscribe(
            &self,
            _subject: &str,
            _group: &str,
            _handler: Box<dyn MessageHandler>,
            _opts: Option<SubscribeOpts>,
        ) -> Result<Box<dyn Subscription>, TransportError> {
            unimplemented!()
        }

        async fn ensure_stream(&self, _config: StreamConfig) -> Result<(), TransportError> {
            unimplemented!()
        }

        async fn trim_stream(
            &self,
            _subject: &str,
            _max_age: Duration,
        ) -> Result<u64, TransportError> {
            unimplemented!()
        }

        async fn ping(&self) -> Result<bool, TransportError> {
            Ok(true)
        }

        async fn close(&self) -> Result<(), TransportError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn emit_wraps_in_domain_payload() {
        let transport = Arc::new(MockTransport::new());
        let emitter = EventEmitter::new(transport.clone(), "operative", "op-123");

        let result = emitter
            .emit(
                "gbe.events.lifecycle.operative.started",
                1,
                "start-op-123",
                TestEvent {
                    msg: "hello".into(),
                },
            )
            .await;

        assert!(result.is_ok());

        let published = transport.published.lock().unwrap();
        assert_eq!(published.len(), 1);
        assert_eq!(
            published[0].subject,
            "gbe.events.lifecycle.operative.started"
        );

        let decoded: DomainPayload<TestEvent> =
            DomainPayload::from_bytes(&published[0].payload).unwrap();
        assert_eq!(decoded.v, 1);
        assert_eq!(decoded.id, "start-op-123");
        assert_eq!(decoded.data.msg, "hello");
        assert!(decoded.ts > 0);
    }

    #[tokio::test]
    async fn emit_traced_includes_trace_id() {
        let transport = Arc::new(MockTransport::new());
        let emitter = EventEmitter::new(transport.clone(), "oracle", "orc-456");

        emitter
            .emit_traced(
                "gbe.jobs.report.created",
                1,
                "job-001",
                TestEvent {
                    msg: "traced".into(),
                },
                "trace-abc",
            )
            .await
            .unwrap();

        let published = transport.published.lock().unwrap();
        let opts = published[0].opts.as_ref().unwrap();
        assert_eq!(opts.trace_id.as_deref(), Some("trace-abc"));
    }

    #[test]
    fn accessors_return_identity() {
        let transport = Arc::new(MockTransport::new());
        let emitter = EventEmitter::new(transport, "sentinel", "snt-789");

        assert_eq!(emitter.component(), "sentinel");
        assert_eq!(emitter.instance_id(), "snt-789");
    }

    #[test]
    fn dedup_id_format() {
        let id = dedup_id("operative", "op-123", "started");
        assert!(id.starts_with("operative-op-123-started-"));
        let parts: Vec<&str> = id.rsplitn(2, '-').collect();
        assert!(parts[0].parse::<u64>().is_ok());
    }
}
