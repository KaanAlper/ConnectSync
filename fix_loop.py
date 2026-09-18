with open("src/main.rs", "r") as f:
    content = f.read()

import re

old_loop_start = re.search(r'    let folder_name = format!\("ConnectSync_\{\}", &sync_code\[0\.\.8\.min\(sync_code\.len\(\)\)\]\);\n    let folder_id = match drive\.get_or_create_folder\(&folder_name, None\)\.await \{.*?\};\n\n    // Drive\'dan ya salt oku ya da üret \(vault\.json\)', content, re.DOTALL).group(0)

new_loop_start = """    let parts: Vec<&str> = sync_code.split('-').collect();
    let (folder_id, hex_key) = if parts.len() == 3 && parts[0] == "cs" {
        (parts[1].to_string(), parts[2].to_string())
    } else {
        // Backward compatibility
        let folder_name = format!("ConnectSync_{}", &sync_code[0..8.min(sync_code.len())]);
        let id = match drive.get_or_create_folder(&folder_name, None).await {
            Ok(id) => id,
            Err(e) => {
                let msg = format!("Drive klasörü oluşturulamadı: {e}");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_error_text(msg.as_str().into());
                        ui.set_is_syncing(false);
                    }
                });
                return;
            }
        };
        (id, sync_code.clone())
    };

    // Drive'dan ya salt oku ya da üret (vault.json)"""

content = content.replace(old_loop_start, new_loop_start)
with open("src/main.rs", "w") as f:
    f.write(content)
