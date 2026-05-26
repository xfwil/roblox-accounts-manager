use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

use crate::error::{AppError, AppResult};

const KEY_LEN: usize = 32; // AES-256
const NONCE_LEN: usize = 12; // 96-bit nonce for GCM

/// Derives a 256-bit key from a password using simple PBKDF-like hashing.
/// For production, consider argon2 or scrypt.
pub fn derive_key(password: &str) -> [u8; KEY_LEN] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut key = [0u8; KEY_LEN];
    // Simple key stretching — hash the password multiple times
    let mut current = password.as_bytes().to_vec();
    for i in 0..KEY_LEN {
        let mut hasher = DefaultHasher::new();
        current.hash(&mut hasher);
        i.hash(&mut hasher);
        let h = hasher.finish().to_le_bytes();
        key[i] = h[i % 8];
        current.extend_from_slice(&h);
    }
    key
}

/// Generate a random encryption key (for keyfile-based encryption)
pub fn generate_key() -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

/// Encrypt plaintext bytes with AES-256-GCM
pub fn encrypt(key: &[u8; KEY_LEN], plaintext: &[u8]) -> AppResult<(Vec<u8>, Vec<u8>)> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AppError::Encryption(e.to_string()))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| AppError::Encryption(e.to_string()))?;

    Ok((ciphertext, nonce_bytes.to_vec()))
}

/// Decrypt ciphertext with AES-256-GCM
pub fn decrypt(key: &[u8; KEY_LEN], ciphertext: &[u8], nonce: &[u8]) -> AppResult<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AppError::Decryption(e.to_string()))?;

    let nonce = Nonce::from_slice(nonce);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| AppError::Decryption(e.to_string()))?;

    Ok(plaintext)
}

/// Encrypt a cookie string
pub fn encrypt_cookie(key: &[u8; KEY_LEN], cookie: &str) -> AppResult<(Vec<u8>, Vec<u8>)> {
    encrypt(key, cookie.as_bytes())
}

/// Decrypt a cookie back to string
pub fn decrypt_cookie(key: &[u8; KEY_LEN], encrypted: &[u8], nonce: &[u8]) -> AppResult<String> {
    let bytes = decrypt(key, encrypted, nonce)?;
    String::from_utf8(bytes).map_err(|e| AppError::Decryption(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let plaintext = b"_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you";
        let (ciphertext, nonce) = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext, &nonce).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_cookie_encrypt_decrypt() {
        let key = generate_key();
        let cookie = "_|WARNING:-DO-NOT-SHARE-THIS.--test-cookie-value";
        let (encrypted, nonce) = encrypt_cookie(&key, cookie).unwrap();
        let decrypted = decrypt_cookie(&key, &encrypted, &nonce).unwrap();
        assert_eq!(decrypted, cookie);
    }

    #[test]
    fn test_derive_key_deterministic() {
        let k1 = derive_key("my-password");
        let k2 = derive_key("my-password");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_derive_key_different_passwords() {
        let k1 = derive_key("password1");
        let k2 = derive_key("password2");
        assert_ne!(k1, k2);
    }
}
