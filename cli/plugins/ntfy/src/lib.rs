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
    let mut emit_events = Vec::new();
    
    // 1. Отправка сообщений
    if event.topic == "CRYPTO_ENCRYPTED" {
        let req = HttpRequest::new("https://ntfy.sh/void_messenger_test_channel")
            .with_method("POST");
        let _ = http::request::<String>(&req, Some(event.data));
    } 
    // 2. Получение новых сообщений (Пульс)
    else if event.topic == "SYS_TICK" {
        let since: String = var::get("since")?.unwrap_or_else(|| "all".to_string());
        let url = format!("https://ntfy.sh/void_messenger_test_channel/json?poll=1&since={}", since);
        
        let req = HttpRequest::new(&url).with_method("GET");
        if let Ok(res) = http::request::<Vec<u8>>(&req, None) {
            let body = String::from_utf8_lossy(&res.body());
            
            for line in body.lines() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    if val["event"].as_str() == Some("message") {
                        if let Some(msg) = val["message"].as_str() {
                            emit_events.push(Event {
                                topic: "NET_RECEIVED".to_string(),
                                data: msg.to_string(),
                            });
                        }
                    }
                    if let Some(id) = val["id"].as_str() {
                        var::set("since", id)?; // Запоминаем ID последнего сообщения
                    }
                }
            }
        }
    }

    Ok(serde_json::to_string(&PluginResponse { log: log_msg, emit: emit_events })?)
}
