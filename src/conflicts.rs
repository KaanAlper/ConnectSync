//! Çakışma yönetimi: hem bu bilgisayarda hem başka bir bilgisayarda değişmiş dosyalar.
//!
//! Motor (`connectsync_core::sync_state`) böyle bir dosyanın üzerine ASLA sessizce yazmaz; pull onu
//! atlar, push de yüklemez. Bu modül durumu tutar, kullanıcıya popup ile sorar (uygulama tepsideyken
//! bile pencere öne getirilir) ve seçimi motora iletir:
//!
//! - **Yedekle ve buluttakini al:** yerel sürüm `… (yerel yedek …)` adıyla korunur, uzak sürüm yazılır.
//! - **Yerel sürümü koru:** bir sonraki eşitlemede yerel sürüm uzağın yerine yüklenir.
//! - **Sonra karar ver:** hiçbir şey değişmez; aynı çakışma kümesi için tekrar TEKRAR sorulmaz
//!   (yeni bir çakışma çıkarsa ya da uygulama yeniden başlarsa sorulur).

use crate::{i18n, AppState, MainWindow};
use connectsync_core::engine::ConflictChoice;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Popup gövdesinde en fazla kaç dosya adı listelenir (gerisi "+N daha").
const MAX_LISTED: usize = 4;

#[derive(Default)]
pub struct ConflictStore {
    /// sync kodu → çakışan göreli yollar (sıralı)
    pending: HashMap<String, Vec<String>>,
    /// sync kodu → kullanıcının "sonra karar ver" dediği kümenin imzası
    deferred: HashMap<String, String>,
    /// Popup'ın (yeniden) gösterilmesi gerekiyor
    dirty: bool,
}

/// Bir çakışma kümesinin sıradan bağımsız imzası (aynı küme = aynı imza).
pub fn signature(files: &[String]) -> String {
    let mut v = files.to_vec();
    v.sort();
    v.join("\n")
}

impl ConflictStore {
    /// Bir pull turunun sonucunu kaydeder. Boş liste = o sync'te çakışma kalmadı.
    pub fn record(&mut self, code: &str, mut files: Vec<String>) {
        files.sort();
        if files.is_empty() {
            self.pending.remove(code);
            self.deferred.remove(code);
            return;
        }
        let deferred_same = self.deferred.get(code) == Some(&signature(&files));
        self.pending.insert(code.to_string(), files);
        if !deferred_same {
            // Yeni ya da değişmiş küme: önceki erteleme geçersiz, tekrar sor
            self.deferred.remove(code);
            self.dirty = true;
        }
    }

    /// "Sonra karar ver": bu küme için tekrar sorma.
    pub fn defer(&mut self, code: &str) {
        if let Some(files) = self.pending.get(code) {
            self.deferred.insert(code.to_string(), signature(files));
        }
    }

    pub fn resolved(&mut self, code: &str) {
        self.pending.remove(code);
        self.deferred.remove(code);
    }

    pub fn files(&self, code: &str) -> Vec<String> {
        self.pending.get(code).cloned().unwrap_or_default()
    }

    pub fn count(&self, code: &str) -> usize {
        self.pending.get(code).map_or(0, Vec::len)
    }

    /// Gösterilecek ilk (ertelenmemiş) çakışma. Sıra deterministik: sync koduna göre.
    pub fn next_to_show(&self) -> Option<(&str, &[String])> {
        let mut codes: Vec<&String> = self.pending.keys().collect();
        codes.sort();
        codes.into_iter().find_map(|code| {
            let files = &self.pending[code];
            (self.deferred.get(code) != Some(&signature(files))).then_some((code.as_str(), files.as_slice()))
        })
    }

    /// Kullanıcının dikkatine ihtiyaç var mı? (Pencere gizliyse öne getirilir.)
    pub fn needs_attention(&self) -> bool {
        self.dirty && self.next_to_show().is_some()
    }

    /// Bayrağı okur ve temizler.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// Başka bir çakışma kümesi de sırada bekliyorsa popup yeniden gösterilsin.
    pub fn mark_dirty_if_more(&mut self) {
        if self.next_to_show().is_some() {
            self.dirty = true;
        }
    }
}

/// Popup için dosya listesi: en fazla `max` ad + kalan sayısı.
pub fn list_preview(files: &[String], max: usize) -> (String, usize) {
    let shown = files.iter().take(max).map(String::as_str).collect::<Vec<_>>().join("\n");
    (shown, files.len().saturating_sub(max))
}

// ---------------------------------------------------------------------------
// Uygulama tarafı
// ---------------------------------------------------------------------------

/// Sync döngüsü her pull turundan sonra çağırır (çakışma yoksa boş liste ile: eski çakışmalar temizlenir).
pub fn report(app_state: &Arc<Mutex<AppState>>, code: &str, files: Vec<String>) {
    app_state.lock().unwrap().conflicts.record(code, files);
}

/// Tray zamanlayıcısı: pencere gizliyse/yoksa öne getirilmeli mi?
pub fn needs_attention(app_state: &Arc<Mutex<AppState>>) -> bool {
    app_state.lock().unwrap().conflicts.needs_attention()
}

/// Tray zamanlayıcısından (UI thread'i): bekleyen çakışma varsa popup'ı açar.
pub fn sync_window(ui: &MainWindow, app_state: &Arc<Mutex<AppState>>) {
    let (code, files) = {
        let mut st = app_state.lock().unwrap();
        if !st.conflicts.take_dirty() {
            return;
        }
        match st.conflicts.next_to_show() {
            Some((code, files)) => (code.to_string(), files.to_vec()),
            None => return,
        }
    };
    let name = sync_name(&code);
    let (listed, more) = list_preview(&files, MAX_LISTED);
    let mut body = i18n::tf(
        "conflict_body",
        &[("name", name.as_str()), ("n", &files.len().to_string())],
    );
    body.push_str("\n\n");
    body.push_str(&listed);
    if more > 0 {
        body.push('\n');
        body.push_str(&i18n::tf("conflict_more", &[("n", &more.to_string())]));
    }
    ui.set_conflict_sync_id(code.as_str().into());
    ui.set_conflict_body(body.as_str().into());
    ui.set_show_conflict_dialog(true);
}

fn sync_name(code: &str) -> String {
    crate::sync_core::config::AppConfig::load()
        .sync_folders
        .iter()
        .find(|f| f.id == code)
        .map(|f| f.name.clone())
        .unwrap_or_default()
}

/// "Sonra karar ver": bu küme için tekrar sorma; dosyalara dokunulmaz.
pub fn defer(app_state: &Arc<Mutex<AppState>>, code: &str) {
    let mut st = app_state.lock().unwrap();
    st.conflicts.defer(code);
    st.conflicts.mark_dirty_if_more();
}

/// Kullanıcının seçimini motora iletir (arka planda). Başarılı olan her çakışma temizlenir; hata olanlar
/// listede kalır ve hata popup ile bildirilir.
pub fn resolve(
    ui_weak: slint::Weak<MainWindow>,
    app_state: Arc<Mutex<AppState>>,
    code: String,
    choice: ConflictChoice,
) {
    let (engine, files) = {
        let st = app_state.lock().unwrap();
        (st.engines.get(&code).cloned(), st.conflicts.files(&code))
    };
    let Some(engine) = engine else {
        show_error_later(ui_weak, i18n::t("err_conflict_no_sync"));
        return;
    };
    tokio::spawn(async move {
        let mut resolved = 0usize;
        let mut failures: Vec<String> = Vec::new();
        for rel in &files {
            match engine.resolve_conflict(rel, choice).await {
                Ok(_) => resolved += 1,
                Err(e) => failures.push(format!("{rel}: {e}")),
            }
        }
        {
            let mut st = app_state.lock().unwrap();
            if failures.is_empty() {
                st.conflicts.resolved(&code);
            }
            st.conflicts.mark_dirty_if_more();
            // Eşitleme hemen bir tur çalışsın: KeepLocal yerel sürümü yükler, TakeRemote durumu güncel görür
            if let Some(trigger) = st.triggers.get(&code) {
                trigger.notify_one();
            }
        }
        if resolved > 0 {
            crate::update_status(
                &ui_weak,
                &app_state,
                &code,
                &i18n::tf("status_conflicts_resolved", &[("n", &resolved.to_string())]),
                false,
            );
        }
        if !failures.is_empty() {
            let msg = i18n::tf("err_conflict_resolve", &[("e", &failures.join("; "))]);
            show_error_later(ui_weak, msg);
        }
    });
}

fn show_error_later(ui_weak: slint::Weak<MainWindow>, msg: String) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            crate::show_error(&ui, msg.as_str().into());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_new_conflict_asks_the_user() {
        let mut s = ConflictStore::default();
        s.record("sync1", v(&["b.txt", "a.txt"]));
        assert!(s.needs_attention());
        assert_eq!(s.next_to_show().map(|(c, f)| (c, f.to_vec())), Some(("sync1", v(&["a.txt", "b.txt"]))));
        assert_eq!(s.count("sync1"), 2);
    }

    #[test]
    fn deferring_stops_the_same_set_from_nagging_on_every_sync_round() {
        let mut s = ConflictStore::default();
        s.record("s", v(&["a.txt"]));
        assert!(s.take_dirty());
        s.defer("s");
        // Her eşitleme turu aynı çakışmayı yeniden bildirir: sormamalı
        for _ in 0..3 {
            s.record("s", v(&["a.txt"]));
            assert!(!s.needs_attention());
            assert!(s.next_to_show().is_none());
        }
        // Dosyalar hâlâ çakışma olarak duruyor (çözülmedi), sadece ertelendi
        assert_eq!(s.count("s"), 1);
    }

    #[test]
    fn a_changed_conflict_set_asks_again_even_after_deferring() {
        let mut s = ConflictStore::default();
        s.record("s", v(&["a.txt"]));
        s.take_dirty();
        s.defer("s");
        s.record("s", v(&["a.txt", "b.txt"])); // yeni bir çakışma eklendi
        assert!(s.needs_attention());
    }

    #[test]
    fn the_signature_ignores_order() {
        assert_eq!(signature(&v(&["b", "a"])), signature(&v(&["a", "b"])));
        assert_ne!(signature(&v(&["a"])), signature(&v(&["a", "b"])));
    }

    #[test]
    fn an_empty_report_clears_the_conflict_and_the_deferral() {
        let mut s = ConflictStore::default();
        s.record("s", v(&["a.txt"]));
        s.defer("s");
        s.record("s", vec![]);
        assert_eq!(s.count("s"), 0);
        // Sonradan aynı dosya yeniden çakışırsa erteleme unutulmuş olmalı: tekrar sorar
        s.record("s", v(&["a.txt"]));
        assert!(s.needs_attention());
    }

    #[test]
    fn resolving_removes_the_conflict() {
        let mut s = ConflictStore::default();
        s.record("s", v(&["a.txt"]));
        s.resolved("s");
        assert!(s.next_to_show().is_none() && s.count("s") == 0);
    }

    #[test]
    fn several_syncs_are_shown_one_at_a_time_in_a_stable_order() {
        let mut s = ConflictStore::default();
        s.record("s2", v(&["x"]));
        s.record("s1", v(&["y"]));
        assert_eq!(s.next_to_show().map(|(c, _)| c), Some("s1"));
        s.defer("s1");
        assert_eq!(s.next_to_show().map(|(c, _)| c), Some("s2"));
    }

    #[test]
    fn after_handling_one_sync_the_next_pending_one_is_shown() {
        let mut s = ConflictStore::default();
        s.record("s1", v(&["a"]));
        s.record("s2", v(&["b"]));
        s.take_dirty();
        s.resolved("s1");
        s.mark_dirty_if_more();
        assert!(s.needs_attention());
        assert_eq!(s.next_to_show().map(|(c, _)| c), Some("s2"));
    }

    #[test]
    fn no_attention_is_needed_when_nothing_changed() {
        let mut s = ConflictStore::default();
        assert!(!s.needs_attention());
        s.record("s", v(&["a"]));
        s.take_dirty();
        assert!(!s.needs_attention(), "bayrak temizlendi: tekrar öne getirme");
    }

    #[test]
    fn the_preview_lists_a_few_files_and_counts_the_rest() {
        let files = v(&["a", "b", "c", "d", "e", "f"]);
        assert_eq!(list_preview(&files, 4), ("a\nb\nc\nd".to_string(), 2));
        assert_eq!(list_preview(&files[..2], 4), ("a\nb".to_string(), 0));
    }
}
