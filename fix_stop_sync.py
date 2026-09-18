with open("src/main.rs", "r") as f:
    content = f.read()

import re

old_stop_sync = """    ui.on_stop_sync(move |_id| {
        let mut state = app_state_stop.lock().unwrap();
        for (_, tx) in state.tasks.drain() {
            let _ = tx.send(());
        }
        state.sync_running = false;
        drop(state);

        // Config'den sync kodu temizle
        let mut config = sync_core::config::AppConfig::load();
        config.sync_folders.clear();
        let _ = config.save();

        if let Some(ui) = ui_weak.upgrade() {
            update_ui_folders(&ui, &app_state_stop);
            ui.set_active_sync_code("".into());
            ui.set_status_text("Bağlantı kesildi.".into());
            ui.set_is_syncing(false);
        }
    });"""

new_stop_sync = """    ui.on_stop_sync(move |id| {
        let mut state = app_state_stop.lock().unwrap();
        let id_str = id.to_string();
        if let Some(tx) = state.tasks.remove(&id_str) {
            let _ = tx.send(());
        }
        state.folder_states.remove(&id_str);
        drop(state);

        let mut config = sync_core::config::AppConfig::load();
        config.sync_folders.retain(|f| f.id != id_str);
        let _ = config.save();

        if let Some(ui) = ui_weak.upgrade() {
            update_ui_folders(&ui, &app_state_stop);
        }
    });"""

content = content.replace(old_stop_sync, new_stop_sync)

with open("src/main.rs", "w") as f:
    f.write(content)
