mod bus;
mod core;
mod errors;
mod manifest;
mod models;
mod plugin_manager;

use base64::Engine;
use errors::CoreError;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn cstr(ptr: *const c_char) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("")
}

fn to_c(s: String) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

fn ok_json(v: serde_json::Value) -> *mut c_char {
    to_c(v.to_string())
}

// ── FFI exports ───────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn messenger_init() {
    // HOME нужен Extism на Android
    std::env::set_var("HOME", std::env::temp_dir());
}

#[no_mangle]
pub extern "C" fn messenger_load_plugin(
    wasm_ptr: *const u8,
    wasm_len: i32,
    manifest_ptr: *const c_char,
) -> *mut c_char {
    let wasm_bytes =
        unsafe { std::slice::from_raw_parts(wasm_ptr, wasm_len as usize).to_vec() };
    let manifest_str = cstr(manifest_ptr);

    let result = match plugin_manager::load_plugin(wasm_bytes, manifest_str) {
        Ok(info) => serde_json::json!({ "ok": true, "plugin": info }),
        Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
    };
    ok_json(result)
}

#[no_mangle]
pub extern "C" fn messenger_list_plugins() -> *mut c_char {
    let plugins = plugin_manager::list_plugins();
    let json = serde_json::to_string(&plugins).unwrap_or_else(|_| "[]".to_string());
    to_c(json)
}

#[no_mangle]
pub extern "C" fn messenger_unload_plugin(id_ptr: *const c_char) {
    plugin_manager::unload_plugin(cstr(id_ptr));
}

#[no_mangle]
pub extern "C" fn messenger_send_message(
    to_ptr: *const c_char,
    text_ptr: *const c_char,
) -> *mut c_char {
    let to = cstr(to_ptr).to_string();
    let text = cstr(text_ptr).to_string();
    ok_json(dispatch_send(&to, &text))
}

#[no_mangle]
pub extern "C" fn messenger_get_messages(contact_ptr: *const c_char) -> *mut c_char {
    let contact = cstr(contact_ptr);

    let result = match plugin_manager::find_plugin_by_category("storage") {
        Some(id) => {
            let input = serde_json::json!({ "contact": contact }).to_string();
            plugin_manager::call_plugin_fn(&id, "get_messages", &input)
                .unwrap_or_else(|_| "[]".to_string())
        }
        None => "[]".to_string(),
    };
    to_c(result)
}

#[no_mangle]
pub extern "C" fn messenger_poll_transport(since_ptr: *const c_char) -> *mut c_char {
    let since_ts: u64 = cstr(since_ptr).parse().unwrap_or(0);
    let count = dispatch_poll_incoming(since_ts);
    ok_json(serde_json::json!({ "processed": count }))
}

#[no_mangle]
pub extern "C" fn messenger_configure_transport(address_ptr: *const c_char) -> *mut c_char {
    let my_address = cstr(address_ptr);

    // Если transport плагин не загружен — это НЕ фатальная ошибка для MVP
    // Возвращаем ok: true чтобы приложение запустилось
    let result = match plugin_manager::find_plugin_by_category("transport") {
        Some(transport_id) => {
            let config = serde_json::json!({ "address": my_address }).to_string();
            match plugin_manager::call_plugin_fn(&transport_id, "configure", &config) {
                Ok(r) => {
                    // Парсим ответ плагина
                    serde_json::from_str::<serde_json::Value>(&r)
                        .unwrap_or_else(|_| serde_json::json!({ "ok": true }))
                }
                Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
            }
        }
        None => {
            // Нет transport плагина — работаем в offline режиме
            eprintln!("[core] no transport plugin loaded — offline mode");
            serde_json::json!({ "ok": true, "warning": "no transport plugin — offline mode" })
        }
    };

    ok_json(result)
}

#[no_mangle]
pub extern "C" fn messenger_poll_event() -> *mut c_char {
    match bus::poll_event() {
        Some(event) => {
            let json = serde_json::to_string(&event).unwrap_or_default();
            to_c(json)
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

// ── Internal dispatch ─────────────────────────────────────────────────────────

fn dispatch_send(to: &str, text: &str) -> serde_json::Value {
    let payload_b64 = encrypt_via_plugin(text);

    // Сохраняем в storage (не фатально если нет)
    if let Some(storage_id) = plugin_manager::find_plugin_by_category("storage") {
        let input = serde_json::json!({
            "from": "me",
            "to": to,
            "text": text,
            "timestamp": current_timestamp(),
        })
        .to_string();
        if let Err(e) = plugin_manager::call_plugin_fn(&storage_id, "store_message", &input) {
            eprintln!("[core] store_message failed: {e}");
        }
    }

    // Отправляем через transport
    match plugin_manager::find_plugin_by_category("transport") {
        Some(transport_id) => {
            let input = serde_json::json!({
                "to_topic": to,
                "payload_b64": payload_b64,
            })
            .to_string();
            match plugin_manager::call_plugin_fn(&transport_id, "send", &input) {
                Ok(resp) => serde_json::from_str(&resp)
                    .unwrap_or_else(|_| serde_json::json!({ "ok": true })),
                Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
            }
        }
        None => {
            // Offline: сообщение сохранено локально
            serde_json::json!({ "ok": true, "warning": "offline — saved locally only" })
        }
    }
}

fn dispatch_poll_incoming(since_ts: u64) -> u32 {
    let transport_id = match plugin_manager::find_plugin_by_category("transport") {
        Some(id) => id,
        None => return 0,
    };

    let input = serde_json::json!({ "since": since_ts, "limit": 50 }).to_string();
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

        let text = decrypt_via_plugin(&payload_b64);

        if let Some(storage_id) = plugin_manager::find_plugin_by_category("storage") {
            let store_input = serde_json::json!({
                "from": from_topic,
                "to": "me",
                "text": text,
                "timestamp": timestamp,
            })
            .to_string();
            let _ = plugin_manager::call_plugin_fn(&storage_id, "store_message", &store_input);
        }

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
