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

fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

#[plugin_fn]
pub fn handle_event(input: String) -> FnResult<String> {
    let event: Event = serde_json::from_str(&input)?;
    let mut log_msg = None;
    let mut emit_events = Vec::new();
    let channel = "void_messenger_test";
    
    if event.topic == "CRYPTO_ENCRYPTED" {
        let encoded_msg = url_encode(&event.data);
        let url = format!("https://dweetr.io/dweet/quietly/for/{}?message={}", channel, encoded_msg);
        let req = HttpRequest::new(&url).with_method("GET");
            
        if let Ok(res) = http::request::<String>(&req, None) {
            log_msg = Some(format!("Отправлено в dweetr! Статус: {}", res.status_code()));
        }
    } 
    else if event.topic == "SYS_TICK" {
        let last_time: String = var::get("last_dweet_time")?.unwrap_or_else(|| "".to_string());
        let url = format!("https://dweetr.io/get/dweets/for/{}", channel);
        
        let req = HttpRequest::new(&url).with_method("GET");
        if let Ok(res) = http::request::<Vec<u8>>(&req, None) {
            let body_bytes = res.body();
            let body_str = String::from_utf8_lossy(&body_bytes);
            
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_str) {
                if let Some(dweets) = json.get("with").and_then(|w| w.as_array()) {
                    let mut new_last_time = last_time.clone();
                    
                    for dweet in dweets.iter().rev() {
                        if let Some(created) = dweet.get("created").and_then(|c| c.as_str()) {
                            if created > last_time.as_str() {
                                if let Some(content) = dweet.get("content") {
                                    if let Some(msg) = content.get("message").and_then(|m| m.as_str()) {
                                        emit_events.push(Event {
                                            topic: "NET_RECEIVED".to_string(),
                                            data: msg.to_string(),
                                        });
                                    }
                                }
                                new_last_time = created.to_string();
                            }
                        }
                    }
                    
                    if new_last_time != last_time {
                        var::set("last_dweet_time", new_last_time)?;
                    }
                }
            }
        }
    }

    Ok(serde_json::to_string(&PluginResponse { log: log_msg, emit: emit_events })?)
}
