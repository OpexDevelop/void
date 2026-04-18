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
    let mut log_msg = String::new();
    
    if event.topic == "CRYPTO_ENCRYPTED" {
        let req = HttpRequest::new("https://ntfy.sh/void_messenger_test_channel")
            .with_method("POST")
            .with_header("Title", "Void Host");
            
        match http::request::<String>(&req, Some(event.data)) {
            Ok(res) => log_msg = format!("Отправлено в ntfy! HTTP: {}", res.status_code()),
            Err(e) => log_msg = format!("Ошибка сети: {}", e),
        }
    }

    let response = PluginResponse {
        log: Some(log_msg),
        emit: vec![],
    };
    
    Ok(serde_json::to_string(&response)?)
}
