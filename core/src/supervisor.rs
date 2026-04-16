use std::sync::Arc;
use std::time::Duration;
use std::path::PathBuf;

use anyhow::Result;
use notify::{Event as FsEvent, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::bus::{Event, EventMeta, SYS_SHUTDOWN};
use crate::engine::{LinkerConfig, PluginInstance, PluginRuntime};
use crate::manifest::{PluginManifest, RestartPolicy};

struct PluginEntry {
    manifest:   PluginManifest,
    wasm_bytes: Vec<u8>,
    tx:         mpsc::Sender<Event>,
}

pub struct Supervisor {
    runtime:   Arc<dyn PluginRuntime>,
    global_tx: mpsc::UnboundedSender<Event>,
    dlq_tx:    mpsc::UnboundedSender<Event>,
    plugins:   Vec<PluginEntry>,
}

impl Supervisor {
    pub fn new(
        runtime:   Arc<dyn PluginRuntime>,
        global_tx: mpsc::UnboundedSender<Event>,
        dlq_tx:    mpsc::UnboundedSender<Event>,
    ) -> Self {
        Self { runtime, global_tx, dlq_tx, plugins: Vec::new() }
    }

    pub async fn load_plugin(
        &mut self,
        manifest:   PluginManifest,
        wasm_bytes: Vec<u8>,
    ) -> Result<()> {
        let queue_size = manifest.events.max_queue_size;
        let (tx, rx)   = mpsc::channel::<Event>(queue_size);

        info!(id = %manifest.plugin.id, version = %manifest.plugin.version, "Loading plugin");

        spawn_plugin_worker(
            Arc::clone(&self.runtime),
            wasm_bytes.clone(),
            manifest.clone(),
            rx,
            self.global_tx.clone(),
            self.dlq_tx.clone(),
        );

        self.plugins.push(PluginEntry { manifest, wasm_bytes, tx });
        Ok(())
    }

    pub fn start_routing(&self, mut global_rx: mpsc::UnboundedReceiver<Event>) {
        let routes: Vec<(Vec<String>, mpsc::Sender<Event>)> = self
            .plugins
            .iter()
            .map(|p| (p.manifest.events.subscriptions.clone(), p.tx.clone()))
            .collect();

        let dlq_tx = self.dlq_tx.clone();

        tokio::spawn(async move {
            while let Some(event) = global_rx.recv().await {
                debug!(topic = %event.meta.topic, id = %event.meta.id, "routing");

                let mut routed = false;
                for (subs, tx) in &routes {
                    if subs.iter().any(|s| s == &event.meta.topic) {
                        routed = true;
                        match tx.try_send(event.clone()) {
                            Ok(_) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                warn!(topic = %event.meta.topic, "queue full → DLQ");
                                let _ = dlq_tx.send(event.clone());
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                warn!(topic = %event.meta.topic, "plugin channel closed");
                            }
                        }
                    }
                }

                if !routed {
                    debug!(topic = %event.meta.topic, "no subscribers");
                }
            }
        });
    }

    pub fn start_hot_swap_watcher(&self) -> Result<RecommendedWatcher> {
        let plugin_paths: Vec<(String, String, mpsc::Sender<Event>)> = self
            .plugins
            .iter()
            .map(|p| (
                p.manifest.plugin.id.clone(),
                p.manifest.plugin.wasm_path.clone(),
                p.tx.clone(),
            ))
            .collect();

        let (fs_tx, mut fs_rx) = mpsc::unbounded_channel::<PathBuf>();

        tokio::spawn(async move {
            while let Some(path) = fs_rx.recv().await {
                let path_str = path.to_string_lossy().to_string();
                for (id, wasm_path, plugin_tx) in &plugin_paths {
                    if *wasm_path == path_str {
                        info!(plugin = %id, "Hot-swap detected");
                        let shutdown = Event {
                            meta:    EventMeta::new(SYS_SHUTDOWN),
                            payload: vec![],
                        };
                        let _ = plugin_tx.try_send(shutdown);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        match std::fs::read(wasm_path) {
                            Ok(_)  => info!(plugin = %id, "Hot-swap complete"),
                            Err(e) => error!(plugin = %id, error = %e, "Hot-swap failed"),
                        }
                    }
                }
            }
        });

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<FsEvent>| {
            if let Ok(event) = res {
                for path in event.paths {
                    let _ = fs_tx.send(path);
                }
            }
        })?;

        for entry in &self.plugins {
            let path = std::path::Path::new(&entry.manifest.plugin.wasm_path);
            if path.exists() {
                watcher.watch(path, RecursiveMode::NonRecursive)?;
            }
        }

        Ok(watcher)
    }
}

fn spawn_plugin_worker(
    runtime:    Arc<dyn PluginRuntime>,
    wasm_bytes: Vec<u8>,
    manifest:   PluginManifest,
    rx:         mpsc::Receiver<Event>,
    global_tx:  mpsc::UnboundedSender<Event>,
    dlq_tx:     mpsc::UnboundedSender<Event>,
) {
    tokio::spawn(async move {
        plugin_worker_loop(runtime, wasm_bytes, manifest, rx, global_tx, dlq_tx).await;
    });
}

async fn plugin_worker_loop(
    runtime:    Arc<dyn PluginRuntime>,
    wasm_bytes: Vec<u8>,
    manifest:   PluginManifest,
    mut rx:     mpsc::Receiver<Event>,
    global_tx:  mpsc::UnboundedSender<Event>,
    dlq_tx:     mpsc::UnboundedSender<Event>,
) {
    let id = manifest.plugin.id.clone();

    // ┌─────────────────────────────────────────────────────────────────────┐
    // │ FIX: wasmtime-wasi preview1 sync bridge вызывает Handle::block_on   │
    // │ внутри async-задачи → panic.                                         │
    // │ block_in_place сигнализирует планировщику "этот поток будет блокирован│
    // │ — мигрируй другие задачи", что позволяет block_on работать без panic. │
    // └─────────────────────────────────────────────────────────────────────┘
    let make_instance = || {
        let rt    = Arc::clone(&runtime);
        let bytes = wasm_bytes.clone();
        let cfg   = LinkerConfig {
            event_tx: global_tx.clone(),
            manifest: manifest.clone(),
        };
        tokio::task::block_in_place(|| rt.instantiate(&bytes, cfg))
    };

    let mut instance: Box<dyn PluginInstance> = match make_instance() {
        Ok(i)  => i,
        Err(e) => {
            error!(plugin = %id, error = %e, "instantiation failed");
            return;
        }
    };

    let mut retry_count = 0u32;

    while let Some(event) = rx.recv().await {
        if event.meta.topic == SYS_SHUTDOWN {
            info!(plugin = %id, "SYS_SHUTDOWN received, stopping");
            break;
        }

        let meta_json = match serde_json::to_vec(&event.meta) {
            Ok(v)  => v,
            Err(e) => {
                error!(plugin = %id, error = %e, "meta serialization failed");
                let _ = dlq_tx.send(event);
                continue;
            }
        };

        let fuel_before = instance.fuel_consumed();

        // FIX: любой WASI syscall (fs read/write/rename) идёт через sync bridge
        // → без block_in_place storage падает на первом же обращении к диску
        let handle_result = tokio::task::block_in_place(|| {
            instance.handle_event(&meta_json, &event.payload)
        });

        match handle_result {
            Ok(_) => {
                let fuel_used = instance.fuel_consumed().saturating_sub(fuel_before);
                debug!(
                    plugin = %id,
                    topic  = %event.meta.topic,
                    fuel   = fuel_used,
                    "handled"
                );
                retry_count = 0;
            }
            Err(e) => {
                error!(plugin = %id, topic = %event.meta.topic, error = %e, "handler error");
                let _ = dlq_tx.send(event);

                let should_restart = matches!(
                    manifest.supervisor.restart_policy,
                    RestartPolicy::Always | RestartPolicy::OnFailure
                );

                if should_restart && retry_count < manifest.supervisor.max_retries {
                    retry_count += 1;
                    warn!(plugin = %id, attempt = retry_count, "restarting");
                    match make_instance() {
                        Ok(new) => { instance = new; }
                        Err(e2) => {
                            error!(plugin = %id, error = %e2, "restart failed");
                            return;
                        }
                    }
                } else {
                    error!(plugin = %id, "max retries exceeded, stopping worker");
                    return;
                }
            }
        }
    }

    info!(plugin = %id, "worker stopped");
}
