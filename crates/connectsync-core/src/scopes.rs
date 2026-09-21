//! Google Drive yetki (OAuth kapsamı) profilleri. Platformdan bağımsızdır: masaüstü ve mobil aynı
//! kapsam kümelerini kullanır.
//!
//! - [`DriveAccess::AppOnly`] (`drive.file`): yalnızca uygulamanın KENDİ oluşturduğu (ya da kullanıcının
//!   Picker ile seçtiği) dosyalar. Güvenli varsayılan; ama başka bir Google hesabının oluşturduğu klasöre
//!   sync koduyla bağlanılamaz (API "404 dosya yok" döner).
//! - [`DriveAccess::Full`] (`drive`): tüm Drive. Farklı hesaplar arası paylaşımı mümkün kılar; Google'ın
//!   "kısıtlı kapsam" doğrulamasını gerektirir ve uygulamaya kullanıcının TÜM Drive'ına erişim verir.
//!
//! İkisinde de `drive.appdata` (anahtar kaydı `connectsync_keys.json`) istenir.

pub const SCOPE_DRIVE_FILE: &str = "https://www.googleapis.com/auth/drive.file";
pub const SCOPE_DRIVE_FULL: &str = "https://www.googleapis.com/auth/drive";
pub const SCOPE_DRIVE_APPDATA: &str = "https://www.googleapis.com/auth/drive.appdata";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DriveAccess {
    #[default]
    AppOnly,
    Full,
}

impl DriveAccess {
    pub fn from_full_flag(full: bool) -> Self {
        if full { Self::Full } else { Self::AppOnly }
    }

    pub fn is_full(self) -> bool {
        self == Self::Full
    }

    /// Google'dan istenecek kapsamlar.
    pub fn scopes(self) -> &'static [&'static str] {
        match self {
            Self::AppOnly => &[SCOPE_DRIVE_FILE, SCOPE_DRIVE_APPDATA],
            Self::Full => &[SCOPE_DRIVE_FULL, SCOPE_DRIVE_APPDATA],
        }
    }

    /// Belirteç önbelleğinde profile özgü ek. Her profilin belirteci AYRI saklanır; aksi halde kapsam
    /// değiştiğinde eski (dar) belirteç yeniden kullanılır ve API yine "dosya yok" derdi.
    pub fn cache_tag(self) -> &'static str {
        match self {
            Self::AppOnly => "app",
            Self::Full => "full",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_narrow_safe_profile() {
        assert_eq!(DriveAccess::default(), DriveAccess::AppOnly);
        assert!(!DriveAccess::default().is_full());
    }

    #[test]
    fn the_flag_maps_to_the_right_profile_and_back() {
        for full in [true, false] {
            assert_eq!(DriveAccess::from_full_flag(full).is_full(), full);
        }
    }

    #[test]
    fn app_only_never_requests_the_whole_drive() {
        let s = DriveAccess::AppOnly.scopes();
        assert!(s.contains(&SCOPE_DRIVE_FILE));
        assert!(!s.contains(&SCOPE_DRIVE_FULL));
    }

    #[test]
    fn full_requests_the_whole_drive_instead_of_per_file_access() {
        let s = DriveAccess::Full.scopes();
        assert!(s.contains(&SCOPE_DRIVE_FULL));
        assert!(!s.contains(&SCOPE_DRIVE_FILE), "drive zaten drive.file'ı kapsar");
    }

    #[test]
    fn both_profiles_keep_the_key_registry_scope() {
        for a in [DriveAccess::AppOnly, DriveAccess::Full] {
            assert!(a.scopes().contains(&SCOPE_DRIVE_APPDATA), "{a:?}");
        }
    }

    #[test]
    fn the_scope_strings_are_the_real_google_ones() {
        // Yanlış yazılmış bir kapsam Google'da "invalid_scope" hatası verir ve yalnızca giriş anında fark edilir.
        assert_eq!(SCOPE_DRIVE_FULL, "https://www.googleapis.com/auth/drive");
        assert_eq!(SCOPE_DRIVE_FILE, "https://www.googleapis.com/auth/drive.file");
        assert_eq!(SCOPE_DRIVE_APPDATA, "https://www.googleapis.com/auth/drive.appdata");
    }

    #[test]
    fn profiles_use_distinct_token_caches() {
        assert_ne!(DriveAccess::AppOnly.cache_tag(), DriveAccess::Full.cache_tag());
    }
}
