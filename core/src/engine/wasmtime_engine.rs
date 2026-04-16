use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use wasmtime::{
    component::{bindgen, Component, Linker as ComponentLinker, Val},
    Config, Engine, Store,
};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder, WasiView};
use wasmtime_wasi::preview2::WasiCtx;

use crate::event::{BusTx, Event, EventMeta};
use crate::network;
use super::{HostContext, PluginInstance, PluginRuntime};

const FUEL_LIMIT:   u64   = 50_000_000;
const MEMORY_LIMIT: usize = 50 * 1024 * 1024;

bindgen!({
    path:  "wit/plugin.wit",
    world: "plugin-world",
    async: true,
});

struct HostState {
    wasi:     WasiCtx,
    event_tx: BusTx,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx { &mut self.wasi }
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
    fn instantiate(&self, wasm_bytes: &[u8], ctx: HostContext) -> Result<Box<dyn PluginInstance>> {
        let component = Component::new(&self.engine, wasm_bytes)?;

        let mut linker: ComponentLinker<HostState> = ComponentLinker::new(&self.engine);
        wasmtime_wasi::preview2::command::add_to_linker(&mut linker)?;

        // ── emit_event ────────────────────────────────────────────────────────
        {
            let tx = ctx.event_tx.clone();
            linker.func_wrap_async(
                "void:plugin/plugin-world",
                "emit-event",
                move |_store: wasmtime::StoreContextMut<'_, HostState>,
                      (topic, payload): (String, Vec<u8>)| {
                    let tx = tx.clone();
                    Box::new(async move {
                        let _ = tx.send(Event {
                            meta:    EventMeta::new(topic),
                            payload,
                        }).await;
                        Ok(())
                    })
                },
            )?;
        }

        // ── network (только если разрешено) ───────────────────────────────────
        if ctx.permissions.network {
            linker.func_wrap_async(
                "void:plugin/network-plugin-world",
                "host-http-post",
                |_store: wasmtime::StoreContextMut<'_, HostState>,
                 (url, body): (String, Vec<u8>)| {
                    Box::new(async move {
                        network::http_post(url, body).await;
                        Ok((0i32,))
                    })
                },
            )?;

            {
                let tx = ctx.event_tx.clone();
                linker.func_wrap_async(
                    "void:plugin/network-plugin-world",
                    "host-sse-start",
                    move |_store: wasmtime::StoreContextMut<'_, HostState>,
                          (url,): (String,)| {
                        let tx = tx.clone();
                        Box::new(async move {
                            tokio::spawn(network::sse_loop(url, tx));
                            Ok((0i32,))
                        })
                    },
                )?;
            }
        }

        // ── WASI ──────────────────────────────────────────────────────────────
        let mut builder = WasiCtxBuilder::new();
        builder.inherit_stdio();

        if ctx.permissions.filesystem {
            for dir in &ctx.permissions.allowed_dirs {
                std::fs::create_dir_all(dir)?;
                builder.preopened_dir(dir, ".", DirPerms::all(), FilePerms::all())?;
            }
        }

        let host_state = HostState {
            wasi:     builder.build(),
            event_tx: ctx.event_tx,
        };

        let mut store = Store::new(&self.engine, host_state);
        store.set_fuel(FUEL_LIMIT)?;

        let (plugin, _) = PluginWorld::instantiate(&mut store, &component, &linker)?;

        Ok(Box::new(WasmtimeInstance { store, plugin, initial_fuel: FUEL_LIMIT }))
    }
}

pub struct WasmtimeInstance {
    store:        Store<HostState>,
    plugin:       PluginWorld,
    initial_fuel: u64,
}

impl PluginInstance for WasmtimeInstance {
    fn handle_event(&mut self, meta_json: &[u8], payload: &[u8]) -> Result<(), String> {
        let meta: crate::event::EventMeta = serde_json::from_slice(meta_json)
            .map_err(|e| e.to_string())?;

        let wit_meta = EventMeta {
            id:        meta.id,
            topic:     meta.topic,
            version:   meta.version,
            timestamp: meta.timestamp,
        };

        // async handle_event через block_on — мы в spawn_blocking потоке
        let result = tokio::runtime::Handle::current()
            .block_on(self.plugin.call_handle_event(&mut self.store, &wit_meta, payload))
            .map_err(|e| e.to_string())?;

        if result == 0 { Ok(()) } else { Err(format!("plugin error {result}")) }
    }

    fn fuel_consumed(&self) -> u64 {
        self.initial_fuel.saturating_sub(self.store.get_fuel().unwrap_or(0))
    }
}
