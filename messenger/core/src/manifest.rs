use serde::Deserialize;
use crate::models::{PluginInfo, PluginPermissions};

#[derive(Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginSection,
    pub capabilities: CapabilitiesSection,
    pub permissions: PermissionsSection,
    pub limits: LimitsSection,
}

#[derive(Deserialize)]
pub struct PluginSection {
    pub id: String,
    pub name: String,
    pub version: String,
    pub category: String,
    pub description: String,
}

#[derive(Deserialize)]
pub struct CapabilitiesSection {
    pub provides: Vec<String>,
    pub subscribes_to: Vec<String>,
    pub emits: Vec<String>,
}

#[derive(Deserialize)]
pub struct PermissionsSection {
    pub network: bool,
    pub filesystem: bool,
    pub contacts: bool,
    pub clipboard: bool,
    pub notifications: bool,
}

#[derive(Deserialize)]
pub struct LimitsSection {
    pub max_memory_mb: u32,
    pub timeout_ms: u32,
}

pub fn parse_manifest(toml_str: &str) -> Result<PluginManifest, String> {
    toml::from_str(toml_str).map_err(|e| e.to_string())
}

pub fn manifest_to_plugin_info(m: &PluginManifest) -> PluginInfo {
    PluginInfo {
        id: m.plugin.id.clone(),
        name: m.plugin.name.clone(),
        version: m.plugin.version.clone(),
        category: m.plugin.category.clone(),
        description: m.plugin.description.clone(),
        active: true,
        permissions: PluginPermissions {
            network: m.permissions.network,
            filesystem: m.permissions.filesystem,
            contacts: m.permissions.contacts,
            clipboard: m.permissions.clipboard,
            notifications: m.permissions.notifications,
        },
    }
}
