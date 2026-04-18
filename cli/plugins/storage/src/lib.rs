use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Event {
    topic: String,
    data: String,
}

#[derive(Serialize, Deserialize)]
struct PluginResponse {
    log: Option<String>,
    emit: Vec<Event>,
}

#[plugin_fn]
pub fn handle_event(input: String) -> FnResult<String> {
    let event: Event = serde_json::from_str(&input)?;
    let mut log_msg = None;

    if event.topic == "CRYPTO_DECRYPTED" || event.topic == "UI_SEND_MSG" {
        let file_path = "/data/history.txt";
        let mut current = std::fs::read_to_string(file_path).unwrap_or_default();
        
        let prefix = if event.topic == "UI_SEND_MSG" { "Вы" } else { "Собеседник" };
        let entry = format!("{}: {}\n", prefix, event.data);
        
        current.push_str(&entry);
        
        if std::fs::write(file_path, current).is_ok() {
            log_msg = Some(format!("💾 Сохранено: {}", entry.trim()));
        }
    }

    Ok(serde_json::to_string(&PluginResponse { log: log_msg, emit: vec![] })?)
}
