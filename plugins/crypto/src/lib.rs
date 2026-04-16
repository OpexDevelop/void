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
struct EventMeta {
    topic: String,
}

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
        "UI_SEND_MSG"  => encrypt_and_emit(payload_slice),
        "NET_RECEIVED" => decrypt_and_emit(payload_slice),
        _              => 0,
    }
}

fn cipher() -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(KEY_BYTES))
}

fn encrypt_and_emit(plaintext: &[u8]) -> i32 {
    let nonce = Nonce::from_slice(NONCE_BYTES);
    match cipher().encrypt(nonce, plaintext) {
        Ok(ct) => {
            let encoded = base64_encode(&ct);
            emit("CRYPTO_ENCRYPTED", encoded.as_bytes());
            0
        }
        Err(_) => 1,
    }
}

fn decrypt_and_emit(payload: &[u8]) -> i32 {
    let b64_str = match core::str::from_utf8(payload) {
        Ok(s)  => s.trim(),
        Err(_) => return 1,
    };

    let ciphertext = match base64_decode(b64_str) {
        Some(v) => v,
        None    => return 1,
    };

    let nonce = Nonce::from_slice(NONCE_BYTES);
    match cipher().decrypt(nonce, ciphertext.as_slice()) {
        Ok(pt) => {
            emit("CRYPTO_DECRYPTED", &pt);
            0
        }
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

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> alloc::string::String {
    let mut out = alloc::vec::Vec::new();
    let mut i = 0usize;
    while i < input.len() {
        let b0 = input[i] as u32;
        let b1 = if i + 1 < input.len() { input[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_CHARS[((n >> 18) & 63) as usize]);
        out.push(BASE64_CHARS[((n >> 12) & 63) as usize]);
        if i + 1 < input.len() {
            out.push(BASE64_CHARS[((n >> 6) & 63) as usize]);
        } else {
            out.push(b'=');
        }
        if i + 2 < input.len() {
            out.push(BASE64_CHARS[(n & 63) as usize]);
        } else {
            out.push(b'=');
        }
        i += 3;
    }
    alloc::string::String::from_utf8(out).unwrap_or_default()
}

fn base64_decode(input: &str) -> Option<alloc::vec::Vec<u8>> {
    let input = input.trim_end_matches('=');
    let mut table = [0xffu8; 256];
    for (i, &c) in BASE64_CHARS.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    let mut out = alloc::vec::Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        let c0 = table[bytes[i] as usize];
        let c1 = table[bytes[i + 1] as usize];
        if c0 == 0xff || c1 == 0xff { return None; }
        out.push((c0 << 2) | (c1 >> 4));
        if i + 2 < bytes.len() {
            let c2 = table[bytes[i + 2] as usize];
            if c2 == 0xff { return None; }
            out.push((c1 << 4) | (c2 >> 2));
        }
        if i + 3 < bytes.len() {
            let c3 = table[bytes[i + 3] as usize];
            if c3 == 0xff { return None; }
            out.push((c2 << 6) | c3);
        }
        i += 4;
    }
    Some(out)
}

extern crate alloc;
