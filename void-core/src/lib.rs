mod types;
mod storage;
mod crypto;
mod transport;
mod engine;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;

static CORE: OnceLock<engine::VoidEngine> = OnceLock::new();

#[no_mangle]
pub extern "C" fn void_init(port: i32, key: *const u8) -> i32 {
    let key: [u8; 32] = unsafe { std::slice::from_raw_parts(key, 32) }
        .try_into().unwrap();

    let core = engine::VoidEngine::new(
        Box::new(storage::MemStorage::new()),
        Box::new(crypto::AesCrypto::new(&key)),
        vec![Box::new(transport::TcpTransport::new(port as u16))],
    );
    CORE.set(core).is_ok() as i32
}

#[no_mangle]
pub extern "C" fn void_send(
    chat_id: *const c_char,
    text: *const c_char,
    addr: *const c_char,
) -> i32 {
    let core = match CORE.get() { Some(c) => c, None => return -1 };
    let chat_id = unsafe { CStr::from_ptr(chat_id) }.to_str().unwrap_or("");
    let text = unsafe { CStr::from_ptr(text) }.to_str().unwrap_or("");
    let addr = unsafe { CStr::from_ptr(addr) }.to_str().unwrap_or("");
    core.send_msg(chat_id, text, addr);
    0
}

#[no_mangle]
pub extern "C" fn void_poll() -> *mut c_char {
    let core = match CORE.get() { Some(c) => c, None => return std::ptr::null_mut() };
    match core.poll() {
        Some(msg) => CString::new(serde_json::to_string(&msg).unwrap_or_default())
            .unwrap_or_default().into_raw(),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn void_history(chat_id: *const c_char) -> *mut c_char {
    let core = match CORE.get() { Some(c) => c, None => return std::ptr::null_mut() };
    let chat_id = unsafe { CStr::from_ptr(chat_id) }.to_str().unwrap_or("");
    let msgs = core.history(chat_id);
    CString::new(serde_json::to_string(&msgs).unwrap_or_default())
        .unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn void_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr)) };
    }
}
