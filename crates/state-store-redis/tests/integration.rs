//! Integration tests for the Redis KV state store backend.
//!
//! Requires a running Redis instance. Set REDIS_URL to enable these tests.
//! Default: redis://127.0.0.1:6379
//!
//! Run with: REDIS_URL=redis://localhost:6379 cargo test --package gbe-state-store-redis

use bytes::Bytes;
use std::collections::HashMap;
use std::time::Duration;

use gbe_state_store::{Record, ScanFilter, ScanOp, StateStore, StateStoreConfig};
use gbe_state_store_redis::RedisStateStore;

fn redis_url() -> Option<String> {
    std::env::var("REDIS_URL").ok()
}

async fn connect() -> RedisStateStore {
    let url = redis_url().expect("REDIS_URL must be set for integration tests");
    RedisStateStore::connect(StateStoreConfig { url })
        .await
        .expect("failed to connect to Redis")
}

fn test_key(name: &str) -> String {
    format!(
        "gbe:test:state:{}:{}",
        name,
        ulid::Ulid::new().to_string().to_lowercase()
    )
}

async fn cleanup_keys(keys: &[&str]) {
    let url = redis_url().unwrap();
    let client = redis::Client::open(url.as_str()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    for key in keys {
        let _: Result<(), _> = redis::cmd("DEL").arg(*key).query_async(&mut conn).await;
    }
}

fn make_record(fields: &[(&str, &str)]) -> Record {
    Record {
        fields: fields
            .iter()
            .map(|(k, v)| (k.to_string(), Bytes::from(v.to_string())))
            .collect(),
        ttl: None,
    }
}

// --- Tests ---

#[tokio::test]
async fn test_ping() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    assert!(store.ping().await.unwrap());
}

#[tokio::test]
async fn test_put_get_roundtrip() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let key = test_key("roundtrip");

    let record = make_record(&[("state", "pending"), ("task_type", "email-send")]);
    store.put(&key, record, None).await.unwrap();

    let got = store.get(&key).await.unwrap().expect("expected record");
    assert_eq!(got.fields.get("state").unwrap().as_ref(), b"pending");
    assert_eq!(got.fields.get("task_type").unwrap().as_ref(), b"email-send");

    cleanup_keys(&[&key]).await;
}

#[tokio::test]
async fn test_get_missing_key() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let key = test_key("missing");

    let got = store.get(&key).await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn test_delete() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let key = test_key("delete");

    store
        .put(&key, make_record(&[("x", "1")]), None)
        .await
        .unwrap();
    store.delete(&key).await.unwrap();

    let got = store.get(&key).await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn test_field_ops() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let key = test_key("fieldops");

    store
        .set_field(&key, "state", Bytes::from("pending"))
        .await
        .unwrap();

    let val = store
        .get_field(&key, "state")
        .await
        .unwrap()
        .expect("expected field");
    assert_eq!(val.as_ref(), b"pending");

    // Missing field returns None
    let missing = store.get_field(&key, "nonexistent").await.unwrap();
    assert!(missing.is_none());

    cleanup_keys(&[&key]).await;
}

#[tokio::test]
async fn test_set_fields_batch() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let key = test_key("batch");

    let mut fields = HashMap::new();
    fields.insert("state".to_string(), Bytes::from("running"));
    fields.insert("step".to_string(), Bytes::from("3"));
    fields.insert("worker".to_string(), Bytes::from("host-1"));

    store.set_fields(&key, fields).await.unwrap();

    let got = store.get(&key).await.unwrap().expect("expected record");
    assert_eq!(got.fields.len(), 3);
    assert_eq!(got.fields.get("state").unwrap().as_ref(), b"running");
    assert_eq!(got.fields.get("step").unwrap().as_ref(), b"3");
    assert_eq!(got.fields.get("worker").unwrap().as_ref(), b"host-1");

    cleanup_keys(&[&key]).await;
}

#[tokio::test]
async fn test_compare_and_swap_success() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let key = test_key("cas-ok");

    store
        .set_field(&key, "state", Bytes::from("pending"))
        .await
        .unwrap();

    let swapped = store
        .compare_and_swap(
            &key,
            "state",
            Bytes::from("pending"),
            Bytes::from("claimed"),
        )
        .await
        .unwrap();
    assert!(swapped);

    let val = store.get_field(&key, "state").await.unwrap().unwrap();
    assert_eq!(val.as_ref(), b"claimed");

    cleanup_keys(&[&key]).await;
}

#[tokio::test]
async fn test_compare_and_swap_failure() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let key = test_key("cas-fail");

    store
        .set_field(&key, "state", Bytes::from("running"))
        .await
        .unwrap();

    let swapped = store
        .compare_and_swap(
            &key,
            "state",
            Bytes::from("pending"),
            Bytes::from("claimed"),
        )
        .await
        .unwrap();
    assert!(!swapped);

    // Value unchanged
    let val = store.get_field(&key, "state").await.unwrap().unwrap();
    assert_eq!(val.as_ref(), b"running");

    cleanup_keys(&[&key]).await;
}

#[tokio::test]
async fn test_scan_with_prefix() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let prefix = format!(
        "gbe:test:scan:{}:",
        ulid::Ulid::new().to_string().to_lowercase()
    );
    let key1 = format!("{prefix}job1");
    let key2 = format!("{prefix}job2");
    let decoy = test_key("decoy");

    store
        .put(&key1, make_record(&[("state", "pending")]), None)
        .await
        .unwrap();
    store
        .put(&key2, make_record(&[("state", "running")]), None)
        .await
        .unwrap();
    store
        .put(&decoy, make_record(&[("state", "pending")]), None)
        .await
        .unwrap();

    let results = store.scan(&prefix, None).await.unwrap();
    assert_eq!(results.len(), 2);

    let keys: Vec<&String> = results.iter().map(|(k, _)| k).collect();
    assert!(keys.contains(&&key1));
    assert!(keys.contains(&&key2));

    cleanup_keys(&[&key1, &key2, &decoy]).await;
}

#[tokio::test]
async fn test_scan_with_filter() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let prefix = format!(
        "gbe:test:filter:{}:",
        ulid::Ulid::new().to_string().to_lowercase()
    );
    let key1 = format!("{prefix}job1");
    let key2 = format!("{prefix}job2");

    store
        .put(&key1, make_record(&[("state", "pending")]), None)
        .await
        .unwrap();
    store
        .put(&key2, make_record(&[("state", "running")]), None)
        .await
        .unwrap();

    // Filter: state == "pending"
    let results = store
        .scan(
            &prefix,
            Some(ScanFilter {
                field: "state".to_string(),
                op: ScanOp::Eq,
                value: Bytes::from("pending"),
                max_results: None,
            }),
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, key1);

    cleanup_keys(&[&key1, &key2]).await;
}

#[tokio::test]
async fn test_close_prevents_operations() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    store.close().await.unwrap();

    let result = store.get("anything").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_put_with_ttl() {
    if redis_url().is_none() {
        return;
    }
    let store = connect().await;
    let key = test_key("ttl");

    store
        .put(
            &key,
            make_record(&[("state", "temp")]),
            Some(Duration::from_secs(1)),
        )
        .await
        .unwrap();

    // Key exists now
    let got = store.get(&key).await.unwrap();
    assert!(got.is_some());

    // Wait for expiry
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let got = store.get(&key).await.unwrap();
    assert!(got.is_none());
}
