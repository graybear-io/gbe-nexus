/// Convert a dot-delimited subject to a colon-delimited Redis stream key.
///
/// `gbe.tasks.email-send.queue` → `gbe:tasks:email-send:queue`
pub(crate) fn subject_to_key(subject: &str) -> String {
    subject.replace('.', ":")
}

/// Extract the domain (second token) from a colon-delimited Redis stream key.
///
/// `gbe:tasks:email-send:queue` → `"tasks"`
pub(crate) fn extract_domain(stream_key: &str) -> &str {
    stream_key.split(':').nth(1).unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subject_to_key() {
        assert_eq!(
            subject_to_key("gbe.tasks.email-send.queue"),
            "gbe:tasks:email-send:queue"
        );
        assert_eq!(
            subject_to_key("gbe.notify.topic.alerts"),
            "gbe:notify:topic:alerts"
        );
        assert_eq!(
            subject_to_key("gbe._deadletter.tasks"),
            "gbe:_deadletter:tasks"
        );
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(extract_domain("gbe:tasks:email-send:queue"), "tasks");
        assert_eq!(extract_domain("gbe:notify:topic:alerts"), "notify");
        assert_eq!(extract_domain("gbe:_deadletter:tasks"), "_deadletter");
        assert_eq!(extract_domain("single"), "unknown");
    }
}
