import re

with open('src/main.rs', 'r') as f:
    content = f.read()

old_args = 'let args = if start_in_tray { &["--autostart", "--tray"] } else { &["--autostart"] };'
new_args = 'let args: &[&str] = if start_in_tray { &["--autostart", "--tray"] } else { &["--autostart"] };'

content = content.replace(old_args, new_args)

old_cb = """    // Autostart callbacks
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
    });"""

new_cb = """    // Autostart callbacks
    ui.on_autostart_toggled(move |enabled| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.auto_start_enabled = enabled;
        let _ = cfg.save();
        crate::configure_autostart(enabled, cfg.start_in_tray);
    });

    ui.on_start_in_tray_toggled(move |enabled| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.start_in_tray = enabled;
        let _ = cfg.save();
        crate::configure_autostart(cfg.auto_start_enabled, enabled);
    });

    ui.on_language_changed(move |lang| {
        let mut cfg = crate::sync_core::config::AppConfig::load();
        cfg.language = lang.to_string();
        let _ = cfg.save();
    });"""

content = content.replace(old_cb, new_cb)

with open('src/main.rs', 'w') as f:
    f.write(content)
