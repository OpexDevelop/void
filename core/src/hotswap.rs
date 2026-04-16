use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use notify::{Event as FsEvent, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::event::{Event, EventMeta, SYS_SHUTDOWN};

pub struct HotSwapConfig {
    pub graceful_timeout: Duration,
}

impl Default for HotSwapConfig {
    fn default() -> Self {
        Self { graceful_timeout: Duration::from_millis(500) }
    }
}

pub struct HotSwapper {
    config:   HotSwapConfig,
    watchers: HashMap<String, PathBuf>,
    plugin_txs: HashMap<String, mpsc::Sender<Event>>,
}

impl HotSwapper {
    pub fn new(config: HotSwapConfig) -> Self {
        Self {
            config,
            watchers:   HashMap::new(),
            plugin_txs: HashMap::new(),
        }
    }

    pub fn register(&mut self, id: &str, wasm_path: PathBuf, tx: mpsc::Sender<Event>) {
        self.watchers.insert(id.to_string(), wasm_path);
        self.plugin_txs.insert(id.to_string(), tx);
    }

    pub fn start(self) -> Result<RecommendedWatcher> {
        let (fs_tx, mut fs_rx) = mpsc::unbounded_channel::<PathBuf>();

        let plugin_txs     = self.plugin_txs.clone();
        let path_to_id: HashMap<PathBuf, String> = self.watchers
            .iter()
            .map(|(id, path)| (path.clone(), id.clone()))
            .collect();
        let timeout = self.config.graceful_timeout;

        tokio::spawn(async move {
            while let Some(changed) = fs_rx.recv().await {
                let id = match path_to_id.get(&changed) {
                    Some(id) => id.clone(),
                    None     => continue,
                };

                info!(plugin = %id, path = ?changed, "hot-swap detected");

                if let Some(tx) = plugin_txs.get(&id) {
                    let ev = Event { meta: EventMeta::new(SYS_SHUTDOWN), payload: vec![] };
                    let _ = tx.try_send(ev);
                }

                tokio::time::sleep(timeout).await;

                match std::fs::read(&changed) {
                    Ok(_)  => info!(plugin = %id, "hot-swap: new wasm loaded"),
                    Err(e) => error!(plugin = %id, error = %e, "hot-swap: read failed"),
                }
            }
        });

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<FsEvent>| {
            if let Ok(ev) = res {
                for path in ev.paths {
                    let _ = fs_tx.send(path);
                }
            }
        })?;

        for path in self.watchers.values() {
            if path.exists() {
                watcher.watch(path, RecursiveMode::NonRecursive)?;
            }
        }

        Ok(watcher)
    }
}
