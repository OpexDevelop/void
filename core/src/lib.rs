use extism::{Manifest as ExtismManifest, Plugin, Wasm};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    pub topic: String,
    pub data: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PluginResponse {
    pub log: Option<String>,
    pub emit: Vec<Event>,
}

#[derive(Deserialize, Clone)]
struct PluginConfig {
    name: String,
    wasm: String,
    subscriptions: Vec<String>,
    allowed_hosts: Option<Vec<String>>,
}

struct LoadedPlugin {
    plugin: Plugin,
    subscriptions: HashSet<String>,
}

pub struct Engine {
    plugins: Arc<RwLock<HashMap<String, LoadedPlugin>>>,
    pub tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
}

impl Engine {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            tx,
            rx,
        }
    }

    pub async fn load_plugins(&self, plugins_dir: &PathBuf) -> anyhow::Result<()> {
        if !plugins_dir.exists() { std::fs::create_dir_all(plugins_dir)?; }
        let mut r = self.plugins.write().await;
        
        if let Ok(entries) = std::fs::read_dir(plugins_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("toml") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if let Ok(config) = toml::from_str::<PluginConfig>(&content) {
                            let wasm_path = plugins_dir.join(&config.wasm);
                            if !wasm_path.exists() { continue; }

                            let wasm = Wasm::file(&wasm_path);
                            let mut manifest = ExtismManifest::new([wasm]);
                            
                            if let Some(hosts) = config.allowed_hosts {
                                manifest.allowed_hosts = Some(hosts);
                            }

                            if let Ok(p) = Plugin::new(&manifest, [], false) {
                                let subs: HashSet<String> = config.subscriptions.into_iter().collect();
                                r.insert(config.name.clone(), LoadedPlugin { plugin: p, subscriptions: subs });
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn run(mut self) {
        let tx_for_bus = self.tx.clone();
        let plugins = Arc::clone(&self.plugins);
        
        tokio::spawn(async move {
            while let Some(event) = self.rx.recv().await {
                let event_json = serde_json::to_string(&event).unwrap();
                let mut r = plugins.write().await;
                
                for (name, loaded) in r.iter_mut() {
                    if loaded.subscriptions.contains(&event.topic) {
                        if let Ok(res_str) = loaded.plugin.call::<&str, &str>("handle_event", &event_json) {
                            if let Ok(response) = serde_json::from_str::<PluginResponse>(res_str) {
                                if let Some(log_msg) = response.log {
                                    println!("  └─ [{}] log: {}", name, log_msg);
                                }
                                for new_event in response.emit {
                                    let _ = tx_for_bus.send(new_event).await;
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}
