use std::collections::HashMap;
use std::sync::Mutex;
use crate::types::Message;

pub trait Storage: Send + Sync {
    fn save(&self, msg: &Message);
    fn history(&self, chat_id: &str) -> Vec<Message>;
    fn chats(&self) -> Vec<String>;
}

pub struct MemStorage {
    data: Mutex<HashMap<String, Vec<Message>>>,
}

impl MemStorage {
    pub fn new() -> Self {
        Self { data: Mutex::new(HashMap::new()) }
    }
}

impl Storage for MemStorage {
    fn save(&self, msg: &Message) {
        self.data.lock().unwrap()
            .entry(msg.chat_id.clone())
            .or_default()
            .push(msg.clone());
    }

    fn history(&self, chat_id: &str) -> Vec<Message> {
        self.data.lock().unwrap()
            .get(chat_id).cloned().unwrap_or_default()
    }

    fn chats(&self) -> Vec<String> {
        self.data.lock().unwrap().keys().cloned().collect()
    }
}
