use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

pub fn verify_wasm(
    bytes: &[u8],
    expected_sha256: &str,
    signature_b64: &str,
    verifying_key: &VerifyingKey,
) -> Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash = hasher.finalize();
    let computed_hex = hex::encode(hash);

    if computed_hex != expected_sha256 {
        anyhow::bail!(
            "SHA256 mismatch: expected {}, computed {}",
            expected_sha256,
            computed_hex
        );
    }

    let sig_bytes = STANDARD
        .decode(signature_b64)
        .context("invalid base64 in signature field")?;
    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("signature must be 64 bytes"))?;
    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify(&hash, &signature)
        .context("Ed25519 signature verification failed")?;

    Ok(())
}

pub fn load_verifying_key(b64: &str) -> Result<VerifyingKey> {
    let bytes = STANDARD.decode(b64).context("invalid base64 for public key")?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("verifying key must be 32 bytes"))?;
    VerifyingKey::from_bytes(&arr).context("invalid Ed25519 public key")
}
