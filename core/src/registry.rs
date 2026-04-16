use std::collections::HashMap;
use anyhow::Result;

use crate::manifest::PluginManifest;
use crate::signing;

#[cfg(feature = "verify-signatures")]
use ed25519_dalek::VerifyingKey;

pub struct PluginRecord {
    pub manifest:   PluginManifest,
    pub wasm_bytes: Vec<u8>,
}

pub struct Registry {
    #[cfg(feature = "verify-signatures")]
    verifying_key: Option<VerifyingKey>,
    plugins: HashMap<String, PluginRecord>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "verify-signatures")]
            verifying_key: None,
            plugins: HashMap::new(),
        }
    }

    #[cfg(feature = "verify-signatures")]
    pub fn with_verifying_key(mut self, key: VerifyingKey) -> Self {
        self.verifying_key = Some(key);
        self
    }

    pub fn load(&mut self, manifest: PluginManifest, wasm_bytes: Vec<u8>) -> Result<()> {
        #[cfg(feature = "verify-signatures")]
        if let Some(key) = &self.verifying_key {
            signing::verify_wasm(
                &wasm_bytes,
                &manifest.plugin.sha256,
                &manifest.plugin.signature,
                key,
            )?;
        }

        let id = manifest.plugin.id.clone();
        self.plugins.insert(id, PluginRecord { manifest, wasm_bytes });
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&PluginRecord> {
        self.plugins.get(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<PluginRecord> {
        self.plugins.remove(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.plugins.keys()
    }
}
