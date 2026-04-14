use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PluginManifest {
    pub plugin: PluginSection,
    pub capabilities: CapabilitiesSection,
    pub permissions: PermissionsSection,
    pub limits: LimitsSection,
}

#[derive(Deserialize, Debug)]
pub struct PluginSection {
    pub id: String,
    pub name: String,
    pub version: String,
    pub category: String,
    pub description: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct CapabilitiesSection {
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub lifecycle: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct PermissionsSection {
    pub network: bool,
    pub filesystem: bool,
    pub contacts: bool,
    pub clipboard: bool,
    pub notifications: bool,
}

#[derive(Deserialize, Debug)]
pub struct LimitsSection {
    pub max_memory_mb: u32,
    pub timeout_ms: u32,
}

pub fn parse_manifest(toml_str: &str) -> Result<PluginManifest, String> {
    toml::from_str(toml_str).map_err(|e| format!("manifest parse error: {e}"))
}
