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
        let encrypted = format!("enc({})", event.data);
        Ok(format!("Base64: {}", encrypted))
    } else {
        Ok("ignore".to_string())
    }
}
