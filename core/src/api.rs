use flutter_rust_bridge::frb;
use crate::engine::{Engine, PluginInfo, Message};

/// Глобальный движок — инициализируется один раз
static ENGINE: std::sync::OnceLock<std::sync::Mutex<Engine>> =
    std::sync::OnceLock::new();

fn engine() -> &'static std::sync::Mutex<Engine> {
    ENGINE.get_or_init(|| std::sync::Mutex::new(Engine::new()))
}

// ── Setup ────────────────────────────────────────────────────────────────────

#[frb]
pub fn core_init() {
    std::env::set_var("HOME", std::env::temp_dir());
    let _ = engine();
}

// ── Plugins ──────────────────────────────────────────────────────────────────

#[frb]
pub fn load_plugin(wasm: Vec<u8>, manifest: String) -> Result<PluginInfo, String> {
    engine().lock().unwrap().load_plugin(wasm, &manifest)
}

#[frb]
pub fn list_plugins() -> Vec<PluginInfo> {
    engine().lock().unwrap().list_plugins()
}

#[frb]
pub fn unload_plugin(id: String) {
    engine().lock().unwrap().unload_plugin(&id);
}

#[frb]
pub fn configure_transport(address: String) -> Result<(), String> {
    engine().lock().unwrap().configure_transport(&address)
}

// ── Messaging ────────────────────────────────────────────────────────────────

#[frb]
pub fn send_message(to: String, text: String) -> Result<(), String> {
    engine().lock().unwrap().send_message(&to, &text)
}

#[frb]
pub fn get_messages(contact: String) -> Vec<Message> {
    engine().lock().unwrap().get_messages(&contact)
}

// ── Event stream ─────────────────────────────────────────────────────────────

#[frb]
pub fn poll_events() -> Vec<crate::engine::Event> {
    engine().lock().unwrap().poll_events()
}

#[frb]
pub fn poll_transport(since_ts: u64) -> u32 {
    engine().lock().unwrap().poll_incoming(since_ts)
}
