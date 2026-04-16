wit_bindgen::generate!({
    path:  "../../wit/plugin.wit",
    world: "plugin-world",
});

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};

struct Plugin;

impl Guest for Plugin {
    fn handle_event(meta: EventMeta, payload: Vec<u8>) -> i32 {
        match meta.topic.as_str() {
            "UI_SEND_MSG"      => encrypt_and_emit(&payload),
            "NET_RECEIVED_MSG" => decrypt_and_emit(&payload),
            _                  => 0,
        }
    }
}

export!(Plugin);

const KEY_BYTES:   &[u8; 32] = b"wasm-plugin-host-secret-key-2024";
const NONCE_BYTES: &[u8; 12] = b"unique-nonce";

fn cipher() -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(KEY_BYTES))
}

fn encrypt_and_emit(plaintext: &[u8]) -> i32 {
    let nonce = Nonce::from_slice(NONCE_BYTES);
    match cipher().encrypt(nonce, plaintext) {
        Ok(ct) => { emit_event("CRYPTO_ENCRYPTED", &ct); 0 }
        Err(_) => 1,
    }
}

fn decrypt_and_emit(ciphertext: &[u8]) -> i32 {
    let nonce = Nonce::from_slice(NONCE_BYTES);
    match cipher().decrypt(nonce, ciphertext) {
        Ok(pt) => { emit_event("CRYPTO_DECRYPTED", &pt); 0 }
        Err(_) => 1,
    }
}
