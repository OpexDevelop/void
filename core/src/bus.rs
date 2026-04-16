use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMeta {
    pub id: String,
    pub topic: String,
    pub version: u32,
    pub timestamp: u64,
}

impl EventMeta {
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            topic: topic.into(),
            version: 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    pub meta: EventMeta,
    pub payload: Vec<u8>,
}

pub const SYS_STARTUP:       &str = "SYS_STARTUP";
pub const SYS_SHUTDOWN:      &str = "SYS_SHUTDOWN";
pub const SYS_DLQ:           &str = "SYS_DLQ";
pub const UI_SEND_MSG:       &str = "UI_SEND_MSG";
pub const CRYPTO_ENCRYPTED:  &str = "CRYPTO_ENCRYPTED";
pub const CRYPTO_DECRYPTED:  &str = "CRYPTO_DECRYPTED";
pub const NET_RECEIVED:      &str = "NET_RECEIVED";
pub const DB_READ_CMD:       &str = "DB_READ_CMD";
