use extism_pdk::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
struct StoredMessage {
    from: String,
    to: String,
    text: String,
    timestamp: u64,
}

const STORAGE_KEY: &str = "messages";

fn load_store() -> HashMap<String, Vec<StoredMessage>> {
    var::get(STORAGE_KEY)
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_store(store: &HashMap<String, Vec<StoredMessage>>) -> FnResult<()> {
    let bytes = serde_json::to_vec(store)?;
    var::set(STORAGE_KEY, bytes)?;
    Ok(())
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

    let mut store = load_store();

    store.entry(key).or_default().push(StoredMessage {
        from: msg.from,
        to: msg.to,
        text: msg.text,
        timestamp: msg.timestamp,
    });

    save_store(&store)?;
    Ok(r#"{"ok":true}"#.to_string())
}

#[plugin_fn]
pub fn get_messages(input: String) -> FnResult<String> {
    let req: GetInput = serde_json::from_str(&input)?;
    let store = load_store();
    let msgs = store.get(&req.contact).cloned().unwrap_or_default();
    Ok(serde_json::to_string(&msgs)?)
}

#[plugin_fn]
pub fn clear(_input: String) -> FnResult<String> {
    let empty: HashMap<String, Vec<StoredMessage>> = HashMap::new();
    save_store(&empty)?;
    Ok(r#"{"ok":true}"#.to_string())
}
