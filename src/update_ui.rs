//! Güncelleme akışının arayüz tarafı.
//!
//! Durum (`UpdatePhase`) pencerede değil `AppState` içinde tutulur: uygulama tepsideyken pencere
//! kapalı olabilir, indirme yine de sürer ve pencere sonradan açılınca doğru ekranı göstermelidir.
//! Arka plan görevleri yalnızca `AppState`'i günceller; tray zamanlayıcısı `sync_window` ile
//! (pencere varsa) Slint özelliklerine yansıtır — buluttaki klasör taramasıyla aynı desen.

use crate::sync_core::progress::format_bytes_pair;
use crate::updater::{self, DownloadProgress, InstallOutcome, Release, UpdateError};
use crate::{i18n, AppState, MainWindow, UpdateState};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdatePhase {
    #[default]
    Idle,
    /// GitHub'a soruluyor.
    Checking,
    /// Yeni sürüm var, kullanıcı onayı bekleniyor.
    Available(Release),
    /// İndiriliyor / doğrulanıyor / yerine konuyor. Kapatılamaz.
    Downloading(Release),
    /// Yeni ikili yerinde; yeniden başlatma bekleniyor. Kapatılamaz.
    Ready { exe: PathBuf, dev_build: bool },
    Failed(String),
    /// Zaten en son sürümdesin.
    Latest,
}

#[derive(Default)]
pub struct UpdateStore {
    pub phase: UpdatePhase,
    /// Durum değişti; tray zamanlayıcısı pencereye yansıtıp temizler.
    pub dirty: bool,
    pub progress: Arc<DownloadProgress>,
}

impl UpdateStore {
    fn set(&mut self, phase: UpdatePhase) {
        self.phase = phase;
        self.dirty = true;
    }
}

// ---------------------------------------------------------------------------
// Saf durum geçişleri
// ---------------------------------------------------------------------------

/// Yeni bir denetim başlatılabilir mi? İndirme/yeniden başlatma sürerken hayır.
pub fn can_start_check(phase: &UpdatePhase) -> bool {
    !matches!(phase, UpdatePhase::Checking | UpdatePhase::Downloading(_) | UpdatePhase::Ready { .. })
}

/// İndirilecek sürüm: yalnızca "mevcut" durumundayken.
pub fn download_target(phase: &UpdatePhase) -> Option<Release> {
    match phase {
        UpdatePhase::Available(r) => Some(r.clone()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Görünüm modeli
// ---------------------------------------------------------------------------

pub struct UpdateView {
    pub state: UpdateState,
    pub title: String,
    pub body: String,
    pub fraction: f32,
    pub progress_text: String,
    /// İndirme/yeniden başlatma sırasında popup kapatılamaz; pencere yeniden açılsa bile görünür kalır.
    pub force_open: bool,
}

impl UpdateView {
    fn new(state: UpdateState, title: String, body: String) -> Self {
        Self { state, title, body, fraction: 0.0, progress_text: String::new(), force_open: false }
    }
}

/// `downloaded`/`total`: indirme sayaçları (yalnızca `Downloading`'de kullanılır).
pub fn update_view(phase: &UpdatePhase, downloaded: u64, total: u64) -> UpdateView {
    let current = updater::CURRENT_VERSION;
    match phase {
        UpdatePhase::Idle => UpdateView::new(UpdateState::Idle, String::new(), String::new()),
        UpdatePhase::Checking => {
            UpdateView::new(UpdateState::Checking, i18n::t("update_checking"), String::new())
        }
        UpdatePhase::Available(r) => UpdateView::new(
            UpdateState::Available,
            i18n::t("update_available_title"),
            i18n::tf(
                "update_available_body",
                &[("v", updater::format_version(r.version).as_str()), ("cur", current)],
            ),
        ),
        UpdatePhase::Downloading(_) => {
            let fraction = if total == 0 { 0.0 } else { (downloaded as f32 / total as f32).clamp(0.0, 1.0) };
            let pct = (fraction * 100.0).round() as u32;
            UpdateView {
                fraction,
                progress_text: format!("{} · %{pct}", format_bytes_pair(downloaded, total)),
                force_open: true,
                ..UpdateView::new(UpdateState::Downloading, i18n::t("update_downloading_title"), String::new())
            }
        }
        UpdatePhase::Ready { dev_build, .. } => {
            let mut body = i18n::t("update_ready_body");
            if *dev_build {
                body.push_str("\n\n");
                body.push_str(&i18n::t("update_dev_note"));
            }
            UpdateView {
                force_open: true,
                ..UpdateView::new(UpdateState::Ready, i18n::t("update_ready_title"), body)
            }
        }
        UpdatePhase::Failed(e) => {
            UpdateView::new(UpdateState::Failed, i18n::t("update_failed_title"), e.clone())
        }
        UpdatePhase::Latest => UpdateView::new(
            UpdateState::Latest,
            i18n::t("update_latest_title"),
            i18n::tf("update_latest_body", &[("cur", current)]),
        ),
    }
}

// ---------------------------------------------------------------------------
// Arka plan görevleri (yalnızca AppState'i günceller)
// ---------------------------------------------------------------------------

/// "Güncellemeleri Denetle": GitHub'a sorar; sonuç `Available` / `Latest` / `Failed` olur.
pub fn start_check(app_state: &Arc<Mutex<AppState>>) {
    {
        let mut st = app_state.lock().unwrap();
        if !can_start_check(&st.update.phase) {
            return;
        }
        st.update.set(UpdatePhase::Checking);
    }
    let app_state = app_state.clone();
    tokio::spawn(async move {
        let phase = match updater::check_latest().await {
            Ok(Some(release)) => UpdatePhase::Available(release),
            Ok(None) => UpdatePhase::Latest,
            Err(e) => UpdatePhase::Failed(e.to_string()),
        };
        let mut st = app_state.lock().unwrap();
        // Bu arada durum değiştiyse (ör. yeni bir denetim/indirme) eski sonucu üstüne yazma.
        if st.update.phase == UpdatePhase::Checking {
            st.update.set(phase);
        }
    });
}

/// "Güncelle": indir → SHA-256 doğrula → yerine koy. Bitince `Ready`, hata olursa `Failed`.
pub fn start_download(app_state: &Arc<Mutex<AppState>>) {
    let (release, progress) = {
        let mut st = app_state.lock().unwrap();
        let Some(release) = download_target(&st.update.phase) else { return };
        st.update.progress = Arc::new(DownloadProgress::default());
        st.update.set(UpdatePhase::Downloading(release.clone()));
        (release, st.update.progress.clone())
    };
    let app_state = app_state.clone();
    tokio::spawn(async move {
        let phase = match download_and_install(&release, &progress).await {
            Ok((exe, dev_build)) => UpdatePhase::Ready { exe, dev_build },
            Err(e) => UpdatePhase::Failed(e.to_string()),
        };
        app_state.lock().unwrap().update.set(phase);
    });
}

async fn download_and_install(
    release: &Release,
    progress: &DownloadProgress,
) -> Result<(PathBuf, bool), UpdateError> {
    // Yol, Linux'ta "(deleted)" eki gelmeden ÖNCE alınmalı.
    let exe = updater::current_exe_path()?;
    let staged = updater::staging_path(&exe);
    updater::download_verified(release, &staged, progress).await?;
    let outcome = updater::install(&staged, &exe)?;
    Ok((exe, outcome == InstallOutcome::SkippedDevBuild))
}

/// Yeni ikiliyi başlatır. `true` dönerse çağıran hemen çıkmalıdır; `false` ise hata `Failed`
/// olarak popup'ta gösterilir.
pub fn restart(app_state: &Arc<Mutex<AppState>>) -> bool {
    let mut st = app_state.lock().unwrap();
    let UpdatePhase::Ready { exe, .. } = &st.update.phase else { return false };
    let exe = exe.clone();
    match updater::restart(&exe) {
        Ok(()) => true,
        Err(e) => {
            st.update.set(UpdatePhase::Failed(e.to_string()));
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Pencereye yansıtma
// ---------------------------------------------------------------------------

/// Tray zamanlayıcısından (UI thread'i) çağrılır. Durum değiştiyse ya da indirme sürüyorsa
/// (çubuk ilerlesin) Slint özelliklerini günceller.
pub fn sync_window(ui: &MainWindow, app_state: &Arc<Mutex<AppState>>) {
    let view = {
        let mut st = app_state.lock().unwrap();
        let downloading = matches!(st.update.phase, UpdatePhase::Downloading(_));
        if !st.update.dirty && !downloading {
            return;
        }
        st.update.dirty = false;
        let (done, total) = st.update.progress.snapshot();
        update_view(&st.update.phase, done, total)
    };
    ui.set_update_state(view.state);
    ui.set_update_title(view.title.as_str().into());
    ui.set_update_body(view.body.as_str().into());
    ui.set_update_fraction(view.fraction);
    ui.set_update_progress_text(view.progress_text.as_str().into());
    if view.force_open {
        ui.set_show_update_dialog(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> Release {
        Release {
            tag: "v1.0.42".into(),
            version: (1, 0, 42),
            asset_name: "ConnectSync-Linux".into(),
            asset_url: "https://x/a".into(),
            sha256_url: Some("https://x/a.sha256".into()),
            size: 100,
        }
    }

    fn ready(dev_build: bool) -> UpdatePhase {
        UpdatePhase::Ready { exe: PathBuf::from("/bin/x"), dev_build }
    }

    #[test]
    fn a_new_check_is_blocked_while_busy_but_allowed_after_results() {
        assert!(can_start_check(&UpdatePhase::Idle));
        assert!(can_start_check(&UpdatePhase::Available(release())));
        assert!(can_start_check(&UpdatePhase::Failed("x".into())));
        assert!(can_start_check(&UpdatePhase::Latest));
        assert!(!can_start_check(&UpdatePhase::Checking));
        assert!(!can_start_check(&UpdatePhase::Downloading(release())));
        assert!(!can_start_check(&ready(false)));
    }

    #[test]
    fn only_an_available_update_can_be_downloaded() {
        assert_eq!(download_target(&UpdatePhase::Available(release())), Some(release()));
        for p in [UpdatePhase::Idle, UpdatePhase::Checking, UpdatePhase::Downloading(release()), ready(false), UpdatePhase::Latest] {
            assert_eq!(download_target(&p), None, "{p:?}");
        }
    }

    #[test]
    fn only_downloading_and_ready_force_the_dialog_open() {
        let cases = [
            (UpdatePhase::Idle, false),
            (UpdatePhase::Checking, false),
            (UpdatePhase::Available(release()), false),
            (UpdatePhase::Downloading(release()), true),
            (ready(false), true),
            (UpdatePhase::Failed("x".into()), false),
            (UpdatePhase::Latest, false),
        ];
        for (phase, forced) in cases {
            assert_eq!(update_view(&phase, 0, 0).force_open, forced, "{phase:?}");
        }
    }

    #[test]
    fn available_view_names_the_new_and_current_versions() {
        let v = update_view(&UpdatePhase::Available(release()), 0, 0);
        assert_eq!(v.state, UpdateState::Available);
        assert!(v.body.contains("1.0.42"), "{}", v.body);
        assert!(v.body.contains(updater::CURRENT_VERSION), "{}", v.body);
        assert!(!v.body.contains('{'), "doldurulmamış yer tutucu: {}", v.body);
    }

    #[test]
    fn downloading_view_reports_fraction_and_percent() {
        let v = update_view(&UpdatePhase::Downloading(release()), 50, 200);
        assert_eq!(v.state, UpdateState::Downloading);
        assert!((v.fraction - 0.25).abs() < f32::EPSILON);
        assert!(v.progress_text.contains("%25"), "{}", v.progress_text);
    }

    #[test]
    fn downloading_view_survives_unknown_or_overshooting_totals() {
        let unknown = update_view(&UpdatePhase::Downloading(release()), 10, 0);
        assert_eq!(unknown.fraction, 0.0);
        let over = update_view(&UpdatePhase::Downloading(release()), 300, 100);
        assert_eq!(over.fraction, 1.0);
    }

    #[test]
    fn ready_view_warns_only_for_development_builds() {
        let release_build = update_view(&ready(false), 0, 0);
        let dev_build = update_view(&ready(true), 0, 0);
        assert_eq!(release_build.state, UpdateState::Ready);
        assert!(dev_build.body.len() > release_build.body.len());
        assert!(dev_build.body.starts_with(&release_build.body));
    }

    #[test]
    fn failed_view_shows_the_error_verbatim() {
        let v = update_view(&UpdatePhase::Failed("GitHub'a ulaşılamadı: x".into()), 0, 0);
        assert_eq!(v.state, UpdateState::Failed);
        assert_eq!(v.body, "GitHub'a ulaşılamadı: x");
    }

    #[test]
    fn latest_view_names_the_current_version() {
        let v = update_view(&UpdatePhase::Latest, 0, 0);
        assert_eq!(v.state, UpdateState::Latest);
        assert!(v.body.contains(updater::CURRENT_VERSION), "{}", v.body);
    }
}
