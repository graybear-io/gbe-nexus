use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use gbe_nexus::{Envelope, MessageHandler, SubscribeOpts};

use crate::message::MemoryMessage;
use crate::store::SharedStore;

pub(crate) struct ConsumerParams {
    pub store: SharedStore,
    pub subject: String,
    pub group: String,
    pub handler: Box<dyn MessageHandler>,
    pub opts: SubscribeOpts,
    pub token: CancellationToken,
    pub active: Arc<AtomicBool>,
    pub notify: Arc<tokio::sync::Notify>,
}

pub(crate) async fn run_consumer_loop(params: ConsumerParams) {
    let ConsumerParams {
        store,
        subject,
        group,
        handler,
        opts,
        token,
        active,
        notify,
    } = params;

    loop {
        if token.is_cancelled() {
            break;
        }

        // Register the notification future BEFORE checking for messages
        // to avoid the race between collect_batch releasing the lock
        // and a publish calling notify_waiters.
        let notified = notify.notified();

        let batch = collect_batch(&store, &subject, &group, &opts).await;

        if batch.is_empty() {
            tokio::select! {
                () = notified => {}
                () = token.cancelled() => break,
                () = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
            continue;
        }

        for envelope in batch {
            if token.is_cancelled() {
                break;
            }

            let msg = MemoryMessage {
                envelope,
                subject: subject.clone(),
                group: group.clone(),
                store: store.clone(),
                acked: AtomicBool::new(false),
            };

            if let Err(e) = handler.handle(&msg).await {
                tracing::warn!(error = %e, "handler error, auto-nak");
                let _ = <MemoryMessage as gbe_nexus::Message>::nak(&msg, None).await;
            }
        }
    }

    active.store(false, Ordering::Release);
}

async fn collect_batch(
    store: &SharedStore,
    subject: &str,
    group: &str,
    opts: &SubscribeOpts,
) -> Vec<Envelope> {
    let mut store = store.lock().await;
    let batch_size = opts.batch_size as usize;

    let Some(stream) = store.streams.get_mut(subject) else {
        return Vec::new();
    };

    let Some(consumer_group) = stream.groups.get_mut(group) else {
        return Vec::new();
    };

    // Backpressure: skip if too many pending
    if consumer_group.pending.len() >= opts.max_inflight as usize {
        return Vec::new();
    }

    let remaining_capacity = opts.max_inflight as usize - consumer_group.pending.len();
    let take = batch_size.min(remaining_capacity);

    let mut batch = Vec::new();

    // Phase 1: redeliver nak'd messages first
    while batch.len() < take {
        let Some(msg_id) = consumer_group.redeliver.pop_front() else {
            break;
        };
        // Find the message by ID
        if let Some(&idx) = stream.id_index.get(&msg_id) {
            batch.push(stream.messages[idx].clone());
        }
    }

    // Phase 2: deliver new messages by index
    if batch.len() < take {
        let start_idx = match consumer_group.cursor {
            Some(c) => c + 1,
            None => 0,
        };

        let end_idx = (start_idx + take - batch.len()).min(stream.messages.len());

        for i in start_idx..end_idx {
            let envelope = &stream.messages[i];
            if !consumer_group.pending.contains(&envelope.message_id) {
                consumer_group.pending.insert(envelope.message_id.clone());
                consumer_group.cursor = Some(i);
                batch.push(envelope.clone());
            }
        }
    }

    batch
}
