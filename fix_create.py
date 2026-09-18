with open("src/main.rs", "r") as f:
    content = f.read()

import re

old_create = re.search(r'// ── Yeni Sync Oluştur ─────────────────────────────────────────────────\n.*?start_sync_loop\(ui_weak\.clone\(\), app_state_new\.clone\(\), sync_code, path\);\n    \}\);', content, re.DOTALL).group(0)

new_create = """// ── Yeni Sync Oluştur ─────────────────────────────────────────────────
    let ui_weak = ui.as_weak();
    let app_state_new = app_state.clone();
    ui.on_create_new_sync(move || {
        let Some(path) = FileDialog::new()
            .set_title("Sync edilecek klasörü seçin")
            .pick_folder()
        else { return };

        let mut raw = [0u8; 16];
        if getrandom::fill(&mut raw).is_err() {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_error_text("Sync kodu üretilemedi.".into());
            }
            return;
        }
        let hex_key = hex::encode(&raw);
        let folder_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Klasör")
            .to_string();

        let mut config = sync_core::config::AppConfig::load();
        if config.sync_folders.iter().any(|f| f.path == path.to_string_lossy().to_string()) {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_error_text("Bu klasör zaten eşitleniyor.".into());
            }
            return;
        }

        // Show a loading UI directly on main screen before entering Active View?
        // Or we can enter Active View with a temporary code, and update it later.
        // Let's just enter Active View with "Bekleniyor..."
        let temp_id = hex_key.clone(); // use hex_key temporarily
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_active_sync_code(temp_id.clone().as_str().into());
            ui.set_active_sync_folder(folder_name.as_str().into());
            ui.set_status_text("Drive klasörü oluşturuluyor...".into());
        }

        let ui_weak_bg = ui_weak.clone();
        let app_state_bg = app_state_new.clone();
        tokio::spawn(async move {
            match sync_core::auth::get_drive_token(true).await {
                Ok(token) => {
                    let drive = sync_core::drive::DriveClient::new(token);
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
                            
                            if let Some(ui) = ui_weak_bg.upgrade() {
                                update_ui_folders(&ui, &app_state_bg);
                                ui.set_active_sync_code(universal_code.clone().as_str().into());
                                ui.set_status_text("Sync başlatılıyor...".into());
                            }
                            
                            start_sync_loop(ui_weak_bg, app_state_bg, universal_code, path);
                        },
                        Err(e) => {
                            if let Some(ui) = ui_weak_bg.upgrade() {
                                ui.set_error_text(format!("Drive klasörü oluşturulamadı: {}", e).as_str().into());
                                ui.set_active_sync_code("".into());
                            }
                        }
                    }
                },
                Err(_) => {
                    if let Some(ui) = ui_weak_bg.upgrade() {
                        ui.set_error_text("Google Drive girişi yapılamadı.".into());
                        ui.set_active_sync_code("".into());
                    }
                }
            }
        });
    });"""

content = content.replace(old_create, new_create)
with open("src/main.rs", "w") as f:
    f.write(content)
