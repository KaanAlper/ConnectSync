with open("ui/main.slint", "r") as f:
    content = f.read()

import re

# Add scan_cloud_syncs callback
content = re.sub(
    r'callback connect_to_sync\(string\);',
    r'callback connect_to_sync(string);\n    callback scan_cloud_syncs();',
    content
)

# Add "Buluttan İndir" button inside Senkronizasyonlarım list
old_syncs = re.search(r'// ── Senkronizasyonlarım Listesi ──────────────────────────.*?Text \{ text: "  Senkronizasyonlarım"; color: white; font-size: 1\.2rem; font-weight: 700; vertical-alignment: center; \}', content, re.DOTALL).group(0)

new_syncs = """// ── Senkronizasyonlarım Listesi ──────────────────────────
            if is_logged_in && !show_connect_dialog && show_my_syncs: VerticalLayout {
                spacing: 10px;
                
                HorizontalLayout {
                    alignment: start;
                    IconButton {
                        text: "←";
                        clicked => { root.show_my_syncs = false; }
                    }
                    Text { text: "  Senkronizasyonlarım"; color: white; font-size: 1.2rem; font-weight: 700; vertical-alignment: center; }
                    Rectangle { horizontal-stretch: 1; }
                    ModernButton {
                        text: "Buluttan Bul";
                        width: 120px;
                        clicked => { root.scan_cloud_syncs() }
                    }
                }"""

content = content.replace(old_syncs, new_syncs)

with open("ui/main.slint", "w") as f:
    f.write(content)
