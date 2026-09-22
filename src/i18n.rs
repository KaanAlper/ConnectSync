//! Basit JSON tabanlı çeviri altyapısı.
//! Yeni dil eklemek için: locales/<kod>.json oluştur, aşağıdaki LANGS listesine ekle
//! ve Slint tarafındaki ComboBox modeline ekle.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// (dil kodu, ekranda görünen ad, json içeriği)
const LANGS: &[(&str, &str, &str)] = &[
    ("tr", "Türkçe", include_str!("../locales/tr.json")),
    ("en", "English", include_str!("../locales/en.json")),
    ("de", "Deutsch", include_str!("../locales/de.json")),
    ("es", "Español", include_str!("../locales/es.json")),
    ("fr", "Français", include_str!("../locales/fr.json")),
    ("zh", "中文", include_str!("../locales/zh.json")),
    ("ko", "한국어", include_str!("../locales/ko.json")),
    ("ja", "日本語", include_str!("../locales/ja.json")),
];

const FALLBACK: &str = "tr";

static TABLES: OnceLock<HashMap<&'static str, HashMap<String, String>>> = OnceLock::new();
static CURRENT: RwLock<String> = RwLock::new(String::new());

fn tables() -> &'static HashMap<&'static str, HashMap<String, String>> {
    TABLES.get_or_init(|| {
        LANGS
            .iter()
            .map(|(code, _, json)| (*code, serde_json::from_str(json).unwrap_or_default()))
            .collect()
    })
}

/// Bilinmeyen bir kod gelirse (ör. eski config'teki "de") İngilizceye düşer.
pub fn normalize(code: &str) -> &'static str {
    LANGS
        .iter()
        .map(|(c, _, _)| *c)
        .find(|c| *c == code)
        .unwrap_or("en")
}

pub fn set_language(code: &str) {
    *CURRENT.write().unwrap() = normalize(code).to_string();
}

pub fn current() -> String {
    let cur = CURRENT.read().unwrap().clone();
    if cur.is_empty() { FALLBACK.to_string() } else { cur }
}

/// Anahtarın çevirisi; aktif dilde yoksa Türkçe, o da yoksa anahtarın kendisi.
pub fn t(key: &str) -> String {
    let cur = current();
    let tabs = tables();
    tabs.get(cur.as_str())
        .and_then(|m| m.get(key))
        .or_else(|| tabs.get(FALLBACK).and_then(|m| m.get(key)))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

/// `{n}`, `{e}` gibi yer tutucuları doldurur.
pub fn tf<V: AsRef<str>>(key: &str, args: &[(&str, V)]) -> String {
    let mut s = t(key);
    for (k, v) in args {
        s = s.replace(&format!("{{{}}}", k), v.as_ref());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_language_has_the_same_keys_as_the_fallback() {
        let tabs = tables();
        let base: BTreeSet<&String> = tabs[FALLBACK].keys().collect();
        for (code, table) in tabs {
            let keys: BTreeSet<&String> = table.keys().collect();
            let missing: Vec<_> = base.difference(&keys).collect();
            let extra: Vec<_> = keys.difference(&base).collect();
            assert!(missing.is_empty(), "{code}: eksik anahtarlar: {missing:?}");
            assert!(extra.is_empty(), "{code}: fazladan anahtarlar: {extra:?}");
        }
    }

    #[test]
    fn every_key_used_in_rust_source_exists_in_the_fallback_locale() {
        // Kodda `i18n::t("anahtar")` yazıp locale'e eklemeyi unutmak ekranda ham anahtar gösterir.
        let sources = [
            ("main.rs", include_str!("main.rs")),
            ("update_ui.rs", include_str!("update_ui.rs")),
            ("drive_missing.rs", include_str!("drive_missing.rs")),
            ("conflicts.rs", include_str!("conflicts.rs")),
        ];
        let fallback = &tables()[FALLBACK];
        let mut checked = 0;
        for (file, src) in sources {
            for call in ["i18n::t(\"", "i18n::tf(\"", "tr_ss(\""] {
                for chunk in src.split(call).skip(1) {
                    let key = chunk.split('"').next().unwrap();
                    assert!(fallback.contains_key(key), "{file}: '{key}' anahtarı locales/{FALLBACK}.json'da yok");
                    checked += 1;
                }
            }
        }
        assert!(checked > 20, "anahtarlar ayrıştırılamadı ({checked})");
    }

    #[test]
    fn edit_error_messages_exist_in_every_language() {
        // `SyncEditError::locale_key()` dinamik olduğu için kaynak taramasında yakalanmaz.
        use crate::sync_core::config::SyncEditError::{EmptyName, EmptyPath, PathInUse, PathIsFile};
        for err in [EmptyName, EmptyPath, PathInUse, PathIsFile] {
            for (code, table) in tables() {
                assert!(table.contains_key(err.locale_key()), "{code}: '{}' yok", err.locale_key());
            }
        }
    }

    #[test]
    fn no_language_has_empty_values() {
        for (code, table) in tables() {
            for (key, value) in table {
                assert!(!value.trim().is_empty(), "{code}.{key} boş");
            }
        }
    }

    #[test]
    fn placeholders_survive_translation() {
        // `{n}` yer tutucusu her dilde bulunmalı, yoksa deneme sayısı arayüzde kaybolur.
        for (code, table) in tables() {
            assert!(table["net_waiting"].contains("{n}"), "{code}: net_waiting'de {{n}} yok");
        }
    }

    #[test]
    fn tf_fills_placeholders() {
        let s = tf("net_waiting", &[("n", "7")]);
        assert!(s.contains('7') && !s.contains("{n}"), "{s}");
    }

    #[test]
    fn missing_key_falls_back_to_the_key_itself() {
        assert_eq!(t("__yok_boyle_bir_anahtar__"), "__yok_boyle_bir_anahtar__");
    }
}
