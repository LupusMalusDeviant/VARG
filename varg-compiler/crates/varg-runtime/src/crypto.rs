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

pub fn __varg_encrypt(data: &str, key: &str) -> Result<String, String> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let derived_key = __varg_derive_key(key, &salt);
    // The derived key is always 32 bytes, so this cannot fail; the encryption below can.
    let cipher = Aes256Gcm::new_from_slice(&derived_key)
        .map_err(|e| format!("encrypt: {}", e))?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .map_err(|e| format!("encrypt: {}", e))?;

    // Pack: Salt (16) + Nonce (12) + Ciphertext
    let mut packed = Vec::with_capacity(16 + 12 + ciphertext.len());
    packed.extend_from_slice(&salt);
    packed.extend_from_slice(&nonce_bytes);
    packed.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(packed))
}

/// B10: decrypt is fed attacker/user-controlled input (the ciphertext string and the key).
/// It used to `expect`/`unwrap` on every failure mode — invalid Base64, truncated payload,
/// wrong password, non-UTF-8 plaintext — which aborted the whole process. That is a trivial
/// denial-of-service. Return a clear error-marker string instead so the caller stays alive
/// and can branch on the result (consistent with other string-returning runtime builtins).
/// Decrypt, or say why it could not be done.
///
/// Every failure used to come back as the *plaintext*: a wrong password produced the string
/// "[VargOS] decrypt error: wrong password or corrupted data", which the caller then stored,
/// printed or sent on as though it were the secret. The same shape as the retired `file_read`,
/// which handed back its error as the file's contents.
pub fn __varg_decrypt(data: &str, key: &str) -> Result<String, String> {
    let packed = STANDARD
        .decode(data)
        .map_err(|_| "decrypt: the payload is not valid Base64".to_string())?;
    if packed.len() < 16 + 12 {
        return Err("decrypt: the payload is too short to be AES-GCM".to_string());
    }
    let salt = &packed[0..16];
    let nonce_bytes = &packed[16..28];
    let ciphertext = &packed[28..];
    let derived_key = __varg_derive_key(key, salt);
    let cipher = Aes256Gcm::new_from_slice(&derived_key)
        .map_err(|e| format!("decrypt: key setup failed: {}", e))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "decrypt: wrong password, or the data has been altered".to_string())?;
    String::from_utf8(plaintext)
        .map_err(|_| "decrypt: the decrypted bytes are not valid UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let original = "Hello, Varg!";
        let key = "test-password-123";
        let encrypted = __varg_encrypt(original, key).unwrap();
        let decrypted = __varg_decrypt(&encrypted, key).unwrap();
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
    fn test_decrypt_wrong_password_is_an_error_not_a_plaintext() {
        // These three asserted that decrypt returns "[VargOS] decrypt error: ..." -- as the
        // *plaintext*. That is what the failure looked like to a caller: a string it would store,
        // print or send on as though it were the secret. The original B7 fix stopped the panic;
        // the value it produced instead was still a lie.
        let encrypted = __varg_encrypt("secret data", "correct-password").unwrap();
        let result = __varg_decrypt(&encrypted, "wrong-password");
        assert!(result.is_err(), "a wrong password must be an error, got: {:?}", result);
        assert!(result.unwrap_err().contains("wrong password"));
    }

    #[test]
    fn test_decrypt_invalid_base64_is_an_error() {
        let result = __varg_decrypt("!!!not base64!!!", "any-key");
        assert!(result.is_err(), "malformed input must be an error, got: {:?}", result);
    }

    #[test]
    fn test_decrypt_truncated_payload_is_an_error() {
        // Valid Base64, but under the 28-byte salt+nonce minimum.
        let result = __varg_decrypt("YWJj", "any-key");
        assert!(result.is_err(), "a truncated payload must be an error, got: {:?}", result);
    }
}
