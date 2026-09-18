use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod, parse_application_secret};
use yup_oauth2::authenticator_delegate::InstalledFlowDelegate;
use std::future::Future;
use std::pin::Pin;
use async_trait::async_trait;
use yup_oauth2::storage::{TokenStorage, TokenStorageError, TokenInfo};
use keyring::Entry;

#[derive(Copy, Clone)]
struct CustomBrowserDelegate {
    interactive: bool,
}

impl InstalledFlowDelegate for CustomBrowserDelegate {
    fn present_user_url<'a>(
        &'a self,
        url: &'a str,
        _need_code: bool,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        let interactive = self.interactive;
        let url = url.to_string();
        Box::pin(async move {
            if !interactive {
                return Err("Etkileşimsiz mod: Tarayıcı açılamaz, yeniden giriş gerekli.".into());
            }
            println!("Tarayıcı açılıyor...");
            if webbrowser::open(&url).is_err() {
                println!("Tarayıcı otomatik açılamadı. Lütfen şu linki kopyalayıp tarayıcıda açın:\n{}", url);
            }
            Ok(String::new())
        })
    }
}

struct KeyringTokenStorage;

#[async_trait]
impl TokenStorage for KeyringTokenStorage {
    async fn set(&self, _scopes: &[&str], token: TokenInfo) -> Result<(), TokenStorageError> {
        let json = serde_json::to_string(&token).map_err(|e| {
            TokenStorageError::Other(std::borrow::Cow::Owned(format!("Serialization error: {}", e)))
        })?;
        let entry = Entry::new("ConnectSync", "google_token")
            .map_err(|e| TokenStorageError::Other(std::borrow::Cow::Owned(format!("Keyring error: {}", e))))?;
        entry.set_password(&json).map_err(|e| {
            TokenStorageError::Other(std::borrow::Cow::Owned(format!("Keyring save error: {}", e)))
        })?;
        Ok(())
    }

    async fn get(&self, _scopes: &[&str]) -> Option<TokenInfo> {
        let entry = Entry::new("ConnectSync", "google_token").ok()?;
        let json = entry.get_password().ok()?;
        serde_json::from_str(&json).ok()
    }
}

pub async fn get_drive_token(interactive: bool) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let secret_json = include_bytes!("../../client_secret.json");
    let secret = parse_application_secret(secret_json)?;

    let auth = InstalledFlowAuthenticator::builder(
        secret,
        InstalledFlowReturnMethod::HTTPRedirect,
    )
    .with_storage(Box::new(KeyringTokenStorage))
    .flow_delegate(Box::new(CustomBrowserDelegate { interactive }))
    .build()
    .await?;

    let scopes = &["https://www.googleapis.com/auth/drive.file"];
    
    // yup_oauth2 caches in memory and keyring transparently
    let token = tokio::time::timeout(std::time::Duration::from_secs(120), auth.token(scopes))
        .await
        .map_err(|_| "Giriş işlemi zaman aşımına uğradı (tarayıcı kapatılmış olabilir)")??;
        
    let token_str = token.token().ok_or("Token alınamadı")?.to_string();
    Ok(token_str)
}

pub fn is_token_cached() -> bool {
    if let Ok(entry) = Entry::new("ConnectSync", "google_token") {
        entry.get_password().is_ok()
    } else {
        false
    }
}

pub fn logout() {
    if let Ok(entry) = Entry::new("ConnectSync", "google_token") {
        let _ = entry.delete_credential(); // V1 credential deletion method
    }
    
    if let Ok(entry) = Entry::new("ConnectSync", "sync_code") {
        let _ = entry.delete_credential();
    }
    
    // Legacy disk clear
    let token_dir = directories::ProjectDirs::from("com", "ConnectSync", "ConnectSync")
        .map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let _ = std::fs::remove_file(token_dir.join("tokencache.json"));
    let _ = std::fs::remove_file(token_dir.join("tokencache.json.enc"));
}
