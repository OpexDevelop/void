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

    // Применяем лимит памяти из манифеста
    let memory_limit_pages = {
        // Extism считает память в страницах по 64KiB
        // max_memory_mb * 1024 * 1024 / 65536 = max_memory_mb * 16
        let pages = manifest_data.limits.max_memory_mb as u64 * 16;
        pages.max(1) // минимум 1 страница
    };
    ext_manifest = ext_manifest.with_memory_max(memory_limit_pages);

    // Применяем сетевые разрешения из манифеста
    if manifest_data.permissions.network {
        // Разрешаем любые хосты если network = true
        ext_manifest = ext_manifest.with_allowed_host("*");
    }
    // Если network = false — не добавляем ни одного allowed_host,
    // Extism по умолчанию блокирует все сетевые вызовы

    // Таймаут на вызовы плагина (Extism v1 поддерживает через WithTimeout)
    // Примечание: timeout применяется в call_plugin_fn через Plugin::set_timeout
    // Сохраняем в LoadedPlugin для последующего применения

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

    // Применяем таймаут из манифеста
    let timeout_ms = loaded.manifest_data.limits.timeout_ms;
    let mut plugin = loaded.plugin.lock().unwrap();

    if timeout_ms > 0 {
        plugin.set_timeout_ms(timeout_ms as u64);
    }

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
