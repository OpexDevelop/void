use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub kind: String,
    pub payload: serde_json::Value,
}

static QUEUE: OnceLock<Mutex<VecDeque<Event>>> = OnceLock::new();

fn queue() -> &'static Mutex<VecDeque<Event>> {
    QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub fn push_event(event: Event) {
    queue().lock().unwrap().push_back(event);
}

pub fn poll_event() -> Option<Event> {
    queue().lock().unwrap().pop_front()
}
