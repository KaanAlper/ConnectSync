with open("src/main.rs", "r") as f:
    content = f.read()

old_clip = """    let ui_weak = ui.as_weak();
    ui.on_copy_to_clipboard(move |text| {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(text.to_string());
        }
    });"""

new_clip = """    let ui_weak_clip = ui.as_weak();
    ui.on_copy_to_clipboard(move |code, id| {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(code.to_string());
            if let Some(ui) = ui_weak_clip.upgrade() {
                ui.set_copy_feedback_id(id.clone());
                let ui_weak_timer = ui_weak_clip.clone();
                let id_clone = id.to_string();
                slint::Timer::single_shot(std::time::Duration::from_secs(2), move || {
                    if let Some(ui) = ui_weak_timer.upgrade() {
                        if ui.get_copy_feedback_id().to_string() == id_clone {
                            ui.set_copy_feedback_id("".into());
                        }
                    }
                });
            }
        }
    });"""

content = content.replace(old_clip, new_clip)
with open("src/main.rs", "w") as f:
    f.write(content)
