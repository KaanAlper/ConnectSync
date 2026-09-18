with open("src/main.rs", "r") as f:
    content = f.read()

import re

old_update_status = """fn update_status(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, msg: &str, syncing: bool) {
    let msg = msg.to_string();
    {
        let mut st = app_state.lock().unwrap();
        st.ui_status_text = msg.clone();
        st.ui_is_syncing = syncing;
    }
    let uw = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = uw.upgrade() {
            ui.set_status_text(msg.as_str().into());
            ui.set_is_syncing(syncing);
        }
    });
}"""

new_update_status = """fn update_status(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, folder_id: &str, msg: &str, syncing: bool) {
    let msg = msg.to_string();
    let fid = folder_id.to_string();
    let app_state_clone = app_state.clone();
    {
        let mut st = app_state.lock().unwrap();
        let fs = st.folder_states.entry(folder_id.to_string()).or_insert_with(crate::FolderState::default);
        fs.status = msg.clone();
        fs.is_syncing = syncing;
        fs.error = "".into();
    }
    let uw = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = uw.upgrade() {
            update_ui_folders(&ui, &app_state_clone);
        }
    });
}"""

old_update_error = """fn update_error(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, err: &str) {
    let err = err.to_string();
    {
        let mut st = app_state.lock().unwrap();
        st.ui_error_text = err.clone();
    }
    let uw = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = uw.upgrade() {
            ui.set_error_text(err.as_str().into());
        }
    });
}"""

new_update_error = """fn update_error(ui_weak: &slint::Weak<MainWindow>, app_state: &Arc<Mutex<AppState>>, folder_id: &str, err: &str) {
    let err = err.to_string();
    let fid = folder_id.to_string();
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
}"""

content = content.replace(old_update_status, new_update_status)
content = content.replace(old_update_error, new_update_error)

with open("src/main.rs", "w") as f:
    f.write(content)
