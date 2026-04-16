use serde::{Deserialize, Serialize};
use std::collections::HashMap;

extern "C" {
    fn emit_event(
        topic_ptr: *const u8, topic_len: i32,
        payload_ptr: *const u8, payload_len: i32,
    );
}

const DB_PATH:     &str = "./messages.json";
const DB_TMP_PATH: &str = "./messages.json.tmp";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMessage {
    id:      String,
    payload: Vec<u8>,
    ts:      u64,
}

type MessageStore = HashMap<String, StoredMessage>;

#[derive(Deserialize)]
struct EventMeta {
    id:        String,
    topic:     String,
    timestamp: u64,
}

#[no_mangle]
pub extern "C" fn alloc(size: i32) -> *mut u8 {
    let mut buf: Vec<u8> = Vec::with_capacity(size as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: i32) {
    unsafe { drop(Vec::from_raw_parts(ptr, 0, size as usize)) };
}

#[no_mangle]
pub extern "C" fn handle_event(
    meta_ptr:    *const u8, meta_len:    i32,
    payload_ptr: *const u8, payload_len: i32,
) -> i32 {
    let meta_slice    = unsafe { std::slice::from_raw_parts(meta_ptr,    meta_len    as usize) };
    let payload_slice = unsafe { std::slice::from_raw_parts(payload_ptr, payload_len as usize) };

    let meta: EventMeta = match serde_json::from_slice(meta_slice) {
        Ok(m)  => m,
        Err(_) => return 1,
    };

    match meta.topic.as_str() {
        "UI_SEND_MSG" | "CRYPTO_DECRYPTED" => store_message(&meta.id, meta.timestamp, payload_slice),
        "DB_READ_CMD"                      => read_and_emit_history(),
        _                                  => 0,
    }
}

fn load_store() -> MessageStore {
    std::fs::read(DB_PATH)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn persist_store(store: &MessageStore) -> bool {
    let bytes = match serde_json::to_vec(store) {
        Ok(b)  => b,
        Err(_) => return false,
    };
    if std::fs::write(DB_TMP_PATH, &bytes).is_err() {
        return false;
    }
    std::fs::rename(DB_TMP_PATH, DB_PATH).is_ok()
}

fn store_message(id: &str, ts: u64, payload: &[u8]) -> i32 {
    let mut store = load_store();
    if store.contains_key(id) {
        return 0;
    }
    store.insert(id.to_string(), StoredMessage {
        id:      id.to_string(),
        payload: payload.to_vec(),
        ts,
    });
    if persist_store(&store) { 0 } else { 1 }
}

fn read_and_emit_history() -> i32 {
    let store = load_store();
    let mut messages: Vec<StoredMessage> = store.into_values().collect();
    messages.sort_by_key(|m| m.ts);
    let result = match serde_json::to_vec(&messages) {
        Ok(v)  => v,
        Err(_) => return 1,
    };
    emit("DB_HISTORY_RESULT", &result);
    0
}

fn emit(topic: &str, payload: &[u8]) {
    unsafe {
        emit_event(
            topic.as_ptr(),   topic.len()   as i32,
            payload.as_ptr(), payload.len() as i32,
        );
    }
}
