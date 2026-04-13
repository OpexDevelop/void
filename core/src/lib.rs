mod core;
mod models;
mod manifest;
mod bus;
mod plugin_manager;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use base64::Engine;

// ──────────────────────────────────────────────────────────────
// FFI инициализация
// ──────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn messenger_init(port: i32) {
    std::env::set_var("HOME", std::env::temp_dir());
    core::init(port as u16);
    // TCP сервер больше не запускается здесь
    // Транспорт — это плагин, ядро не знает о нём
}

// ──────────────────────────────────────────────────────────────
// Plugin management FFI
// ──────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn messenger_load_plugin(
    wasm_ptr: *const u8,
    wasm_len: i32,
    manifest_ptr: *const c_char,
) -> *mut c_char {
    let wasm_bytes =
        unsafe { std::slice::from_raw_parts(wasm_ptr, wasm_len as usize).to_vec() };
    let manifest_str =
        unsafe { CStr::from_ptr(manifest_ptr).to_str().unwrap_or("") };

    let result = match plugin_manager::load_plugin(wasm_bytes, manifest_str) {
        Ok(info) => serde_json::json!({ "ok": true, "plugin": info }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    };
    CString::new(result.to_string()).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn messenger_list_plugins() -> *mut c_char {
    let plugins = plugin_manager::list_plugins();
    let json = serde_json::to_string(&plugins).unwrap_or_else(|_| "[]".to_string());
    CString::new(json).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn messenger_unload_plugin(id_ptr: *const c_char) {
    let id = unsafe { CStr::from_ptr(id_ptr).to_str().unwrap_or("") };
    plugin_manager::unload_plugin(id);
}

// ──────────────────────────────────────────────────────────────
// Messaging FFI
// Ядро диспетчеризует через плагины, не содержит логики
// ──────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn messenger_send_message(
    to_ptr: *const c_char,
    text_ptr: *const c_char,
) -> *mut c_char {
    let to = unsafe { CStr::from_ptr(to_ptr).to_str().unwrap_or("").to_string() };
    let text = unsafe { CStr::from_ptr(text_ptr).to_str().unwrap_or("").to_string() };

    let result = dispatch_send(&to, &text);
    CString::new(result.to_string()).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn messenger_get_messages(contact_ptr: *const c_char) -> *mut c_char {
    let contact = unsafe { CStr::from_ptr(contact_ptr).to_str().unwrap_or("") };

    let result = if let Some(storage_id) =
        plugin_manager::find_plugin_by_category("storage")
    {
        let input =
            serde_json::json!({ "contact": contact }).to_string();
        plugin_manager::call_plugin_fn(&storage_id, "get_messages", &input)
            .unwrap_or_else(|_| "[]".to_string())
    } else {
        "[]".to_string()
    };

    CString::new(result).unwrap().into_raw()
}

/// Flutter вызывает это периодически чтобы забрать входящие
/// через транспортный плагин и положить в storage + event bus
#[no_mangle]
pub extern "C" fn messenger_poll_transport(since_ptr: *const c_char) -> *mut c_char {
    let since_ts: u64 = if since_ptr.is_null() {
        0
    } else {
        unsafe { CStr::from_ptr(since_ptr).to_str().unwrap_or("0") }
            .parse()
            .unwrap_or(0)
    };

    let count = dispatch_poll_incoming(since_ts);
    let json = serde_json::json!({ "processed": count });
    CString::new(json.to_string()).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn messenger_configure_transport(address_ptr: *const c_char) -> *mut c_char {
    let my_address = unsafe { CStr::from_ptr(address_ptr).to_str().unwrap_or("") };

    // Ядро упаковывает адрес в универсальный конверт
    // Плагин сам интерпретирует поле "address" как нужно:
    // ntfy плагин — как topic
    // tcp плагин — как host:port
    // любой другой — как угодно
    let config = serde_json::json!({ "address": my_address }).to_string();

    let result = if let Some(transport_id) =
        plugin_manager::find_plugin_by_category("transport")
    {
        match plugin_manager::call_plugin_fn(&transport_id, "configure", &config) {
            Ok(r) => r,
            Err(e) => serde_json::json!({ "ok": false, "error": e }).to_string(),
        }
    } else {
        serde_json::json!({ "ok": false, "error": "no transport plugin loaded" }).to_string()
    };

    CString::new(result).unwrap().into_raw()
}


#[no_mangle]
pub extern "C" fn messenger_poll_event() -> *mut c_char {
    match bus::poll_event() {
        Some(event) => {
            let json = serde_json::to_string(&event).unwrap_or_default();
            CString::new(json).unwrap().into_raw()
        }
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn messenger_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr)) };
    }
}

// ──────────────────────────────────────────────────────────────
// Внутренняя диспетчеризация — ядро оркестрирует плагины
// но не содержит бизнес-логику шифрования/транспорта
// ──────────────────────────────────────────────────────────────

/// Отправить сообщение:
/// 1. Зашифровать через crypto плагин (если загружен)
/// 2. Сохранить исходящее через storage плагин (если загружен)
/// 3. Отправить через transport плагин
fn dispatch_send(to: &str, text: &str) -> serde_json::Value {
    // Шаг 1: шифрование через crypto плагин
    let payload_b64 = encrypt_via_plugin(text);

    // Шаг 2: сохранить исходящее в storage
    if let Some(storage_id) = plugin_manager::find_plugin_by_category("storage") {
        let input = serde_json::json!({
            "from": "me",
            "to": to,
            "text": text,
            "timestamp": current_timestamp(),
        })
        .to_string();
        let _ = plugin_manager::call_plugin_fn(&storage_id, "store_message", &input);
    }

    // Шаг 3: отправить через transport плагин
    if let Some(transport_id) = plugin_manager::find_plugin_by_category("transport") {
        let input = serde_json::json!({
            "to_topic": to,
            "payload_b64": payload_b64,
        })
        .to_string();

        match plugin_manager::call_plugin_fn(&transport_id, "send", &input) {
            Ok(resp) => {
                // Парсим ответ транспортного плагина
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&resp) {
                    return v;
                }
                serde_json::json!({ "ok": true })
            }
            Err(e) => serde_json::json!({ "ok": false, "error": e }),
        }
    } else {
        serde_json::json!({
            "ok": false,
            "error": "no transport plugin loaded"
        })
    }
}

/// Опросить транспортный плагин за входящими, обработать их
/// Возвращает количество обработанных сообщений
fn dispatch_poll_incoming(since_ts: u64) -> u32 {
    let transport_id = match plugin_manager::find_plugin_by_category("transport") {
        Some(id) => id,
        None => return 0,
    };

    let input = serde_json::json!({
        "since": since_ts,
        "limit": 50,
    })
    .to_string();

    let resp = match plugin_manager::call_plugin_fn(&transport_id, "get_pending", &input) {
        Ok(r) => r,
        Err(_) => return 0,
    };

    let parsed: serde_json::Value = match serde_json::from_str(&resp) {
        Ok(v) => v,
        Err(_) => return 0,
    };

    let messages = match parsed["messages"].as_array() {
        Some(arr) => arr.clone(),
        None => return 0,
    };

    let mut count = 0u32;

    for msg in &messages {
        let from_topic = msg["from_topic"].as_str().unwrap_or("unknown").to_string();
        let payload_b64 = msg["payload_b64"].as_str().unwrap_or("").to_string();
        let timestamp = msg["timestamp"].as_u64().unwrap_or_else(current_timestamp);

        // Расшифровать через crypto плагин
        let text = decrypt_via_plugin(&payload_b64);

        // Сохранить входящее в storage
        if let Some(storage_id) = plugin_manager::find_plugin_by_category("storage") {
            let store_input = serde_json::json!({
                "from": from_topic,
                "to": "me",
                "text": text,
                "timestamp": timestamp,
            })
            .to_string();
            let _ =
                plugin_manager::call_plugin_fn(&storage_id, "store_message", &store_input);
        }

        // Пустить событие в bus
        bus::push_event(bus::Event {
            kind: "message_received".to_string(),
            payload: serde_json::json!({
                "from": from_topic,
                "text": text,
                "timestamp": timestamp,
            }),
        });

        count += 1;
    }

    count
}

// ──────────────────────────────────────────────────────────────
// Вспомогательные функции через плагины
// (не публичные — ядро использует их внутри dispatch_*)
// ──────────────────────────────────────────────────────────────

fn encrypt_via_plugin(text: &str) -> String {
    if let Some(id) = plugin_manager::find_plugin_by_category("crypto") {
        let input = serde_json::json!({ "plaintext": text }).to_string();
        if let Ok(res) = plugin_manager::call_plugin_fn(&id, "encrypt", &input) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&res) {
                if let Some(b64) = v["ciphertext"].as_str() {
                    return b64.to_string();
                }
            }
        }
    }
    // Нет crypto плагина — передаём plaintext в base64
    base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
}

fn decrypt_via_plugin(payload_b64: &str) -> String {
    if let Some(id) = plugin_manager::find_plugin_by_category("crypto") {
        let input = serde_json::json!({ "ciphertext": payload_b64 }).to_string();
        if let Ok(res) = plugin_manager::call_plugin_fn(&id, "decrypt", &input) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&res) {
                if let Some(text) = v["plaintext"].as_str() {
                    return text.to_string();
                }
            }
        }
    }
    // Нет crypto плагина — декодируем base64 как plaintext
    base64::engine::general_purpose::STANDARD
        .decode(payload_b64)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_else(|| payload_b64.to_string())
}

pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
