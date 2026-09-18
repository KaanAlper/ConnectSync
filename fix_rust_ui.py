with open("src/main.rs", "r") as f:
    content = f.read()

import re

# Add show_my_syncs property init somewhere?
# No need, Slint initializes bool to false.

# Fix update_status to also update root.status_text if it matches active_sync_code
old_status = """fn update_status(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, folder_id: &str, msg: &str, syncing: bool) {
    let fid = folder_id.to_string();
    {
        let mut state = app_state.lock().unwrap();
        let fs = state.folder_states.entry(folder_id.to_string()).or_default();
        fs.status = msg.to_string();
        fs.is_syncing = syncing;
        fs.error.clear();
    }
    if let Some(ui) = ui_weak.upgrade() {
        update_ui_folders(&ui, app_state);
    }
}"""

new_status = """fn update_status(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, folder_id: &str, msg: &str, syncing: bool) {
    let fid = folder_id.to_string();
    {
        let mut state = app_state.lock().unwrap();
        let fs = state.folder_states.entry(folder_id.to_string()).or_default();
        fs.status = msg.to_string();
        fs.is_syncing = syncing;
        fs.error.clear();
    }
    if let Some(ui) = ui_weak.upgrade() {
        update_ui_folders(&ui, app_state);
        if ui.get_active_sync_code().as_str() == folder_id {
            ui.set_status_text(msg.into());
            ui.set_is_syncing(syncing);
        }
    }
}"""

content = content.replace(old_status, new_status)

old_error = """fn update_error(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, folder_id: &str, err: &str) {
    let fid = folder_id.to_string();
    {
        let mut state = app_state.lock().unwrap();
        let fs = state.folder_states.entry(folder_id.to_string()).or_default();
        fs.error = err.to_string();
        fs.is_syncing = false;
    }
    if let Some(ui) = ui_weak.upgrade() {
        update_ui_folders(&ui, app_state);
    }
}"""

new_error = """fn update_error(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, folder_id: &str, err: &str) {
    let fid = folder_id.to_string();
    {
        let mut state = app_state.lock().unwrap();
        let fs = state.folder_states.entry(folder_id.to_string()).or_default();
        fs.error = err.to_string();
        fs.is_syncing = false;
    }
    if let Some(ui) = ui_weak.upgrade() {
        update_ui_folders(&ui, app_state);
        if ui.get_active_sync_code().as_str() == folder_id {
            ui.set_status_text(err.into());
            ui.set_is_syncing(false);
        }
    }
}"""

content = content.replace(old_error, new_error)
with open("src/main.rs", "w") as f:
    f.write(content)
