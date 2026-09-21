#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use auto_launch::AutoLaunchBuilder;

fn configure_autostart(enable: bool, start_in_tray: bool) {
    let app_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("connectsync"));
    let app_path_str = app_path.to_str().unwrap();
    
    let args: &[&str] = if start_in_tray { &["--autostart", "--tray"] } else { &["--autostart"] };
    
    let auto = match AutoLaunchBuilder::new()
        .set_app_name("ConnectSync")
        .set_app_path(app_path_str)
        .set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent)
        .set_args(args)
        .build() {
            Ok(a) => a,
            Err(_) => return,
        };

    if enable {
        let _ = auto.enable();
    } else {
        let _ = auto.disable();
    }
}

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rfd::FileDialog;
use tray_icon::{TrayIconBuilder, menu::{Menu, MenuItem}};
use std::rc::Rc;
use std::cell::RefCell;

slint::include_modules!();
mod sync_core;
mod i18n;
mod drive_missing;
mod update_ui;
mod updater;

/// Hata çubuğunu gösterir ve 8 sn sonra (hâlâ aynı mesajsa) kendiliğinden kapatır.
fn show_error(ui: &MainWindow, msg: slint::SharedString) {
    if msg.is_empty() {
        ui.set_error_text(msg);
        return;
    }
    // Popup kapatıldıysa hiç gösterme; hata yine de satırda (folder.status) görünür kalır.
    if !sync_core::config::AppConfig::load().show_error_popups {
        return;
    }
    ui.set_error_text(msg.clone());
}

/// Çeviriyi doğrudan Slint string'ine çevirir.
fn tr_ss(key: &str) -> slint::SharedString {
    i18n::t(key).into()
}

/// Seçili dile göre tüm arayüz metinlerini günceller.
fn apply_language(ui: &MainWindow, lang: &str) {
    i18n::set_language(lang);
    ui.set_setting_language(i18n::current().as_str().into());
    let tr = ui.global::<Tr>();
    macro_rules! set_all {
        ($($setter:ident => $key:literal),* $(,)?) => { $( tr.$setter(tr_ss($key)); )* };
    }
    set_all!(
        set_app_subtitle => "app_subtitle",
        set_missing_title => "missing_title",
        set_missing_upload => "missing_upload",
        set_missing_remove => "missing_remove",
        set_edit_sync_title => "edit_sync_title",
        set_edit_name => "edit_name",
        set_new_drive_folder_name => "new_drive_folder_name",
        set_edit_path => "edit_path",
        set_edit_change_path => "edit_change_path",
        set_edit_rename_drive => "edit_rename_drive",
        set_edit_save => "edit_save",
        set_edit_sync_btn => "edit_sync_btn",
        set_create_new_sync_title => "create_new_sync_title",
        set_can_others_write => "can_others_write",
        set_settings => "settings",
        set_my_syncs => "my_syncs",
        set_find_from_cloud => "find_from_cloud",
        set_logout => "logout",
        set_check_updates => "check_updates",
        set_sync_code_title => "sync_code_title",
        set_cancel => "cancel",
        set_connect => "connect",
        set_login_google => "login_google",
        set_retry => "retry",
        set_new_sync => "new_sync",
        set_connect_to_code => "connect_to_code",
        set_copy_code => "copy_code",
        set_copied => "copied",
        set_main_menu => "main_menu",
        set_stop_and_delete => "stop_and_delete",
        set_no_syncs_title => "no_syncs_title",
        set_no_syncs_hint => "no_syncs_hint",
        set_status_syncing => "status_syncing",
        set_status_connected => "status_connected",
        set_status_connecting => "status_connecting",
        set_status_waiting_login => "status_waiting_login",
        set_quit_title => "quit_title",
        set_quit_body => "quit_body",
        set_quit => "quit",
        set_section_startup => "section_startup",
        set_autostart => "autostart",
        set_start_in_tray => "start_in_tray",
        set_section_performance => "section_performance",
        set_threads_label => "threads_label",
        set_threads_hint => "threads_hint",
        set_language_label => "language_label",
        set_section_system => "section_system",
        set_remove_sync_title => "remove_sync_title",
        set_remove_sync_body => "remove_sync_body",
        set_delete_from_drive => "delete_from_drive",
        set_remove => "remove",
        set_update_ready_title => "update_ready_title",
        set_update_ready_body => "update_ready_body",
        set_later => "later",
        set_restart => "restart",
        set_section_this_pc => "section_this_pc",
        set_section_on_drive => "section_on_drive",
        set_add_from_drive => "add_from_drive",
        set_not_added_hint => "not_added_hint",
        set_section_advanced => "section_advanced",
        set_watch_changes_label => "watch_changes_label",
        set_watch_changes_hint => "watch_changes_hint",
        set_auto_gc_label => "auto_gc_label",
        set_error_popups_label => "error_popups_label",
        set_network_popups_label => "network_popups_label",
        set_check_interval_label => "check_interval_label",
        set_dark_theme_label => "dark_theme_label",
        set_update_now => "update_now",
        set_ok => "ok",
    );
}

// ---------------------------------------------------------------------------
// Tray ikonunu PNG'den yükler
// ---------------------------------------------------------------------------
fn load_icon(bytes: &[u8]) -> tray_icon::Icon {
    let image = image::load_from_memory(bytes)
        .expect("Logo decode edilemedi")
        .into_rgba8();
    let (w, h) = image.dimensions();
    tray_icon::Icon::from_rgba(image.into_raw(), w, h).expect("Icon oluşturulamadı")
}

// ---------------------------------------------------------------------------
// Paylaşılan uygulama durumu — UI thread'den değil Arc<Mutex> ile erişilir
// ---------------------------------------------------------------------------
#[derive(Default, Clone)]
pub struct FolderState {
    pub status: String,
    pub is_syncing: bool,
    pub error: String,
    /// Aktif sync döngüsü çalışıyorsa onun ilerleme/ağ durumu (MB/GB çubuğu, bağlantı kopukluğu).
    pub progress: Option<Arc<sync_core::progress::Progress>>,
}

#[derive(Default)]
pub struct AppState {
    pub tasks: std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>,
    /// Çalışan sync döngüsünü beklemeden uyandırmak için (manuel eşitle butonu)
    pub triggers: std::collections::HashMap<String, Arc<tokio::sync::Notify>>,
    pub folder_states: std::collections::HashMap<String, FolderState>,
    /// Drive'da bulunan (kod, ad) listesi; bu PC'de olmayanlar "Drive'da" bölümünde gösterilir
    pub cloud_items: Vec<(String, String)>,
    pub missing: drive_missing::MissingStore,
    /// Arka plan taraması `cloud_items`'ı değiştirdi; tray zamanlayıcısı arayüzü yenileyip
    /// bayrağı temizler (pencere yoksa bir sonraki `setup_ui` zaten güncel listeyi çeker).
    pub cloud_dirty: bool,
    /// Güncelleme popup'ının durumu (pencere kapalıyken de indirme sürer): bkz. `update_ui`.
    pub update: update_ui::UpdateStore,
}

// ---------------------------------------------------------------------------
// UI kurulumu ve tüm callback bağlantıları
// ---------------------------------------------------------------------------
/// "cs-{driveFolderId}-{hexKey}" kodundan Drive klasör ID'sini çıkarır.
/// Drive ID'leri '-' içerebildiği için baştaki "cs-" atılır, SON '-' ayırıcı sayılır.
fn drive_folder_id(code: &str) -> &str {
    code.strip_prefix("cs-")
        .and_then(|r| r.rsplit_once('-'))
        .map(|(f, _)| f)
        .unwrap_or(code)
}

/// Drive'da olup bu bilgisayara henüz eklenmemiş (kod, ad) çiftleri.
fn cloud_only(config: &crate::sync_core::config::AppConfig, state: &AppState) -> Vec<(String, String)> {
    let local: std::collections::HashSet<String> = config
        .sync_folders
        .iter()
        .map(|f| drive_folder_id(if f.code.is_empty() { &f.id } else { &f.code }).to_string())
        .collect();
    state
        .cloud_items
        .iter()
        .filter(|(code, _)| !local.contains(drive_folder_id(code)))
        .cloned()
        .collect()
}

pub fn update_ui_folders(ui: &crate::MainWindow, app_state: &std::sync::Arc<std::sync::Mutex<crate::AppState>>) {
    let config = crate::sync_core::config::AppConfig::load();
    let model = std::rc::Rc::new(slint::VecModel::<crate::SyncFolderItem>::default());
    let cloud_model = std::rc::Rc::new(slint::VecModel::<crate::SyncFolderItem>::default());
    let state = app_state.lock().unwrap();
    for f in &config.sync_folders {
        let default_state = crate::FolderState::default();
        let fs = state.folder_states.get(&f.id).unwrap_or(&default_state);

        model.push(crate::SyncFolderItem {
            id: f.id.clone().into(),
            name: f.name.clone().into(),
            path: f.path.clone().into(),
            code: f.code.clone().into(),
            status: if fs.error.is_empty() { fs.status.as_str().into() } else { fs.error.as_str().into() },
            is_syncing: fs.is_syncing,
        });
    }
    // Sadece Drive'da olanlar (bu PC'ye eklenmemiş)
    for (code, name) in cloud_only(&config, &state) {
        cloud_model.push(crate::SyncFolderItem {
            id: code.clone().into(),
            name: name.into(),
            path: "".into(),
            code: code.into(),
            status: "".into(),
            is_syncing: false,
        });
    }
    drop(state);
    ui.set_sync_folders(model.into());
    ui.set_cloud_folders(cloud_model.into());
}

/// Bir sync'i bu bilgisayardan kaldırır: döngüyü durdurur, config'ten siler, listeyi yeniler.
/// Drive'daki veriye dokunmaz (klasör "Drive'da" bölümüne geri düşer).
fn remove_local_sync(
    ui_weak: &slint::Weak<MainWindow>,
    app_state: &Arc<Mutex<AppState>>,
    id: &str,
) {
    {
        let mut st = app_state.lock().unwrap();
        if let Some(tx) = st.tasks.remove(id) {
            let _ = tx.send(());
        }
        st.triggers.remove(id);
        st.folder_states.remove(id);
    }
    let mut config = sync_core::config::AppConfig::load();
    config.sync_folders.retain(|f| f.id != id);
    let _ = config.save();
    if let Some(ui) = ui_weak.upgrade() {
        update_ui_folders(&ui, app_state);
    }
}

/// Drive'daki sync klasörünü ve anahtar kaydını siler.
async fn delete_drive_sync(folder_id: &str) -> Result<(), String> {
    if folder_id.is_empty() || folder_id == "pending" {
        return Ok(());
    }
    let token = sync_core::auth::get_drive_token(false)
        .await
        .map_err(|e| i18n::tf("err_drive_login_needed", &[("e", &e.to_string())]))?;
    let drive = sync_core::drive::DriveClient::new(token, None).map_err(|e| e.to_string())?;
    drive
        .delete_file(folder_id)
        .await
        .map_err(|e| i18n::tf("err_delete_drive", &[("e", &e.to_string())]))?;
    let _ = drive.remove_sync_key(folder_id).await;
    Ok(())
}

fn setup_ui(
    ui: &MainWindow,
    _ui_handle: Rc<RefCell<Option<MainWindow>>>,
    app_state: Arc<Mutex<AppState>>,
) {
    // ── Dil ──────────────────────────────────────────────────────────────
    let cfg = sync_core::config::AppConfig::load();
    apply_language(ui, &cfg.language);
    ui.set_status_text(tr_ss("ready"));

    // ── Ayarlar ekranının başlangıç değerleri (config'ten) ─────────────────
    ui.set_setting_autostart(cfg.auto_start_enabled);
    ui.set_setting_start_in_tray(cfg.start_in_tray);
    ui.set_setting_watch_changes(cfg.watch_local_changes);
    ui.set_setting_auto_gc(cfg.auto_gc);
    ui.set_setting_error_popups(cfg.show_error_popups);
    ui.set_setting_network_popups(cfg.show_network_popups);
    ui.set_setting_check_interval(
        cfg.check_interval_minutes()
            .to_string()
            .into(),
    );
    ui.set_setting_threads(sync_core::config::clamp_threads(cfg.concurrent_threads).to_string().into());
    ui.set_setting_dark_theme(cfg.is_dark_theme());
    // Pencere (yeniden) açılınca sürmekte olan güncelleme durumunu göstersin
    app_state.lock().unwrap().update.dirty = true;

    // ── Başlangıç durumu: keyring'den token var mı? ──────────────────────
    if sync_core::auth::is_token_cached() {
        ui.set_is_logged_in(true);
    } else {
        // Autostart senaryosu: session henüz tam açılmamış, keyring kilitli olabilir.
        // Etkileşimsiz modda sessizce dene; başarılıysa UI'ı güncelle.
        let ui_weak = ui.as_weak();
        tokio::spawn(async move {
            if sync_core::auth::get_drive_token(false).await.is_ok() {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_is_logged_in(true);
                    }
                });
            }
        });
    }

    // Kaydedilmiş sync durumunu yükle
    update_ui_folders(ui, &app_state);


    // ── Login ────────────────────────────────────────────────────────────
    let ui_weak = ui.as_weak();
    ui.on_login_requested(move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_is_logging_in(true);
            ui.set_error_text("".into());
        }
        let ui_weak2 = ui_weak.clone();
        tokio::spawn(async move {
            match sync_core::auth::get_drive_token(true).await {
                Ok(_) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak2.upgrade() {
                            ui.set_is_logged_in(true);
                            ui.set_is_logging_in(false);
                        }
                    });
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak2.upgrade() {
                            ui.set_is_logging_in(false);
                            show_error(&ui, msg.as_str().into());
                        }
                    });
                }
            }
        });
    });

    let ui_weak_clip = ui.as_weak();
    ui.on_copy_to_clipboard(move |code, id| {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(code.to_string());
            if let Some(ui) = ui_weak_clip.upgrade() {
                ui.set_copy_feedback_id(id.clone());
                let ui_weak_timer = ui_weak_clip.clone();
                let id_clone = id.to_string();
                slint::Timer::single_shot(std::time::Duration::from_secs(2), move || {
                    if let Some(ui) = ui_weak_timer.upgrade()
                        && ui.get_copy_feedback_id() == id_clone {
                            ui.set_copy_feedback_id("".into());
                        }
                });
            }
        }
    });

    ui.on_fully_quit_requested(move || {
        let _ = slint::quit_event_loop();
        std::process::exit(0);
    });
    
    let app_state_remove = app_state.clone();
    let ui_remove = ui.as_weak();
    let ui_weak_scan = ui.as_weak();
    let app_state_scan = app_state.clone();
    
    // Autostart callbacks
    ui.on_autostart_toggled(move |enabled| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.auto_start_enabled = enabled;
        let _ = cfg.save();
        crate::configure_autostart(enabled, cfg.start_in_tray);
    });

    ui.on_start_in_tray_toggled(move |enabled| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.start_in_tray = enabled;
        let _ = cfg.save();
        crate::configure_autostart(cfg.auto_start_enabled, enabled);
    });

    // ── Yeni ayarlar: değişiklik izleme, otomatik temizlik, hata/bağlantı popup'ları ──
    // Not: çalışan bir sync döngüsü zaten başlamışsa bu ayarlar bir sonraki
    // döngü turunda (veya sync yeniden başlatıldığında) etkili olur.
    ui.on_watch_changes_toggled(move |enabled| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.watch_local_changes = enabled;
        let _ = cfg.save();
    });

    ui.on_auto_gc_toggled(move |enabled| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.auto_gc = enabled;
        let _ = cfg.save();
    });

    ui.on_error_popups_toggled(move |enabled| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.show_error_popups = enabled;
        let _ = cfg.save();
    });

    ui.on_network_popups_toggled(move |enabled| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.show_network_popups = enabled;
        let _ = cfg.save();
    });

    let ui_weak_interval = ui.as_weak();
    ui.on_check_interval_changed(move |text| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        let parsed: u32 = text.trim().parse().unwrap_or(0);
        let clamped = parsed.clamp(1, 1440); // 1 dk - 24 saat arası mantıklı sınır
        cfg.sync_interval_minutes = clamped;
        let _ = cfg.save();
        if let Some(ui) = ui_weak_interval.upgrade() {
            ui.set_setting_check_interval(clamped.to_string().into());
        }
    });

    // ── Güncelleme: denetle / güncelle / yeniden başlat ──────────────────────
    // Durum değişince pencereye HEMEN yansıt: bir sonraki tray tikini (≤100 ms) beklerse popup
    // kısa süre önceki oturumun eski durumunu (ör. "Güncelsin") gösterir.
    let app_state_check = app_state.clone();
    let ui_weak_check = ui.as_weak();
    ui.on_check_updates_requested(move || {
        update_ui::start_check(&app_state_check);
        if let Some(ui) = ui_weak_check.upgrade() {
            update_ui::sync_window(&ui, &app_state_check);
        }
    });
    let app_state_download = app_state.clone();
    let ui_weak_download = ui.as_weak();
    ui.on_update_now_requested(move || {
        update_ui::start_download(&app_state_download);
        if let Some(ui) = ui_weak_download.upgrade() {
            update_ui::sync_window(&ui, &app_state_download);
        }
    });
    let app_state_restart = app_state.clone();
    ui.on_restart_requested(move || {
        // Yeni ikili başlatıldıysa çık; başlatılamadıysa hata popup'ta gösterilir.
        if update_ui::restart(&app_state_restart) {
            let _ = slint::quit_event_loop();
            std::process::exit(0);
        }
    });

    // Tema anında arayüzde uygulanır (Theme.dark çift yönlü bağlı); burada yalnızca kalıcı kılınır.
    ui.on_theme_changed(move |dark| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.theme = crate::sync_core::config::theme_name(dark).to_string();
        let _ = cfg.save();
    });

    let ui_weak_threads = ui.as_weak();
    ui.on_threads_changed(move |text| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        // Boş/geçersiz girdi varsayılana döner; aralık dışı değer sınırlara çekilir.
        let parsed: u32 = text
            .trim()
            .parse()
            .unwrap_or(crate::sync_core::config::DEFAULT_THREADS);
        let clamped = crate::sync_core::config::clamp_threads(parsed);
        cfg.concurrent_threads = clamped;
        let _ = cfg.save();
        if let Some(ui) = ui_weak_threads.upgrade() {
            ui.set_setting_threads(clamped.to_string().into());
        }
    });

    let ui_weak_lang = ui.as_weak();
    ui.on_language_changed(move |lang| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.language = lang.to_string();
        let _ = cfg.save();
        if let Some(ui) = ui_weak_lang.upgrade() {
            apply_language(&ui, &lang);
            // Durum metni hazır/boşta ise yeni dilde göster
            if !ui.get_is_syncing() {
                ui.set_status_text(tr_ss("ready"));
            }
        }
    });

    ui.on_scan_cloud_syncs(move || {
        let ui_weak_bg = ui_weak_scan.clone();
        let app_state_bg = app_state_scan.clone();

        if let Some(ui) = ui_weak_bg.upgrade() {
            // Aktif bir sync varsa onun durum metnini ezme
            if !ui.get_is_syncing() {
                ui.set_status_text(tr_ss("scanning_drive"));
            }
        }

        tokio::spawn(async move {
            let result = scan_cloud_folders().await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_bg.upgrade() {
                    match result {
                        Ok(items) => {
                            app_state_bg.lock().unwrap().cloud_items = items;
                            update_ui_folders(&ui, &app_state_bg);
                            let n = {
                                let cfg = sync_core::config::AppConfig::load();
                                let st = app_state_bg.lock().unwrap();
                                cloud_only(&cfg, &st).len()
                            };
                            if !ui.get_is_syncing() {
                                if n > 0 {
                                    ui.set_status_text(i18n::tf("syncs_found", &[("n", &n.to_string())]).into());
                                } else {
                                    ui.set_status_text(tr_ss("no_new_syncs"));
                                }
                            }
                        }
                        Err(e) => {
                            update_ui_folders(&ui, &app_state_bg);
                            show_error(&ui, e.as_str().into());
                        }
                    }
                }
            });
        });
    });

    // ── Drive'daki bir sync'i bu bilgisayara ekle (klasörü kullanıcı seçer) ──
    let ui_weak_add = ui.as_weak();
    let _app_state_add = app_state.clone();
    ui.on_add_cloud_sync(move |code, name| {
        let code = code.to_string();
        let cloud_name = name.to_string();
        let ui_weak_add = ui_weak_add.clone();
        std::thread::spawn(move || {
            let Some(parent_path) = FileDialog::new()
                .set_title(i18n::t("pick_download_folder"))
                .pick_folder()
            else { return };
            
            // Seçilen klasörün içine buluttaki isimle yeni bir klasör ekle
            let path = parent_path.join(&cloud_name);
            

            let _ = slint::invoke_from_event_loop(move || {

        let config = sync_core::config::AppConfig::load();
        if config.sync_folders.iter().any(|f| drive_folder_id(&f.id) == drive_folder_id(&code)) {
            return;
        }
        if config.sync_folders.iter().any(|f| f.path == path.to_string_lossy()) {
            if let Some(ui) = ui_weak_add.upgrade() {
                show_error(&ui, tr_ss("err_folder_exists"));
            }
            return;
        }

        let _display_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_string())
            .unwrap_or(cloud_name.clone());

        // Kaydetme işlemini edit_sync_save'e bırakıyoruz
        if let Some(ui) = ui_weak_add.upgrade() {
            ui.invoke_show_edit_sync(code.clone().into(), cloud_name.clone().into(), path.to_string_lossy().to_string().into());
        }
            });
        });
    });

    // ── Silme dialogu onayı: bu PC'den kaldır (+ isteğe bağlı Drive'dan sil) ──
    let ui_weak_del = ui.as_weak();
    let app_state_del = app_state.clone();
    ui.on_confirm_delete_sync(move |id, delete_from_drive| {
        let id = id.to_string();
        remove_local_sync(&ui_weak_del, &app_state_del, &id);
        if let Some(ui) = ui_weak_del.upgrade() {
            if ui.get_active_sync_code().as_str() == id.as_str() {
                ui.set_active_sync_code("".into());
            }
        }

        if delete_from_drive {
            let fid = drive_folder_id(&id).to_string();
            let ui_bg = ui_weak_del.clone();
            let st_bg = app_state_del.clone();
            tokio::spawn(async move {
                let res = delete_drive_sync(&fid).await;
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_bg.upgrade() {
                        match res {
                            Ok(()) => {
                                st_bg.lock().unwrap().cloud_items.retain(|(c, _)| drive_folder_id(c) != fid);
                                update_ui_folders(&ui, &st_bg);
                            }
                            Err(e) => show_error(&ui, e.as_str().into()),
                        }
                    }
                });
            });
        }
    });

    let ui_weak_missing = ui.as_weak();
    let app_state_missing = app_state.clone();
    ui.on_missing_reupload(move |id| {
        drive_missing::reupload(ui_weak_missing.clone(), app_state_missing.clone(), id.to_string());
    });

    let ui_weak_missing_rm = ui.as_weak();
    let app_state_missing_rm = app_state.clone();
    ui.on_missing_remove(move |id| {
        if let Some(ui) = ui_weak_missing_rm.upgrade() {
            drive_missing::remove(&ui, &app_state_missing_rm, id.as_str());
        }
    });

    let ui_weak_missing_up = ui.as_weak();
    let app_state_missing_up = app_state.clone();
    ui.on_missing_upload(move |id| {
        drive_missing::reupload(
            ui_weak_missing_up.clone(),
            app_state_missing_up.clone(),
            id.to_string(),
        );
    });


    ui.on_remove_sync_folder(move |id| {
        remove_local_sync(&ui_remove, &app_state_remove, id.as_str());
    });

    // ── Edit Sync ──────────────────────────────────────────────────────────
    let ui_weak_edit_pick = ui.as_weak();
    ui.on_edit_pick_path(move |_id| {
        let ui_weak = ui_weak_edit_pick.clone();
        std::thread::spawn(move || {
            let Some(path) = rfd::FileDialog::new()
                .set_title(i18n::t("pick_sync_folder"))
                .pick_folder()
            else { return };
            
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_edit_sync_path(path.to_string_lossy().to_string().into());
                }
            });
        });
    });

    let ui_weak_edit_save = ui.as_weak();
    let app_state_edit = app_state.clone();
    ui.on_edit_sync_save(move |id, name, path, rename_on_drive| {
        let id_str = id.to_string();
        let name_str = name.to_string();
        let path_str = path.to_string();
        
        let mut config = sync_core::config::AppConfig::load();
        
        let mut old_path = String::new();
        let do_rename;
        let code;
        let mut is_new = false;
        
        if let Some(folder) = config.sync_folders.iter_mut().find(|f| f.id == id_str) {
            old_path = folder.path.clone();
            folder.name = name_str.clone();
            folder.path = path_str.clone();
            code = folder.code.clone();
            do_rename = rename_on_drive;
        } else {
            is_new = true;
            config.sync_folders.push(sync_core::config::SyncFolder {
                id: id_str.clone(),
                name: name_str.clone(),
                path: path_str.clone(),
                code: id_str.clone(),
            });
            code = id_str.clone();
            do_rename = rename_on_drive;
        }
        
        let _ = config.save();
        
        if is_new {
            if let Some(ui) = ui_weak_edit_save.upgrade() {
                ui.set_show_my_syncs(false);
                ui.set_active_sync_code(id_str.clone().as_str().into());
                ui.set_active_hidden(false);
                ui.set_active_sync_folder(name_str.as_str().into());
                ui.set_status_text(tr_ss("status_pulling"));
                ui.set_is_syncing(true);
            }
            start_sync_loop(ui_weak_edit_save.clone(), app_state_edit.clone(), id_str.clone(), std::path::PathBuf::from(path_str.clone()));
        }

        if old_path != "" || is_new {
            if do_rename {
                let name_clone = name_str.clone();
                tokio::spawn(async move {
                    if let Ok(token) = sync_core::auth::get_drive_token(false).await {
                        if let Ok(drive) = sync_core::drive::DriveClient::new(token, None) {
                            if let Some((fid, _)) = drive_missing::parse_code(&code) {
                                let _ = drive.set_folder_display_name(fid, &name_clone).await;
                            }
                        }
                    }
                });
            }
            
            if !is_new && old_path != path_str {
                // Yol değiştiyse eski döngüyü kapatıp yenisini başlat
                let app_state_clone = app_state_edit.clone();
                let id_clone = id_str.clone();
                
                let mut st = app_state_clone.lock().unwrap();
                if let Some(tx) = st.tasks.remove(&id_clone) {
                    let _ = tx.send(()); 
                }
                drop(st);
                
                start_sync_loop(ui_weak_edit_save.clone(), app_state_edit.clone(), id_str.clone(), std::path::PathBuf::from(path_str.clone()));
            }
        }
        
        if let Some(ui) = ui_weak_edit_save.upgrade() {
            update_ui_folders(&ui, &app_state_edit);
        }
    });

    let ui_weak_manual = ui.as_weak();
    let app_state_manual = app_state.clone();
    ui.on_manual_sync_requested(move |id| {
        let id = id.to_string();

        // Zaten eşitleniyorsa hiçbir şey yapma (buton arayüzde de pasif)
        {
            let st = app_state_manual.lock().unwrap();
            if st.folder_states.get(&id).map(|f| f.is_syncing).unwrap_or(false) {
                return;
            }
        }

        // Döngü çalışıyorsa sadece uyandır: anahtar türetme / salt indirme tekrarlanmaz
        let trigger = {
            let st = app_state_manual.lock().unwrap();
            match (st.tasks.get(&id), st.triggers.get(&id)) {
                (Some(tx), Some(t)) if !tx.is_closed() => Some(t.clone()),
                _ => None,
            }
        };

        if let Some(t) = trigger {
            // Anında geri bildirim: satır hemen "eşitleniyor" olsun, buton pasifleşsin
            update_status(&ui_weak_manual, &app_state_manual, &id, &i18n::t("status_checking"), true);
            t.notify_one();
            return;
        }

        // Döngü hiç başlamamış (ör. buluttan bulunan klasör) → başlat
        let config = sync_core::config::AppConfig::load();
        if let Some(f) = config.sync_folders.iter().find(|f| f.id == id) {
            let code = if f.code.is_empty() { f.id.clone() } else { f.code.clone() };
            update_status(&ui_weak_manual, &app_state_manual, &id, &i18n::t("status_checking"), true);
            start_sync_loop(
                ui_weak_manual.clone(),
                app_state_manual.clone(),
                code,
                PathBuf::from(&f.path),
            );
        }
    });

    ui.on_open_sync_folder(move |_id| {
        let config = sync_core::config::AppConfig::load();
        if let Some(f) = config.sync_folders.first() { let path = &f.path;
            let _ = webbrowser::open(path);
        }
    });

    // ── Pencere sürükleme (no-frame pencere) ──────────────────────────────
    let ui_weak_move = ui.as_weak();
    ui.on_move_window(move |dx, dy| {
        if let Some(ui) = ui_weak_move.upgrade() {
            let w = ui.window();
            let scale = w.scale_factor();
            let pos = w.position();
            w.set_position(slint::PhysicalPosition::new(
                pos.x + (dx * scale).round() as i32,
                pos.y + (dy * scale).round() as i32,
            ));
        }
    });

    // ── Logout ───────────────────────────────────────────────────────────
    let ui_weak = ui.as_weak();
    let app_state_logout = app_state.clone();
    ui.on_logout_requested(move || {
        // Varsa sync'i durdur
        let mut state = app_state_logout.lock().unwrap();
        for (_, tx) in state.tasks.drain() {
            let _ = tx.send(());
        }
        state.cloud_items.clear();
        drop(state);

        sync_core::auth::logout();
        if let Some(ui) = ui_weak.upgrade() {
            update_ui_folders(&ui, &app_state_logout);
        }

        if let Some(ui) = ui_weak.upgrade() {
            ui.set_is_logged_in(false);
            ui.set_active_sync_code("".into());
            ui.set_active_hidden(false);
            ui.set_active_sync_folder("".into());
            ui.set_status_text("".into());
            ui.set_error_text("".into());
            ui.set_is_syncing(false);
        }
    });

    // ── Yeni Sync Oluştur ─────────────────────────────────────────────────
    let ui_weak = ui.as_weak();
    ui.on_create_new_sync(move || {
        let ui_weak = ui_weak.clone();
        std::thread::spawn(move || {
            let Some(path) = rfd::FileDialog::new()
                .set_title(i18n::t("pick_sync_folder"))
                .pick_folder()
            else { return };
            
            let _ = slint::invoke_from_event_loop(move || {
                let folder_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(i18n::t("default_folder_name").as_str())
                    .to_string();

                let config = sync_core::config::AppConfig::load();
                if config.sync_folders.iter().any(|f| f.path == path.to_string_lossy()) {
                    if let Some(ui) = ui_weak.upgrade() {
                        show_error(&ui, tr_ss("err_folder_exists"));
                    }
                    return;
                }

                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_show_new_sync(folder_name.into(), path.to_string_lossy().to_string().into());
                }
            });
        });
    });

    // ── Yeni Sync Kaydet ──────────────────────────────────────────────────
    let ui_weak = ui.as_weak();
    let app_state_new = app_state.clone();
    ui.on_new_sync_save(move |name, path_str, can_others_write| {
        let ui_weak = ui_weak.clone();
        let app_state_bg = app_state_new.clone();
        let folder_name = name.to_string();
        let path = std::path::PathBuf::from(path_str.as_str());

        // Hata ayıklama vs
        let config = sync_core::config::AppConfig::load();
        if config.sync_folders.iter().any(|f| f.path == path.to_string_lossy()) {
            if let Some(ui) = ui_weak.upgrade() {
                show_error(&ui, tr_ss("err_folder_exists"));
            }
            return;
        }

        let mut raw = [0u8; 16];
        if getrandom::fill(&mut raw).is_err() {
            if let Some(ui) = ui_weak.upgrade() {
                show_error(&ui, tr_ss("err_code_gen"));
            }
            return;
        }
        let hex_key = hex::encode(raw);

        if let Some(ui) = ui_weak.upgrade() {
            ui.set_active_sync_code("pending".into()); // Geçici - kopyala butonu gizlenecek
            ui.set_active_hidden(false);
            ui.set_active_sync_folder(folder_name.as_str().into());
            ui.set_status_text(tr_ss("creating_drive_folder"));
            ui.set_is_syncing(true);
        }

        tokio::spawn(async move {
            match sync_core::auth::get_drive_token(true).await {
                Ok(token) => {
                    let drive = sync_core::drive::DriveClient::new(token, None).unwrap();
                    let drive_folder_name = format!("ConnectSync_{}", &hex_key[0..8.min(hex_key.len())]);
                    match drive.get_or_create_folder(&drive_folder_name, None, can_others_write).await {
                        Ok(folder_id) => {
                            // save keys
                            let _ = drive.save_sync_key(&folder_id, &hex_key).await;
                            let _ = drive.set_folder_display_name(&folder_id, &folder_name).await;

                            let universal_code = format!("cs-{}-{}", folder_id, hex_key);
                            
                            let mut config = sync_core::config::AppConfig::load();
                            config.sync_folders.push(sync_core::config::SyncFolder { 
                                id: universal_code.clone(), 
                                name: folder_name.clone(), 
                                path: path.to_string_lossy().to_string(), 
                                code: universal_code.clone() 
                            });
                            let _ = config.save();
                            
                            let code_for_ui = universal_code.clone();
                            let ui_w_update = ui_weak.clone();
                            let app_state_update = app_state_bg.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_w_update.upgrade() {
                                    update_ui_folders(&ui, &app_state_update);
                                    ui.set_active_sync_code(code_for_ui.as_str().into());
                                    ui.set_status_text(tr_ss("sync_ready"));
                                }
                            });

                            start_sync_loop(ui_weak, app_state_bg, universal_code, path);
                        },
                        Err(e) => {
                            let msg = i18n::tf("err_create_drive_folder", &[("e", &e.to_string())]);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    show_error(&ui, msg.as_str().into());
                                    ui.set_active_sync_code("".into());
                                    ui.set_active_hidden(false);
                                    ui.set_is_syncing(false);
                                }
                            });
                        }
                    }
                },
                Err(_) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            show_error(&ui, tr_ss("err_drive_login"));
                            ui.set_active_sync_code("".into());
                            ui.set_active_hidden(false);
                            ui.set_is_syncing(false);
                        }
                    });
                }
            }
        });
    });

    // ── Sync Koduna Bağlan ───────────────────────────────────────────────
    let ui_weak = ui.as_weak();
    ui.on_connect_to_sync(move |sync_code: slint::SharedString| {
        let sync_code = sync_code.to_string().trim().to_string();
        if sync_code.len() < 8 {
            if let Some(ui) = ui_weak.upgrade() {
                show_error(&ui, tr_ss("err_invalid_code"));
            }
            return;
        }

        let ui_weak = ui_weak.clone();
        std::thread::spawn(move || {
            let cloud_name = i18n::t("default_folder_name").to_string();
            let ui_weak2 = ui_weak.clone();
            let sync_code2 = sync_code.clone();
            
            let Some(parent_path) = rfd::FileDialog::new()
                    .set_title(i18n::t("pick_download_folder"))
                    .pick_folder()
                else { return };
                
                // Seçilen klasörün içine buluttaki isimle yeni bir klasör ekle
                let path = parent_path.join(&cloud_name);

                let _ = slint::invoke_from_event_loop(move || {
                    let config = sync_core::config::AppConfig::load();
                    if config.sync_folders.iter().any(|f| f.path == path.to_string_lossy()) {
                        if let Some(ui) = ui_weak2.upgrade() {
                            show_error(&ui, tr_ss("err_folder_exists"));
                        }
                        return;
                    }
                    
                    // Kaydetme işlemini edit_sync_save'e bırakıyoruz
                    if let Some(ui) = ui_weak2.upgrade() {
                        ui.invoke_show_edit_sync(sync_code2.clone().into(), cloud_name.clone().into(), path.to_string_lossy().to_string().into());
                    }
                });
        });
    });

    // ── Stop Sync ─────────────────────────────────────────────────────────
    let ui_weak = ui.as_weak();
    let app_state_stop = app_state.clone();
    ui.on_stop_sync(move |id| {
        remove_local_sync(&ui_weak, &app_state_stop, id.as_str());

        if let Some(ui) = ui_weak.upgrade() {
            ui.set_active_sync_code("".into());
            ui.set_active_hidden(false);
            ui.set_active_sync_folder("".into());
            ui.set_status_text("".into());
            ui.set_is_syncing(false);
        }
    });

    let app_state_pause = app_state.clone();
    let ui_weak_pause = ui.as_weak();
    ui.on_pause_sync_for_dialog(move |id| {
        let id_str = id.to_string();
        if let Some(tx) = app_state_pause.lock().unwrap().tasks.remove(&id_str) {
            let _ = tx.send(()); // Görevi durdur
        }
        if let Some(ui) = ui_weak_pause.upgrade() {
            ui.set_status_text(tr_ss("status_paused"));
            ui.set_is_syncing(false);
        }
    });

    let app_state_resume = app_state.clone();
    let ui_weak_resume = ui.as_weak();
    ui.on_resume_sync_after_dialog(move |id| {
        let id_str = id.to_string();
        if let Some(ui) = ui_weak_resume.upgrade() {
            ui.set_status_text(tr_ss("status_checking"));
            ui.set_is_syncing(true);
        }
        let path = {
            let cfg = sync_core::config::AppConfig::load();
            cfg.sync_folders.iter().find(|f| f.id == id_str).map(|f| f.path.clone())
        };
        if let Some(p) = path {
            start_sync_loop(ui_weak_resume.clone(), app_state_resume.clone(), id_str, std::path::PathBuf::from(p));
        }
    });

    // ── Pencere Kontrolleri ───────────────────────────────────────────────
    // Pencere yok edilmez, sadece gizlenir (tray'e iner); tray'den tekrar aynı pencere gösterilir.
    let ui_weak_min = ui.as_weak();
    ui.on_minimize_requested(move || {
        if let Some(ui) = ui_weak_min.upgrade() {
            let _ = ui.hide();
        }
    });

    let ui_weak_close = ui.as_weak();
    ui.on_close_requested(move || {
        if let Some(ui) = ui_weak_close.upgrade() {
            let _ = ui.hide();
        }
    });

    // Alt+F4 / işletim sistemi kapatma isteği: pencereyi kapatma, çıkış onay penceresini göster
    let ui_weak_os_close = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = ui_weak_os_close.upgrade() {
            ui.set_show_quit_dialog(true);
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
}

// ---------------------------------------------------------------------------
// Periyodik sync döngüsü — watcher + timer birleşik
// ---------------------------------------------------------------------------
fn start_sync_loop(
    ui_weak: slint::Weak<MainWindow>,
    app_state: Arc<Mutex<AppState>>,
    sync_code: String,
    folder: PathBuf,
) {
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let trigger = Arc::new(tokio::sync::Notify::new());
    {
        let mut state = app_state.lock().unwrap();
        if let Some(old) = state.tasks.insert(sync_code.clone(), stop_tx) {
            let _ = old.send(());
        }
        state.triggers.insert(sync_code.clone(), trigger.clone());
    }

    tokio::spawn(async move {
        sync_loop_task(ui_weak, sync_code, folder, stop_rx, trigger, app_state.clone()).await;
    });
}

async fn sync_loop_task(
    ui_weak: slint::Weak<MainWindow>,
    sync_code: String,
    folder: PathBuf,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    trigger: Arc<tokio::sync::Notify>,
    app_state_loop: Arc<Mutex<AppState>>,
) {
    // Token al
    let token = match sync_core::auth::get_drive_token(false).await {
        Ok(t) => t,
        Err(e) => {
            let msg = i18n::tf("err_token", &[("e", &e.to_string())]);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    show_error(&ui, msg.as_str().into());
                    ui.set_is_syncing(false);
                }
            });
            return;
        }
    };

    // Provider: token geçersizse etkileşimsiz yenile, olmuyorsa hata döndür
    let ui_weak_prov = ui_weak.clone();
    let provider: sync_core::drive::TokenProvider = Arc::new(move || {
        let ui_w = ui_weak_prov.clone();
        Box::pin(async move {
            match sync_core::auth::get_drive_token(false).await {
                Ok(t) => Ok(t),
                Err(_) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_w.upgrade() {
                            show_error(&ui, tr_ss("err_token_invalid"));
                            ui.set_is_logged_in(false);
                        }
                    });
                    Err(i18n::t("err_relogin"))
                }
            }
        })
    });

    let drive = match sync_core::drive::DriveClient::new(token, Some(provider)) {
        Ok(d) => d,
        Err(e) => {
            let msg = i18n::tf("err_drive_connect", &[("e", &e.to_string())]);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    show_error(&ui, msg.as_str().into());
                    ui.set_is_syncing(false);
                }
            });
            return;
        }
    };

    // Drive'da ana klasörü oluştur / bul (Vault başına ayrı klasör)
    // Kod formatı: cs-{driveFolderId}-{hexKey}. Drive ID'leri '-' içerebildiği için SON '-' ayırıcıdır.
    let parsed = sync_code.strip_prefix("cs-").and_then(|r| r.rsplit_once('-'));
    let (folder_id, _hex_key) = if let Some((fid, hk)) = parsed {
        (fid.to_string(), hk.to_string())
    } else {
        // Backward compatibility
        let folder_name = format!("ConnectSync_{}", &sync_code[0..8.min(sync_code.len())]);
        let id = match drive.get_or_create_folder(&folder_name, None, true).await {
            Ok(id) => id,
            Err(e) => {
                let msg = i18n::tf("err_create_drive_folder", &[("e", &e.to_string())]);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        show_error(&ui, msg.as_str().into());
                        ui.set_is_syncing(false);
                    }
                });
                return;
            }
        };
        (id, sync_code.clone())
    };

    // Drive'dan ya salt oku ya da üret (vault.json)
    // Arc ile drive'ı paylaş; engine'de zaten Arc<DriveClient> var
    let drive = Arc::new(drive);

    // Dummy keys ile geçici engine oluştur, sadece vault salt okumak için
    let dummy_keys = sync_core::crypto::Keys {
        enc_key: [0u8; 32],
        hmac_key: [0u8; 32],
    };
    let temp_engine = sync_core::engine::SyncEngine::new_with_arc(
        drive.clone(),
        dummy_keys,
        folder_id.clone(),
        folder.clone(),
    );
    let salt = match temp_engine.load_or_create_salt().await {
        Ok(s) => s,
        Err(e) => {
            let err_str = e.to_string();
            // Eğer Google Drive API'den 404 geldiyse, klasör Drive'da yok demektir!
            if err_str.contains("404") || err_str.contains("not found") || err_str.contains("notFound") {
                drive_missing::report(&ui_weak, &app_state_loop, &sync_code);
                return;
            }
            
            let msg = i18n::tf("err_salt", &[("e", &err_str)]);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    show_error(&ui, msg.as_str().into());
                    ui.set_is_syncing(false);
                }
            });
            return;
        }
    };
    drop(temp_engine);

    // Anahtarları türet (Argon2 thread'i bloklamaması için)
    let sync_code_clone = sync_code.clone();
    let keys_res = tokio::task::spawn_blocking(move || {
        sync_core::crypto::derive_keys(&sync_code_clone, &salt)
    }).await.unwrap();

    let keys = match keys_res {
        Ok(k) => k,
        Err(e) => {
            let msg = i18n::tf("err_key_derive", &[("e", &e.to_string())]);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    show_error(&ui, msg.as_str().into());
                    ui.set_is_syncing(false);
                }
            });
            return;
        }
    };

    // Arayüz zamanlayıcısının okuyacağı ilerleme/ağ durumu; engine'e taşınmadan önce sakla.
    let progress_handle = drive.progress.clone();
    {
        let mut st = app_state_loop.lock().unwrap();
        let fs = st
            .folder_states
            .entry(sync_code.clone())
            .or_default();
        fs.progress = Some(progress_handle);
    }

    // Ayarlar sync başlarken bir kez okunur (eşzamanlı aktarım sayısı bir sonraki
    // sync başlangıcında geçerli olur).
    let config = sync_core::config::AppConfig::load();
    let engine = Arc::new(
        sync_core::engine::SyncEngine::new_with_arc(drive, keys, folder_id, folder.clone())
            .with_threads(config.concurrent_threads),
    );

    // Watcher başlat (ayarlardan kapatılabilir: yalnızca periyodik taramayla yetin)
    let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel::<String>(64);
    let _debouncer = if config.watch_local_changes {
        let watcher_obj = sync_core::watcher::LocalWatcher::new(folder.to_str().unwrap_or("."));
        watcher_obj.start_watching(watcher_tx).ok()
    } else {
        drop(watcher_tx);
        None
    };

    // Periyodik timer (Config'den oku, yoksa 5 dk)
    let mins = u64::from(config.check_interval_minutes());
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(mins * 60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    // Döngü her turda bir kere sync yapıp sonra bekler
    loop {
        match engine.drive_client.folder_exists(&engine.drive_folder_id).await {
            Ok(false) => {
                drive_missing::report(&ui_weak, &app_state_loop, &sync_code);
                break;
            }
            _ => {} // Other network errors or success -> continue to pull
        }

        // 1. Önce Pull (Drive -> Yerel). "İndiriliyor" denmez: önce farklar incelenir; metin,
        // motorun gerçek fazına göre kendiliğinden değişir (bkz. `with_phase_status`).
        update_status(&ui_weak, &app_state_loop, &sync_code, &i18n::t("status_checking"), true);
        let mut pulled = 0usize;
        tokio::select! {
            _ = &mut stop_rx => {
                
                break;
            }
            res = with_phase_status(
                engine.run_sync_pull(&folder, true),
                &engine.drive_client.progress,
                &ui_weak,
                &app_state_loop,
                &sync_code,
            ) => {
                match res {
                    Ok(report) => pulled = report.restored_files,
                    Err(e) => update_status(&ui_weak, &app_state_loop, &sync_code, &i18n::tf("err_pull", &[("e", &e.to_string())]), false),
                }
            }
        }

        // 2. Sonra Push (Yerel -> Drive)
        tokio::select! {
            _ = &mut stop_rx => {
                
                break;
            }
            _ = run_push(&engine, &ui_weak, &app_state_loop, &sync_code, pulled) => {}
        }

        tokio::select! {
            _ = &mut stop_rx => {
                
                break;
            }
            _ = interval.tick() => {}
            // Manuel eşitle butonu: döngüyü yeniden başlatmadan hemen bir tur çalıştır
            _ = trigger.notified() => {}
            Some(_changed) = watcher_rx.recv() => {
                // Debounce
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                while watcher_rx.try_recv().is_ok() {}
            }
        }
    }
}

/// Motorun o anki fazına göre gösterilecek durum metninin anahtarı.
/// - Hazırlık (index/manifest/yerel tarama) = farklar inceleniyor.
/// - İndirme/yükleme yalnızca gerçekten aktarılacak dosya varsa söylenir; motor aktarılacak
///   dosya olmasa da `begin(PULL/PUSH, 0, 0)` çağırdığı için `total_files` bakılır.
/// - Boşta (pull ile push arası) `None`: mevcut metin değişmez.
fn phase_status_key(s: &sync_core::progress::Snapshot) -> Option<&'static str> {
    use sync_core::progress::{PHASE_PREPARE, PHASE_PULL, PHASE_PUSH};
    match s.phase {
        PHASE_PULL if s.total_files > 0 => Some("status_pulling"),
        PHASE_PUSH if s.total_files > 0 => Some("status_uploading"),
        PHASE_PREPARE | PHASE_PULL | PHASE_PUSH => Some("status_checking"),
        _ => None,
    }
}

/// Bir sync turu bittiğinde gösterilecek metin: (locale anahtarı, `{n}` değeri).
/// Hata her şeyin önüne geçer; hiçbir şey değişmediyse "fark yok".
fn final_sync_message(
    pulled: usize,
    uploaded: usize,
    removed: usize,
    failed: usize,
) -> (&'static str, Option<usize>) {
    if failed > 0 {
        return ("status_files_failed", Some(failed));
    }
    let changed = pulled + uploaded + removed;
    if changed > 0 {
        ("status_files_updated", Some(changed))
    } else {
        ("status_no_diff", None)
    }
}

/// `fut` çalışırken motorun fazını izler ve durum metnini fazla birlikte günceller
/// ("inceleniyor" → "indiriliyor"/"yükleniyor"). Metin yalnızca faz değişince yazılır.
async fn with_phase_status<F: std::future::Future>(
    fut: F,
    progress: &sync_core::progress::Progress,
    ui_weak: &slint::Weak<MainWindow>,
    app_state: &Arc<Mutex<AppState>>,
    sync_code: &str,
) -> F::Output {
    tokio::pin!(fut);
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(200));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last: Option<&'static str> = None;
    loop {
        tokio::select! {
            out = &mut fut => return out,
            _ = ticker.tick() => {
                if let Some(key) = phase_status_key(&progress.snapshot())
                    && last != Some(key)
                {
                    last = Some(key);
                    update_status(ui_weak, app_state, sync_code, &i18n::t(key), true);
                }
            }
        }
    }
}

async fn run_push(
    engine: &Arc<sync_core::engine::SyncEngine>,
    ui_weak: &slint::Weak<MainWindow>,
    app_state: &Arc<Mutex<AppState>>,
    sync_code: &str,
    pulled: usize,
) {
    update_status(ui_weak, app_state, sync_code, &i18n::t("status_checking"), true);
    let result = with_phase_status(
        engine.run_sync_push(),
        &engine.drive_client.progress,
        ui_weak,
        app_state,
        sync_code,
    )
    .await;
    match result {
        Ok(report) => {
            // Kullanıcıya teknik sayaçlar yerine sade bir durum göster
            let (key, n) = final_sync_message(
                pulled,
                report.uploaded_files,
                report.removed_from_manifest,
                report.failed_files.len(),
            );
            let msg = match n {
                Some(n) => i18n::tf(key, &[("n", &n.to_string())]),
                None => i18n::t(key),
            };
            update_status(ui_weak, app_state, sync_code, &msg, false);
            if !report.failed_files.is_empty() {
                let err = report.failed_files[0].1.clone();
                update_error(ui_weak, app_state, sync_code, &err);
            } else {
                update_error(ui_weak, app_state, sync_code, "");
            }
            
            // Eğer yeni dosyalar yüklendiyse veya ara sıra GC tetiklemek isteniyorsa:
            // Arka planda GC çalıştır (Push akışını bloklamaması için tokio::spawn); ayarlardan kapatılabilir.
            if sync_core::config::AppConfig::load().auto_gc {
                let engine_clone = engine.clone();
                tokio::spawn(async move {
                    let _ = engine_clone.run_gc().await;
                });
            }
        }
        Err(e) => {
            let msg = i18n::tf("err_sync", &[("e", &e.to_string())]);
            update_status(ui_weak, app_state, sync_code, &msg, false);
            update_error(ui_weak, app_state, sync_code, &msg);
        }
    }
}

fn update_status(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, folder_id: &str, msg: &str, syncing: bool) {
    let msg = msg.to_string();
    let app_state_clone = app_state.clone();
    {
        let mut st = app_state.lock().unwrap();
        let fs = st.folder_states.entry(folder_id.to_string()).or_default();
        fs.status = msg.clone();
        fs.is_syncing = syncing;
        fs.error = "".into();
    }
    let uw = ui_weak.clone();
    let msg_clone = msg.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = uw.upgrade() {
            update_ui_folders(&ui, &app_state_clone);
            ui.set_status_text(msg_clone.as_str().into());
            ui.set_is_syncing(syncing);
        }
    });
}

fn update_error(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, folder_id: &str, err: &str) {
    let err = err.to_string();
    let app_state_clone = app_state.clone();
    {
        let mut st = app_state.lock().unwrap();
        let fs = st.folder_states.entry(folder_id.to_string()).or_default();
        fs.error = err.clone();
        fs.is_syncing = false;
    }
    let uw = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = uw.upgrade() {
            update_ui_folders(&ui, &app_state_clone);
            show_error(&ui, err.as_str().into()); // Show global toast as well if needed
        }
    });
}

/// Aktif sync ekranındaki ilerleme çubuğunu (MB/GB) ve bağlantı uyarısını,
/// o anda ekranda gösterilen `active_sync_code`'un ilerleme durumuna göre günceller.
/// Ana pencere zamanlayıcısından (tray timer) periyodik olarak çağrılır.
fn update_progress_ui(ui: &MainWindow, app_state: &Arc<Mutex<AppState>>) {
    let active_id = ui.get_active_sync_code().to_string();
    if active_id.is_empty() || active_id == "pending" || !ui.get_is_syncing() {
        if ui.get_progress_visible() {
            ui.set_progress_visible(false);
        }
        if ui.get_net_warning() {
            ui.set_net_warning(false);
        }
        return;
    }

    let snapshot = {
        let st = app_state.lock().unwrap();
        st.folder_states
            .get(&active_id)
            .and_then(|fs| fs.progress.as_ref())
            .map(|p| p.snapshot())
    };

    let Some(s) = snapshot else {
        ui.set_progress_visible(false);
        ui.set_net_warning(false);
        return;
    };

    let show_bar = s.total_bytes > 0;
    ui.set_progress_visible(show_bar);
    if show_bar {
        let frac = (s.done_bytes as f32 / s.total_bytes as f32).clamp(0.0, 1.0);
        ui.set_progress_fraction(frac);
        let pct = (frac * 100.0).round() as u32;
        ui.set_progress_text(
            format!(
                "{} · %{pct} · {}/{} dosya",
                sync_core::progress::format_bytes_pair(s.done_bytes, s.total_bytes),
                s.done_files,
                s.total_files
            )
            .into(),
        );
    } else {
        ui.set_progress_text("".into());
    }

    // ~300 ms'de bir çalıştığı için config'i (dosya + keyring) her seferinde okumak yerine
    // ayarın arayüzdeki yansımasını kullan; başlangıçta config'ten dolduruluyor ve anahtara bağlı.
    let net_issue = s.net_retry > 0 && ui.get_setting_network_popups();
    ui.set_net_warning(net_issue);
    if net_issue {
        ui.set_net_warning_text(
            i18n::tf("net_waiting", &[("n", s.net_retry.to_string())]).into(),
        );
    }
}

fn create_window(
    ui_handle: Rc<RefCell<Option<MainWindow>>>,
    app_state: Arc<Mutex<AppState>>,
) -> MainWindow {
    let ui = MainWindow::new().unwrap();
    setup_ui(&ui, ui_handle, app_state);
    ui
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    gtk::init().unwrap();

    let app_state: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState::default()));
    let ui_handle: Rc<RefCell<Option<MainWindow>>> = Rc::new(RefCell::new(None));

    // Dil (tray menüsü dahil, başlangıçta config'ten)
    i18n::set_language(&sync_core::config::AppConfig::load().language);

    // Tray
    let tray_menu = Menu::new();
    let show_i = MenuItem::new(i18n::t("tray_show"), true, None);
    let quit_i = MenuItem::new(i18n::t("tray_quit"), true, None);
    tray_menu.append(&show_i).unwrap();
    tray_menu.append(&quit_i).unwrap();

    let icon = load_icon(include_bytes!("../assets/logo_square.png"));
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("ConnectSync")
        .with_icon(icon)
        .build()
        .unwrap();

    let menu_channel = tray_icon::menu::MenuEvent::receiver();
    let tray_channel = tray_icon::TrayIconEvent::receiver();

    // Autostart + otomatik başlatma kaydı
    let config = sync_core::config::AppConfig::load();

    let is_autostart = std::env::args().any(|a| a == "--autostart");
    if let Ok(app_path) = std::env::current_exe()
        && let Some(s) = app_path.to_str()
            && let Ok(auto) = auto_launch::AutoLaunchBuilder::new()
                .set_app_name("ConnectSync")
                .set_app_path(s)
                .set_args(&["--autostart"])
                .build()
            {
                if config.auto_start_enabled { let _ = auto.enable(); }
                else { let _ = auto.disable(); }
            }

    // Autostart'ta pencere gizli başlar; sync daha önce aktifse arka planda başlat
    if !is_autostart {
        let ui = create_window(ui_handle.clone(), app_state.clone());
        ui.show().unwrap();
        *ui_handle.borrow_mut() = Some(ui);
    } else {
        println!("Autostart: tray'de çalışıyor...");
        for f in config.sync_folders {
            let sync_code = f.code.clone();
            let folder_path = PathBuf::from(f.path.clone());
            let app_state_bg = app_state.clone();
            tokio::spawn(async move {
                for _ in 0..10 {
                    if sync_core::auth::get_drive_token(false).await.is_ok() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                let dummy_ui: slint::Weak<MainWindow> = slint::Weak::default();
                start_sync_loop(dummy_ui, app_state_bg, sync_code, folder_path);
            });
        }
    }

    // Önceki güncellemeden kalan geçici/eski ikili dosyaları temizle
    updater::cleanup_stale_files();

    // Buluttaki (bu PC'de olmayan) sync'leri açılışta ve aralıklarla kendiliğinden bul
    spawn_background_cloud_scan(app_state.clone());

    // 100ms tray timer
    let slint_timer = slint::Timer::default();
    let handle_for_timer = ui_handle.clone();
    let app_state_timer = app_state.clone();
    let progress_tick = std::cell::Cell::new(0u32);
    slint_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            #[cfg(target_os = "linux")]
            {
                while gtk::glib::MainContext::default().pending() {
                    gtk::glib::MainContext::default().iteration(false);
                }
            }

            let mut should_show = false;

            while let Ok(event) = menu_channel.try_recv() {
                if event.id == show_i.id() {
                    should_show = true;
                } else if event.id == quit_i.id() {
                    let _ = slint::quit_event_loop();
                }
            }

            while let Ok(event) = tray_channel.try_recv() {
                if let tray_icon::TrayIconEvent::Click { button, .. } = event
                    && button == tray_icon::MouseButton::Left {
                        should_show = true;
                    }
            }

            if should_show {
                let mut handle = handle_for_timer.borrow_mut();
                if handle.is_none() {
                    let new_ui = create_window(handle_for_timer.clone(), app_state_timer.clone());
                    new_ui.show().unwrap();
                    *handle = Some(new_ui);
                } else {
                    // Aynı pencereyi göster/öne getir (ikinci pencere açma)
                    let w = handle.as_ref().unwrap();
                    w.show().unwrap();
                    w.window().set_minimized(false);
                    w.window().request_redraw();
                }
            }

            // Arka plan bulut taraması yeni liste getirdiyse arayüzü yenile.
            let cloud_changed = std::mem::take(&mut app_state_timer.lock().unwrap().cloud_dirty);
            if cloud_changed
                && let Some(ui) = handle_for_timer.borrow().as_ref()
            {
                update_ui_folders(ui, &app_state_timer);
            }

            if let Some(ui) = handle_for_timer.borrow().as_ref() {
                drive_missing::sync_window(ui, &app_state_timer);
            }


            // Güncelleme popup'u: durum değiştiyse ya da indirme sürüyorsa pencereye yansıt.
            if let Some(ui) = handle_for_timer.borrow().as_ref() {
                update_ui::sync_window(ui, &app_state_timer);
            }

            // İlerleme çubuğu / bağlantı uyarısı: ~300ms'de bir güncelle (her 100ms'de gerek yok).
            progress_tick.set(progress_tick.get().wrapping_add(1));
            if progress_tick.get().is_multiple_of(3)
                && let Some(ui) = handle_for_timer.borrow().as_ref() {
                    update_progress_ui(ui, &app_state_timer);
                }
        },
    );

    slint::run_event_loop_until_quit().unwrap();
    Ok(())
}
/// Arka plan taramasında hata sonrası ilk yeniden deneme bekleme süresi.
const CLOUD_SCAN_RETRY_BASE: std::time::Duration = std::time::Duration::from_secs(30);

/// Bir sonraki bulut taramasına kadar beklenecek süre. Başarıda ayarlardaki kontrol aralığı;
/// ardışık hatalarda 30 sn'den başlayıp iki katına çıkan (ama aralığı aşmayan) bir bekleme.
/// Böylece otomatik başlatmada ağ henüz hazır değilse liste 5 dk beklemeden dolar, ağ
/// kalıcı olarak yoksa da log/istek boğulmaz.
fn next_scan_delay(interval: std::time::Duration, consecutive_failures: u32) -> std::time::Duration {
    if consecutive_failures == 0 {
        return interval;
    }
    let exp = consecutive_failures.saturating_sub(1).min(6);
    (CLOUD_SCAN_RETRY_BASE * 2u32.pow(exp)).min(interval)
}

/// Tarama sonucunu `AppState`'e yazar; liste değiştiyse arayüz yenileme bayrağını kaldırır.
/// Tarama sürerken çıkış yapıldıysa (oturum silinmişse) eski hesabın klasörleri geri gelmesin
/// diye sonuç atılır.
fn publish_cloud_items(app_state: &Arc<Mutex<AppState>>, items: Vec<(String, String)>) {
    if !sync_core::auth::is_token_cached() {
        return;
    }
    let mut st = app_state.lock().unwrap();
    if st.cloud_items != items {
        st.cloud_items = items;
        st.cloud_dirty = true;
    }
}

/// Uygulama açılır açılmaz ve sonra "Kontrol aralığı" ayarında Drive'ı sessizce tarar: bulutta
/// olup bu PC'ye eklenmemiş sync'ler "Buluttan Bul"a basmadan listede görünür (~1 MB'ın altı).
/// Oturum yoksa tarama atlanır. Pencereye dokunmaz (tray modunda pencere olmayabilir); yalnızca
/// `AppState`'i günceller, arayüzü tray zamanlayıcısı yeniler. Hatalar popup açmaz, log'lanır.
fn spawn_background_cloud_scan(app_state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        let mut failures = 0u32;
        loop {
            if sync_core::auth::is_token_cached() {
                match scan_cloud_folders().await {
                    Ok(items) => {
                        failures = 0;
                        publish_cloud_items(&app_state, items);
                    }
                    Err(e) => {
                        failures = failures.saturating_add(1);
                        println!("Bulut taraması başarısız ({failures}. deneme): {e}");
                    }
                }
            }
            let config = sync_core::config::AppConfig::load();
            let interval = std::time::Duration::from_secs(u64::from(config.check_interval_minutes()) * 60);
            tokio::time::sleep(next_scan_delay(interval, failures)).await;
        }
    });
}

/// Drive'daki ConnectSync klasörlerini + appDataFolder'daki anahtar kaydını okur.
/// Anahtarı bilinen her klasör için (kod, ad) döndürür; hangilerinin bu PC'de olduğu
/// `cloud_only` ile ayıklanır. Config'e hiçbir şey eklenmez — kullanıcı "Ekle / İndir" der.
async fn scan_cloud_folders() -> Result<Vec<(String, String)>, String> {
    let token = sync_core::auth::get_drive_token(false)
        .await
        .map_err(|e| i18n::tf("err_drive_login_needed", &[("e", &e.to_string())]))?;
    let drive = sync_core::drive::DriveClient::new(token, None).map_err(|e| e.to_string())?;

    let cloud_folders = drive
        .list_cloud_folders()
        .await
        .map_err(|e| i18n::tf("err_list_folders", &[("e", &e.to_string())]))?;
    let mut keys = drive
        .get_sync_keys()
        .await
        .map_err(|e| i18n::tf("err_read_keys", &[("e", &e.to_string())]))?;

    let config = sync_core::config::AppConfig::load();

    // Bu PC'de olup bulutta anahtarı kayıtlı olmayan sync'lerin anahtarını kaydet
    // (eski sürümde kayıt sessizce başarısız olmuş olabilir). Kod formatı: cs-{folderId}-{hexKey}
    let local_codes: Vec<String> = config
        .sync_folders
        .iter()
        .map(|f| if f.code.is_empty() { f.id.clone() } else { f.code.clone() })
        .collect();
    for code in local_codes {
        if let Some((fid, hex_key)) = code.strip_prefix("cs-").and_then(|r| r.rsplit_once('-'))
            && !keys.contains_key(fid) && drive.save_sync_key(fid, hex_key).await.is_ok() {
                keys.insert(fid.to_string(), hex_key.to_string());
            }
    }

    let mut items = Vec::new();
    for (fid, name) in cloud_folders {
        let Some(hex_key) = keys.get(&fid) else { continue };
        // `list_cloud_folders` zaten kullanıcının verdiği gerçek adı döndürür (appProperties'ten);
        // yalnızca eski/adsız klasörlerde hâlâ "ConnectSync_<hex>" biçimindeki Drive adı gelir —
        // o durumda önek temizlenir (aksi halde kullanıcıya rastgele bir hex görünür).
        let display = if name.starts_with("ConnectSync_") {
            name.trim_start_matches("ConnectSync_").to_string()
        } else {
            name
        };
        items.push((format!("cs-{}-{}", fid, hex_key), display));
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_core::config::{AppConfig, SyncFolder};
    use std::time::Duration;

    fn folder(id: &str, code: &str) -> SyncFolder {
        SyncFolder { id: id.into(), name: "x".into(), path: "/tmp/x".into(), code: code.into() }
    }

    fn state_with(items: &[(&str, &str)]) -> AppState {
        AppState {
            cloud_items: items.iter().map(|(c, n)| (c.to_string(), n.to_string())).collect(),
            ..AppState::default()
        }
    }

    #[test]
    fn drive_folder_id_takes_everything_between_prefix_and_last_dash() {
        // Drive ID'leri '-' içerebilir; yalnızca SON '-' ayırıcıdır.
        assert_eq!(drive_folder_id("cs-1a-B_c-d-deadbeef"), "1a-B_c-d");
        assert_eq!(drive_folder_id("cs-abc-key"), "abc");
        assert_eq!(drive_folder_id("kodsuz"), "kodsuz");
    }

    #[test]
    fn cloud_only_hides_folders_already_on_this_pc() {
        let mut cfg = AppConfig::default();
        cfg.sync_folders.push(folder("ignored", "cs-F1-k1"));
        let st = state_with(&[("cs-F1-k1", "Müzik"), ("cs-F2-k2", "Belgeler")]);
        assert_eq!(cloud_only(&cfg, &st), vec![("cs-F2-k2".to_string(), "Belgeler".to_string())]);
    }

    #[test]
    fn cloud_only_falls_back_to_id_when_code_is_missing() {
        let mut cfg = AppConfig::default();
        cfg.sync_folders.push(folder("cs-F1-k1", ""));
        let st = state_with(&[("cs-F1-k1", "Müzik")]);
        assert!(cloud_only(&cfg, &st).is_empty());
    }

    #[test]
    fn every_tr_property_is_filled_from_an_existing_locale_key() {
        // Slint'te `Tr.*` property'sinin setter'ı `apply_language`'de unutulursa metin sessizce
        // boş kalır; anahtar locale'de yoksa da ekranda ham anahtar görünür.
        let slint = include_str!("../ui/main.slint");
        let tr_block = slint
            .split("export global Tr {")
            .nth(1)
            .and_then(|r| r.split("\n}").next())
            .expect("Tr global'i bulunamadı");
        let props: Vec<&str> = tr_block
            .lines()
            .filter_map(|l| l.trim().strip_prefix("in property <string> "))
            .map(|r| r.split([';', ':']).next().unwrap().trim())
            .collect();
        assert!(props.len() > 40, "Tr property'leri ayrıştırılamadı: {}", props.len());

        let source = include_str!("main.rs");
        for prop in props {
            let needle = format!("set_{prop} => \"");
            let key = source
                .split(&needle)
                .nth(1)
                .and_then(|r| r.split('"').next())
                .unwrap_or_else(|| panic!("Tr.{prop} için apply_language'de setter yok"));
            assert_ne!(i18n::t(key), key, "Tr.{prop}: '{key}' anahtarı locale'de yok");
        }
    }

    fn snapshot(phase: u8, total_files: u64) -> sync_core::progress::Snapshot {
        sync_core::progress::Snapshot { phase, total_files, ..Default::default() }
    }

    #[test]
    fn status_says_checking_while_diffing() {
        use sync_core::progress::PHASE_PREPARE;
        assert_eq!(phase_status_key(&snapshot(PHASE_PREPARE, 0)), Some("status_checking"));
    }

    #[test]
    fn status_only_says_transfer_when_there_are_files_to_transfer() {
        use sync_core::progress::{PHASE_PULL, PHASE_PUSH};
        // Motor, aktarılacak dosya olmasa da begin(PULL/PUSH, 0, 0) çağırır: bu "indiriliyor" demek olmamalı.
        assert_eq!(phase_status_key(&snapshot(PHASE_PULL, 0)), Some("status_checking"));
        assert_eq!(phase_status_key(&snapshot(PHASE_PUSH, 0)), Some("status_checking"));
        assert_eq!(phase_status_key(&snapshot(PHASE_PULL, 3)), Some("status_pulling"));
        assert_eq!(phase_status_key(&snapshot(PHASE_PUSH, 3)), Some("status_uploading"));
    }

    #[test]
    fn status_is_left_alone_when_idle() {
        // Pull ile push arasındaki boşlukta (IDLE) metin değişmemeli.
        assert_eq!(phase_status_key(&snapshot(sync_core::progress::PHASE_IDLE, 0)), None);
    }

    #[test]
    fn final_message_counts_pulled_and_pushed_changes() {
        assert_eq!(final_sync_message(0, 0, 0, 0), ("status_no_diff", None));
        assert_eq!(final_sync_message(2, 0, 0, 0), ("status_files_updated", Some(2)));
        assert_eq!(final_sync_message(0, 3, 1, 0), ("status_files_updated", Some(4)));
        assert_eq!(final_sync_message(5, 0, 0, 0), ("status_files_updated", Some(5)));
        // Hata her şeyin önüne geçer
        assert_eq!(final_sync_message(5, 2, 0, 1), ("status_files_failed", Some(1)));
    }

    #[test]
    fn scan_delay_is_the_interval_when_healthy() {
        let interval = Duration::from_secs(300);
        assert_eq!(next_scan_delay(interval, 0), interval);
    }

    #[test]
    fn scan_delay_backs_off_but_never_exceeds_interval() {
        let interval = Duration::from_secs(300);
        let delays: Vec<_> = (1..=10).map(|n| next_scan_delay(interval, n)).collect();
        assert_eq!(delays[0], Duration::from_secs(30));
        assert_eq!(delays[1], Duration::from_secs(60));
        assert!(delays.windows(2).all(|w| w[0] <= w[1]), "azalmamalı: {delays:?}");
        assert!(delays.iter().all(|d| *d <= interval && *d > Duration::ZERO));
        assert_eq!(*delays.last().unwrap(), interval);
    }

    #[test]
    fn scan_delay_respects_short_intervals() {
        // Aralık 30 sn'den kısaysa yeniden deneme aralığı aşmaz.
        let interval = Duration::from_secs(10);
        assert_eq!(next_scan_delay(interval, 1), interval);
    }

    #[test]
    fn scan_delay_survives_u32_max_failures() {
        let interval = Duration::from_secs(60);
        assert_eq!(next_scan_delay(interval, u32::MAX), interval);
    }

}
