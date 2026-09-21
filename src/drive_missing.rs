// Drive'daki sync klasörü silinmişse (başka bir bilgisayardan ya da çöp kutusuna atılarak)
use slint::ComponentHandle;
// ham "404 File not found" hatası yerine kullanıcıya iki yol sunulur:
//
// - **Yeniden yükle:** Aynı anahtarla Drive'da yeni klasör açılır, bu bilgisayardaki dosyalar
//   yeniden yüklenir. Klasör adı anahtardan türediği için başka bir bilgisayar önce
//   davranıp aynı klasörü yeniden yarattıysa ona KATILINIR (ikinci bir kopya oluşmaz).
//   Sync kodundaki klasör kimliği değiştiği için kod da değişir.
// - **Sync'i sil:** Yalnızca bu bilgisayardaki eşitleme kaldırılır; yerel dosyalara dokunulmaz.
//
// Durum (`MissingStore`) `AppState`'te tutulur: sync döngüsü pencereyi bilmez, tray zamanlayıcısı
// (`sync_window`) pencere varsa popup'ı açar. Kullanıcı Esc ile kapatırsa satırda "Drive'daki veri
// silinmiş" durumu kalır; manuel eşitleme popup'ı yeniden açar.

use crate::sync_core::auth;
use crate::sync_core::config::AppConfig;
use crate::sync_core::drive::DriveClient;
use crate::{i18n, AppState, MainWindow};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingSync {
    /// Config'teki sync kimliği (= sync kodu: `cs-{klasörId}-{hexAnahtar}`).
    pub code: String,
    pub name: String,
}

#[derive(Default)]
pub struct MissingStore {
    pub pending: Option<MissingSync>,
    /// Popup'ın (yeniden) gösterilmesi gerekiyor; tray zamanlayıcısı pencereye yansıtıp temizler.
    pub dirty: bool,
}

// ---------------------------------------------------------------------------
// Saf yardımcılar
// ---------------------------------------------------------------------------

/// `cs-{klasörId}-{hexAnahtar}` → (klasörId, hexAnahtar). Drive kimlikleri '-' içerebildiği için
/// SON '-' ayırıcıdır. Eski biçimdeki (öneksiz) kodlar için `None`.
pub fn parse_code(code: &str) -> Option<(&str, &str)> {
    code.strip_prefix("cs-")?.rsplit_once('-').filter(|(f, k)| !f.is_empty() && !k.is_empty())
}

pub fn new_code(folder_id: &str, hex_key: &str) -> String {
    format!("cs-{folder_id}-{hex_key}")
}

/// Drive'daki klasör adı: anahtarın ilk 8 karakteri. Aynı anahtar aynı adı verir; bu yüzden iki
/// bilgisayar aynı silinmiş sync'i yeniden yüklemeye kalkarsa aynı klasörde buluşur.
pub fn drive_folder_name(hex_key: &str) -> String {
    format!("ConnectSync_{}", &hex_key[..8.min(hex_key.len())])
}

fn fresh_key() -> Result<String, String> {
    let mut raw = [0u8; 16];
    getrandom::fill(&mut raw).map_err(|e| e.to_string())?;
    Ok(hex::encode(raw))
}

// ---------------------------------------------------------------------------
// Tespit
// ---------------------------------------------------------------------------

/// Sync döngüsü klasörün silindiğini fark edince çağırır: satır durumunu günceller ve popup'ı ister.
pub fn report(
    ui_weak: &slint::Weak<MainWindow>,
    app_state: &Arc<Mutex<AppState>>,
    code: &str,
) {
    let name = AppConfig::load()
        .sync_folders
        .iter()
        .find(|f| f.id == code)
        .map(|f| f.name.clone())
        .unwrap_or_default();
    {
        let mut st = app_state.lock().unwrap();
        st.missing.pending = Some(MissingSync { code: code.to_string(), name });
        st.missing.dirty = true;
    }
    crate::update_status(ui_weak, app_state, code, &i18n::t("status_drive_missing"), false);
}

/// Tray zamanlayıcısından (UI thread'i): bekleyen bir bildirim varsa popup'ı açar.
pub fn sync_window(ui: &MainWindow, app_state: &Arc<Mutex<AppState>>) {
    let pending = {
        let mut st = app_state.lock().unwrap();
        if !st.missing.dirty {
            return;
        }
        st.missing.dirty = false;
        st.missing.pending.clone()
    };
    let Some(m) = pending else { return };
    ui.set_missing_sync_id(m.code.as_str().into());
    ui.set_missing_sync_body(i18n::tf("missing_body", &[("name", m.name.as_str())]).into());
    ui.set_show_missing_dialog(true);
}

// ---------------------------------------------------------------------------
// Eylemler
// ---------------------------------------------------------------------------

/// "Sync'i sil": yalnızca bu bilgisayardaki eşitleme kalkar, yerel dosyalar yerinde durur.
/// (UI thread'inden çağrılır.)
pub fn remove(ui: &MainWindow, app_state: &Arc<Mutex<AppState>>, code: &str) {
    crate::remove_local_sync(&ui.as_weak(), app_state, code);
    app_state.lock().unwrap().missing.pending = None;
    if ui.get_active_sync_code() == code {
        ui.set_active_sync_code("".into());
        ui.set_is_syncing(false);
    }
    // Buluttaki anahtar kaydı artık ölü bir klasörü gösteriyor: en iyi çabayla temizle.
    let code = code.to_string();
    tokio::spawn(async move {
        let Some((folder_id, _)) = parse_code(&code) else { return };
        if let Ok(token) = auth::get_drive_token(false).await
            && let Ok(drive) = DriveClient::new(token, None)
        {
            let _ = drive.remove_sync_key(folder_id).await;
        }
    });
}

/// "Yeniden yükle": Drive'da yeni klasör aç, config'i yeni koda taşı, sync'i başlat.
/// Hata olursa popup ile bildirilir; satır "Drive'daki veri silinmiş" durumunda kalır.
pub fn reupload(
    ui_weak: slint::Weak<MainWindow>,
    app_state: Arc<Mutex<AppState>>,
    code: String,
) {
    app_state.lock().unwrap().missing.pending = None;
    crate::update_status(&ui_weak, &app_state, &code, &i18n::t("status_checking"), true);
    tokio::spawn(async move {
        match recreate_on_drive(&code).await {
            Ok((new_code, path)) => finish_reupload(ui_weak, app_state, code, new_code, path),
            Err(e) => {
                let msg = i18n::tf("err_reupload", &[("e", e.as_str())]);
                crate::update_status(&ui_weak, &app_state, &code, &i18n::t("status_drive_missing"), false);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        crate::show_error(&ui, msg.as_str().into());
                    }
                });
            }
        }
    });
}

/// Yeni (ya da başka bir bilgisayarın açtığı aynı adlı) Drive klasörünü hazırlar ve config'i
/// günceller. Döner: (yeni sync kodu, yerel klasör yolu).
async fn recreate_on_drive(code: &str) -> Result<(String, PathBuf), String> {
    let folder = AppConfig::load()
        .sync_folders
        .into_iter()
        .find(|f| f.id == code)
        .ok_or_else(|| "sync bu bilgisayarda kayıtlı değil".to_string())?;

    // Anahtar korunur: yeni klasör aynı sırrı kullanır. Eski biçimli kodda anahtar yoksa yenisi üretilir.
    let old_folder_id = parse_code(code).map(|(f, _)| f.to_string());
    let hex_key = match parse_code(code) {
        Some((_, k)) => k.to_string(),
        None => fresh_key()?,
    };

    let token = auth::get_drive_token(false)
        .await
        .map_err(|e| i18n::tf("err_drive_login_needed", &[("e", &e.to_string())]))?;
    let drive = DriveClient::new(token, None).map_err(|e| e.to_string())?;

    let folder_id = drive
        .get_or_create_folder(&drive_folder_name(&hex_key), None, true)
        .await
        .map_err(|e| i18n::tf("err_create_drive_folder", &[("e", &e.to_string())]))?;
    let _ = drive.save_sync_key(&folder_id, &hex_key).await;
    let _ = drive.set_folder_display_name(&folder_id, &folder.name).await;
    if let Some(old) = old_folder_id.filter(|old| *old != folder_id) {
        let _ = drive.remove_sync_key(&old).await;
    }

    let new_code = new_code(&folder_id, &hex_key);
    let mut config = AppConfig::load();
    if let Some(f) = config.sync_folders.iter_mut().find(|f| f.id == code) {
        f.id = new_code.clone();
        f.code = new_code.clone();
    }
    config.save()?;
    Ok((new_code, PathBuf::from(folder.path)))
}

fn finish_reupload(
    ui_weak: slint::Weak<MainWindow>,
    app_state: Arc<Mutex<AppState>>,
    old_code: String,
    new_code: String,
    path: PathBuf,
) {
    {
        // Eski koda ait ölü görev/durum kayıtlarını temizle
        let mut st = app_state.lock().unwrap();
        if let Some(tx) = st.tasks.remove(&old_code) {
            let _ = tx.send(());
        }
        st.triggers.remove(&old_code);
        st.folder_states.remove(&old_code);
    }
    let ui_for_update = ui_weak.clone();
    let app_state_for_update = app_state.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_for_update.upgrade() {
            if ui.get_active_sync_code() == old_code {
                ui.set_active_sync_code(new_code.as_str().into());
            }
            crate::update_ui_folders(&ui, &app_state_for_update);
        }
    });
    // Döngü yeni kodla başlar: tuz üretilir, ardından tüm yerel dosyalar yüklenir.
    // (Not: `new_code` yukarıdaki kapanışa taşındığı için yolun kendisinden yeniden okunur.)
    let code_for_loop = AppConfig::load()
        .sync_folders
        .into_iter()
        .find(|f| PathBuf::from(&f.path) == path)
        .map(|f| f.id);
    if let Some(code) = code_for_loop {
        crate::start_sync_loop(ui_weak, app_state, code, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_new_style_codes_using_the_last_dash() {
        assert_eq!(parse_code("cs-1wtJ-BcHz-OLuZ-deadbeef"), Some(("1wtJ-BcHz-OLuZ", "deadbeef")));
        assert_eq!(parse_code("cs-abc-key"), Some(("abc", "key")));
    }

    #[test]
    fn legacy_or_malformed_codes_have_no_folder_id() {
        for bad in ["deadbeefdeadbeef", "cs-", "cs-onlyone", "cs--key", "cs-abc-"] {
            assert_eq!(parse_code(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn new_code_round_trips_through_parse() {
        let code = new_code("1a-B_c", "0123456789abcdef");
        assert_eq!(parse_code(&code), Some(("1a-B_c", "0123456789abcdef")));
    }

    #[test]
    fn folder_name_is_derived_from_the_key_so_two_computers_meet_in_one_folder() {
        assert_eq!(drive_folder_name("938891ca00112233"), "ConnectSync_938891ca");
        assert_eq!(drive_folder_name("abc"), "ConnectSync_abc"); // 8'den kısa anahtar taşmaz
        assert_eq!(drive_folder_name("938891ca00112233"), drive_folder_name("938891ca00112233"));
    }

    #[test]
    fn fresh_keys_are_32_hex_chars_and_unique() {
        let a = fresh_key().unwrap();
        let b = fresh_key().unwrap();
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
