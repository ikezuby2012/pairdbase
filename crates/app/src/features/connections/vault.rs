use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::{
    aead::{Aead, AeadCore, Generate, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};

#[derive(Clone)]
pub struct Vault {
    cipher: Aes256Gcm,
}

impl Vault {
    pub fn new(hex_key: &str) -> anyhow::Result<Self> {
        let key_bytes = hex::decode(hex_key)
            .map_err(|_| anyhow::anyhow!("Vault encryption key must be valid hex"))?;

        if key_bytes.len() != 32 {
            anyhow::bail!("Vault encryption key must be exactly 32 bytes (64 hex characters)");
        }

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);

        Ok(Self {
            cipher: Aes256Gcm::new(key),
        })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, String> {
        let nonce = Nonce::generate();

        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| "Encryption failed".to_string())?;

        let mut output = nonce.to_vec();
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<String, String> {
        if data.len() < 12 {
            return Err("Encrypted data too short".into());
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| "Decryption failed".to_string())?;

        String::from_utf8(plaintext).map_err(|_| "Decrypted data is not valid UTF-8".to_string())
    }

    pub fn encrypt_opt(&self, value: Option<&str>) -> Result<Option<Vec<u8>>, String> {
        value.map(|v| self.encrypt(v)).transpose()
    }

    pub fn decrypt_opt(&self, data: Option<&[u8]>) -> Result<Option<String>, String> {
        data.map(|d| self.decrypt(d)).transpose()
    }
}

