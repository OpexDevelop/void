use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::Engine;
use extism_pdk::*;
use serde::{Deserialize, Serialize};

const HARDCODED_KEY: &[u8; 32] = b"messenger_mvp_key_32_bytes_long!";

#[derive(Deserialize)]
struct EncryptInput {
    plaintext: String,
}

#[derive(Serialize)]
struct EncryptOutput {
    ciphertext: String,
}

#[derive(Deserialize)]
struct DecryptInput {
    ciphertext: String,
}

#[derive(Serialize)]
struct DecryptOutput {
    plaintext: String,
}

#[plugin_fn]
pub fn encrypt(input: String) -> FnResult<String> {
    let req: EncryptInput = serde_json::from_str(&input)?;

    let mut nonce_bytes = [0u8; 12];
    getrandom::getrandom(&mut nonce_bytes).map_err(|e| Error::msg(e.to_string()))?;

    let key = Key::<Aes256Gcm>::from_slice(HARDCODED_KEY);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, req.plaintext.as_bytes())
        .map_err(|e| Error::msg(e.to_string()))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);

    let b64 = base64::engine::general_purpose::STANDARD.encode(&combined);
    Ok(serde_json::to_string(&EncryptOutput { ciphertext: b64 })?)
}

#[plugin_fn]
pub fn decrypt(input: String) -> FnResult<String> {
    let req: DecryptInput = serde_json::from_str(&input)?;

    let combined = base64::engine::general_purpose::STANDARD
        .decode(&req.ciphertext)
        .map_err(|e| Error::msg(e.to_string()))?;

    if combined.len() < 12 {
        // Вот здесь мы явно преобразуем тип, как просил компилятор
        return Err(Error::msg("payload too short").into());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let key = Key::<Aes256Gcm>::from_slice(HARDCODED_KEY);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| Error::msg(e.to_string()))?;

    let text = String::from_utf8(plaintext).map_err(|e| Error::msg(e.to_string()))?;
    Ok(serde_json::to_string(&DecryptOutput { plaintext: text })?)
}
