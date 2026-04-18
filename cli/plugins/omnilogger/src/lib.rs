use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Event {
    topic: String,
    data: String,
    #[serde(default)]
    ts: u64,
}

#[derive(Serialize, Deserialize)]
struct PluginResponse {
    log: Option<String>,
    emit: Vec<Event>,
}

#[plugin_fn]
pub fn handle_event(input: String) -> FnResult<String> {
    let event: Event = serde_json::from_str(&input)?;
    
    let log_line = format!("[TS: {}] {} -> {}\n", event.ts, event.topic, event.data);
    
    let path = "/logs/system.log";
    let mut current = std::fs::read_to_string(path).unwrap_or_default();
    current.push_str(&log_line);
    let _ = std::fs::write(path, current);

    // Вернем лог в Ядро, чтобы вывести в консоль (если это не скучный тик)
    let log_msg = if event.topic != "SYS_TICK" {
        Some(format!("ОМНИЛОГ: {} | {:.30}", event.topic, event.data))
    } else {
        None
    };

    Ok(serde_json::to_string(&PluginResponse { log: log_msg, emit: vec![] })?)
}
