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
    
    let mut count: u32 = var::get("msg_count")?.unwrap_or(0);
    count += 1;
    var::set("msg_count", count)?;

    Ok(format!("Сообщение #{} принято", count))
}
