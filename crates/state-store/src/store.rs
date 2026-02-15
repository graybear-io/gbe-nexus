use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::time::Duration;

use crate::error::StateStoreError;

/// KV state store. Streams carry signals; KV carries state.
#[async_trait]
pub trait StateStore: Send + Sync {
    // Single key operations
    async fn get(&self, key: &str) -> Result<Option<Record>, StateStoreError>;
    async fn put(
        &self,
        key: &str,
        record: Record,
        ttl: Option<Duration>,
    ) -> Result<(), StateStoreError>;
    async fn delete(&self, key: &str) -> Result<(), StateStoreError>;

    // Field-level operations
    async fn get_field(&self, key: &str, field: &str) -> Result<Option<Bytes>, StateStoreError>;
    async fn set_field(&self, key: &str, field: &str, value: Bytes) -> Result<(), StateStoreError>;
    async fn set_fields(
        &self,
        key: &str,
        fields: HashMap<String, Bytes>,
    ) -> Result<(), StateStoreError>;

    // Atomic operations
    async fn compare_and_swap(
        &self,
        key: &str,
        field: &str,
        expected: Bytes,
        new: Bytes,
    ) -> Result<bool, StateStoreError>;

    // Scan (for sweeper)
    async fn scan(
        &self,
        prefix: &str,
        filter: Option<ScanFilter>,
    ) -> Result<Vec<(String, Record)>, StateStoreError>;

    // Health
    async fn ping(&self) -> Result<bool, StateStoreError>;

    async fn close(&self) -> Result<(), StateStoreError>;
}

/// A set of field/value pairs with optional TTL.
#[derive(Debug, Clone, Default)]
pub struct Record {
    pub fields: HashMap<String, Bytes>,
    pub ttl: Option<Duration>,
}

/// Simple filter for scan operations.
#[derive(Debug, Clone)]
pub struct ScanFilter {
    pub field: String,
    pub op: ScanOp,
    pub value: Bytes,
    pub max_results: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum ScanOp {
    Eq,
    Lt,
    Gt,
}

#[derive(Debug, Clone)]
pub struct StateStoreConfig {
    pub url: String,
}
