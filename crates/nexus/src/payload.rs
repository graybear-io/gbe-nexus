use bytes::Bytes;
use serde::{Serialize, de::DeserializeOwned};

/// Enforced schema contract for all domain payloads on the transport.
///
/// Wraps domain-specific data with required metadata fields.
/// Guarantees a consistent wire format across all domain crates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DomainPayload<T> {
    /// Schema version (starts at 1, bump on breaking changes).
    pub v: u32,

    /// Event time — when the business event occurred (unix millis).
    pub ts: u64,

    /// Dedup key — consumer-defined, unique per logical event.
    pub id: String,

    /// Domain-specific payload data.
    pub data: T,
}

impl<T: Serialize> DomainPayload<T> {
    /// Create a new domain payload, setting `ts` to now.
    ///
    /// # Panics
    /// Panics if the system clock is before the Unix epoch.
    #[allow(clippy::cast_possible_truncation)] // millis since epoch fits in u64 until year 584556
    pub fn new(v: u32, id: impl Into<String>, data: T) -> Self {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_millis() as u64;

        Self {
            v,
            ts,
            id: id.into(),
            data,
        }
    }

    /// Serialize to bytes for transport publication.
    ///
    /// # Errors
    /// Returns a `serde_json::Error` if serialization fails.
    pub fn to_bytes(&self) -> Result<Bytes, serde_json::Error> {
        serde_json::to_vec(self).map(Bytes::from)
    }
}

impl<T: DeserializeOwned> DomainPayload<T> {
    /// Deserialize from transport payload bytes.
    ///
    /// # Errors
    /// Returns a `serde_json::Error` if deserialization fails.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct CreateSandbox {
        image: String,
        cpu: u32,
    }

    #[test]
    fn round_trip() {
        let payload = DomainPayload::new(
            1,
            "sb-abc123",
            CreateSandbox {
                image: "ubuntu:22.04".into(),
                cpu: 2,
            },
        );

        let bytes = payload.to_bytes().unwrap();
        let decoded: DomainPayload<CreateSandbox> = DomainPayload::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.v, 1);
        assert_eq!(decoded.id, "sb-abc123");
        assert_eq!(decoded.data, payload.data);
        assert!(decoded.ts > 0);
    }

    #[test]
    fn wire_format_has_required_fields() {
        let payload = DomainPayload::new(
            1,
            "test-001",
            CreateSandbox {
                image: "alpine".into(),
                cpu: 1,
            },
        );

        let json: serde_json::Value = serde_json::to_value(&payload).unwrap();
        assert!(json.get("v").is_some());
        assert!(json.get("ts").is_some());
        assert!(json.get("id").is_some());
        assert!(json.get("data").is_some());

        assert_eq!(json["v"], 1);
        assert_eq!(json["id"], "test-001");
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn ts_auto_set_to_now() {
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let payload = DomainPayload::new(
            1,
            "ts-test",
            CreateSandbox {
                image: "test".into(),
                cpu: 1,
            },
        );

        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        assert!(payload.ts >= before);
        assert!(payload.ts <= after);
    }
}
