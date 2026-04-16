use anyhow::{anyhow, Result};
use wasmi::{Engine, Func, Instance, Linker, Memory, Module, Store, Value};
use tokio::sync::mpsc;

use crate::bus::{Event, EventMeta};
use super::{LinkerConfig, PluginInstance, PluginRuntime};

struct HostState {
    event_tx: mpsc::UnboundedSender<Event>,
}

pub struct WasmiRuntime {
    engine: Engine,
}

impl WasmiRuntime {
    pub fn new() -> Result<Self> {
        Ok(Self { engine: Engine::default() })
    }
}

impl PluginRuntime for WasmiRuntime {
    fn instantiate(&self, wasm_bytes: &[u8], cfg: LinkerConfig) -> Result<Box<dyn PluginInstance>> {
        let module = Module::new(&self.engine, wasm_bytes)?;

        let host_state = HostState { event_tx: cfg.event_tx.clone() };
        let mut store: Store<HostState> = Store::new(&self.engine, host_state);

        let mut linker = Linker::<HostState>::new(&self.engine);

        let tx = cfg.event_tx.clone();
        linker.func_wrap(
            "env",
            "emit_event",
            move |mut caller: wasmi::Caller<'_, HostState>,
                  topic_ptr: i32, topic_len: i32,
                  payload_ptr: i32, payload_len: i32| {
                let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                    Some(m) => m,
                    None    => return,
                };
                let data    = mem.data(caller.as_context());
                let t_s     = topic_ptr   as usize;
                let t_e     = t_s + topic_len   as usize;
                let p_s     = payload_ptr as usize;
                let p_e     = p_s + payload_len as usize;
                if t_e > data.len() || p_e > data.len() { return; }
                let topic   = match std::str::from_utf8(&data[t_s..t_e]) { Ok(s) => s.to_string(), Err(_) => return };
                let payload = data[p_s..p_e].to_vec();
                let _       = tx.send(Event { meta: EventMeta::new(&topic), payload });
            },
        )?;

        if cfg.manifest.permissions.network {
            linker.func_wrap("env", "host_http_post",
                |_: wasmi::Caller<'_, HostState>, _: i32, _: i32, _: i32, _: i32| -> i32 { 0 })?;
            linker.func_wrap("env", "host_sse_start",
                |_: wasmi::Caller<'_, HostState>, _: i32, _: i32| -> i32 { 0 })?;
        }

        let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

        Ok(Box::new(WasmiInstance { store, instance }))
    }
}

pub struct WasmiInstance {
    store:    Store<HostState>,
    instance: Instance,
}

impl PluginInstance for WasmiInstance {
    fn handle_event(&mut self, meta_json: &[u8], payload: &[u8]) -> Result<(), String> {
        let alloc_fn = self.instance
            .get_typed_func::<i32, i32>(&self.store, "alloc")
            .map_err(|e| e.to_string())?;

        let meta_ptr    = alloc_fn.call(&mut self.store, meta_json.len() as i32).map_err(|e| e.to_string())?;
        let payload_ptr = alloc_fn.call(&mut self.store, payload.len().max(1) as i32).map_err(|e| e.to_string())?;

        let memory = self.instance
            .get_memory(&self.store, "memory")
            .ok_or_else(|| "no memory export".to_string())?;

        memory.write(&mut self.store, meta_ptr as usize,    meta_json).map_err(|e| e.to_string())?;
        memory.write(&mut self.store, payload_ptr as usize, payload).map_err(|e| e.to_string())?;

        let handle_fn = self.instance
            .get_typed_func::<(i32, i32, i32, i32), i32>(&self.store, "handle_event")
            .map_err(|e| e.to_string())?;

        let result = handle_fn
            .call(&mut self.store, (meta_ptr, meta_json.len() as i32, payload_ptr, payload.len() as i32))
            .map_err(|e| e.to_string())?;

        if let Ok(dealloc_fn) = self.instance.get_typed_func::<(i32, i32), ()>(&self.store, "dealloc") {
            let _ = dealloc_fn.call(&mut self.store, (meta_ptr,    meta_json.len() as i32));
            let _ = dealloc_fn.call(&mut self.store, (payload_ptr, payload.len() as i32));
        }

        if result == 0 { Ok(()) } else { Err(format!("wasmi plugin error {result}")) }
    }

    fn fuel_consumed(&self) -> u64 {
        0
    }
}
