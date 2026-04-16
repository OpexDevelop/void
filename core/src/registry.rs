use std::collections::HashMap;

use anyhow::Result;

use crate::manifest::PluginManifest;

pub struct PluginRecord {
    pub manifest:   PluginManifest,
    pub wasm_bytes: Vec<u8>,
}

pub struct Registry {
    plugins: HashMap<String, PluginRecord>,
}

impl Registry {
    pub fn new() -> Self {
        Self { plugins: HashMap::new() }
    }

    pub fn load(&mut self, manifest: PluginManifest, wasm_bytes: Vec<u8>) -> Result<()> {
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
