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
    let mut log_msg = None;
    
    let log_line = format!("[TS: {}] {} -> {}\n", event.ts, event.topic, event.data);
    let path = "/logs/system.log";
    
    let res = (|| -> std::io::Result<()> {
        let mut current = std::fs::read_to_string(path).unwrap_or_default();
        current.push_str(&log_line);
        std::fs::write(path, current)?;
        Ok(())
    })();

    if event.topic != "SYS_TICK" {
        match res {
            Ok(_) => log_msg = Some(format!("ОМНИЛОГ: {} | записано", event.topic)),
            Err(e) => log_msg = Some(format!("ОМНИЛОГ ОШИБКА: {} | {}", e, event.topic)),
        }
    }

    Ok(serde_json::to_string(&PluginResponse { log: log_msg, emit: vec![] })?)
}
