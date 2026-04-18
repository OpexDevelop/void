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

struct Router {
    plugins: HashMap<String, LoadedPlugin>,
}

impl Router {
    fn new() -> Self {
        Self { plugins: HashMap::new() }
    }

    fn load_from_toml(&mut self, toml_path: &PathBuf, base_dir: &PathBuf) {
        if let Ok(content) = std::fs::read_to_string(toml_path) {
            if let Ok(config) = toml::from_str::<PluginConfig>(&content) {
                let wasm_path = base_dir.join(&config.wasm);
                if !wasm_path.exists() { return; }

                let wasm = Wasm::file(&wasm_path);
                let mut manifest = ExtismManifest::new([wasm]);
                
                if let Some(hosts) = config.allowed_hosts {
                    manifest.allowed_hosts = Some(hosts);
                }

                if let Ok(p) = Plugin::new(&manifest, [], false) {
                    let subs: HashSet<String> = config.subscriptions.into_iter().collect();
                    self.plugins.insert(config.name.clone(), LoadedPlugin { plugin: p, subscriptions: subs });
                    println!("🔄 Загружен плагин [ {} ] (Топики: {:?})", config.name, self.plugins[&config.name].subscriptions);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let plugins_dir = PathBuf::from("./plugins");
    if !plugins_dir.exists() { std::fs::create_dir_all(&plugins_dir)?; }

    let router = Arc::new(RwLock::new(Router::new()));
    let (tx, mut rx) = mpsc::channel::<Event>(1000);

    {
        let mut r = router.write().await;
        if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("toml") {
                    r.load_from_toml(&entry.path(), &plugins_dir);
                }
            }
        }
    }

    let tx_for_bus = tx.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let event_json = serde_json::to_string(&event).unwrap();
            let mut r = router.write().await;
            
            for (name, loaded) in r.plugins.iter_mut() {
                if loaded.subscriptions.contains(&event.topic) {
                    if let Ok(res_str) = loaded.plugin.call::<&str, &str>("handle_event", &event_json) {
                        if let Ok(response) = serde_json::from_str::<PluginResponse>(res_str) {
                            if let Some(log_msg) = response.log {
                                println!("  └─ [{}] log: {}", name, log_msg);
                            }
                            for new_event in response.emit {
                                println!("  ↗  [{}] emit: {}", name, new_event.topic);
                                let _ = tx_for_bus.send(new_event).await;
                            }
                        }
                    }
                }
            }
        }
    });

    println!("✅ void Router запущен. Введите сообщение:");

    let mut input = String::new();
    while std::io::stdin().read_line(&mut input).is_ok() {
        let text = input.trim();
        if text.is_empty() { continue; }
        if text == "/quit" { break; }
        
        println!("\n[void] -> UI_SEND_MSG: '{}'", text);
        let _ = tx.send(Event {
            topic: "UI_SEND_MSG".to_string(),
            data: text.to_string(),
        }).await;
        
        input.clear();
    }

    Ok(())
}
