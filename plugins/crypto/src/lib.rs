use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use serde::Deserialize;

extern "C" {
    fn emit_event(
        topic_ptr:   *const u8, topic_len:   i32,
        payload_ptr: *const u8, payload_len: i32,
    );
}

const KEY_BYTES:   &[u8; 32] = b"wasm-plugin-host-secret-key-2024";
const NONCE_BYTES: &[u8; 12] = b"unique-nonce";

#[no_mangle]
pub extern "C" fn alloc(size: i32) -> *mut u8 {
    let mut buf: Vec<u8> = Vec::with_capacity(size as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: i32) {
    unsafe { drop(Vec::from_raw_parts(ptr, 0, size as usize)) };
}

#[derive(Deserialize)]
struct EventMeta { topic: String }

#[no_mangle]
pub extern "C" fn handle_event(
    meta_ptr: *const u8, meta_len: i32,
    payload_ptr: *const u8, payload_len: i32,
) -> i32 {
    let meta_slice    = unsafe { std::slice::from_raw_parts(meta_ptr,    meta_len    as usize) };
    let payload_slice = unsafe { std::slice::from_raw_parts(payload_ptr, payload_len as usize) };

    let meta: EventMeta = match serde_json::from_slice(meta_slice) {
        Ok(m)  => m,
        Err(_) => return 1,
    };

    match meta.topic.as_str() {
        "UI_SEND_MSG"      => encrypt_and_emit(payload_slice),
        "NET_RECEIVED_MSG" => decrypt_and_emit(payload_slice),
        _                  => 0,
    }
}

fn cipher() -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(KEY_BYTES))
}

fn encrypt_and_emit(plaintext: &[u8]) -> i32 {
    let nonce = Nonce::from_slice(NONCE_BYTES);
    match cipher().encrypt(nonce, plaintext) {
        Ok(ct) => { emit("CRYPTO_ENCRYPTED", &ct); 0 }
        Err(_) => 1,
    }
}

fn decrypt_and_emit(ciphertext: &[u8]) -> i32 {
    let nonce = Nonce::from_slice(NONCE_BYTES);
    match cipher().decrypt(nonce, ciphertext) {
        Ok(pt) => { emit("CRYPTO_DECRYPTED", &pt); 0 }
        Err(_) => 1,
    }
}

fn emit(topic: &str, payload: &[u8]) {
    unsafe {
        emit_event(
            topic.as_ptr(),   topic.len()   as i32,
            payload.as_ptr(), payload.len() as i32,
        );
    }
}
