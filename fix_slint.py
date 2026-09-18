with open("ui/main.slint", "r") as f:
    content = f.read()

import re

# Add show_my_syncs to MainWindow properties
content = re.sub(
    r'in-out property <string> active_sync_folder: "";',
    r'in-out property <string> active_sync_folder: "";\n    in-out property <bool> show_my_syncs: false;',
    content
)

# And replace the main screen (if is_logged_in && !show_connect_dialog)
old_main_screen = re.search(r'// ── Ana Ekran \(Liste ve Butonlar\) ──────────────────────────.*?Çıkış Yap".*?\}\n                \}\n            \}', content, re.DOTALL).group(0)

new_main_screen = """// ── Ana Ekran ──────────────────────────
            if is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code == "": VerticalLayout {
                spacing: 12px;
                alignment: center;

                ModernButton {
                    text: "Yeni Bir Sync Klasörü Oluştur";
                    clicked => { root.create_new_sync() }
                }
                ModernButton {
                    text: "Bir Sync Koduna Bağlan";
                    clicked => { root.show_connect_dialog = true; }
                }
                ModernButton {
                    text: "Senkronizasyonlarım";
                    clicked => { root.show_my_syncs = true; }
                }

                Rectangle { height: 10px; }
                Text {
                    text: "Çıkış Yap";
                    color: #f38ba8;
                    font-size: 1.0rem;
                    horizontal-alignment: center;
                    ta_logout1 := TouchArea { clicked => { root.logout_requested() } }
                }
            }

            // ── Aktif Sync Ekranı ──────────────────────────
            if is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code != "": VerticalLayout {
                spacing: 20px;
                alignment: center;
                
                HorizontalLayout {
                    alignment: center;
                    SmoothSpinner { width: 40px; height: 40px; }
                }

                Text { text: root.active_sync_folder; font-size: 1.2rem; font-weight: 700; color: #cdd6f4; horizontal-alignment: center; }
                
                // Active klasörün durumunu listeden bul
                // Slint doesn't let us easily query the array by ID dynamically here without a helper property,
                // so we can use a helper or just display a generic "Arka planda çalışıyor" message, 
                // OR since active_sync_code is set, we can update a `status_text` from Rust when it changes.
                Text { text: root.status_text; font-size: 0.9rem; color: #a6adc8; horizontal-alignment: center; }

                HorizontalLayout {
                    alignment: center;
                    spacing: 10px;
                    Text { text: "Kod:"; color: #a6adc8; vertical-alignment: center; }
                    Text { text: root.active_sync_code; color: #a6e3a1; font-weight: 700; vertical-alignment: center; font-size: 1.2rem; }
                    IconButton {
                        text: "📋";
                        clicked => { root.copy_to_clipboard(root.active_sync_code) }
                    }
                }

                HorizontalLayout {
                    spacing: 10px;
                    alignment: center;
                    ModernButton { text: "Ana Menü"; width: 120px; clicked => { root.active_sync_code = ""; } }
                    ModernButton { text: "Durdur & Sil"; width: 120px; clicked => { root.stop_sync(root.active_sync_code); root.active_sync_code = ""; } }
                }
            }

            // ── Senkronizasyonlarım Listesi ──────────────────────────
            if is_logged_in && !show_connect_dialog && show_my_syncs: VerticalLayout {
                spacing: 10px;
                
                HorizontalLayout {
                    alignment: start;
                    IconButton {
                        text: "←";
                        clicked => { root.show_my_syncs = false; }
                    }
                    Text { text: "  Senkronizasyonlarım"; color: white; font-size: 1.2rem; font-weight: 700; vertical-alignment: center; }
                }

                if root.sync_folders.length == 0: VerticalLayout {
                    alignment: center;
                    Text { text: "Hiç klasörünüz yok."; color: #6c7086; horizontal-alignment: center; }
                }

                if root.sync_folders.length > 0: ScrollView {
                    width: 100%;
                    VerticalLayout {
                        spacing: 8px;
                        padding-top: 5px;
                        for folder in root.sync_folders : Rectangle {
                            background: #313244;
                            border-radius: 8px;
                            height: 64px;
                            
                            ta_folder := TouchArea {
                                clicked => { 
                                    root.active_sync_code = folder.id; 
                                    root.active_sync_folder = folder.name;
                                    root.show_my_syncs = false;
                                }
                            }

                            HorizontalLayout {
                                padding: 10px;
                                spacing: 10px;
                                VerticalLayout {
                                    alignment: center;
                                    width: 180px;
                                    Text { text: folder.name; font-size: 1.0rem; color: #cdd6f4; font-weight: 600; overflow: elide; }
                                    Text { text: folder.path; font-size: 0.75rem; color: #a6adc8; overflow: elide; }
                                    if folder.status != "": Text { text: folder.status; font-size: 0.7rem; color: folder.is_syncing ? #f9e2af : #a6adc8; overflow: elide; }
                                }
                                Rectangle { horizontal-stretch: 1; }
                                HorizontalLayout {
                                    alignment: center;
                                    spacing: 8px;
                                    if folder.is_syncing: SmoothSpinner { width: 16px; height: 16px; }
                                    
                                    // Kopyalama Butonu
                                    Rectangle {
                                        width: 32px;
                                        height: 32px;
                                        border-radius: 4px;
                                        background: ta_copy.pressed ? #a6e3a1 : (ta_copy.has-hover ? #b4befe : transparent);
                                        Text { text: "📋"; vertical-alignment: center; horizontal-alignment: center; font-size: 1.2rem; }
                                        ta_copy := TouchArea {
                                            clicked => { root.copy_to_clipboard(folder.code) }
                                        }
                                    }
                                    // Sil/Durdur Butonu
                                    Rectangle {
                                        width: 32px;
                                        height: 32px;
                                        border-radius: 4px;
                                        background: ta_remove.pressed ? #f38ba8 : (ta_remove.has-hover ? #eba0ac : transparent);
                                        Text { text: "✖"; vertical-alignment: center; horizontal-alignment: center; font-size: 1.2rem; color: ta_remove.has-hover ? #11111b : #f38ba8; }
                                        ta_remove := TouchArea {
                                            clicked => { root.stop_sync(folder.id) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }"""

content = content.replace(old_main_screen, new_main_screen)
with open("ui/main.slint", "w") as f:
    f.write(content)
