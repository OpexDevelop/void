use std::time::{SystemTime, UNIX_EPOCH};
use crate::types::Message;
use crate::storage::Storage;
use crate::crypto::Crypto;
use crate::transport::Transport;

pub struct VoidEngine {
    storage: Box<dyn Storage>,
    crypto: Box<dyn Crypto>,
    transports: Vec<Box<dyn Transport>>,
}

impl VoidEngine {
    pub fn new(
        storage: Box<dyn Storage>,
        crypto: Box<dyn Crypto>,
        transports: Vec<Box<dyn Transport>>,
    ) -> Self {
        Self { storage, crypto, transports }
    }

    pub fn send_msg(&self, chat_id: &str, text: &str, peer_addr: &str) {
        let msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: chat_id.into(),
            text: text.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_secs(),
            incoming: false,
        };
        self.storage.save(&msg);
        let encrypted = self.crypto.encrypt(&serde_json::to_vec(&msg).unwrap());
        for t in &self.transports {
            let _ = t.send(peer_addr, &encrypted);
        }
    }

    pub fn poll(&self) -> Option<Message> {
        for t in &self.transports {
            if let Some(data) = t.recv() {
                let plain = self.crypto.decrypt(&data)?;
                let mut msg: Message = serde_json::from_slice(&plain).ok()?;
                msg.incoming = true;
                self.storage.save(&msg);
                return Some(msg);
            }
        }
        None
    }

    pub fn history(&self, chat_id: &str) -> Vec<Message> {
        self.storage.history(chat_id)
    }
}
