use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub text: String,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub category: String,
    pub description: String,
    pub active: bool,
    pub permissions: PluginPermissions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginPermissions {
    pub network: bool,
    pub filesystem: bool,
    pub contacts: bool,
    pub clipboard: bool,
    pub notifications: bool,
}
