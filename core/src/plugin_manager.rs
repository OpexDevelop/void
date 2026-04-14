use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use extism::{Manifest, Plugin, Wasm};
use crate::errors::CoreError;
use crate::manifest::{parse_manifest, manifest_to_plugin_info, PluginManifest};
use crate::models::PluginInfo;

struct LoadedPlugin {
    plugin: Mutex<Plugin>,
    info: PluginInfo,
    manifest_data: PluginManifest,
}

// SAFETY: Plugin содержит указатели но мы обращаемся только под Mutex
unsafe impl Send for LoadedPlugin {}
unsafe impl Sync for LoadedPlugin {}

static PLUGINS: OnceLock<Mutex<HashMap<String, LoadedPlugin>>> = OnceLock::new();

fn plugins() -> &'static Mutex<HashMap<String, LoadedPlugin>> {
    PLUGINS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn load_plugin(wasm_bytes: Vec<u8>, manifest_toml: &str) -> Result<PluginInfo, CoreError> {
    let manifest_data =
        parse_manifest(manifest_toml).map_err(|e| CoreError::ManifestParse(e))?;
    let info = manifest_to_plugin_info(&manifest_data);

    // ── 3. Защита от дублей ──────────────────────────────────────────
    {
        let map = plugins().lock().unwrap();
        if map.contains_key(&info.id) {
            return Err(CoreError::PluginAlreadyLoaded(info.id.clone()));
        }
    }

    // ── Валидация размера WASM ───────────────────────────────────────
    let limit_bytes = manifest_data.limits.max_memory_mb as usize * 1024 * 1024;
    if wasm_bytes.len() > limit_bytes {
        return Err(CoreError::WasmSizeExceeded {
            size: wasm_bytes.len(),
            limit: limit_bytes,
        });
    }

    // ── 2. Применяем permissions через Extism Manifest ───────────────
    let wasm = Wasm::data(wasm_bytes);
    let mut ext_manifest = Manifest::new([wasm]);

    // Лимит памяти: max_memory_mb → страницы (1 страница = 64 KiB)
    let pages = (manifest_data.limits.max_memory_mb * 1024 / 64).max(1);
    ext_manifest = ext_manifest.with_memory_max(pages);

    if manifest_data.permissions.network {
        // Разрешаем все хосты если network = true
        ext_manifest = ext_manifest.with_allowed_host("*");
    }
    // Если network = false — не добавляем хосты, Extism блокирует HTTP

    let plugin =
        Plugin::new(&ext_manifest, [], true).map_err(|e| CoreError::PluginCallFailed {
            func: "Plugin::new".to_string(),
            reason: e.to_string(),
        })?;

    let mut map = plugins().lock().unwrap();
    map.insert(
        info.id.clone(),
        LoadedPlugin {
            plugin: Mutex::new(plugin),
            info: info.clone(),
            manifest_data,
        },
    );
    drop(map);

    // ── 4. Lifecycle: on_load ────────────────────────────────────────
    let loaded_map = plugins().lock().unwrap();
    if let Some(lp) = loaded_map.get(&info.id) {
        if lp.manifest_data.capabilities.lifecycle.contains(&"on_load".to_string()) {
            drop(loaded_map); // освобождаем lock перед вызовом
            let config = serde_json::json!({
                "plugin_id": info.id,
                "version": info.version,
            })
            .to_string();
            // on_load не фатален — логируем но не падаем
            if let Err(e) = call_plugin_fn(&info.id, "on_load", &config) {
                eprintln!("[plugin_manager] on_load warning for '{}': {}", info.id, e);
            }
        }
    }

    Ok(info)
}

pub fn unload_plugin(id: &str) {
    // ── 4. Lifecycle: on_unload ──────────────────────────────────────
    {
        let map = plugins().lock().unwrap();
        if let Some(lp) = map.get(id) {
            if lp.manifest_data.capabilities.lifecycle.contains(&"on_unload".to_string()) {
                drop(map);
                let _ = call_plugin_fn(id, "on_unload", "{}");
            }
        }
    }
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

pub fn call_plugin_fn(
    plugin_id: &str,
    func: &str,
    input: &str,
) -> Result<String, CoreError> {
    let map = plugins().lock().unwrap();
    let loaded = map
        .get(plugin_id)
        .ok_or_else(|| CoreError::PluginNotFound(plugin_id.to_string()))?;

    let mut plugin = loaded.plugin.lock().unwrap();
    plugin
        .call::<&str, &str>(func, input)
        .map(|s| s.to_string())
        .map_err(|e| CoreError::PluginCallFailed {
            func: func.to_string(),
            reason: e.to_string(),
        })
}

pub fn find_plugin_by_category(category: &str) -> Option<String> {
    plugins()
        .lock()
        .unwrap()
        .values()
        .find(|p| p.manifest_data.plugin.category == category && p.info.active)
        .map(|p| p.info.id.clone())
}
