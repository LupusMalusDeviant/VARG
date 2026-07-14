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

/// B10: decrypt is fed attacker/user-controlled input (the ciphertext string and the key).
/// It used to `expect`/`unwrap` on every failure mode — invalid Base64, truncated payload,
/// wrong password, non-UTF-8 plaintext — which aborted the whole process. That is a trivial
/// denial-of-service. Return a clear error-marker string instead so the caller stays alive
/// and can branch on the result (consistent with other string-returning runtime builtins).
pub fn __varg_decrypt(data: &str, key: &str) -> String {
    let packed = match STANDARD.decode(data) {
        Ok(p) => p,
        Err(_) => return "[VargOS] decrypt error: invalid Base64 payload".to_string(),
    };
    if packed.len() < 16 + 12 {
        return "[VargOS] decrypt error: payload too short for AES-GCM".to_string();
    }
    let salt = &packed[0..16];
    let nonce_bytes = &packed[16..28];
    let ciphertext = &packed[28..];
    let derived_key = __varg_derive_key(key, salt);
    let cipher = match Aes256Gcm::new_from_slice(&derived_key) {
        Ok(c) => c,
        Err(_) => return "[VargOS] decrypt error: key setup failed".to_string(),
    };
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = match cipher.decrypt(nonce, ciphertext) {
        Ok(p) => p,
        Err(_) => return "[VargOS] decrypt error: wrong password or corrupted data".to_string(),
    };
    match String::from_utf8(plaintext) {
        Ok(s) => s,
        Err(_) => "[VargOS] decrypt error: decrypted bytes are not valid UTF-8".to_string(),
    }
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

    #[test]
    fn test_decrypt_wrong_password_returns_error_not_panic_b7() {
        // B7 regression: a wrong password must yield an error string, never a panic.
        let encrypted = __varg_encrypt("secret data", "correct-password");
        let result = __varg_decrypt(&encrypted, "wrong-password");
        assert!(result.starts_with("[VargOS] decrypt error:"),
            "wrong password must return an error marker, got: {result}");
    }

    #[test]
    fn test_decrypt_invalid_base64_returns_error_not_panic_b7() {
        // B7 regression: malformed (non-Base64) input must not panic.
        let result = __varg_decrypt("!!!not base64!!!", "any-key");
        assert!(result.starts_with("[VargOS] decrypt error:"),
            "invalid payload must return an error marker, got: {result}");
    }

    #[test]
    fn test_decrypt_truncated_payload_returns_error_not_panic_b7() {
        // B7 regression: a valid-Base64 but too-short payload must not panic.
        let result = __varg_decrypt("YWJj", "any-key"); // "abc" — under the 28-byte minimum
        assert!(result.starts_with("[VargOS] decrypt error:"),
            "truncated payload must return an error marker, got: {result}");
    }
}
