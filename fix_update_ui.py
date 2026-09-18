with open("src/main.rs", "r") as f:
    content = f.read()

import re

old_func = re.search(r'pub fn update_ui_folders.*?ui\.set_sync_folders\(model\.into\(\)\);\n}', content, re.DOTALL).group(0)

new_func = """pub fn update_ui_folders(ui: &crate::MainWindow, app_state: &std::sync::Arc<std::sync::Mutex<crate::AppState>>) {
    let config = crate::sync_core::config::AppConfig::load();
    let model = std::rc::Rc::new(slint::VecModel::<crate::SyncFolderItem>::default());
    let state = app_state.lock().unwrap();
    for f in &config.sync_folders {
        let default_state = crate::FolderState::default();
        let fs = state.folder_states.get(&f.id).unwrap_or(&default_state);
        
        model.push(crate::SyncFolderItem {
            id: f.id.clone().into(),
            name: f.name.clone().into(),
            path: f.path.clone().into(),
            code: f.code.clone().into(),
            status: if fs.error.is_empty() { fs.status.as_str().into() } else { fs.error.as_str().into() },
            is_syncing: fs.is_syncing,
        });
    }
    drop(state);
    ui.set_sync_folders(model.into());
}"""

content = content.replace(old_func, new_func)

with open("src/main.rs", "w") as f:
    f.write(content)
