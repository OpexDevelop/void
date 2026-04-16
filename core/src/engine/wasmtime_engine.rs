use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::debug;
use wasmtime::{Caller, Config, Engine, Instance, Linker, Module, ResourceLimiter, Store};
use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

use crate::bus::{Event, EventMeta, NET_RECEIVED};
use super::{LinkerConfig, PluginInstance, PluginRuntime};

const FUEL_LIMIT:   u64   = 50_000_000;
const MEMORY_LIMIT: usize = 50 * 1024 * 1024;

struct PluginLimiter { memory_limit: usize }

impl ResourceLimiter for PluginLimiter {
    fn memory_growing(&mut self, _cur: usize, desired: usize, _max: Option<usize>) -> Result<bool> {
        Ok(desired <= self.memory_limit)
    }
    fn table_growing(&mut self, _cur: u32, _des: u32, _max: Option<u32>) -> Result<bool> {
        Ok(true)
    }
}

struct HostState {
    wasi:     WasiP1Ctx,
    event_tx: mpsc::UnboundedSender<Event>,
    limiter:  PluginLimiter,
}

pub struct WasmtimeRuntime { engine: Engine }

impl WasmtimeRuntime {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        Ok(Self { engine: Engine::new(&config)? })
    }
}

fn read_str(data: &[u8], ptr: i32, len: i32) -> Option<String> {
    let s = ptr as usize;
    let e = s.checked_add(len as usize)?;
    if e > data.len() { return None; }
    std::str::from_utf8(&data[s..e]).ok().map(|x| x.to_string())
}

fn read_bytes(data: &[u8], ptr: i32, len: i32) -> Option<Vec<u8>> {
    let s = ptr as usize;
    let e = s.checked_add(len as usize)?;
    if e > data.len() { return None; }
    Some(data[s..e].to_vec())
}

async fn sse_loop(url: String, tx: mpsc::UnboundedSender<Event>) {
    tracing::info!(url = %url, "SSE stream starting");
    loop {
        let client = reqwest::Client::new();
        let response = match client
            .get(&url)
            .header("Accept", "text/event-stream")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "SSE connect failed, retry in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        for line in text.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "{}" || data.trim().is_empty() {
                                    continue;
                                }
                                let payload = data.as_bytes().to_vec();
                                let event = Event {
                                    meta:    EventMeta::new(NET_RECEIVED),
                                    payload,
                                };
                                if tx.send(event).is_err() {
                                    tracing::warn!("SSE: event bus closed");
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "SSE stream error, reconnecting");
                    break;
                }
            }
        }

        tracing::info!("SSE stream ended, reconnecting in 3s");
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

impl PluginRuntime for WasmtimeRuntime {
    fn instantiate(&self, wasm_bytes: &[u8], cfg: LinkerConfig) -> Result<Box<dyn PluginInstance>> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let mut linker: Linker<HostState> = Linker::new(&self.engine);

        wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |s: &mut HostState| &mut s.wasi)?;

        {
            let tx = cfg.event_tx.clone();
            linker.func_wrap(
                "env", "emit_event",
                move |mut caller: Caller<'_, HostState>,
                      topic_ptr: i32, topic_len: i32,
                      payload_ptr: i32, payload_len: i32| {
                    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m, None => return,
                    };
                    let data    = mem.data(&caller);
                    let topic   = match read_str(data, topic_ptr, topic_len) { Some(t) => t, None => return };
                    let payload = match read_bytes(data, payload_ptr, payload_len) { Some(p) => p, None => return };
                    let _ = tx.send(Event { meta: EventMeta::new(&topic), payload });
                },
            )?;
        }

        if cfg.manifest.permissions.network {
            linker.func_wrap(
                "env", "host_http_post",
                |mut caller: Caller<'_, HostState>,
                 url_ptr: i32, url_len: i32,
                 body_ptr: i32, body_len: i32| -> i32 {
                    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m, None => return -1,
                    };
                    let data = mem.data(&caller);
                    let url  = match read_str(data, url_ptr, url_len) { Some(u) => u, None => return -1 };
                    let body = match read_bytes(data, body_ptr, body_len) { Some(b) => b, None => return -1 };
                    tokio::spawn(async move {
                        let client = reqwest::Client::new();
                        match client
                            .post(&url)
                            .header("Content-Type", "text/plain")
                            .body(body)
                            .send()
                            .await
                        {
                            Ok(resp) => tracing::info!(url = %url, status = %resp.status(), "http_post ok"),
                            Err(e)   => tracing::error!(url = %url, error = %e, "http_post failed"),
                        }
                    });
                    0
                },
            )?;

            {
                let tx = cfg.event_tx.clone();
                linker.func_wrap(
                    "env", "host_sse_start",
                    move |mut caller: Caller<'_, HostState>,
                          url_ptr: i32, url_len: i32| -> i32 {
                        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                            Some(m) => m, None => return -1,
                        };
                        let data = mem.data(&caller);
                        let url  = match read_str(data, url_ptr, url_len) { Some(u) => u, None => return -1 };
                        let tx2  = tx.clone();
                        tokio::spawn(async move {
                            sse_loop(url, tx2).await;
                        });
                        0
                    },
                )?;
            }
        }

        let mut ctx_builder = WasiCtxBuilder::new();
        ctx_builder.inherit_stdio();

        if cfg.manifest.permissions.filesystem {
            for dir in &cfg.manifest.permissions.allowed_dirs {
                std::fs::create_dir_all(dir)?;
                ctx_builder.preopened_dir(dir, ".", DirPerms::all(), FilePerms::all())?;
            }
        }

        let host_state = HostState {
            wasi:     ctx_builder.build_p1(),
            event_tx: cfg.event_tx,
            limiter:  PluginLimiter { memory_limit: MEMORY_LIMIT },
        };

        let mut store = Store::new(&self.engine, host_state);
        store.set_fuel(FUEL_LIMIT)?;
        store.limiter(|s| &mut s.limiter);

        let instance = linker.instantiate(&mut store, &module)?;
        Ok(Box::new(WasmtimeInstance { store, instance, initial_fuel: FUEL_LIMIT }))
    }
}

pub struct WasmtimeInstance {
    store:        Store<HostState>,
    instance:     Instance,
    initial_fuel: u64,
}

impl PluginInstance for WasmtimeInstance {
    fn handle_event(&mut self, meta_json: &[u8], payload: &[u8]) -> Result<(), String> {
        let alloc = self.instance
            .get_typed_func::<i32, i32>(&mut self.store, "alloc")
            .map_err(|e| e.to_string())?;

        let meta_ptr    = alloc.call(&mut self.store, meta_json.len() as i32).map_err(|e| e.to_string())?;
        let payload_ptr = alloc.call(&mut self.store, payload.len().max(1) as i32).map_err(|e| e.to_string())?;

        let memory = self.instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| "no memory export".to_string())?;

        memory.write(&mut self.store, meta_ptr as usize,    meta_json).map_err(|e| e.to_string())?;
        memory.write(&mut self.store, payload_ptr as usize, payload).map_err(|e| e.to_string())?;

        let handle = self.instance
            .get_typed_func::<(i32, i32, i32, i32), i32>(&mut self.store, "handle_event")
            .map_err(|e| e.to_string())?;

        let result = handle
            .call(&mut self.store, (meta_ptr, meta_json.len() as i32, payload_ptr, payload.len() as i32))
            .map_err(|e| e.to_string())?;

        if let Ok(dealloc) = self.instance.get_typed_func::<(i32, i32), ()>(&mut self.store, "dealloc") {
            let _ = dealloc.call(&mut self.store, (meta_ptr,    meta_json.len() as i32));
            let _ = dealloc.call(&mut self.store, (payload_ptr, payload.len() as i32));
        }

        if result == 0 { Ok(()) } else { Err(format!("plugin error code {result}")) }
    }

    fn fuel_consumed(&self) -> u64 {
        self.initial_fuel.saturating_sub(self.store.get_fuel().unwrap_or(0))
    }
}
