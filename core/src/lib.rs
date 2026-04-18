use extism::{Manifest as ExtismManifest, Plugin, Wasm};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    pub topic: String,
    pub data: String,
    #[serde(default)]
    pub ts: u64,
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
    allowed_paths: Option<HashMap<String, String>>,
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

                            if let Some(paths) = config.allowed_paths {
                                let mut btree = BTreeMap::new();
                                for (host_path, guest_path) in paths {
                                    let abs_host_path = std::fs::canonicalize(&host_path)
                                        .unwrap_or_else(|_| {
                                            let _ = std::fs::create_dir_all(&host_path);
                                            std::fs::canonicalize(&host_path).unwrap_or(PathBuf::from(&host_path))
                                        });
                                    btree.insert(abs_host_path.to_string_lossy().to_string(), PathBuf::from(guest_path));
                                }
                                manifest.allowed_paths = Some(btree);
                            }

                            if let Ok(p) = Plugin::new(&manifest, [], true) {
                                let subs: HashSet<String> = config.subscriptions.into_iter().collect();
                                r.insert(config.name.clone(), LoadedPlugin { plugin: p, subscriptions: subs });
                                println!("🔄 Загружен [ {} ]", config.name);
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
            while let Some(mut event) = self.rx.recv().await {
                if event.ts == 0 {
                    event.ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
                }
                
                let event_json = serde_json::to_string(&event).unwrap();
                let mut r = plugins.write().await;
                
                for (name, loaded) in r.iter_mut() {
                    if loaded.subscriptions.contains(&event.topic) {
                        match loaded.plugin.call::<&str, &str>("handle_event", &event_json) {
                            Ok(res_str) => {
                                if let Ok(response) = serde_json::from_str::<PluginResponse>(res_str) {
                                    if let Some(log_msg) = response.log {
                                        if event.topic != "SYS_TICK" {
                                            println!("  └─ [{}] log: {}", name, log_msg);
                                        }
                                    }
                                    for new_event in response.emit {
                                        let _ = tx_for_bus.send(new_event).await;
                                    }
                                }
                            }
                            Err(e) => {
                                if event.topic != "SYS_TICK" {
                                    eprintln!("  ![{}] CRITICAL ERR: {:?}", name, e);
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}
