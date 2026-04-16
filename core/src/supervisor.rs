use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use notify::RecommendedWatcher;
use tokio::sync::mpsc;
use tracing::info;

use crate::engine::PluginRuntime;
use crate::event::{BusRx, BusTx, Event};
use crate::hotswap::{HotSwapConfig, HotSwapper};
use crate::manifest::PluginManifest;
use crate::registry::Registry;
use crate::router::{Router, Sender};
use crate::worker;

struct LoadedPlugin {
    manifest: PluginManifest,
    tx:       mpsc::Sender<Event>,
}

pub struct Supervisor {
    runtime:   Arc<dyn PluginRuntime>,
    global_tx: BusTx,
    dlq_tx:    mpsc::UnboundedSender<Event>,
    registry:  Registry,
    router:    Router,
    plugins:   HashMap<String, LoadedPlugin>,
}

impl Supervisor {
    pub fn new(
        runtime:   Arc<dyn PluginRuntime>,
        global_tx: BusTx,
        dlq_tx:    mpsc::UnboundedSender<Event>,
    ) -> Self {
        let router = Router::new(dlq_tx.clone());
        Self {
            runtime,
            global_tx,
            dlq_tx,
            registry: Registry::new(),
            router,
            plugins: HashMap::new(),
        }
    }

    pub async fn load_plugin(
        &mut self,
        manifest:   PluginManifest,
        wasm_bytes: Vec<u8>,
    ) -> Result<()> {
        let id         = manifest.plugin.id.clone();
        let queue_size = manifest.events.max_queue_size;

        self.registry.load(manifest.clone(), wasm_bytes.clone())?;

        let (tx, rx) = mpsc::channel::<Event>(queue_size);

        info!(id = %id, version = %manifest.plugin.version, "loading plugin");

        worker::spawn(
            Arc::clone(&self.runtime),
            wasm_bytes,
            manifest.clone(),
            rx,
            self.global_tx.clone(),
            self.dlq_tx.clone(),
        );

        self.router.register(&id, &manifest.events.subscriptions, Sender::Bounded(tx.clone()));
        self.plugins.insert(id, LoadedPlugin { manifest, tx });

        Ok(())
    }

    pub fn subscribe_host(
        &mut self,
        topics: &[String],
        tx:     mpsc::UnboundedSender<Event>,
    ) {
        self.router.register("__host__", topics, Sender::Unbounded(tx));
    }

    pub fn start_routing(&self, mut global_rx: BusRx) {
        let table = self.router.clone_table();

        tokio::spawn(async move {
            while let Some(event) = global_rx.recv().await {
                table.route(&event);
            }
        });
    }

    pub fn start_hot_swap(&self, config: HotSwapConfig) -> Result<RecommendedWatcher> {
        let mut swapper = HotSwapper::new(config);
        for (id, entry) in &self.plugins {
            swapper.register(
                id,
                PathBuf::from(&entry.manifest.plugin.wasm_path),
                entry.tx.clone(),
            );
        }
        swapper.start()
    }
}
