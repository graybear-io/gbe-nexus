use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

use gbe_nexus::{
    Message, MessageHandler, PublishOpts, StartPosition, StreamConfig, SubscribeOpts, Transport,
    TransportError,
};
use gbe_nexus_memory::{MemoryTransport, MemoryTransportConfig};

fn create_transport() -> MemoryTransport {
    MemoryTransport::new(MemoryTransportConfig::default())
}

fn test_subject(name: &str) -> String {
    format!("gbe.test.{name}")
}

// --- Handlers ---

struct CollectingHandler {
    payloads: Arc<tokio::sync::Mutex<Vec<Bytes>>>,
    notify: Arc<Notify>,
}

#[async_trait]
impl MessageHandler for CollectingHandler {
    async fn handle(&self, msg: &dyn Message) -> Result<(), TransportError> {
        self.payloads.lock().await.push(msg.payload().clone());
        msg.ack().await?;
        self.notify.notify_one();
        Ok(())
    }
}

struct DeadLetterHandler {
    notify: Arc<Notify>,
}

#[async_trait]
impl MessageHandler for DeadLetterHandler {
    async fn handle(&self, msg: &dyn Message) -> Result<(), TransportError> {
        msg.dead_letter("test: forced dead letter").await?;
        self.notify.notify_one();
        Ok(())
    }
}

struct NakHandler {
    call_count: Arc<tokio::sync::Mutex<u32>>,
    notify: Arc<Notify>,
}

#[async_trait]
impl MessageHandler for NakHandler {
    async fn handle(&self, msg: &dyn Message) -> Result<(), TransportError> {
        let mut count = self.call_count.lock().await;
        *count += 1;
        if *count == 1 {
            // First delivery: nak it
            msg.nak(None).await?;
        } else {
            // Redelivery: ack it
            msg.ack().await?;
        }
        self.notify.notify_one();
        Ok(())
    }
}

// --- Tests ---

#[tokio::test]
async fn test_ping() {
    let transport = create_transport();
    assert!(transport.ping().await.unwrap());
}

#[tokio::test]
async fn test_ensure_stream_idempotent() {
    let transport = create_transport();
    let subject = test_subject("ensure");

    let config = StreamConfig {
        subject: subject.clone(),
        max_age: Duration::from_secs(3600),
        max_bytes: None,
        max_msgs: None,
    };

    transport.ensure_stream(config.clone()).await.unwrap();
    transport.ensure_stream(config).await.unwrap();
}

#[tokio::test]
async fn test_publish_returns_message_id() {
    let transport = create_transport();
    let subject = test_subject("pubid");

    let id = transport
        .publish(&subject, Bytes::from("hello"), None)
        .await
        .unwrap();

    // Message ID should be a valid ULID (26 chars)
    assert_eq!(id.len(), 26);
}

#[tokio::test]
async fn test_payload_too_large() {
    let transport = MemoryTransport::new(MemoryTransportConfig {
        max_payload_size: 100,
    });

    let subject = test_subject("large");
    let big_payload = Bytes::from(vec![0u8; 200]);

    let result = transport.publish(&subject, big_payload, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(
            err,
            TransportError::PayloadTooLarge {
                size: 200,
                max: 100
            }
        ),
        "expected PayloadTooLarge, got: {err:?}"
    );
}

#[tokio::test]
async fn test_publish_subscribe_roundtrip() {
    let transport = create_transport();
    let subject = test_subject("roundtrip");

    let payloads: Arc<tokio::sync::Mutex<Vec<Bytes>>> = Arc::new(tokio::sync::Mutex::new(vec![]));
    let notify = Arc::new(Notify::new());

    let sub = transport
        .subscribe(
            &subject,
            "test-group",
            Box::new(CollectingHandler {
                payloads: payloads.clone(),
                notify: notify.clone(),
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    // Give the consumer loop a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    for i in 0..3 {
        transport
            .publish(&subject, Bytes::from(format!("msg-{i}")), None)
            .await
            .unwrap();
    }

    // Wait until all 3 messages are received
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if payloads.lock().await.len() >= 3 {
                break;
            }
            notify.notified().await;
        }
    })
    .await
    .expect("timed out waiting for messages");

    let received = payloads.lock().await;
    assert_eq!(received.len(), 3);
    assert_eq!(received[0], Bytes::from("msg-0"));
    assert_eq!(received[1], Bytes::from("msg-1"));
    assert_eq!(received[2], Bytes::from("msg-2"));

    assert!(sub.is_active());
    sub.unsubscribe().await.unwrap();
}

#[tokio::test]
async fn test_dead_letter_routing() {
    let transport = create_transport();
    let subject = test_subject("deadletter");

    let notify = Arc::new(Notify::new());

    let sub = transport
        .subscribe(
            &subject,
            "dl-group",
            Box::new(DeadLetterHandler {
                notify: notify.clone(),
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    transport
        .publish(&subject, Bytes::from("doomed"), None)
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), notify.notified())
        .await
        .expect("timed out waiting for dead letter handler");

    // Give a moment for the dead letter write to complete
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify dead letter stream has an entry by subscribing to it
    let dl_payloads: Arc<tokio::sync::Mutex<Vec<Bytes>>> =
        Arc::new(tokio::sync::Mutex::new(vec![]));
    let dl_notify = Arc::new(Notify::new());

    let dl_sub = transport
        .subscribe(
            "gbe._deadletter.test",
            "dl-check",
            Box::new(CollectingHandler {
                payloads: dl_payloads.clone(),
                notify: dl_notify.clone(),
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), dl_notify.notified())
        .await
        .expect("timed out waiting for dead letter message");

    let received = dl_payloads.lock().await;
    assert_eq!(received.len(), 1);
    // Payload should contain the reason
    let payload_str = String::from_utf8_lossy(&received[0]);
    assert!(
        payload_str.contains("forced dead letter"),
        "dead letter payload should contain reason"
    );

    sub.unsubscribe().await.unwrap();
    dl_sub.unsubscribe().await.unwrap();
}

#[tokio::test]
async fn test_close_prevents_operations() {
    let transport = create_transport();
    transport.close().await.unwrap();

    let result = transport
        .publish("gbe.test.closed", Bytes::from("nope"), None)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[allow(clippy::items_after_statements)]
async fn test_trace_id_propagation() {
    let transport = create_transport();
    let subject = test_subject("trace");

    let trace_ids: Arc<tokio::sync::Mutex<Vec<Option<String>>>> =
        Arc::new(tokio::sync::Mutex::new(vec![]));
    let notify = Arc::new(Notify::new());

    struct TraceHandler {
        trace_ids: Arc<tokio::sync::Mutex<Vec<Option<String>>>>,
        notify: Arc<Notify>,
    }

    #[async_trait]
    impl MessageHandler for TraceHandler {
        async fn handle(&self, msg: &dyn Message) -> Result<(), TransportError> {
            self.trace_ids
                .lock()
                .await
                .push(msg.envelope().trace_id.clone());
            msg.ack().await?;
            self.notify.notify_one();
            Ok(())
        }
    }

    let sub = transport
        .subscribe(
            &subject,
            "trace-group",
            Box::new(TraceHandler {
                trace_ids: trace_ids.clone(),
                notify: notify.clone(),
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    transport
        .publish(
            &subject,
            Bytes::from("traced"),
            Some(PublishOpts {
                trace_id: Some("abc-123-trace".to_string()),
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), notify.notified())
        .await
        .expect("timed out waiting for traced message");

    let ids = trace_ids.lock().await;
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], Some("abc-123-trace".to_string()));

    sub.unsubscribe().await.unwrap();
}

#[tokio::test]
async fn test_nak_triggers_redelivery() {
    let transport = create_transport();
    let subject = test_subject("nak");

    let call_count = Arc::new(tokio::sync::Mutex::new(0u32));
    let notify = Arc::new(Notify::new());

    let sub = transport
        .subscribe(
            &subject,
            "nak-group",
            Box::new(NakHandler {
                call_count: call_count.clone(),
                notify: notify.clone(),
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    transport
        .publish(&subject, Bytes::from("retry-me"), None)
        .await
        .unwrap();

    // Wait for first delivery (nak)
    tokio::time::timeout(Duration::from_secs(2), notify.notified())
        .await
        .expect("timed out waiting for first delivery");

    // Wait for redelivery (ack)
    tokio::time::timeout(Duration::from_secs(2), notify.notified())
        .await
        .expect("timed out waiting for redelivery");

    let count = *call_count.lock().await;
    assert_eq!(count, 2, "handler should have been called twice");

    sub.unsubscribe().await.unwrap();
}

#[tokio::test]
async fn test_trim_stream() {
    let transport = create_transport();
    let subject = test_subject("trim");

    // Publish a message
    transport
        .publish(&subject, Bytes::from("old"), None)
        .await
        .unwrap();

    // Trim with zero max_age should remove everything
    let trimmed = transport
        .trim_stream(&subject, Duration::from_secs(0))
        .await
        .unwrap();
    assert_eq!(trimmed, 1);

    // Trim non-existent stream returns 0
    let trimmed = transport
        .trim_stream("gbe.test.nonexistent", Duration::from_secs(0))
        .await
        .unwrap();
    assert_eq!(trimmed, 0);
}

#[tokio::test]
async fn test_start_position_latest() {
    let transport = create_transport();
    let subject = test_subject("latest");

    // Publish before subscribing
    transport
        .publish(&subject, Bytes::from("before-subscribe"), None)
        .await
        .unwrap();

    let payloads: Arc<tokio::sync::Mutex<Vec<Bytes>>> = Arc::new(tokio::sync::Mutex::new(vec![]));
    let notify = Arc::new(Notify::new());

    let sub = transport
        .subscribe(
            &subject,
            "latest-group",
            Box::new(CollectingHandler {
                payloads: payloads.clone(),
                notify: notify.clone(),
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Latest,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Publish after subscribing
    transport
        .publish(&subject, Bytes::from("after-subscribe"), None)
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), notify.notified())
        .await
        .expect("timed out waiting for message");

    let received = payloads.lock().await;
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], Bytes::from("after-subscribe"));

    sub.unsubscribe().await.unwrap();
}

#[tokio::test]
async fn test_multiple_groups_see_same_messages() {
    let transport = create_transport();
    let subject = test_subject("multigroup");

    let payloads_a: Arc<tokio::sync::Mutex<Vec<Bytes>>> = Arc::new(tokio::sync::Mutex::new(vec![]));
    let notify_a = Arc::new(Notify::new());

    let payloads_b: Arc<tokio::sync::Mutex<Vec<Bytes>>> = Arc::new(tokio::sync::Mutex::new(vec![]));
    let notify_b = Arc::new(Notify::new());

    let sub_a = transport
        .subscribe(
            &subject,
            "group-a",
            Box::new(CollectingHandler {
                payloads: payloads_a.clone(),
                notify: notify_a.clone(),
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    let sub_b = transport
        .subscribe(
            &subject,
            "group-b",
            Box::new(CollectingHandler {
                payloads: payloads_b.clone(),
                notify: notify_b.clone(),
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    transport
        .publish(&subject, Bytes::from("shared"), None)
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), notify_a.notified())
        .await
        .expect("group-a timed out");
    tokio::time::timeout(Duration::from_secs(2), notify_b.notified())
        .await
        .expect("group-b timed out");

    assert_eq!(payloads_a.lock().await.len(), 1);
    assert_eq!(payloads_b.lock().await.len(), 1);

    sub_a.unsubscribe().await.unwrap();
    sub_b.unsubscribe().await.unwrap();
}

#[tokio::test]
#[allow(clippy::items_after_statements)]
async fn test_backpressure_max_inflight() {
    let transport = create_transport();
    let subject = test_subject("backpressure");

    // Handler that never acks — messages stay pending
    struct NoAckHandler {
        count: Arc<tokio::sync::Mutex<u32>>,
        notify: Arc<Notify>,
    }

    #[async_trait]
    impl MessageHandler for NoAckHandler {
        async fn handle(&self, _msg: &dyn Message) -> Result<(), TransportError> {
            let mut count = self.count.lock().await;
            *count += 1;
            self.notify.notify_one();
            // Don't ack — message stays in pending
            Ok(())
        }
    }

    let count = Arc::new(tokio::sync::Mutex::new(0u32));
    let notify = Arc::new(Notify::new());

    let sub = transport
        .subscribe(
            &subject,
            "bp-group",
            Box::new(NoAckHandler {
                count: count.clone(),
                notify: notify.clone(),
            }),
            Some(SubscribeOpts {
                start_from: StartPosition::Earliest,
                max_inflight: 2,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Publish 5 messages
    for i in 0..5 {
        transport
            .publish(&subject, Bytes::from(format!("bp-{i}")), None)
            .await
            .unwrap();
    }

    // Wait for the first 2 to be delivered
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(2), notify.notified())
            .await
            .expect("timed out waiting for delivery");
    }

    // Give time for any additional deliveries
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Should have delivered at most max_inflight messages
    let delivered = *count.lock().await;
    assert!(
        delivered <= 2,
        "expected at most 2 delivered (max_inflight), got {delivered}"
    );

    sub.unsubscribe().await.unwrap();
}
