with open("src/main.rs", "r") as f:
    content = f.read()

import re

old_autostart = """    } else {
        println!("Autostart: tray'de çalışıyor...");
        // Önceki sync kaydı varsa arka planda başlat
        if let Some(f) = config.sync_folders.first() { let code = &f.code; let folder = &f.path;
            // Sadece arka plan sync — UI güncellemeleri pencere açılınca yeniden bağlanacak
            let sync_code = code.clone();
            let folder_path = PathBuf::from(folder);
            let app_state_bg = app_state.clone();
            tokio::spawn(async move {
                // Token hazır olana kadar bekle (keyring kilidi açılsın)
                for _ in 0..10 {
                    if sync_core::auth::get_drive_token(false).await.is_ok() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
                {
                    let mut s = app_state_bg.lock().unwrap();
                    if let Some(old) = s.stop_tx.take() { let _ = old.send(()); }
                    s.stop_tx = Some(stop_tx);
                    s.sync_running = true;
                }
                // Boş ui_weak geçiyoruz, hatalar AppState üzerinden loglanıp sonradan UI açıldığında okunur.
                let dummy_ui: slint::Weak<MainWindow> = slint::Weak::default();
                sync_loop_task(dummy_ui, sync_code, folder_path, stop_rx, app_state_bg.clone()).await;
            });
        }
    }"""

new_autostart = """    } else {
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
    }"""

content = content.replace(old_autostart, new_autostart)
with open("src/main.rs", "w") as f:
    f.write(content)
