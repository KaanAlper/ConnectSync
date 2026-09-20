import re

with open('ui/main.slint', 'r') as f:
    content = f.read()

# find the end of the MainWindow
# We'll insert before the last closing brace of MainWindow
drawer_code = """
        // ── Active Sync Mini Drawer (Right edge) ─────────────────────────
        if (is_syncing && !show_my_syncs && active_sync_folder != ""): Rectangle {
            x: root.width - self.width;
            y: root.height / 2 - self.height / 2;
            width: 48px;
            height: 64px;
            background: #181825cc; // Yari saydam, arka planla uyumlu
            border-top-left-radius: 32px;
            border-bottom-left-radius: 32px;
            border-width: 1px;
            border-color: #313244;
            // Sag kenari cercevesiz gosterip, duvara yapisik hissi vermek icin:
            
            SmoothSpinner {
                x: 12px;
                y: 17px;
            }

            ta_mini := TouchArea {
                clicked => {
                    // Aktif senkronizasyon klasorunun detayina git veya sync sayfasini ac
                    root.show_my_syncs = true;
                }
            }
        }
"""

content = content.rsplit('}', 1)
new_content = content[0] + drawer_code + "\n}\n"

with open('ui/main.slint', 'w') as f:
    f.write(new_content)
