use anyhow::Result;
use async_trait::async_trait;
use wasmtime::{
    component::{bindgen, Component, Linker as ComponentLinker},
    Config, Engine, Store,
};
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

use crate::event::{BusTx, Event, EventMeta as HostEventMeta};
use crate::network;
use super::{HostContext, PluginInstance, PluginRuntime};

const FUEL_LIMIT: u64 = 50_000_000;

bindgen!({
    path:  "wit/plugin.wit",
    world: "network-plugin-world",
});

struct HostState {
    wasi:        WasiCtx,
    table:       ResourceTable,
    event_tx:    BusTx,
    permissions: super::Permissions,
}

impl WasiView for HostState {
    fn ctx(&mut self)   -> &mut WasiCtx      { &mut self.wasi  }
    fn table(&mut self) -> &mut ResourceTable { &mut self.table }
}

// Без async: true — методы синхронные, но handle_event в плагине вызывается
// через call_handle_event_async на Store с async_support(true)
impl NetworkPluginWorldImports for HostState {
    fn emit_event(&mut self, topic: String, payload: Vec<u8>) -> wasmtime::Result<()> {
        // try_send не блокирует — корректно в sync контексте
        let _ = self.event_tx.try_send(Event {
            meta:    HostEventMeta::new(topic),
            payload,
        });
        Ok(())
    }

    fn host_http_post(&mut self, url: String, body: Vec<u8>) -> wasmtime::Result<i32> {
        if !self.permissions.network { return Ok(-1); }
        tokio::spawn(network::http_post(url, body));
        Ok(0)
    }

    fn host_sse_start(&mut self, url: String) -> wasmtime::Result<i32> {
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

#[async_trait]
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

        // async instantiate — не блокирует планировщик при инициализации
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

#[async_trait]
impl PluginInstance for WasmtimeInstance {
    async fn handle_event(&mut self, meta_json: &[u8], payload: &[u8]) -> Result<(), String> {
        let meta: HostEventMeta = serde_json::from_slice(meta_json)
            .map_err(|e| e.to_string())?;

        // call_handle_event — async вызов wasm компонента
        // Store имеет async_support(true) — не блокирует поток
        let result = self.plugin
            .call_handle_event(
                &mut self.store,
                &meta.id,
                &meta.topic,
                meta.version,
                meta.timestamp,
                payload,
            )
            .await
            .map_err(|e| e.to_string())?;

        if result == 0 { Ok(()) } else { Err(format!("plugin error {result}")) }
    }

    fn fuel_consumed(&self) -> u64 {
        self.initial_fuel.saturating_sub(self.store.get_fuel().unwrap_or(0))
    }
}
