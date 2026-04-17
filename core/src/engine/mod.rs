use anyhow::Result;
use std::path::PathBuf;

use crate::event::BusTx;

#[cfg(feature = "wasmtime-backend")]
pub mod wasmtime_engine;

#[cfg(feature = "wasmi-backend")]
pub mod wasmi_engine;

#[derive(Debug, Clone)]
pub struct Permissions {
    pub network:      bool,
    pub filesystem:   bool,
    pub allowed_dirs: Vec<PathBuf>,
}

pub struct HostContext {
    pub event_tx:    BusTx,
    pub permissions: Permissions,
}

pub trait PluginRuntime: Send + Sync + 'static {
    async fn instantiate(
        &self,
        wasm_bytes: &[u8],
        ctx:        HostContext,
    ) -> Result<Box<dyn PluginInstance>>;
}

pub trait PluginInstance: Send {
    async fn handle_event(&mut self, meta_json: &[u8], payload: &[u8]) -> Result<(), String>;
    fn fuel_consumed(&self) -> u64;
}
