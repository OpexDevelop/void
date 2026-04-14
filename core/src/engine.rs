use std::collections::HashMap;
use std::sync::Mutex;
use extism::{Manifest, Plugin, Wasm};
use serde::{Deserialize, Serialize};
use flutter_rust_bridge::frb;
use crate::manifest::{parse_manifest, PluginManifest};

// ── Public types (FRB генерирует Dart классы) ─────────────────────────────────

#[frb]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub category: String,
    pub description: String,
    pub active: bool,
    pub network: bool,
    pub filesystem: bool,
}

#[frb]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub text: String,
    pub timestamp: u64,
}

#[frb]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub kind: String,
    pub from: String,
    pub text: String,
    pub timestamp: u64,
}

// ── Internal plugin storage ───────────────────────────────────────────────────

struct LoadedPlugin {
    plugin: Mutex<Plugin>,
    info: PluginInfo,
    manifest: PluginManifest,
}

unsafe impl Send for LoadedPlugin {}
unsafe impl Sync for LoadedPlugin {}

// ── Engine ────────────────────────────────────────────────────────────────────

pub struct Engine {
    plugins: HashMap<String, LoadedPlugin>,
    events: Vec<Event>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            events: Vec::new(),
        }
    }

    // ── Plugin management ─────────────────────────────────────────────────────

    pub fn load_plugin(&mut self, wasm: Vec<u8>, manifest_toml: &str) -> Result<PluginInfo, String> {
        let manifest = parse_manifest(manifest_toml)?;

        let id = manifest.plugin.id.clone();
        if self.plugins.contains_key(&id) {
            return Err(format!("plugin '{}' already loaded", id));
        }

        let info = PluginInfo {
            id: id.clone(),
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            category: manifest.plugin.category.clone(),
            description: manifest.plugin.description.clone(),
            active: true,
            network: manifest.permissions.network,
            filesystem: manifest.permissions.filesystem,
        };

        let wasm_src = Wasm::data(wasm);
        let mut ext = Manifest::new([wasm_src]);

        // Лимит памяти
        let pages = (manifest.limits.max_memory_mb * 1024 / 64).max(1);
        ext = ext.with_memory_max(pages);

        // Сеть
        if manifest.permissions.network {
            ext = ext.with_allowed_host("*");
        }

        let plugin = Plugin::new(&ext, [], true)
            .map_err(|e| format!("wasm init failed: {e}"))?;

        self.plugins.insert(id, LoadedPlugin {
            plugin: Mutex::new(plugin),
            info: info.clone(),
            manifest,
        });

        Ok(info)
    }

    pub fn unload_plugin(&mut self, id: &str) {
        if let Some(lp) = self.plugins.get(id) {
            if lp.manifest.capabilities.lifecycle.contains(&"on_unload".to_string()) {
                let _ = self.call(id, "on_unload", "{}");
            }
        }
        self.plugins.remove(id);
    }

    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.values().map(|p| p.info.clone()).collect()
    }

    // ── Plugin calls ──────────────────────────────────────────────────────────

    fn call(&self, id: &str, func: &str, input: &str) -> Result<String, String> {
        let lp = self.plugins.get(id)
            .ok_or_else(|| format!("plugin '{}' not found", id))?;
        let mut p = lp.plugin.lock().unwrap();
        p.call::<&str, &str>(func, input)
            .map(|s| s.to_string())
            .map_err(|e| format!("{func} failed: {e}"))
    }

    fn find_category(&self, cat: &str) -> Option<&str> {
        self.plugins.values()
            .find(|p| p.info.category == cat && p.info.active)
            .map(|p| p.info.id.as_str())
    }

    // ── Transport ─────────────────────────────────────────────────────────────

    pub fn configure_transport(&self, address: &str) -> Result<(), String> {
        match self.find_category("transport") {
            Some(id) => {
                let input = serde_json::json!({ "address": address }).to_string();
                let raw = self.call(id, "configure", &input)?;
                let v: serde_json::Value = serde_json::from_str(&raw)
                    .map_err(|e| e.to_string())?;
                if v["ok"] == true {
                    Ok(())
                } else {
                    Err(v["error"].as_str().unwrap_or("configure failed").to_string())
                }
            }
            None => {
                // Нет transport плагина — offline, не ошибка
                Ok(())
            }
        }
    }

    pub fn poll_incoming(&mut self, since_ts: u64) -> u32 {
        let id = match self.find_category("transport").map(|s| s.to_string()) {
            Some(id) => id,
            None => return 0,
        };

        let input = serde_json::json!({ "since": since_ts, "limit": 50 }).to_string();
        let raw = match self.call(&id, "get_pending", &input) {
            Ok(r) => r,
            Err(_) => return 0,
        };

        let parsed: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => return 0,
        };

        let msgs = match parsed["messages"].as_array() {
            Some(a) => a.clone(),
            None => return 0,
        };

        let mut count = 0u32;
        for msg in &msgs {
            let from = msg["from_topic"].as_str().unwrap_or("unknown").to_string();
            let payload = msg["payload_b64"].as_str().unwrap_or("").to_string();
            let ts = msg["timestamp"].as_u64().unwrap_or_else(now_secs);
            let text = self.decrypt(&payload);

            self.store_message("incoming", &from, "me", &text, ts);
            self.events.push(Event { kind: "message_received".to_string(), from, text, timestamp: ts });
            count += 1;
        }
        count
    }

    // ── Messaging ─────────────────────────────────────────────────────────────

    pub fn send_message(&mut self, to: &str, text: &str) -> Result<(), String> {
        let payload = self.encrypt(text);
        let ts = now_secs();

        self.store_message("outgoing", "me", to, text, ts);

        match self.find_category("transport").map(|s| s.to_string()) {
            Some(id) => {
                let input = serde_json::json!({
                    "to_topic": to,
                    "payload_b64": payload,
                }).to_string();
                let raw = self.call(&id, "send", &input)?;
                let v: serde_json::Value = serde_json::from_str(&raw)
                    .map_err(|e| e.to_string())?;
                if v["ok"] == true {
                    Ok(())
                } else {
                    Err(v["error"].as_str().unwrap_or("send failed").to_string())
                }
            }
            None => Ok(()), // offline — сохранили локально
        }
    }

    pub fn get_messages(&self, contact: &str) -> Vec<Message> {
        match self.find_category("storage").map(|s| s.to_string()) {
            Some(id) => {
                let input = serde_json::json!({ "contact": contact }).to_string();
                match self.call(&id, "get_messages", &input) {
                    Ok(raw) => serde_json::from_str::<Vec<Message>>(&raw).unwrap_or_default(),
                    Err(_) => vec![],
                }
            }
            None => vec![],
        }
    }

    pub fn poll_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    // ── Crypto helpers ────────────────────────────────────────────────────────

    fn encrypt(&self, text: &str) -> String {
        if let Some(id) = self.find_category("crypto").map(|s| s.to_string()) {
            let input = serde_json::json!({ "plaintext": text }).to_string();
            if let Ok(raw) = self.call(&id, "encrypt", &input) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(b64) = v["ciphertext"].as_str() {
                        return b64.to_string();
                    }
                }
            }
        }
        base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
    }

    fn decrypt(&self, payload: &str) -> String {
        if let Some(id) = self.find_category("crypto").map(|s| s.to_string()) {
            let input = serde_json::json!({ "ciphertext": payload }).to_string();
            if let Ok(raw) = self.call(&id, "decrypt", &input) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(text) = v["plaintext"].as_str() {
                        return text.to_string();
                    }
                }
            }
        }
        base64::engine::general_purpose::STANDARD
            .decode(payload)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_else(|| payload.to_string())
    }

    fn store_message(&self, _dir: &str, from: &str, to: &str, text: &str, ts: u64) {
        if let Some(id) = self.find_category("storage").map(|s| s.to_string()) {
            let input = serde_json::json!({
                "from": from,
                "to": to,
                "text": text,
                "timestamp": ts,
            }).to_string();
            let _ = self.call(&id, "store_message", &input);
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
