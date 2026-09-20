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
