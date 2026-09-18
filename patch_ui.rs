pub fn update_ui_folders(ui: &crate::MainWindow, app_state: &std::sync::Arc<std::sync::Mutex<crate::AppState>>) {
    let config = crate::sync_core::config::AppConfig::load();
    let model = std::rc::Rc::new(slint::VecModel::<crate::SyncFolderItem>::default());
    let state = app_state.lock().unwrap();
    let status_str: slint::SharedString = state.ui_status_text.as_str().into();
    let is_sync = state.ui_is_syncing;
    drop(state);

    for f in &config.sync_folders {
        model.push(crate::SyncFolderItem {
            id: f.id.clone().into(),
            name: f.name.clone().into(),
            path: f.path.clone().into(),
            code: f.code.clone().into(),
            status: status_str.clone(),
            is_syncing: is_sync,
        });
    }
    ui.set_sync_folders(model.into());
}
