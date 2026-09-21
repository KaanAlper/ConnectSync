use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use directories::ProjectDirs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncFolder {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(skip)]
    pub code: String,
    /// Klasör oluşturulurken seçilen izin: `Some(true)` = bağlantıyı bilen herkes yazabilir,
    /// `Some(false)` = yalnızca okur. `None` = bilinmiyor (bu alan eklenmeden önceki ya da koddan
    /// katılınan sync'ler); yeniden yüklemede eski davranış (yazılabilir) korunur.
    #[serde(default)]
    pub can_others_write: Option<bool>,
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

/// Sync düzenleme/oluşturma girdisinin neden reddedildiği.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncEditError {
    EmptyName,
    EmptyPath,
    /// Aynı yerel klasör başka bir sync'te zaten kullanılıyor.
    PathInUse,
    /// Seçilen yol var ama bir klasör değil.
    PathIsFile,
}

impl SyncEditError {
    /// Kullanıcıya gösterilecek metnin locale anahtarı.
    pub fn locale_key(self) -> &'static str {
        match self {
            Self::EmptyName => "err_edit_name_empty",
            Self::EmptyPath => "err_edit_path_empty",
            Self::PathInUse => "err_folder_exists",
            Self::PathIsFile => "err_path_not_dir",
        }
    }
}

/// `apply_sync_edit` sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditApplied {
    /// Sync config'te yoktu, yeni eklendi (ör. buluttan eklenen).
    pub is_new: bool,
    /// Var olan sync'in önceki yerel yolu.
    pub old_path: Option<String>,
    /// Sync kodu (Drive klasör kimliği bundan çıkarılır).
    pub code: String,
}

/// Tüm süreçte tek seferde bir config oku-değiştir-yaz işlemi çalışsın diye.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

impl AppConfig {
    /// Oku → değiştir → yaz işlemini süreç genelinde kilitli yapar. Arka plan görevleri ile arayüz
    /// callback'leri aynı anda `load()`/`save()` yaparsa son yazan diğerinin değişikliğini ezerdi
    /// (özellikle `sync_folders` listesi). Liste değiştiren her yer bunu kullanmalı.
    pub fn update<R>(f: impl FnOnce(&mut Self) -> R) -> Result<R, String> {
        Self::locked_update(Self::load, |c| c.save(), f)
    }

    fn locked_update<R>(
        load: impl FnOnce() -> Self,
        save: impl FnOnce(&Self) -> Result<(), String>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, String> {
        // Zehirlenmiş kilit (başka bir thread panik etti) config'i kullanılmaz yapmasın.
        let _guard = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut cfg = load();
        let out = f(&mut cfg);
        save(&cfg)?;
        Ok(out)
    }

    /// Ad/yol girdisi kaydedilebilir mi? `id`: düzenlenen sync (yeni sync için boş bırak).
    pub fn validate_sync_edit(&self, id: &str, name: &str, path: &str) -> Result<(), SyncEditError> {
        if name.trim().is_empty() {
            return Err(SyncEditError::EmptyName);
        }
        let path = path.trim();
        if path.is_empty() {
            return Err(SyncEditError::EmptyPath);
        }
        let p = Path::new(path);
        if self.sync_folders.iter().any(|f| f.id != id && Path::new(&f.path) == p) {
            return Err(SyncEditError::PathInUse);
        }
        if p.exists() && !p.is_dir() {
            return Err(SyncEditError::PathIsFile);
        }
        Ok(())
    }

    /// Var olan sync'in adını/yolunu günceller; yoksa yeni sync olarak ekler (`kod = id`).
    pub fn apply_sync_edit(&mut self, id: &str, name: &str, path: &str) -> EditApplied {
        if let Some(f) = self.sync_folders.iter_mut().find(|f| f.id == id) {
            let old = std::mem::replace(&mut f.path, path.to_string());
            f.name = name.to_string();
            let code = if f.code.is_empty() { f.id.clone() } else { f.code.clone() };
            return EditApplied { is_new: false, old_path: Some(old), code };
        }
        self.sync_folders.push(SyncFolder {
            id: id.to_string(),
            name: name.to_string(),
            path: path.to_string(),
            code: id.to_string(),
            can_others_write: None,
        });
        EditApplied { is_new: true, old_path: None, code: id.to_string() }
    }

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

    // ── sync düzenleme doğrulaması ──────────────────────────────────────────

    fn folder(id: &str, path: &str) -> SyncFolder {
        SyncFolder {
            id: id.into(),
            name: "x".into(),
            path: path.into(),
            code: id.into(),
            can_others_write: None,
        }
    }

    fn config_with(folders: Vec<SyncFolder>) -> AppConfig {
        AppConfig { sync_folders: folders, ..AppConfig::default() }
    }

    #[test]
    fn rejects_empty_name_and_path() {
        let c = config_with(vec![]);
        assert_eq!(c.validate_sync_edit("", "  ", "/tmp/a"), Err(SyncEditError::EmptyName));
        assert_eq!(c.validate_sync_edit("", "Ad", "   "), Err(SyncEditError::EmptyPath));
        assert_eq!(c.validate_sync_edit("", "Ad", ""), Err(SyncEditError::EmptyPath));
    }

    #[test]
    fn rejects_a_path_used_by_another_sync_but_not_by_itself() {
        let c = config_with(vec![folder("a", "/data/music"), folder("b", "/data/docs")]);
        // b, a'nın klasörünü almaya çalışıyor
        assert_eq!(c.validate_sync_edit("b", "Ad", "/data/music"), Err(SyncEditError::PathInUse));
        // a kendi yolunu (ya da yalnızca adını) değiştirirken çakışma yok
        assert_eq!(c.validate_sync_edit("a", "Yeni ad", "/data/music"), Ok(()));
        // yeni sync (id boş) kullanılmış bir yolu seçemez; sondaki '/' fark yaratmaz
        assert_eq!(c.validate_sync_edit("", "Ad", "/data/music/"), Err(SyncEditError::PathInUse));
    }

    #[test]
    fn rejects_a_path_that_is_a_file_but_accepts_missing_or_existing_dirs() {
        let dir = std::env::temp_dir().join(format!("connectsync-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("not-a-dir.txt");
        std::fs::write(&file, b"x").unwrap();
        let c = config_with(vec![]);

        assert_eq!(
            c.validate_sync_edit("", "Ad", file.to_str().unwrap()),
            Err(SyncEditError::PathIsFile)
        );
        assert_eq!(c.validate_sync_edit("", "Ad", dir.to_str().unwrap()), Ok(()));
        // Henüz olmayan klasör geçerli (buluttan eklerken indirmede oluşturulur)
        assert_eq!(c.validate_sync_edit("", "Ad", dir.join("yeni").to_str().unwrap()), Ok(()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_edit_error_has_a_distinct_locale_key() {
        use std::collections::HashSet;
        let keys: HashSet<_> = [
            SyncEditError::EmptyName,
            SyncEditError::EmptyPath,
            SyncEditError::PathInUse,
            SyncEditError::PathIsFile,
        ]
        .into_iter()
        .map(SyncEditError::locale_key)
        .collect();
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn applying_an_edit_updates_an_existing_sync_and_reports_the_old_path() {
        let mut c = config_with(vec![folder("s1", "/old")]);
        let r = c.apply_sync_edit("s1", "Yeni", "/new");
        assert_eq!(r, EditApplied { is_new: false, old_path: Some("/old".into()), code: "s1".into() });
        assert_eq!((c.sync_folders[0].name.as_str(), c.sync_folders[0].path.as_str()), ("Yeni", "/new"));
        assert_eq!(c.sync_folders.len(), 1);
    }

    #[test]
    fn applying_an_edit_for_an_unknown_id_adds_a_new_sync_with_code_equal_to_id() {
        let mut c = config_with(vec![]);
        let r = c.apply_sync_edit("cs-abc-key", "Müzik", "/music");
        assert_eq!(r, EditApplied { is_new: true, old_path: None, code: "cs-abc-key".into() });
        assert_eq!(c.sync_folders.len(), 1);
        assert_eq!(c.sync_folders[0].code, "cs-abc-key");
        assert_eq!(c.sync_folders[0].can_others_write, None);
    }

    #[test]
    fn an_existing_sync_with_an_empty_code_falls_back_to_its_id() {
        let mut f = folder("cs-abc-key", "/p");
        f.code.clear(); // anahtar zincirinden okunamadı
        let mut c = config_with(vec![f]);
        assert_eq!(c.apply_sync_edit("cs-abc-key", "Ad", "/p").code, "cs-abc-key");
    }

    // ── paylaşım izni saklama ───────────────────────────────────────────────

    #[test]
    fn old_folder_entries_without_the_permission_field_load_as_unknown() {
        let c: AppConfig = serde_json::from_str(
            r#"{"sync_folders":[{"id":"a","name":"n","path":"/p"}]}"#,
        )
        .unwrap();
        assert_eq!(c.sync_folders[0].can_others_write, None);
    }

    #[test]
    fn the_permission_choice_survives_a_save_load_round_trip() {
        for choice in [Some(true), Some(false), None] {
            let mut f = folder("a", "/p");
            f.can_others_write = choice;
            let json = serde_json::to_string(&config_with(vec![f])).unwrap();
            let back: AppConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(back.sync_folders[0].can_others_write, choice);
        }
    }

    // ── kilitli güncelleme ──────────────────────────────────────────────────

    #[test]
    fn locked_update_does_not_lose_concurrent_changes() {
        use std::sync::Arc;
        // Sahte "disk": gerçek config dosyasına/anahtarlığa dokunmaz.
        let disk = Arc::new(Mutex::new(AppConfig { sync_interval_minutes: 0, ..AppConfig::default() }));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let disk = disk.clone();
                std::thread::spawn(move || {
                    let d_load = disk.clone();
                    let d_save = disk.clone();
                    AppConfig::locked_update(
                        move || d_load.lock().unwrap().clone(),
                        move |c| {
                            *d_save.lock().unwrap() = c.clone();
                            Ok(())
                        },
                        |c| {
                            // Okuma ile yazma arasında bekle: kilit yoksa diğer thread'ler araya girip ezerdi
                            std::thread::sleep(std::time::Duration::from_millis(5));
                            c.sync_interval_minutes += 1;
                        },
                    )
                    .unwrap();
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(disk.lock().unwrap().sync_interval_minutes, 8);
    }

    #[test]
    fn locked_update_propagates_save_errors_and_returns_the_closure_value() {
        let ok = AppConfig::locked_update(AppConfig::default, |_| Ok(()), |_| 42);
        assert_eq!(ok, Ok(42));
        let err = AppConfig::locked_update(AppConfig::default, |_| Err("disk dolu".into()), |_| ());
        assert_eq!(err, Err("disk dolu".to_string()));
    }
}
