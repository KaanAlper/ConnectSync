with open("ui/main.slint", "r") as f:
    content = f.read()

import re

# Remove the big hardcoded logo and "ConnectSync" text
old_header = re.search(r'            HorizontalLayout \{\n                alignment: center;\n                Image \{\n                    source: @image-url\("\.\./logo\.png"\);\n.*?\n            Rectangle \{ height: 2px; \}', content, re.DOTALL)
if old_header:
    content = content.replace(old_header.group(0), "")

# We also need to remove the OTHER logo I added in Ana Ekran (assets/logo.png)
old_logo_2 = re.search(r'                Image \{\n                    source: @image-url\("\.\./assets/logo\.png"\);\n                    width: 180px;\n                    height: 90px;\n                    image-fit: contain;\n                \}\n                \n                Rectangle \{ height: 10px; \} // spacer', content, re.DOTALL)
if old_logo_2:
    content = content.replace(old_logo_2.group(0), "")

# Now let's redesign the Ana Ekran
old_ana_ekran = re.search(r'// ── Ana Ekran ──────────────────────────.*?Çıkış Yap".*?\}\n                \}\n            \}', content, re.DOTALL).group(0)

new_ana_ekran = """// ── Ana Ekran ──────────────────────────
            if is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code == "": VerticalLayout {
                spacing: 12px;
                alignment: center;
                padding-top: 20px;

                // Logo only here
                HorizontalLayout {
                    alignment: center;
                    Image {
                        source: @image-url("../assets/logo.png");
                        width: 140px;
                        height: 70px;
                        image-fit: contain;
                    }
                }
                Text {
                    text: "ConnectSync";
                    font-size: 1.5rem;
                    font-weight: 800;
                    horizontal-alignment: center;
                    color: white;
                }
                Rectangle { height: 10px; }

                VerticalLayout {
                    spacing: 8px;
                    alignment: center;
                    width: 260px; // make buttons narrower

                    Rectangle {
                        height: 38px;
                        border-radius: 6px;
                        border-width: 1px;
                        border-color: ta_btn1.has-hover ? #89dceb : #313244;
                        background: ta_btn1.pressed ? #313244 : (ta_btn1.has-hover ? #1e1e2e : #181825);
                        Text { text: "Yeni Bir Sync Klasörü Oluştur"; color: #cdd6f4; font-size: 0.95rem; horizontal-alignment: center; vertical-alignment: center; }
                        ta_btn1 := TouchArea { clicked => { root.create_new_sync() } }
                    }

                    Rectangle {
                        height: 38px;
                        border-radius: 6px;
                        border-width: 1px;
                        border-color: ta_btn2.has-hover ? #89dceb : #313244;
                        background: ta_btn2.pressed ? #313244 : (ta_btn2.has-hover ? #1e1e2e : #181825);
                        Text { text: "Bir Sync Koduna Bağlan"; color: #cdd6f4; font-size: 0.95rem; horizontal-alignment: center; vertical-alignment: center; }
                        ta_btn2 := TouchArea { clicked => { root.show_connect_dialog = true; } }
                    }

                    Rectangle {
                        height: 38px;
                        border-radius: 6px;
                        background: ta_btn3.pressed ? #74c7ec : (ta_btn3.has-hover ? #89dceb : #89b4fa);
                        Text { text: "Senkronizasyonlarım"; color: #11111b; font-weight: 700; font-size: 0.95rem; horizontal-alignment: center; vertical-alignment: center; }
                        ta_btn3 := TouchArea { clicked => { root.show_my_syncs = true; } }
                    }
                }

                Rectangle { height: 10px; }
            }"""

content = content.replace(old_ana_ekran, new_ana_ekran)

# Fix Senkronizasyonlarım view header
old_syncs = re.search(r'// ── Senkronizasyonlarım Listesi ──────────────────────────.*?if root\.sync_folders\.length == 0:', content, re.DOTALL).group(0)

new_syncs = """// ── Senkronizasyonlarım Listesi ──────────────────────────
            if is_logged_in && !show_connect_dialog && show_my_syncs: VerticalLayout {
                spacing: 10px;
                
                HorizontalLayout {
                    alignment: start;
                    IconButton {
                        icon: @image-url("back.svg");
                        clicked => { root.show_my_syncs = false; }
                    }
                    Text { text: "  Senkronizasyonlarım"; color: white; font-size: 1.2rem; font-weight: 700; vertical-alignment: center; }
                    Rectangle { horizontal-stretch: 1; }
                    Rectangle {
                        width: 100px;
                        height: 30px;
                        border-radius: 5px;
                        background: ta_scan.pressed ? #74c7ec : (ta_scan.has-hover ? #89dceb : #89b4fa);
                        Text { text: "Buluttan Bul"; color: #11111b; font-weight: 700; font-size: 0.85rem; horizontal-alignment: center; vertical-alignment: center; }
                        ta_scan := TouchArea { clicked => { root.scan_cloud_syncs() } }
                    }
                }

                if root.sync_folders.length == 0:"""

content = content.replace(old_syncs, new_syncs)

# Move the Çıkış Yap and Status bar out of the Main Content scroll/flow so they are FIXED at the bottom.
# First, remove them from where they are.
# Find the Hata barı, Status bar, Çıkış Yap
# Wait, let's just make the Main Content a ScrollView or stretch it, and put the bottom bar in the outermost VerticalLayout.

with open("ui/main.slint", "w") as f:
    f.write(content)
