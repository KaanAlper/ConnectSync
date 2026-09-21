use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod, parse_application_secret};
use yup_oauth2::authenticator_delegate::InstalledFlowDelegate;
use std::future::Future;
use std::pin::Pin;
use async_trait::async_trait;
use yup_oauth2::storage::{TokenStorage, TokenStorageError, TokenInfo};
use keyring::Entry;
use connectsync_core::scopes::DriveAccess;

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

// Her yetki profilinin belirteci AYRI anahtarda saklanır. Tek anahtar kullanılsaydı kapsam değiştiğinde
// (ör. drive.file -> drive) eski, dar belirteç yeniden kullanılır ve API yine "dosya yok" derdi.
// `AppOnly` anahtarı eskisiyle aynı kalır: mevcut kullanıcılar yeniden giriş yapmaz.
const OLD_TOKEN_ENTRY: &str = "google_token";

/// Yetki profiline karşılık gelen anahtarlık kaydı adı.
fn token_entry(access: DriveAccess) -> &'static str {
    match access {
        DriveAccess::AppOnly => "google_token_v2",
        DriveAccess::Full => "google_token_v3_full",
    }
}

/// Ayarlardaki güncel yetki profili.
pub fn current_drive_access() -> DriveAccess {
    super::config::AppConfig::load().drive_access()
}

struct KeyringTokenStorage {
    entry: &'static str,
}

#[async_trait]
impl TokenStorage for KeyringTokenStorage {
    async fn set(&self, _scopes: &[&str], token: TokenInfo) -> Result<(), TokenStorageError> {
        let json = serde_json::to_string(&token).map_err(|e| {
            TokenStorageError::Other(std::borrow::Cow::Owned(format!("Serialization error: {}", e)))
        })?;
        let entry = Entry::new("ConnectSync", self.entry)
            .map_err(|e| TokenStorageError::Other(std::borrow::Cow::Owned(format!("Keyring error: {}", e))))?;
        entry.set_password(&json).map_err(|e| {
            TokenStorageError::Other(std::borrow::Cow::Owned(format!("Keyring save error: {}", e)))
        })?;
        Ok(())
    }

    async fn get(&self, _scopes: &[&str]) -> Option<TokenInfo> {
        let entry = Entry::new("ConnectSync", self.entry).ok()?;
        let json = entry.get_password().ok()?;
        serde_json::from_str(&json).ok()
    }
}

pub async fn get_drive_token(interactive: bool) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let secret_json = include_bytes!("../../client_secret.json");
    let secret = parse_application_secret(secret_json)?;
    let access = current_drive_access();

    let auth = InstalledFlowAuthenticator::builder(
        secret,
        InstalledFlowReturnMethod::HTTPRedirect,
    )
    .with_storage(Box::new(KeyringTokenStorage { entry: token_entry(access) }))
    .flow_delegate(Box::new(CustomBrowserDelegate { interactive }))
    .build()
    .await?;

    // Kapsamlar yetki profilinden gelir (bkz. connectsync_core::scopes):
    // - AppOnly: drive.file (yalnızca uygulamanın oluşturduğu klasörler) + drive.appdata
    // - Full:    drive (tüm Drive; başka hesapların sync kodlarına bağlanmak için) + drive.appdata
    // drive.appdata: appDataFolder'daki anahtar kaydı (connectsync_keys.json) — diğer PC'lerde sync'leri bulmak için şart
    let scopes = access.scopes();
    
    // yup_oauth2 caches in memory and keyring transparently
    let token = tokio::time::timeout(std::time::Duration::from_secs(120), auth.token(scopes))
        .await
        .map_err(|_| "Giriş işlemi zaman aşımına uğradı (tarayıcı kapatılmış olabilir)")??;
        
    let token_str = token.token().ok_or("Token alınamadı")?.to_string();
    Ok(token_str)
}

/// Güncel yetki profili için önbellekte belirteç var mı? (Diğer profilin belirteci sayılmaz.)
pub fn is_token_cached() -> bool {
    if let Ok(entry) = Entry::new("ConnectSync", token_entry(current_drive_access())) {
        entry.get_password().is_ok()
    } else {
        false
    }
}

/// Bir yetki profilinin belirtecini bu bilgisayarın anahtarlığından siler. Google tarafında İPTAL ETMEZ:
/// aynı istemcinin diğer profilinin belirteci de aynı Google iznine bağlı olabilir; kullanıcı erişimi
/// tamamen kaldırmak isterse Google Hesap izinlerinden yapar. (Kayıt yoksa zaten yapılacak bir şey yok.)
pub fn forget_token(access: DriveAccess) {
    if let Ok(entry) = Entry::new("ConnectSync", token_entry(access)) {
        let _ = entry.delete_credential();
    }
}

pub fn logout() {
    // Çıkışta İKİ profilin belirteci de silinir.
    for access in [DriveAccess::AppOnly, DriveAccess::Full] {
        forget_token(access);
    }
    if let Ok(entry) = Entry::new("ConnectSync", OLD_TOKEN_ENTRY) {
        let _ = entry.delete_credential();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_profile_has_its_own_token_entry() {
        assert_ne!(token_entry(DriveAccess::AppOnly), token_entry(DriveAccess::Full));
    }

    #[test]
    fn the_app_only_entry_is_unchanged_so_existing_users_stay_signed_in() {
        assert_eq!(token_entry(DriveAccess::AppOnly), "google_token_v2");
    }
}
