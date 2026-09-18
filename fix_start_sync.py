with open("src/main.rs", "r") as f:
    content = f.read()

import re

old_start = """fn start_sync_loop(
    ui_weak: slint::Weak<MainWindow>,
    app_state: Arc<Mutex<AppState>>,
    sync_code: String,
    folder: PathBuf,
) {
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut state = app_state.lock().unwrap();
        // Önceki döngüyü durdur
        if let Some(old) = state.stop_tx.take() {
            let _ = old.send(());
        }
        state.stop_tx = Some(stop_tx);
        state.sync_running = true;
    }

    tokio::spawn(async move {
        sync_loop_task(ui_weak, sync_code, folder, stop_rx, app_state.clone()).await;
    });
}"""

new_start = """fn start_sync_loop(
    ui_weak: slint::Weak<MainWindow>,
    app_state: Arc<Mutex<AppState>>,
    sync_code: String,
    folder: PathBuf,
) {
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut state = app_state.lock().unwrap();
        if let Some(old) = state.tasks.insert(sync_code.clone(), stop_tx) {
            let _ = old.send(());
        }
    }

    tokio::spawn(async move {
        sync_loop_task(ui_weak, sync_code, folder, stop_rx, app_state.clone()).await;
    });
}"""

content = content.replace(old_start, new_start)
with open("src/main.rs", "w") as f:
    f.write(content)
