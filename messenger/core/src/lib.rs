mod core;
mod models;
mod manifest;
mod bus;
mod plugin_manager;
mod transport;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn messenger_init(port: i32) {
    // Безопасный костыль: Android всегда дает доступ к своей временной папке (cache)
    std::env::set_var("HOME", std::env::temp_dir());
    
    core::init(port as u16);
    transport::start_tcp_server(port as u16);
}

#[no_mangle]
pub extern "C" fn messenger_load_plugin(wasm_ptr: *const u8, wasm_len: i32, manifest_ptr: *const c_char) -> *mut c_char {
    let wasm_bytes = unsafe { std::slice::from_raw_parts(wasm_ptr, wasm_len as usize).to_vec() };
    let manifest_str = unsafe { CStr::from_ptr(manifest_ptr).to_str().unwrap_or("") };
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

#[no_mangle]
pub extern "C" fn messenger_send_message(to_ptr: *const c_char, text_ptr: *const c_char) -> *mut c_char {
    let to = unsafe { CStr::from_ptr(to_ptr).to_str().unwrap_or("").to_string() };
    let text = unsafe { CStr::from_ptr(text_ptr).to_str().unwrap_or("").to_string() };
    let payload = encrypt_text(&text);
    if let Some(storage_id) = plugin_manager::find_plugin_by_category("storage") {
        let input = serde_json::json!({ "from": "me", "to": to, "text": text, "timestamp": current_timestamp() }).to_string();
        let _ = plugin_manager::call_plugin_fn(&storage_id, "store_message", &input);
    }
    let result = transport::send_tcp_message(&to, &payload);
    let json = match result {
        Ok(()) => serde_json::json!({ "ok": true }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    };
    CString::new(json.to_string()).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn messenger_get_messages(contact_ptr: *const c_char) -> *mut c_char {
    let contact = unsafe { CStr::from_ptr(contact_ptr).to_str().unwrap_or("") };
    let result = if let Some(storage_id) = plugin_manager::find_plugin_by_category("storage") {
        let input = serde_json::json!({ "contact": contact }).to_string();
        plugin_manager::call_plugin_fn(&storage_id, "get_messages", &input).unwrap_or_else(|_| "[]".to_string())
    } else {
        "[]".to_string()
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

pub fn encrypt_text(text: &str) -> Vec<u8> {
    use base64::Engine;
    if let Some(id) = plugin_manager::find_plugin_by_category("crypto") {
        let input = serde_json::json!({ "plaintext": text }).to_string();
        if let Ok(res) = plugin_manager::call_plugin_fn(&id, "encrypt", &input) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&res) {
                if let Some(b64) = v["ciphertext"].as_str() {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                        return bytes;
                    }
                }
            }
        }
    }
    text.as_bytes().to_vec()
}

pub fn decrypt_bytes(data: &[u8]) -> String {
    use base64::Engine;
    if let Some(id) = plugin_manager::find_plugin_by_category("crypto") {
        let b64 = base64::engine::general_purpose::STANDARD.encode(data);
        let input = serde_json::json!({ "ciphertext": b64 }).to_string();
        if let Ok(res) = plugin_manager::call_plugin_fn(&id, "decrypt", &input) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&res) {
                if let Some(text) = v["plaintext"].as_str() {
                    return text.to_string();
                }
            }
        }
    }
    String::from_utf8_lossy(data).to_string()
}

pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}
