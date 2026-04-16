use anyhow::Result;
use tokio::sync::mpsc;

use crate::bus::Event;
use crate::manifest::PluginManifest;

#[cfg(feature = "wasmtime-backend")]
pub mod wasmtime_engine;

#[cfg(feature = "wasmi-backend")]
pub mod wasmi_engine;

pub struct LinkerConfig {
    pub event_tx: mpsc::UnboundedSender<Event>,
    pub manifest: PluginManifest,
}

pub trait PluginRuntime: Send + Sync + 'static {
    fn instantiate(
        &self,
        wasm_bytes: &[u8],
        config:     LinkerConfig,
    ) -> Result<Box<dyn PluginInstance>>;
}

pub trait PluginInstance: Send {
    fn handle_event(&mut self, meta_json: &[u8], payload: &[u8]) -> Result<(), String>;
    fn fuel_consumed(&self) -> u64;
}
