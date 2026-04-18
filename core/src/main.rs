use extism::{Manifest, Plugin, Wasm};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
struct Event {
    topic: String,
    data: String,
}

fn main() {
    println!("📡 void: Загрузка плагинов...");
    
    let plugins_dir = Path::new("./plugins");
    let mut loaded_plugins = Vec::new();

    if let Ok(entries) = fs::read_dir(plugins_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                let name = path.file_stem().unwrap().to_str().unwrap().to_string();
                let wasm = Wasm::file(&path);
                let manifest = Manifest::new([wasm]);
                
                if let Ok(p) = Plugin::new(&manifest, [], false) {
                    loaded_plugins.push((name, p));
                    println!("  + запущен [ {} ]", entry.file_name().to_str().unwrap());
                }
            }
        }
    }

    if loaded_plugins.is_empty() {
        println!("❌ Нет плагинов для работы. Выход.");
        return;
    }

    println!("✅ void готов. Введи сообщение для рассылки:");

    let stdin = std::io::stdin();
    let mut input = String::new();

    while stdin.read_line(&mut input).is_ok() {
        let text = input.trim();
        if text == "/quit" { break; }

        let event = Event {
            topic: "user_input".to_string(),
            data: text.to_string(),
        };
        let event_json = serde_json::to_string(&event).unwrap();

        for (name, plugin) in loaded_plugins.iter_mut() {
            match plugin.call::<&str, &str>("handle_event", &event_json) {
                Ok(res) => println!("[{}] вернул: {}", name, res),
                Err(e) => eprintln!("![{}] ошибка: {:?}", name, e),
            }
        }
        
        input.clear();
    }
}
