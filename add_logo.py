with open("ui/main.slint", "r") as f:
    content = f.read()

import re

old_main = """            // ── Ana Ekran ──────────────────────────
            if is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code == "": VerticalLayout {
                spacing: 12px;
                alignment: center;

                ModernButton {"""

new_main = """            // ── Ana Ekran ──────────────────────────
            if is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code == "": VerticalLayout {
                spacing: 12px;
                alignment: center;
                
                Image {
                    source: @image-url("../assets/logo.png");
                    width: 180px;
                    height: 90px;
                    image-fit: contain;
                }
                
                Rectangle { height: 10px; } // spacer

                ModernButton {"""

content = content.replace(old_main, new_main)
with open("ui/main.slint", "w") as f:
    f.write(content)
