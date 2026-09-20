import re

with open('src/main.rs', 'r') as f:
    content = f.read()

autostart_fn = """
use auto_launch::AutoLaunchBuilder;

fn configure_autostart(enable: bool, start_in_tray: bool) {
    let app_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("connectsync"));
    let app_path_str = app_path.to_str().unwrap();
    
    let args = if start_in_tray { &["--autostart", "--tray"] } else { &["--autostart"] };
    
    let auto = match AutoLaunchBuilder::new()
        .set_app_name("ConnectSync")
        .set_app_path(app_path_str)
        .set_use_launch_agent(true)
        .set_args(args)
        .build() {
            Ok(a) => a,
            Err(_) => return,
        };

    if enable {
        let _ = auto.enable();
    } else {
        let _ = auto.disable();
    }
}
"""

if "configure_autostart" not in content:
    content = content.replace("use std::error::Error;", "use std::error::Error;\n" + autostart_fn)

callbacks_rust = """
    // Autostart callbacks
    let app_state_auto = app_state.clone();
    ui.on_autostart_toggled(move |enabled| {
        let mut state = app_state_auto.lock().unwrap();
        state.config.auto_start_enabled = enabled;
        let _ = state.config.save();
        crate::configure_autostart(enabled, state.config.start_in_tray);
    });

    let app_state_tray = app_state.clone();
    ui.on_start_in_tray_toggled(move |enabled| {
        let mut state = app_state_tray.lock().unwrap();
        state.config.start_in_tray = enabled;
        let _ = state.config.save();
        crate::configure_autostart(state.config.auto_start_enabled, enabled);
    });

    let app_state_lang = app_state.clone();
    ui.on_language_changed(move |lang| {
        let mut state = app_state_lang.lock().unwrap();
        state.config.language = lang.to_string();
        let _ = state.config.save();
    });
"""

if "on_autostart_toggled" not in content:
    content = content.replace("ui.on_scan_cloud_syncs(", callbacks_rust + "\n    ui.on_scan_cloud_syncs(")

# initial setup_ui icinde de slint property'leri config'den okuyalim
initial_props = """
    ui.set_setting_autostart(config.auto_start_enabled);
    ui.set_setting_start_in_tray(config.start_in_tray);
"""
if "ui.set_setting_autostart" not in content:
    content = content.replace("ui.set_sync_interval(config.sync_interval_minutes as i32);", "ui.set_sync_interval(config.sync_interval_minutes as i32);\n" + initial_props)


with open('src/main.rs', 'w') as f:
    f.write(content)
