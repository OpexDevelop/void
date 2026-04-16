use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub plugin:      PluginInfo,
    pub events:      EventsConfig,
    pub supervisor:  SupervisorConfig,
    pub permissions: PermissionsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginInfo {
    pub id:        String,
    pub version:   String,
    pub sha256:    String,
    pub signature: String,
    pub wasm_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventsConfig {
    pub subscriptions:  Vec<String>,
    pub max_queue_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupervisorConfig {
    pub restart_policy: RestartPolicy,
    pub max_retries:    u32,
}

// BUG FIX #1: было `lowercase` → "onfailure", манифест пишет "on_failure"
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub filesystem: bool,
    #[serde(default)]
    pub allowed_dirs: Vec<String>,
}

impl PluginManifest {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
