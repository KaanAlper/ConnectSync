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
        .set_use_launch_agent(true)
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
}

#[derive(Default)]
pub struct AppState {
    pub tasks: std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>,
    /// Çalışan sync döngüsünü beklemeden uyandırmak için (manuel eşitle butonu)
    pub triggers: std::collections::HashMap<String, Arc<tokio::sync::Notify>>,
    pub folder_states: std::collections::HashMap<String, FolderState>,
}

// ---------------------------------------------------------------------------
// UI kurulumu ve tüm callback bağlantıları
// ---------------------------------------------------------------------------
pub fn update_ui_folders(ui: &crate::MainWindow, app_state: &std::sync::Arc<std::sync::Mutex<crate::AppState>>) {
    let config = crate::sync_core::config::AppConfig::load();
    let model = std::rc::Rc::new(slint::VecModel::<crate::SyncFolderItem>::default());
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
    drop(state);
    ui.set_sync_folders(model.into());
}

fn setup_ui(
    ui: &MainWindow,
    ui_handle: Rc<RefCell<Option<MainWindow>>>,
    app_state: Arc<Mutex<AppState>>,
) {
    // ── Dil ──────────────────────────────────────────────────────────────
    apply_language(ui, &sync_core::config::AppConfig::load().language);
    ui.set_status_text(tr_ss("ready"));

    // ── Başlangıç durumu: keyring'den token var mı? ──────────────────────
    if sync_core::auth::is_token_cached() {
        ui.set_is_logged_in(true);
        let ui_w3 = ui.as_weak();
        tokio::spawn(async move { fetch_cloud_folders(ui_w3).await; });
    } else {
        // Autostart senaryosu: session henüz tam açılmamış, keyring kilitli olabilir.
        // Etkileşimsiz modda sessizce dene; başarılıysa UI'ı güncelle.
        let ui_w3_b = ui.as_weak();
        let ui_weak = ui.as_weak();
        tokio::spawn(async move {
            if sync_core::auth::get_drive_token(false).await.is_ok() {
                tokio::spawn(async move { fetch_cloud_folders(ui_w3_b).await; });
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
                            let ui_w3 = ui_weak2.clone();
                            tokio::spawn(async move { fetch_cloud_folders(ui_w3).await; });
                        }
                    });
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak2.upgrade() {
                            ui.set_is_logging_in(false);
                            let ui_w3 = ui_weak2.clone();
                            ui.set_error_text(msg.as_str().into());
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
                    if let Some(ui) = ui_weak_timer.upgrade() {
                        if ui.get_copy_feedback_id().to_string() == id_clone {
                            ui.set_copy_feedback_id("".into());
                        }
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
            let result = scan_cloud_into_config().await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_bg.upgrade() {
                    update_ui_folders(&ui, &app_state_bg);
                    match result {
                        Ok(added) => {
                            if !ui.get_is_syncing() {
                                if added > 0 {
                                    ui.set_status_text(i18n::tf("syncs_found", &[("n", &added.to_string())]).into());
                                } else {
                                    ui.set_status_text(tr_ss("no_new_syncs"));
                                }
                            }
                        }
                        Err(e) => ui.set_error_text(e.as_str().into()),
                    }
                }
            });
        });
    });

    ui.on_remove_sync_folder(move |id| {
        let id_str = id.to_string();
        {
            let mut state = app_state_remove.lock().unwrap();
            if let Some(tx) = state.tasks.remove(&id_str) {
                let _ = tx.send(());
            }
            state.folder_states.remove(&id_str);
        }
        let mut config = sync_core::config::AppConfig::load();
        config.sync_folders.retain(|f| f.id != id_str);
        let _ = config.save();
        if let Some(u) = ui_remove.upgrade() {
            update_ui_folders(&u, &app_state_remove);
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
            update_status(&ui_weak_manual, &app_state_manual, &id, &i18n::t("status_pushing"), true);
            t.notify_one();
            return;
        }

        // Döngü hiç başlamamış (ör. buluttan bulunan klasör) → başlat
        let config = sync_core::config::AppConfig::load();
        if let Some(f) = config.sync_folders.iter().find(|f| f.id == id) {
            let code = if f.code.is_empty() { f.id.clone() } else { f.code.clone() };
            update_status(&ui_weak_manual, &app_state_manual, &id, &i18n::t("status_pulling"), true);
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
            let _ = webbrowser::open(&path);
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
        drop(state);

        sync_core::auth::logout();

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
    let app_state_new = app_state.clone();
    ui.on_create_new_sync(move || {
        let Some(path) = FileDialog::new()
            .set_title(i18n::t("pick_sync_folder"))
            .pick_folder()
        else { return };

        let mut raw = [0u8; 16];
        if getrandom::fill(&mut raw).is_err() {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_error_text(tr_ss("err_code_gen"));
            }
            return;
        }
        let hex_key = hex::encode(&raw);
        let folder_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(i18n::t("default_folder_name").as_str())
            .to_string();

        let mut config = sync_core::config::AppConfig::load();
        if config.sync_folders.iter().any(|f| f.path == path.to_string_lossy().to_string()) {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_error_text(tr_ss("err_folder_exists"));
            }
            return;
        }

        // Show a loading UI directly on main screen before entering Active View?
        // Or we can enter Active View with a temporary code, and update it later.
        // Let's just enter Active View with "Bekleniyor..."
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_active_sync_code("pending".into()); // Geçici - kopyala butonu gizlenecek
            ui.set_active_hidden(false);
            ui.set_active_sync_folder(folder_name.as_str().into());
            ui.set_status_text(tr_ss("creating_drive_folder"));
            ui.set_is_syncing(true);
        }

        let ui_weak_bg = ui_weak.clone();
        let app_state_bg = app_state_new.clone();
        tokio::spawn(async move {
            match sync_core::auth::get_drive_token(true).await {
                Ok(token) => {
                    let drive = sync_core::drive::DriveClient::new(token, None).unwrap();
                    let drive_folder_name = format!("ConnectSync_{}", &hex_key[0..8.min(hex_key.len())]);
                    match drive.get_or_create_folder(&drive_folder_name, None).await {
                        Ok(folder_id) => {
                            // save keys
                            let _ = drive.save_sync_key(&folder_id, &hex_key).await;

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
                            let ui_w_update = ui_weak_bg.clone();
                            let app_state_update = app_state_bg.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_w_update.upgrade() {
                                    update_ui_folders(&ui, &app_state_update);
                                    ui.set_active_sync_code(code_for_ui.as_str().into());
                                    ui.set_status_text(tr_ss("sync_ready"));
                                }
                            });
                            
                            start_sync_loop(ui_weak_bg, app_state_bg, universal_code, path);
                        },
                        Err(e) => {
                            let msg = i18n::tf("err_create_drive_folder", &[("e", &e.to_string())]);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak_bg.upgrade() {
                                    ui.set_error_text(msg.as_str().into());
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
                        if let Some(ui) = ui_weak_bg.upgrade() {
                            ui.set_error_text(tr_ss("err_drive_login"));
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
    let app_state_connect = app_state.clone();
    ui.on_connect_to_sync(move |sync_code: slint::SharedString| {
        let sync_code = sync_code.to_string().trim().to_string(); // trim eklendi
        if sync_code.len() < 8 {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_error_text(tr_ss("err_invalid_code"));
            }
            return;
        }

        let Some(path) = FileDialog::new()
            .set_title(i18n::t("pick_download_folder"))
            .pick_folder()
        else { return };

        let folder_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(i18n::t("default_folder_name").as_str())
            .to_string();

        // Konfigürasyona kaydet
        let mut config = sync_core::config::AppConfig::load();
        config.sync_folders.push(sync_core::config::SyncFolder { id: sync_code.clone(), name: folder_name.clone(), path: path.to_string_lossy().to_string(), code: sync_code.clone() });
        
        if let Some(ui) = ui_weak.upgrade() { update_ui_folders(&ui, &app_state_connect); }
        if let Err(e) = config.save() {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_error_text(e.as_str().into());
            }
        }

        {
            let mut state = app_state_connect.lock().unwrap();
        }

        if let Some(ui) = ui_weak.upgrade() {
            ui.set_active_sync_code(sync_code.clone().as_str().into());
            ui.set_active_hidden(false);
            ui.set_active_sync_folder(folder_name.as_str().into());
            // ui.set_error_text("".into()); // Hata varsa silmemesi için yoruma alıyoruz
            ui.set_status_text(tr_ss("connecting_starting"));
        }

        start_sync_loop(ui_weak.clone(), app_state_connect.clone(), sync_code, path);
    });

    // ── Stop Sync ─────────────────────────────────────────────────────────
    let ui_weak = ui.as_weak();
    let app_state_stop = app_state.clone();
    ui.on_stop_sync(move |_id| {
        let mut state = app_state_stop.lock().unwrap();
        for (_, tx) in state.tasks.drain() {
            let _ = tx.send(());
        }
        drop(state);

        // Config'den sync kodu temizle
        let mut config = sync_core::config::AppConfig::load();
        config.sync_folders.clear();
        let _ = config.save();

        if let Some(ui) = ui_weak.upgrade() {
            ui.set_active_sync_code("".into());
            ui.set_active_hidden(false);
            ui.set_active_sync_folder("".into());
            ui.set_status_text("".into());
            ui.set_is_syncing(false);
        }
    });

    // ── Pencere Kontrolleri ───────────────────────────────────────────────
    let handle_clone = ui_handle.clone();
    ui.on_minimize_requested(move || {
        let h = handle_clone.clone();
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            *h.borrow_mut() = None;
        });
    });

    let handle_clone2 = ui_handle.clone();
    ui.on_close_requested(move || {
        let h = handle_clone2.clone();
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            *h.borrow_mut() = None;
        });
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
                    ui.set_error_text(msg.as_str().into());
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
                            ui.set_error_text(tr_ss("err_token_invalid"));
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
                    ui.set_error_text(msg.as_str().into());
                    ui.set_is_syncing(false);
                }
            });
            return;
        }
    };

    // Drive'da ana klasörü oluştur / bul (Vault başına ayrı klasör)
    let parts: Vec<&str> = sync_code.split('-').collect();
    let (folder_id, hex_key) = if parts.len() == 3 && parts[0] == "cs" {
        (parts[1].to_string(), parts[2].to_string())
    } else {
        // Backward compatibility
        let folder_name = format!("ConnectSync_{}", &sync_code[0..8.min(sync_code.len())]);
        let id = match drive.get_or_create_folder(&folder_name, None).await {
            Ok(id) => id,
            Err(e) => {
                let msg = i18n::tf("err_create_drive_folder", &[("e", &e.to_string())]);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_error_text(msg.as_str().into());
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
            let msg = i18n::tf("err_salt", &[("e", &e.to_string())]);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_error_text(msg.as_str().into());
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
                    ui.set_error_text(msg.as_str().into());
                    ui.set_is_syncing(false);
                }
            });
            return;
        }
    };

    let engine = Arc::new(sync_core::engine::SyncEngine::new_with_arc(
        drive,
        keys,
        folder_id,
        folder.clone(),
    ));

    // Watcher başlat
    let watcher_obj = sync_core::watcher::LocalWatcher::new(
        folder.to_str().unwrap_or(".")
    );
    let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel::<String>(64);
    let _debouncer = watcher_obj.start_watching(watcher_tx).ok();

    // Periyodik timer (Config'den oku, yoksa 5 dk)
    let config = sync_core::config::AppConfig::load();
    let mins = if config.sync_interval_minutes == 0 { 5 } else { config.sync_interval_minutes as u64 };
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(mins * 60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    // Döngü her turda bir kere sync yapıp sonra bekler
    loop {
        // 1. Önce Pull (Drive -> Yerel)
        update_status(&ui_weak, &app_state_loop, &sync_code, &i18n::t("status_pulling"), true);
        tokio::select! {
            _ = &mut stop_rx => {
                update_status(&ui_weak, &app_state_loop, &sync_code, &i18n::t("status_stopped"), false);
                break;
            }
            res = engine.run_sync_pull(&folder, true) => {
                if let Err(e) = res {
                    update_status(&ui_weak, &app_state_loop, &sync_code, &i18n::tf("err_pull", &[("e", &e.to_string())]), false);
                }
            }
        }

        // 2. Sonra Push (Yerel -> Drive)
        tokio::select! {
            _ = &mut stop_rx => {
                update_status(&ui_weak, &app_state_loop, &sync_code, &i18n::t("status_stopped"), false);
                break;
            }
            _ = run_push(&engine, &ui_weak, &app_state_loop, &sync_code) => {}
        }

        tokio::select! {
            _ = &mut stop_rx => {
                update_status(&ui_weak, &app_state_loop, &sync_code, &i18n::t("status_stopped"), false);
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

async fn run_push(engine: &Arc<sync_core::engine::SyncEngine>, ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, sync_code: &str) {
    update_status(ui_weak, app_state, sync_code, &i18n::t("status_pushing"), true);
    match engine.run_sync_push().await {
        Ok(report) => {
            // Kullanıcıya teknik sayaçlar yerine sade bir durum göster
            let msg = if !report.failed_files.is_empty() {
                i18n::tf("status_files_failed", &[("n", &report.failed_files.len().to_string())])
            } else if report.uploaded_files > 0 {
                i18n::tf("status_files_updated", &[("n", &report.uploaded_files.to_string())])
            } else {
                i18n::t("status_up_to_date")
            };
            update_status(ui_weak, app_state, sync_code, &msg, false);
            if !report.failed_files.is_empty() {
                let err = report.failed_files[0].1.clone();
                update_error(ui_weak, app_state, sync_code, &err);
            } else {
                update_error(ui_weak, app_state, sync_code, "");
            }
            
            // Eğer yeni dosyalar yüklendiyse veya ara sıra GC tetiklemek isteniyorsa:
            // Arka planda GC çalıştır (Push akışını bloklamaması için tokio::spawn)
            let engine_clone = engine.clone();
            tokio::spawn(async move {
                let _ = engine_clone.run_gc().await;
            });
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
        let fs = st.folder_states.entry(folder_id.to_string()).or_insert_with(crate::FolderState::default);
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
        let fs = st.folder_states.entry(folder_id.to_string()).or_insert_with(crate::FolderState::default);
        fs.error = err.clone();
        fs.is_syncing = false;
    }
    let uw = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = uw.upgrade() {
            update_ui_folders(&ui, &app_state_clone);
            ui.set_error_text(err.as_str().into()); // Show global toast as well if needed
        }
    });
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

    let icon = load_icon(include_bytes!("../logo.png"));
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
    if let Ok(app_path) = std::env::current_exe() {
        if let Some(s) = app_path.to_str() {
            if let Ok(auto) = auto_launch::AutoLaunchBuilder::new()
                .set_app_name("ConnectSync")
                .set_app_path(s)
                .set_args(&["--autostart"])
                .build()
            {
                if config.auto_start_enabled { let _ = auto.enable(); }
                else { let _ = auto.disable(); }
            }
        }
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

    // 100ms tray timer
    let slint_timer = slint::Timer::default();
    let handle_for_timer = ui_handle.clone();
    let app_state_timer = app_state.clone();
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
                if let tray_icon::TrayIconEvent::Click { button, .. } = event {
                    if button == tray_icon::MouseButton::Left {
                        should_show = true;
                    }
                }
            }

            if should_show {
                let mut handle = handle_for_timer.borrow_mut();
                if handle.is_none() {
                    let new_ui = create_window(handle_for_timer.clone(), app_state_timer.clone());
                    new_ui.show().unwrap();
                    *handle = Some(new_ui);
                } else {
                    let w = handle.as_ref().unwrap();
                    w.show().unwrap();
                    w.window().request_redraw();
                }
            }
        },
    );

    slint::run_event_loop_until_quit().unwrap();
    Ok(())
}
async fn fetch_cloud_folders(ui_weak: slint::Weak<crate::MainWindow>) {
    if let Ok(token) = crate::sync_core::auth::get_drive_token(false).await {
        if let Ok(client) = crate::sync_core::drive::DriveClient::new(token, None) {
            if let Ok(folders) = client.list_cloud_folders().await {
                let mut config = crate::sync_core::config::AppConfig::load();
                let local_codes: std::collections::HashSet<String> = config.sync_folders.iter().map(|f| {
                    if f.code.len() >= 8 { f.code[0..8].to_string() } else { f.code.clone() }
                }).collect();

                let mut cloud_items = Vec::new();
                for (_, name) in folders {
                    let prefix = name.replace("ConnectSync_", "");
                    // Sadece sistemde aktif olmayanları göster
                    if !local_codes.contains(&prefix) {
                        cloud_items.push(crate::SyncFolderItem {
                            id: "".into(),
                            name: prefix.into(),
                            path: "Bulutta".into(),
                            code: "".into(),
                            status: "".into(),
                            is_syncing: false,
                        });
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let model = std::rc::Rc::new(slint::VecModel::<crate::SyncFolderItem>::default());
                        for item in cloud_items {
                            model.push(item);
                        }
                        ui.set_cloud_folders(model.into());
                    }
                });
            }
        }
    }
}

/// Drive'daki ConnectSync klasörlerini + appDataFolder'daki anahtar kaydını okuyup
/// bu PC'nin config'ine ekler. Eklenen sync sayısını döndürür.
async fn scan_cloud_into_config() -> Result<usize, String> {
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

    let mut config = sync_core::config::AppConfig::load();

    // Bu PC'de zaten olan ama bulutta anahtarı kayıtlı olmayan sync'leri kaydet
    // (eski sürümde kayıt sessizce başarısız olmuş olabilir). Kod formatı: cs-{folderId}-{hexKey}
    let local_codes: Vec<String> = config
        .sync_folders
        .iter()
        .map(|f| if f.code.is_empty() { f.id.clone() } else { f.code.clone() })
        .collect();
    for code in local_codes {
        if let Some((fid, hex_key)) = code.strip_prefix("cs-").and_then(|r| r.rsplit_once('-')) {
            if !keys.contains_key(fid) && drive.save_sync_key(fid, hex_key).await.is_ok() {
                keys.insert(fid.to_string(), hex_key.to_string());
            }
        }
    }

    let mut added = 0;
    for (fid, name) in cloud_folders {
        let Some(hex_key) = keys.get(&fid) else { continue };
        let code = format!("cs-{}-{}", fid, hex_key);
        let prefix = format!("cs-{}-", fid);
        if config.sync_folders.iter().any(|f| f.id == code || f.code.starts_with(&prefix) || f.id.starts_with(&prefix)) {
            continue;
        }
        let display_name = name.replace("ConnectSync_", "");
        config.sync_folders.push(sync_core::config::SyncFolder {
            id: code.clone(),
            name: display_name.clone(),
            path: dirs::download_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("ConnectSync")
                .join(&display_name)
                .to_string_lossy()
                .to_string(),
            code,
        });
        added += 1;
    }
    let _ = config.save();
    Ok(added)
}
