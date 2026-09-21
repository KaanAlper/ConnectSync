use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncFolder {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(skip)]
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    pub sync_interval_minutes: u32,
    pub auto_start_enabled: bool,
    pub start_in_tray: bool,
    pub concurrent_threads: u32,
    pub language: String,
    pub sync_folders: Vec<SyncFolder>,
    /// Yerel klasördeki değişiklikleri anında algılayıp eşitle (kapatılırsa yalnızca
    /// periyodik tarama ile eşitlenir).
    #[serde(default = "default_true")]
    pub watch_local_changes: bool,
    /// Kullanılmayan (orphan) chunk'ları Drive'dan otomatik temizle.
    #[serde(default = "default_true")]
    pub auto_gc: bool,
    /// Hata oluştuğunda ekranda popup/toast göster (kapatılırsa hata yalnızca satırda görünür).
    #[serde(default = "default_true")]
    pub show_error_popups: bool,
    /// İnternet bağlantısı kesildiğinde ayrı bir uyarı popup'ı göster.
    #[serde(default = "default_true")]
    pub show_network_popups: bool,
    /// Arayüz teması: `THEME_DARK` (varsayılan) ya da `THEME_LIGHT`. Tanınmayan değer koyu sayılır.
    pub theme: String,
}

pub const THEME_DARK: &str = "dark";
pub const THEME_LIGHT: &str = "light";

/// Arayüzdeki "koyu tema" anahtarının değerini config'teki tema adına çevirir.
pub fn theme_name(dark: bool) -> &'static str {
    if dark { THEME_DARK } else { THEME_LIGHT }
}

fn default_true() -> bool {
    true
}

/// Eşzamanlı aktarım ayarı için varsayılan/sınır değerler.
/// Bellek üst sınırı için bkz. `engine.rs` başındaki not (16'da ≈ 350 MiB).
pub const DEFAULT_THREADS: u32 = 4;
pub const MIN_THREADS: u32 = 1;
pub const MAX_THREADS: u32 = 16;

/// Kullanıcının girdiği/okunan değeri izin verilen aralığa çeker.
pub fn clamp_threads(threads: u32) -> u32 {
    threads.clamp(MIN_THREADS, MAX_THREADS)
}

/// "Kontrol aralığı" ayarının varsayılanı (dakika).
pub const DEFAULT_CHECK_INTERVAL_MINUTES: u32 = 5;

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sync_interval_minutes: DEFAULT_CHECK_INTERVAL_MINUTES,
            auto_start_enabled: false,
            start_in_tray: true,
            concurrent_threads: DEFAULT_THREADS,
            language: "tr".to_string(),
            sync_folders: Vec::new(),
            watch_local_changes: true,
            auto_gc: true,
            show_error_popups: true,
            show_network_popups: true,
            theme: THEME_DARK.to_string(),
        }
    }
}

impl AppConfig {
    /// Koyu palet mi kullanılacak? Yalnızca açıkça `light` denmişse hayır; bozuk/eski değerde
    /// uygulamanın mevcut (koyu) görünümü korunur.
    pub fn is_dark_theme(&self) -> bool {
        self.theme != THEME_LIGHT
    }

    /// Periyodik kontrol aralığı (dakika). Eski config'lerdeki 0/eksik değer varsayılana düşer.
    pub fn check_interval_minutes(&self) -> u32 {
        if self.sync_interval_minutes == 0 {
            DEFAULT_CHECK_INTERVAL_MINUTES
        } else {
            self.sync_interval_minutes
        }
    }

    fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "ConnectSync", "ConnectSync")
            .map(|proj_dirs| {
                let dir = proj_dirs.config_dir();
                fs::create_dir_all(dir).ok();
                dir.join("config.json")
            })
    }

    pub fn load() -> Self {
        let mut config = Self::default();
        if let Some(path) = Self::config_path()
            && let Ok(data) = fs::read_to_string(path)
                && let Ok(loaded) = serde_json::from_str::<Self>(&data) {
                    config = loaded;
                }
        
        // Load codes from keyring
        if let Ok(entry) = keyring::Entry::new("ConnectSync", "sync_codes_v2")
            && let Ok(data) = entry.get_password()
                && let Ok(codes_map) = serde_json::from_str::<std::collections::HashMap<String, String>>(&data) {
                    for f in &mut config.sync_folders {
                        if let Some(code) = codes_map.get(&f.id) {
                            f.code = code.clone();
                        }
                    }
                }
        
        config
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(path) = Self::config_path()
            && let Ok(json) = serde_json::to_string_pretty(self) {
                let temp_path = path.with_extension("json.tmp");
                if fs::write(&temp_path, json).is_ok() {
                    let _ = fs::rename(temp_path, path);
                }
            }
        
        if let Ok(entry) = keyring::Entry::new("ConnectSync", "sync_codes_v2") {
            let mut codes_map = std::collections::HashMap::new();
            for f in &self.sync_folders {
                codes_map.insert(f.id.clone(), f.code.clone());
            }
            if let Ok(json) = serde_json::to_string(&codes_map) {
                let _ = entry.set_password(&json);
            } else {
                let _ = entry.delete_credential();
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_dark() {
        assert!(AppConfig::default().is_dark_theme());
    }

    #[test]
    fn only_explicit_light_turns_dark_off() {
        let with_theme = |theme: &str| AppConfig { theme: theme.into(), ..AppConfig::default() };
        assert!(!with_theme(THEME_LIGHT).is_dark_theme());
        for junk in ["", "Light", "system", "solarized"] {
            assert!(with_theme(junk).is_dark_theme(), "{junk:?} koyu sayılmalı");
        }
    }

    #[test]
    fn theme_name_round_trips() {
        for dark in [true, false] {
            let cfg = AppConfig { theme: theme_name(dark).into(), ..AppConfig::default() };
            assert_eq!(cfg.is_dark_theme(), dark);
        }
    }

    #[test]
    fn old_config_without_theme_field_loads_as_dark() {
        // Tema alanı eklenmeden önce yazılmış bir config.json.
        let cfg: AppConfig = serde_json::from_str(r#"{"sync_interval_minutes": 9, "language": "en"}"#).unwrap();
        assert_eq!(cfg.sync_interval_minutes, 9);
        assert!(cfg.is_dark_theme());
    }

    #[test]
    fn check_interval_zero_falls_back_to_default() {
        let with_interval = |m: u32| AppConfig { sync_interval_minutes: m, ..AppConfig::default() };
        assert_eq!(with_interval(0).check_interval_minutes(), DEFAULT_CHECK_INTERVAL_MINUTES);
        assert_eq!(with_interval(17).check_interval_minutes(), 17);
    }
}
