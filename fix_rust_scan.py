with open("src/main.rs", "r") as f:
    content = f.read()

import re

# We will inject the handler just before ui.on_remove_sync_folder (line ~150)
old_line = "    ui.on_remove_sync_folder(move |id| {"

new_handler = """    let ui_weak_scan = ui.as_weak();
    let app_state_scan = app_state.clone();
    ui.on_scan_cloud_syncs(move || {
        let ui_weak_bg = ui_weak_scan.clone();
        let app_state_bg = app_state_scan.clone();
        
        if let Some(ui) = ui_weak_bg.upgrade() {
            ui.set_status_text("Drive taranıyor...".into());
        }

        tokio::spawn(async move {
            match sync_core::auth::get_drive_token(true).await {
                Ok(token) => {
                    let drive = sync_core::drive::DriveClient::new(token, None).unwrap();
                    if let Ok(cloud_folders) = drive.list_cloud_folders().await {
                        if let Ok(keys) = drive.get_sync_keys().await {
                            let mut config = sync_core::config::AppConfig::load();
                            let mut added = 0;
                            for (fid, name) in cloud_folders {
                                if let Some(hex_key) = keys.get(&fid) {
                                    let code = format!("cs-{}-{}", fid, hex_key);
                                    if !config.sync_folders.iter().any(|f| f.id == code) {
                                        config.sync_folders.push(sync_core::config::SyncFolder {
                                            id: code.clone(),
                                            name: name.replace("ConnectSync_", ""),
                                            path: "İndirmek için klasör seçin (Manuel)".to_string(), // They will need to pick a path to start!
                                            code: code.clone(),
                                        });
                                        added += 1;
                                    }
                                }
                            }
                            let _ = config.save();
                            
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak_bg.upgrade() {
                                    update_ui_folders(&ui, &app_state_bg);
                                    if added > 0 {
                                        ui.set_status_text(format!("{} senkronizasyon bulundu!", added).as_str().into());
                                    } else {
                                        ui.set_status_text("Yeni senkronizasyon bulunamadı.".into());
                                    }
                                }
                            });
                        }
                    }
                },
                Err(_) => {}
            }
        });
    });

    ui.on_remove_sync_folder(move |id| {"""

content = content.replace(old_line, new_handler)
with open("src/main.rs", "w") as f:
    f.write(content)
