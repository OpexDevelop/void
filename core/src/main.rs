use extism::{Manifest, Plugin, Wasm};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use notify::{Watcher, RecursiveMode};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Event {
    topic: String,
    data: String,
}

struct PluginManager {
    plugins: HashMap<String, Plugin>,
}

impl PluginManager {
    fn new() -> Self {
        Self { plugins: HashMap::new() }
    }

    fn load_plugin(&mut self, path: PathBuf) {
        if path.extension().and_then(|s| s.to_str()) != Some("wasm") { return; }
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let wasm = Wasm::file(&path);
        
        let mut manifest = Manifest::new([wasm]);
        manifest.allowed_hosts = Some(vec!["ntfy.sh".to_string()]);

        if let Ok(p) = Plugin::new(&manifest, [], false) {
            self.plugins.insert(name.clone(), p);
            println!("🔄 Плагин [ {} ] загружен/обновлен", name);
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let plugins_dir = PathBuf::from("./plugins");
    if !plugins_dir.exists() { std::fs::create_dir_all(&plugins_dir)?; }

    let manager = Arc::new(RwLock::new(PluginManager::new()));
    let (tx, mut rx) = mpsc::channel::<Event>(100);

    {
        let mut m = manager.write().await;
        if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                m.load_plugin(entry.path());
            }
        }
    }

    let manager_for_watcher = Arc::clone(&manager);
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(ev) = res {
            if ev.kind.is_modify() || ev.kind.is_create() {
                for path in ev.paths {
                    let m = Arc::clone(&manager_for_watcher);
                    tokio::spawn(async move {
                        let mut guard = m.write().await;
                        guard.load_plugin(path);
                    });
                }
            }
        }
    })?;
    watcher.watch(&plugins_dir, RecursiveMode::NonRecursive)?;

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            println!("[void] Обработка сообщения: '{}'", event.data);
            let event_json = serde_json::to_string(&event).unwrap();
            let mut m = manager.write().await;
            for (name, plugin) in m.plugins.iter_mut() {
                if let Ok(res) = plugin.call::<&str, &str>("handle_event", &event_json) {
                    println!("  └─ [{}] -> {}", name, res);
                }
            }
        }
    });

    println!("✅ void запущен. Автообновление плагинов включено.");

    let mut input = String::new();
    while std::io::stdin().read_line(&mut input).is_ok() {
        let text = input.trim();
        if text.is_empty() { continue; }
        if text == "/quit" { break; }
        
        let _ = tx.send(Event {
            topic: "user_input".to_string(),
            data: text.to_string(),
        }).await;
        
        input.clear();
    }

    Ok(())
}
