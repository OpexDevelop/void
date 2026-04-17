wit_bindgen::generate!({
    path:  "../../core/wit/plugin.wit",
    world: "plugin-world",
});

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

struct Plugin;

impl Guest for Plugin {
    fn handle_event(
        id:         String,
        topic:      String,
        _version:   u32,
        timestamp:  u64,
        payload:    Vec<u8>,
    ) -> i32 {
        match topic.as_str() {
            "UI_SEND_MSG" | "CRYPTO_DECRYPTED" => store_message(&id, timestamp, &payload),
            "DB_READ_CMD"                      => read_and_emit_history(),
            _                                  => 0,
        }
    }
}

export!(Plugin);

const DB_PATH:     &str = "./messages.json";
const DB_TMP_PATH: &str = "./messages.json.tmp";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMessage {
    id:      String,
    payload: Vec<u8>,
    ts:      u64,
}

type MessageStore = HashMap<String, StoredMessage>;

fn load_store() -> MessageStore {
    std::fs::read(DB_PATH)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn persist_store(store: &MessageStore) -> bool {
    let bytes = match serde_json::to_vec(store) {
        Ok(b)  => b,
        Err(_) => return false,
    };
    if std::fs::write(DB_TMP_PATH, &bytes).is_err() { return false; }
    std::fs::rename(DB_TMP_PATH, DB_PATH).is_ok()
}

fn store_message(id: &str, ts: u64, payload: &[u8]) -> i32 {
    let mut store = load_store();
    if store.contains_key(id) { return 0; }
    store.insert(id.to_string(), StoredMessage {
        id:      id.to_string(),
        payload: payload.to_vec(),
        ts,
    });
    if persist_store(&store) { 0 } else { 1 }
}

fn read_and_emit_history() -> i32 {
    let store = load_store();
    let mut msgs: Vec<StoredMessage> = store.into_values().collect();
    msgs.sort_by_key(|m| m.ts);
    match serde_json::to_vec(&msgs) {
        Ok(v)  => { emit_event("DB_HISTORY_RESULT", &v); 0 }
        Err(_) => 1,
    }
}
