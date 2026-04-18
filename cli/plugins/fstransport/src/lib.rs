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
    let mut emit_events = Vec::new();

    // Получаем или создаем уникальный ID клиента, чтобы не читать свои же сообщения
    let client_id: String = match var::get("client_id")? {
        Some(id) => id,
        None => {
            let new_id = format!("C_{}", event.ts);
            var::set("client_id", &new_id)?;
            new_id
        }
    };

    if event.topic == "CRYPTO_ENCRYPTED" {
        let path = format!("/net/{}_{}.msg", event.ts, client_id);
        if std::fs::write(&path, &event.data).is_ok() {
            log_msg = Some("Файл отправлен в общую сеть".to_string());
        }
    } 
    else if event.topic == "SYS_TICK" {
        let last_ts: u64 = var::get("last_ts")?.unwrap_or(0);
        let mut max_ts = last_ts;

        if let Ok(entries) = std::fs::read_dir("/net") {
            for entry in entries.flatten() {
                let filename = entry.file_name().into_string().unwrap_or_default();
                
                // Читаем только файлы с расширением .msg и НЕ от нашего client_id
                if filename.ends_with(".msg") && !filename.contains(&client_id) {
                    if let Some(ts_str) = filename.split('_').next() {
                        if let Ok(file_ts) = ts_str.parse::<u64>() {
                            if file_ts > last_ts {
                                if let Ok(data) = std::fs::read_to_string(entry.path()) {
                                    emit_events.push(Event {
                                        topic: "NET_RECEIVED".to_string(),
                                        data,
                                        ts: 0,
                                    });
                                }
                                if file_ts > max_ts { max_ts = file_ts; }
                            }
                        }
                    }
                }
            }
        }
        var::set("last_ts", max_ts)?;
    }

    Ok(serde_json::to_string(&PluginResponse { log: log_msg, emit: emit_events })?)
}
