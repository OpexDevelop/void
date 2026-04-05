use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub text: String,
    pub timestamp: u64,
    pub incoming: bool,
}
