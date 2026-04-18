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
    let mut emit_events = Vec::new();
    
    if event.topic == "UI_SEND_MSG" {
        let encrypted = format!("enc({})", event.data);
        emit_events.push(Event {
            topic: "CRYPTO_ENCRYPTED".to_string(),
            data: encrypted,
            ts: 0
        });
    } else if event.topic == "NET_RECEIVED" {
        let decrypted = event.data.replace("enc(", "").replace(")", "");
        emit_events.push(Event {
            topic: "CRYPTO_DECRYPTED".to_string(),
            data: decrypted,
            ts: 0
        });
    }

    let response = PluginResponse {
        log: None,
        emit: emit_events,
    };
    
    Ok(serde_json::to_string(&response)?)
}
