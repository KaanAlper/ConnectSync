with open("src/main.rs", "r") as f:
    content = f.read()

old_rem = """    ui.on_remove_sync_folder(move |id| {
        let mut config = sync_core::config::AppConfig::load();
        config.sync_folders.retain(|f| f.id != id.as_str());
        let _ = config.save();
        if let Some(u) = ui_remove.upgrade() {
            update_ui_folders(&u, &app_state_remove);
        }
    });"""

new_rem = """    ui.on_remove_sync_folder(move |id| {
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
    });"""

content = content.replace(old_rem, new_rem)
with open("src/main.rs", "w") as f:
    f.write(content)
