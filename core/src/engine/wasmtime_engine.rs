use anyhow::Result;
use tokio::sync::mpsc;
use tracing::debug;
use wasmtime::{Caller, Config, Engine, Instance, Linker, Module, ResourceLimiter, Store};
use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

use crate::bus::{Event, EventMeta};
use super::{LinkerConfig, PluginInstance, PluginRuntime};

const FUEL_LIMIT:   u64   = 50_000_000;
const MEMORY_LIMIT: usize = 50 * 1024 * 1024;

struct PluginLimiter {
    memory_limit: usize,
}

impl ResourceLimiter for PluginLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool> {
        Ok(desired <= self.memory_limit)
    }

    fn table_growing(
        &mut self,
        _current: u32,
        _desired: u32,
        _maximum: Option<u32>,
    ) -> Result<bool> {
        Ok(true)
    }
}

struct HostState {
    wasi:     WasiP1Ctx,
    event_tx: mpsc::UnboundedSender<Event>,
    limiter:  PluginLimiter,
}

pub struct WasmtimeRuntime {
    engine: Engine,
}

impl WasmtimeRuntime {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        Ok(Self { engine: Engine::new(&config)? })
    }
}

fn read_mem_str(data: &[u8], ptr: i32, len: i32) -> Option<String> {
    let start = ptr as usize;
    let end   = start.checked_add(len as usize)?;
    if end > data.len() { return None; }
    std::str::from_utf8(&data[start..end]).ok().map(|s| s.to_string())
}

fn read_mem_bytes(data: &[u8], ptr: i32, len: i32) -> Option<Vec<u8>> {
    let start = ptr as usize;
    let end   = start.checked_add(len as usize)?;
    if end > data.len() { return None; }
    Some(data[start..end].to_vec())
}

impl PluginRuntime for WasmtimeRuntime {
    fn instantiate(&self, wasm_bytes: &[u8], cfg: LinkerConfig) -> Result<Box<dyn PluginInstance>> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let mut linker: Linker<HostState> = Linker::new(&self.engine);

        wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |s: &mut HostState| &mut s.wasi)?;

        {
            let tx = cfg.event_tx.clone();
            linker.func_wrap(
                "env",
                "emit_event",
                move |mut caller: Caller<'_, HostState>,
                      topic_ptr: i32, topic_len: i32,
                      payload_ptr: i32, payload_len: i32| {
                    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m,
                        None    => return,
                    };
                    let data    = mem.data(&caller);
                    let topic   = match read_mem_str(data, topic_ptr, topic_len) {
                        Some(t) => t,
                        None    => return,
                    };
                    let payload = match read_mem_bytes(data, payload_ptr, payload_len) {
                        Some(p) => p,
                        None    => return,
                    };
                    let meta  = EventMeta::new(&topic);
                    let event = Event { meta, payload };
                    let _     = tx.send(event);
                },
            )?;
        }

        if cfg.manifest.permissions.network {
            linker.func_wrap(
                "env",
                "host_http_post",
                |mut caller: Caller<'_, HostState>,
                 url_ptr: i32, url_len: i32,
                 body_ptr: i32, body_len: i32| -> i32 {
                    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m,
                        None    => return -1,
                    };
                    let data = mem.data(&caller);
                    let url  = match read_mem_str(data, url_ptr, url_len) {
                        Some(u) => u,
                        None    => return -1,
                    };
                    let body = match read_mem_bytes(data, body_ptr, body_len) {
                        Some(b) => b,
                        None    => return -1,
                    };
                    tokio::spawn(async move {
                        debug!(url = %url, bytes = body.len(), "host_http_post");
                        // Production: reqwest::Client::new().post(&url).body(body).send().await
                    });
                    0
                },
            )?;

            {
                let tx = cfg.event_tx.clone();
                linker.func_wrap(
                    "env",
                    "host_sse_start",
                    move |mut caller: Caller<'_, HostState>,
                          url_ptr: i32, url_len: i32| -> i32 {
                        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                            Some(m) => m,
                            None    => return -1,
                        };
                        let data = mem.data(&caller);
                        let url  = match read_mem_str(data, url_ptr, url_len) {
                            Some(u) => u,
                            None    => return -1,
                        };
                        let tx2 = tx.clone();
                        tokio::spawn(async move {
                            debug!(url = %url, "host_sse_start");
                            // Production: stream SSE from &url, emit Event { topic: NET_RECEIVED }
                            // Example skeleton:
                            // let resp = reqwest::get(&url).await?;
                            // while let Some(chunk) = resp.bytes_stream().next().await {
                            //     let _ = tx2.send(Event { meta: EventMeta::new(NET_RECEIVED), payload: chunk?.to_vec() });
                            // }
                            let _ = tx2;
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

        let wasi = ctx_builder.build_p1();

        let host_state = HostState {
            wasi,
            event_tx: cfg.event_tx,
            limiter: PluginLimiter { memory_limit: MEMORY_LIMIT },
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
        let alloc_fn = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "alloc")
            .map_err(|e| e.to_string())?;

        let meta_ptr    = alloc_fn.call(&mut self.store, meta_json.len() as i32).map_err(|e| e.to_string())?;
        let payload_ptr = alloc_fn.call(&mut self.store, payload.len().max(1) as i32).map_err(|e| e.to_string())?;

        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| "no memory export".to_string())?;

        memory.write(&mut self.store, meta_ptr as usize,    meta_json).map_err(|e| e.to_string())?;
        memory.write(&mut self.store, payload_ptr as usize, payload).map_err(|e| e.to_string())?;

        let handle_fn = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), i32>(&mut self.store, "handle_event")
            .map_err(|e| e.to_string())?;

        let result = handle_fn
            .call(
                &mut self.store,
                (meta_ptr, meta_json.len() as i32, payload_ptr, payload.len() as i32),
            )
            .map_err(|e| e.to_string())?;

        if let Ok(dealloc_fn) =
            self.instance.get_typed_func::<(i32, i32), ()>(&mut self.store, "dealloc")
        {
            let _ = dealloc_fn.call(&mut self.store, (meta_ptr,    meta_json.len() as i32));
            let _ = dealloc_fn.call(&mut self.store, (payload_ptr, payload.len() as i32));
        }

        if result == 0 { Ok(()) } else { Err(format!("plugin error code {result}")) }
    }

    fn fuel_consumed(&self) -> u64 {
        self.initial_fuel.saturating_sub(self.store.get_fuel().unwrap_or(0))
    }
}
