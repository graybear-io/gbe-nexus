use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Wire envelope wrapping every message on the transport.
///
/// The transport creates and reads the envelope. Domain code only sees the payload.
/// Serialized as JSON on the wire; payload is opaque bytes within it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Unique, transport-generated (ULID).
    pub message_id: String,

    /// Routing subject, e.g. "gbe.tasks.email-send.queue".
    pub subject: String,

    /// Unix milliseconds, set by transport at publish time.
    pub timestamp: u64,

    /// Optional trace ID for observability propagation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,

    /// Opaque payload bytes — domain-specific schema.
    #[serde(with = "base64_bytes")]
    pub payload: Bytes,
}

impl Envelope {
    pub fn new(subject: String, payload: Bytes, trace_id: Option<String>) -> Self {
        let id = ulid::Ulid::new();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_millis() as u64;

        Self {
            message_id: id.to_string(),
            subject,
            timestamp: ts,
            trace_id,
            payload,
        }
    }
}

/// Serde helper: serialize Bytes as base64 in JSON.
mod base64_bytes {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(data: &Bytes, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(data))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Bytes, D::Error> {
        let s = String::deserialize(d)?;
        STANDARD
            .decode(s)
            .map(Bytes::from)
            .map_err(serde::de::Error::custom)
    }
}
