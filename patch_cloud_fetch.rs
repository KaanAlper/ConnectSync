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
