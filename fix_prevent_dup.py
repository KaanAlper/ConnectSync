with open("src/main.rs", "r") as f:
    content = f.read()

import re

# In on_create_new_sync:
# let mut config = sync_core::config::AppConfig::load();
# we should check if path.to_string_lossy() is already in config.sync_folders

old_create = """        let folder_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Klasör")
            .to_string();

        // Konfigürasyona kaydet
        let mut config = sync_core::config::AppConfig::load();
        config.sync_folders.push(sync_core::config::SyncFolder { id: sync_code.clone(), name: folder_name.clone(), path: path.to_string_lossy().to_string(), code: sync_code.clone() });
        
        if let Some(ui) = ui_weak.upgrade() { update_ui_folders(&ui, &app_state_new); }"""

new_create = """        let folder_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Klasör")
            .to_string();

        // Konfigürasyona kaydet
        let mut config = sync_core::config::AppConfig::load();
        
        // Prevent duplicate path
        if config.sync_folders.iter().any(|f| f.path == path.to_string_lossy().to_string()) {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_error_text("Bu klasör zaten eşitleniyor.".into());
            }
            return;
        }

        config.sync_folders.push(sync_core::config::SyncFolder { id: sync_code.clone(), name: folder_name.clone(), path: path.to_string_lossy().to_string(), code: sync_code.clone() });
        
        if let Some(ui) = ui_weak.upgrade() { update_ui_folders(&ui, &app_state_new); }"""

content = content.replace(old_create, new_create)
with open("src/main.rs", "w") as f:
    f.write(content)
