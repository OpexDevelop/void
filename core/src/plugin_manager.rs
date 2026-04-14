use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use extism::{Manifest, Plugin, Wasm};
use crate::manifest::{parse_manifest, manifest_to_plugin_info, PluginManifest};
use crate::models::PluginInfo;

struct LoadedPlugin {
    plugin: Mutex<Plugin>,
    info: PluginInfo,
    manifest_data: PluginManifest,
}

unsafe impl Send for LoadedPlugin {}
unsafe impl Sync for LoadedPlugin {}

static PLUGINS: OnceLock<Mutex<HashMap<String, LoadedPlugin>>> = OnceLock::new();

fn plugins() -> &'static Mutex<HashMap<String, LoadedPlugin>> {
    PLUGINS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn load_plugin(wasm_bytes: Vec<u8>, manifest_toml: &str) -> Result<PluginInfo, String> {
    let manifest_data = parse_manifest(manifest_toml)?;
    let info = manifest_to_plugin_info(&manifest_data);

    let wasm = Wasm::data(wasm_bytes);
    let mut ext_manifest = Manifest::new([wasm]);

    // u32 напрямую — никакого u64
    let memory_limit_pages: u32 = (manifest_data.limits.max_memory_mb * 16).max(1);
    ext_manifest = ext_manifest.with_memory_max(memory_limit_pages);

    if manifest_data.permissions.network {
        ext_manifest = ext_manifest.with_allowed_host("*");
    }

    let plugin = Plugin::new(&ext_manifest, [], true).map_err(|e| e.to_string())?;

    plugins().lock().unwrap().insert(
        info.id.clone(),
        LoadedPlugin {
            plugin: Mutex::new(plugin),
            info: info.clone(),
            manifest_data,
        },
    );

    Ok(info)
}

pub fn unload_plugin(id: &str) {
    plugins().lock().unwrap().remove(id);
}

pub fn list_plugins() -> Vec<PluginInfo> {
    plugins()
        .lock()
        .unwrap()
        .values()
        .map(|p| p.info.clone())
        .collect()
}

pub fn call_plugin_fn(plugin_id: &str, func: &str, input: &str) -> Result<String, String> {
    let map = plugins().lock().unwrap();
    let loaded = map
        .get(plugin_id)
        .ok_or_else(|| format!("plugin '{}' not found", plugin_id))?;

    let mut plugin = loaded.plugin.lock().unwrap();
    let result: String = plugin.call(func, input).map_err(|e| e.to_string())?;
    Ok(result)
}

pub fn find_plugin_by_category(category: &str) -> Option<String> {
    plugins()
        .lock()
        .unwrap()
        .values()
        .find(|p| p.manifest_data.plugin.category == category && p.info.active)
        .map(|p| p.info.id.clone())
}
