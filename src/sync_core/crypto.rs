use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes256Gcm;
use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use sha2::Sha256;
use hmac::{Hmac, Mac};
use std::error::Error;

pub struct Keys {
    pub enc_key: [u8; 32],
    pub hmac_key: [u8; 32],
}

pub fn derive_keys(sync_code: &str, salt: &[u8]) -> Result<Keys, Box<dyn Error + Send + Sync>> {
    let mut ikm = [0u8; 32];
    
    // Argon2 with explicit safe parameters to prevent silent upgrade breakage
    let params = Params::new(64 * 1024, 3, 1, Some(32))
        .map_err(|e| format!("Argon2 params hatası: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    argon2.hash_password_into(sync_code.as_bytes(), salt, &mut ikm)
        .map_err(|e| format!("Anahtar türetme hatası: {}", e))?;

    let hkdf = Hkdf::<Sha256>::new(None, &ikm);
    
    let mut enc_key = [0u8; 32];
    let mut hmac_key = [0u8; 32];
    
    hkdf.expand(b"ConnectSync_Encryption", &mut enc_key)
        .map_err(|e| format!("HKDF enc error: {}", e))?;
    hkdf.expand(b"ConnectSync_HMAC", &mut hmac_key)
        .map_err(|e| format!("HKDF hmac error: {}", e))?;
        
    Ok(Keys { enc_key, hmac_key })
}

pub fn hash_chunk_name(hmac_key: &[u8; 32], data: &[u8]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(hmac_key).expect("HMAC can take key of any size");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

pub fn encrypt_chunk(
    enc_key: &[u8; 32],
    name_aad: &str,
    data: &[u8],
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    #[allow(deprecated)]
    let cipher = Aes256Gcm::new(enc_key.into());
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| format!("Rastgele nonce üretilemedi: {}", e))?;

    let payload = Payload {
        msg: data,
        aad: name_aad.as_bytes(),
    };

    let mut encrypted = cipher
        .encrypt(&nonce.into(), payload)
        .map_err(|e| format!("Şifreleme hatası: {}", e))?;

    let mut out = nonce.to_vec();
    out.append(&mut encrypted);
    Ok(out)
}

pub fn decrypt_chunk(
    enc_key: &[u8; 32],
    name_aad: &str,
    encrypted_data: &[u8],
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    if encrypted_data.len() < 12 {
        return Err("Geçersiz şifreli veri boyutu (nonce eksik)".into());
    }
    let (nonce, ciphertext) = encrypted_data.split_at(12);

    #[allow(deprecated)]
    let cipher = Aes256Gcm::new(enc_key.into());
    let payload = Payload {
        msg: ciphertext,
        aad: name_aad.as_bytes(),
    };

    let decrypted = cipher
        .decrypt(nonce.try_into().expect("Geçersiz nonce boyutu"), payload)
        .map_err(|e| -> Box<dyn Error + Send + Sync> { format!("Şifre çözme hatası (veya veri bozulmuş): {}", e).into() })?;
        
    Ok(decrypted)
}
