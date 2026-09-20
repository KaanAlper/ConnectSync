import re

with open('ui/main.slint', 'r') as f:
    content = f.read()

topbar_gear = """                IconButton {
                    icon: @image-url("settings.svg");
                    clicked => {
                        root.show_connect_dialog = false;
                        root.show_my_syncs = false;
                        root.show_settings = true;
                    }
                }"""

content = content.replace(topbar_gear, "")

# Ensure ana ekran x kuralina !show_settings eklenmis
old_x = """x: (is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code == "") ? 0 :"""
new_x = """x: (is_logged_in && !show_connect_dialog && !show_my_syncs && !show_settings && active_sync_code == "") ? 0 :"""
content = content.replace(old_x, new_x)

floating_gear = """
        // ── Floating Settings Button (Ana Ekranda Sağ Üst) ─────────────────────────
        if (is_logged_in && !show_connect_dialog && !show_my_syncs && !show_settings && active_sync_code == ""): IconButton {
            x: root.width - 45px;
            y: 50px;
            width: 32px;
            height: 32px;
            icon: @image-url("settings.svg");
            clicked => {
                root.show_settings = true;
            }
        }
"""

content = content.rsplit('}', 1)
content = content[0] + floating_gear + "\n}\n"

with open('ui/main.slint', 'w') as f:
    f.write(content)
