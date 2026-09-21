//! ConnectSync mobil (Android) giriş noktası — İSKELET.
//!
//! Henüz arayüz, giriş (OAuth) ve depolama yok; bunlar `crates/connectsync-mobile/PLAN.md`'deki deneme
//! (spike) ile doğrulanacak. Bu crate şimdilik yalnızca ortak çekirdeği (`connectsync-core`)
//! başka bir uygulamadan kullanabildiğimizi ve çalışma alanı düzeninin derlendiğini kanıtlar.
//!
//! Slint'in Android desteği eklendiğinde (bkz. plan) buraya şunlar gelecek:
//! `#[cfg(target_os = "android")] #[unsafe(no_mangle)] fn android_main(app: slint::android::AndroidApp)`
//! ve `slint` bağımlılığına `backend-android-activity-06` özelliği.

pub use connectsync_core::limits::{clamp_threads, DEFAULT_THREADS};

/// Mobil uygulamanın kullandığı çekirdek sürümü (sağlama amaçlı).
pub fn core_limits_summary() -> String {
    format!("varsayılan eşzamanlılık: {DEFAULT_THREADS}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_can_use_the_shared_core() {
        assert_eq!(clamp_threads(0), 1);
        assert!(core_limits_summary().contains(&DEFAULT_THREADS.to_string()));
    }
}
