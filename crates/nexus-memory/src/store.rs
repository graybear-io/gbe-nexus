use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

use gbe_nexus::{Envelope, StreamConfig};

pub(crate) type SharedStore = Arc<Mutex<StreamStore>>;

pub(crate) struct StreamStore {
    pub streams: HashMap<String, StreamData>,
}

pub(crate) struct StreamData {
    /// Messages in insertion order. Index is the cursor position.
    pub messages: Vec<Envelope>,
    /// Quick lookup: message_id -> index in messages vec.
    pub id_index: HashMap<String, usize>,
    pub groups: HashMap<String, ConsumerGroup>,
    pub config: Option<StreamConfig>,
    pub notify: Arc<Notify>,
}

pub(crate) struct ConsumerGroup {
    /// Index of the last delivered message. None means start from beginning.
    pub cursor: Option<usize>,
    pub pending: HashSet<String>,
    pub redeliver: VecDeque<String>,
}

impl StreamStore {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    pub fn get_or_create_stream(&mut self, subject: &str) -> &mut StreamData {
        self.streams
            .entry(subject.to_string())
            .or_insert_with(|| StreamData {
                messages: Vec::new(),
                id_index: HashMap::new(),
                groups: HashMap::new(),
                config: None,
                notify: Arc::new(Notify::new()),
            })
    }
}

impl ConsumerGroup {
    pub fn new(cursor: Option<usize>) -> Self {
        Self {
            cursor,
            pending: HashSet::new(),
            redeliver: VecDeque::new(),
        }
    }
}

/// Extract the domain (second token) from a dot-delimited subject.
///
/// `gbe.tasks.email-send.queue` → `"tasks"`
pub(crate) fn extract_domain(subject: &str) -> &str {
    subject.split('.').nth(1).unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain() {
        assert_eq!(extract_domain("gbe.tasks.email-send.queue"), "tasks");
        assert_eq!(extract_domain("gbe.notify.topic.alerts"), "notify");
        assert_eq!(extract_domain("gbe._deadletter.tasks"), "_deadletter");
        assert_eq!(extract_domain("single"), "unknown");
    }
}
