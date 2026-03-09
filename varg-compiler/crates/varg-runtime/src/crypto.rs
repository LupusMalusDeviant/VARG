// Varg Runtime: AES-256-GCM Encryption/Decryption

use aes_gcm::{aead::{Aead, KeyInit, OsRng}, Aes256Gcm, Nonce};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha512;
use rand::RngCore;
use base64::{engine::general_purpose::STANDARD, Engine as _};

pub fn __varg_derive_key(key: &str, salt: &[u8]) -> [u8; 32] {
    let mut derived_key = [0u8; 32];
    pbkdf2_hmac::<Sha512>(key.as_bytes(), salt, 600_000, &mut derived_key);
    derived_key
}

pub fn __varg_encrypt(data: &str, key: &str) -> String {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let derived_key = __varg_derive_key(key, &salt);
    let cipher = Aes256Gcm::new_from_slice(&derived_key).unwrap();
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, data.as_bytes()).expect("[VargOS] Encryption failed");

    // Pack: Salt (16) + Nonce (12) + Ciphertext
    let mut packed = Vec::with_capacity(16 + 12 + ciphertext.len());
    packed.extend_from_slice(&salt);
    packed.extend_from_slice(&nonce_bytes);
    packed.extend_from_slice(&ciphertext);
    STANDARD.encode(packed)
}

pub fn __varg_decrypt(data: &str, key: &str) -> String {
    let packed = STANDARD.decode(data).expect("[VargOS] Invalid Base64 Encryption Payload");
    if packed.len() < 16 + 12 {
        panic!("[VargOS] Payload too short for AES-GCM.");
    }
    let salt = &packed[0..16];
    let nonce_bytes = &packed[16..28];
    let ciphertext = &packed[28..];
    let derived_key = __varg_derive_key(key, salt);
    let cipher = Aes256Gcm::new_from_slice(&derived_key).unwrap();
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext).expect("[VargOS] Decryption failed or wrong password");
    String::from_utf8(plaintext).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let original = "Hello, Varg!";
        let key = "test-password-123";
        let encrypted = __varg_encrypt(original, key);
        let decrypted = __varg_decrypt(&encrypted, key);
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_derive_key_deterministic() {
        let salt = [0u8; 16];
        let key1 = __varg_derive_key("password", &salt);
        let key2 = __varg_derive_key("password", &salt);
        assert_eq!(key1, key2);
    }
}
