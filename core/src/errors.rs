#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("plugin not found: {0}")]
    PluginNotFound(String),

    #[error("plugin '{0}' already loaded — unload it first")]
    PluginAlreadyLoaded(String),

    #[error("plugin call '{func}' failed: {reason}")]
    PluginCallFailed { func: String, reason: String },

    #[error("manifest parse error: {0}")]
    ManifestParse(String),

    #[error("wasm size {size} bytes exceeds limit {limit} bytes")]
    WasmSizeExceeded { size: usize, limit: usize },

    #[error("lifecycle error in '{plugin}' fn '{func}': {reason}")]
    LifecycleError {
        plugin: String,
        func: String,
        reason: String,
    },

    #[error("no plugin handles category '{0}'")]
    NoCategoryHandler(String),
}

impl CoreError {
    pub fn to_json(&self) -> String {
        serde_json::json!({ "ok": false, "error": self.to_string() }).to_string()
    }
}
