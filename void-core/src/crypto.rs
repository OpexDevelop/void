use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};

pub trait Crypto: Send + Sync {
    fn encrypt(&self, data: &[u8]) -> Vec<u8>;
    fn decrypt(&self, data: &[u8]) -> Option<Vec<u8>>;
}

pub struct AesCrypto {
    cipher: Aes256Gcm,
}

impl AesCrypto {
    pub fn new(key: &[u8; 32]) -> Self {
        Self { cipher: Aes256Gcm::new(key.into()) }
    }
}

impl Crypto for AesCrypto {
    fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ct = self.cipher.encrypt(nonce, data).unwrap();
        let mut out = nonce_bytes.to_vec();
        out.extend(ct);
        out
    }

    fn decrypt(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 13 { return None; }
        let nonce = Nonce::from_slice(&data[..12]);
        self.cipher.decrypt(nonce, &data[12..]).ok()
    }
}
