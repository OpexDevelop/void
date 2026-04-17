use anyhow::Result;
use wasmtime::{
    component::{bindgen, Component, Linker as ComponentLinker},
    Config, Engine, Store,
};
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

use crate::event::{BusTx, Event, EventMeta};
use crate::network;
use super::{HostContext, PluginInstance, PluginRuntime};

const FUEL_LIMIT: u64 = 50_000_000;

bindgen!({
    path:  "wit/plugin.wit",
    world: "network-plugin-world",
    async: true,
});

struct HostState {
    wasi:        WasiCtx,
    table:       ResourceTable,
    event_tx:    BusTx,
    permissions: super::Permissions,
}

impl WasiView for HostState {
    fn ctx(&mut self)   -> &mut WasiCtx       { &mut self.wasi  }
    fn table(&mut self) -> &mut ResourceTable  { &mut self.table }
}

impl NetworkPluginWorldImports for HostState {
    async fn emit_event(&mut self, topic: String, payload: Vec<u8>) -> wasmtime::Result<()> {
        let _ = self.event_tx.send(Event {
            meta:    EventMeta::new(topic),
            payload,
        }).await;
        Ok(())
    }

    async fn host_http_post(&mut self, url: String, body: Vec<u8>) -> wasmtime::Result<i32> {
        if !self.permissions.network { return Ok(-1); }
        network::http_post(url, body).await;
        Ok(0)
    }

    async fn host_sse_start(&mut self, url: String) -> wasmtime::Result<i32> {
        if !self.permissions.network { return Ok(-1); }
        tokio::spawn(network::sse_loop(url, self.event_tx.clone()));
        Ok(0)
    }
}

pub struct WasmtimeRuntime { engine: Engine }

impl WasmtimeRuntime {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.async_support(true);
        config.consume_fuel(true);
        config.wasm_component_model(true);
        Ok(Self { engine: Engine::new(&config)? })
    }
}

impl PluginRuntime for WasmtimeRuntime {
    async fn instantiate(&self, wasm_bytes: &[u8], ctx: HostContext) -> Result<Box<dyn PluginInstance>> {
        let component = Component::new(&self.engine, wasm_bytes)?;
        let mut linker: ComponentLinker<HostState> = ComponentLinker::new(&self.engine);

        wasmtime_wasi::add_to_linker_async(&mut linker)?;
        NetworkPluginWorld::add_to_linker(&mut linker, |state: &mut HostState| state)?;

        let mut builder = WasiCtxBuilder::new();
        builder.inherit_stdio();

        if ctx.permissions.filesystem {
            for dir in &ctx.permissions.allowed_dirs {
                std::fs::create_dir_all(dir)?;
                builder.preopened_dir(dir, ".", DirPerms::all(), FilePerms::all())?;
            }
        }

        let host_state = HostState {
            wasi:        builder.build(),
            table:       ResourceTable::new(),
            event_tx:    ctx.event_tx,
            permissions: ctx.permissions,
        };

        let mut store = Store::new(&self.engine, host_state);
        store.set_fuel(FUEL_LIMIT)?;

        let (plugin, _) = NetworkPluginWorld::instantiate_async(
            &mut store,
            &component,
            &linker,
        ).await?;

        Ok(Box::new(WasmtimeInstance {
            store,
            plugin,
            initial_fuel: FUEL_LIMIT,
        }))
    }
}

pub struct WasmtimeInstance {
    store:        Store<HostState>,
    plugin:       NetworkPluginWorld,
    initial_fuel: u64,
}

impl PluginInstance for WasmtimeInstance {
    async fn handle_event(&mut self, meta_json: &[u8], payload: &[u8]) -> Result<(), String> {
        let meta: crate::event::EventMeta = serde_json::from_slice(meta_json)
            .map_err(|e| e.to_string())?;

        let wit_meta = void::plugin::types::EventMeta {
            id:        meta.id,
            topic:     meta.topic,
            version:   meta.version,
            timestamp: meta.timestamp,
        };

        let result = self.plugin
            .call_handle_event(&mut self.store, &wit_meta, payload)
            .await
            .map_err(|e| e.to_string())?;

        if result == 0 { Ok(()) } else { Err(format!("plugin error {result}")) }
    }

    fn fuel_consumed(&self) -> u64 {
        self.initial_fuel.saturating_sub(self.store.get_fuel().unwrap_or(0))
    }
}
