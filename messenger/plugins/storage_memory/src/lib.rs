use extism_pdk::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Serialize, Deserialize)]
struct StoredMessage {
    from: String,
    to: String,
    text: String,
    timestamp: u64,
}

fn storage() -> &'static Mutex<HashMap<String, Vec<StoredMessage>>> {
    static S: OnceLock<Mutex<HashMap<String, Vec<StoredMessage>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Deserialize)]
struct StoreInput {
    from: String,
    to: String,
    text: String,
    timestamp: u64,
}

#[derive(Deserialize)]
struct GetInput {
    contact: String,
}

#[plugin_fn]
pub fn store_message(input: String) -> FnResult<String> {
    let msg: StoreInput = serde_json::from_str(&input)?;
    let key = if msg.from == "me" {
        msg.to.clone()
    } else {
        msg.from.clone()
    };
    storage()
        .lock()
        .unwrap()
        .entry(key)
        .or_default()
        .push(StoredMessage {
            from: msg.from,
            to: msg.to,
            text: msg.text,
            timestamp: msg.timestamp,
        });
    Ok(r#"{"ok":true}"#.to_string())
}

#[plugin_fn]
pub fn get_messages(input: String) -> FnResult<String> {
    let req: GetInput = serde_json::from_str(&input)?;
    let store = storage().lock().unwrap();
    let msgs = store.get(&req.contact).cloned().unwrap_or_default();
    Ok(serde_json::to_string(&msgs)?)
}

#[plugin_fn]
pub fn clear(_input: String) -> FnResult<String> {
    storage().lock().unwrap().clear();
    Ok(r#"{"ok":true}"#.to_string())
}
