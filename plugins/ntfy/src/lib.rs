use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Event {
    topic: String,
    data: String,
}

#[plugin_fn]
pub fn handle_event(input: String) -> FnResult<String> {
    let event: Event = serde_json::from_str(&input)?;
    
    if event.topic == "user_input" {
        let req = HttpRequest::new("https://ntfy.sh/void_messenger_test_channel")
            .with_method("POST")
            .with_header("Title", "Void Host");
            
        let res = http::request::<String>(&req, Some(event.data))?;
        
        Ok(format!("Отправлено в ntfy! HTTP Статус: {}", res.status_code()))
    } else {
        Ok("ignore".to_string())
    }
}
