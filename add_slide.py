with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# We want to find the Main Content Area
start_str = "// ── Main Content Area ────────────────────────────────────\n        VerticalLayout {"
end_str = "} // End Main Content Area"

start_idx = text.find(start_str)
end_idx = text.find(end_str)

if start_idx == -1 or end_idx == -1:
    print("Could not find Main Content Area")
    exit(1)

main_content = text[start_idx:end_idx + len(end_str)]

# We will replace `VerticalLayout {` with `Rectangle { clip: true; vertical-stretch: 1;`
# and wrap each `if` block with `VerticalLayout { width: parent.width; height: parent.height; alignment: center; x: ... animate x ... }`
# Actually, since they currently have `if is_logged_in ...`, if we remove `if` they will be rendered always, which is good for animations!

new_main_content = """        // ── Main Content Area ────────────────────────────────────
        Rectangle {
            vertical-stretch: 1;
            clip: true;

            // Logo for Login and Main Screen
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;
                x: (!is_logged_in || (!show_connect_dialog && !show_my_syncs && active_sync_code == "")) ? 0 : -self.width;
                animate x { duration: 350ms; easing: cubic-bezier(0.2, 0.8, 0.2, 1); }
                
                spacing: 5px;
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
                Text {
                    text: "Zero-Knowledge P2P";
                    font-size: 0.8rem;
                    color: #a6adc8;
                    horizontal-alignment: center;
                }
                Rectangle { height: 15px; }
            }

            // ── Giriş Ekranı
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;
                x: (!is_logged_in && !is_logging_in) ? 0 : -self.width;
                animate x { duration: 350ms; easing: cubic-bezier(0.2, 0.8, 0.2, 1); }
                
                ModernButton {
                    text: "Google Drive ile Giriş Yap";
                    clicked => { root.login_requested() }
                }
            }

            if !is_logged_in && is_logging_in: HorizontalLayout {
                width: parent.width; height: parent.height; alignment: center;
                spacing: 20px;
                SmoothSpinner { width: 30px; height: 30px; }
                ModernButton {
                    text: "Tekrar Dene";
                    width: 150px;
                    clicked => { root.login_requested() }
                }
            }

            // ── Ana Ekran (Menü)
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;
                x: (is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code == "") ? 0 : (show_connect_dialog || show_my_syncs || active_sync_code != "" ? -self.width : self.width);
                animate x { duration: 350ms; easing: cubic-bezier(0.2, 0.8, 0.2, 1); }
                
                spacing: 10px;
                // Since this uses parent.width, we need to center the buttons manually.
                HorizontalLayout {
                    alignment: center;
                    VerticalLayout {
                        width: 260px;
                        spacing: 10px;

                        Rectangle {
                            height: 40px;
                            border-radius: 8px;
                            border-width: 1px;
                            border-color: ta_btn1.has-hover ? #89b4fa : #313244;
                            background: ta_btn1.pressed ? #313244 : (ta_btn1.has-hover ? #1e1e2e : #11111b);
                            Text { text: "Yeni Bir Sync Klasörü Oluştur"; color: #cdd6f4; font-size: 0.95rem; font-weight: 600; horizontal-alignment: center; vertical-alignment: center; }
                            ta_btn1 := TouchArea { clicked => { root.create_new_sync() } }
                        }

                        Rectangle {
                            height: 40px;
                            border-radius: 8px;
                            border-width: 1px;
                            border-color: ta_btn2.has-hover ? #89b4fa : #313244;
                            background: ta_btn2.pressed ? #313244 : (ta_btn2.has-hover ? #1e1e2e : #11111b);
                            Text { text: "Bir Sync Koduna Bağlan"; color: #cdd6f4; font-size: 0.95rem; font-weight: 600; horizontal-alignment: center; vertical-alignment: center; }
                            ta_btn2 := TouchArea { clicked => { root.show_connect_dialog = true; } }
                        }

                        Rectangle {
                            height: 40px;
                            border-radius: 8px;
                            background: ta_btn3.pressed ? #74c7ec : (ta_btn3.has-hover ? #89dceb : #89b4fa);
                            Text { text: "Senkronizasyonlarım"; color: #11111b; font-weight: 700; font-size: 0.95rem; horizontal-alignment: center; vertical-alignment: center; }
                            ta_btn3 := TouchArea { clicked => { root.show_my_syncs = true; } }
                        }
                    }
                }
            }

            // ── Aktif Sync Ekranı
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;
                x: (is_logged_in && active_sync_code != "") ? 0 : self.width;
                animate x { duration: 350ms; easing: cubic-bezier(0.2, 0.8, 0.2, 1); }
                
                spacing: 20px;
                
                HorizontalLayout {
                    alignment: center;
                    SmoothSpinner { width: 50px; height: 50px; }
                }

                Text { text: root.active_sync_folder; font-size: 1.4rem; font-weight: 700; color: #cdd6f4; horizontal-alignment: center; }
                Text { text: root.status_text; font-size: 0.95rem; color: #a6adc8; horizontal-alignment: center; }

                VerticalLayout {
                    alignment: center;
                    spacing: 5px;
                    HorizontalLayout {
                        alignment: center;
                        spacing: 10px;
                        Rectangle {
                            width: 160px;
                            height: 36px;
                            border-radius: 18px;
                            border-width: 1px;
                            border-color: #89b4fa;
                            background: ta_acopy.has-hover ? #1e1e2e : transparent;
                            HorizontalLayout {
                                alignment: center;
                                spacing: 8px;
                                Image { source: @image-url("copy.svg"); width: 14px; height: 14px; colorize: #89b4fa; y: 11px; }
                                Text { text: "Kodu Kopyala"; color: #89b4fa; font-weight: 600; font-size: 0.9rem; vertical-alignment: center; }
                            }
                            ta_acopy := TouchArea { clicked => { root.copy_to_clipboard(root.active_sync_code, "active"); } }
                        }
                    }
                    Text {
                        text: "Kopyalandı!";
                        color: #a6e3a1;
                        font-size: 0.85rem;
                        horizontal-alignment: center;
                        opacity: root.copy_feedback_id == "active" ? 1.0 : 0.0;
                        animate opacity { duration: 250ms; }
                    }
                }

                HorizontalLayout {
                    spacing: 15px;
                    alignment: center;
                    ModernButton { text: "Ana Menü"; width: 120px; clicked => { root.active_sync_code = ""; } }
                    ModernButton { text: "Durdur & Sil"; width: 120px; clicked => { root.stop_sync(root.active_sync_code); root.active_sync_code = ""; } }
                }
            }

            // ── Senkronizasyonlarım Listesi
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: start;
                x: (is_logged_in && show_my_syncs) ? 0 : self.width;
                animate x { duration: 350ms; easing: cubic-bezier(0.2, 0.8, 0.2, 1); }
                
                spacing: 10px;
                
                if root.sync_folders.length == 0: VerticalLayout {
                    alignment: center;
                    vertical-stretch: 1;
                    Text { text: "Hiç klasörünüz yok."; color: #6c7086; horizontal-alignment: center; }
                }

                if root.sync_folders.length > 0: ScrollView {
                    width: 100%;
                    VerticalLayout {
                        spacing: 8px;
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
                                    
                                    VerticalLayout {
                                        alignment: center;
                                        Rectangle {
                                            width: 32px;
                                            height: 32px;
                                            border-radius: 4px;
                                            background: ta_copy.pressed ? #a6e3a1 : (ta_copy.has-hover ? #b4befe : transparent);
                                            Image { source: @image-url("copy.svg"); width: 16px; height: 16px; colorize: ta_copy.has-hover ? #11111b : #cdd6f4; }
                                            ta_copy := TouchArea {
                                                clicked => { root.copy_to_clipboard(folder.code, folder.id) }
                                            }
                                        }
                                        Text {
                                            text: "Kopyalandı!";
                                            color: #a6e3a1;
                                            font-size: 0.6rem;
                                            horizontal-alignment: center;
                                            opacity: root.copy_feedback_id == folder.id ? 1.0 : 0.0;
                                            animate opacity { duration: 250ms; }
                                        }
                                    }

                                    Rectangle {
                                        width: 32px;
                                        height: 32px;
                                        border-radius: 4px;
                                        background: ta_remove.pressed ? #f38ba8 : (ta_remove.has-hover ? #eba0ac : transparent);
                                        Image { source: @image-url("trash.svg"); width: 16px; height: 16px; colorize: ta_remove.has-hover ? #11111b : #f38ba8; }
                                        ta_remove := TouchArea {
                                            clicked => { root.stop_sync(folder.id) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Sync kodu giriş dialogu
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;
                x: (is_logged_in && show_connect_dialog) ? 0 : self.width;
                animate x { duration: 350ms; easing: cubic-bezier(0.2, 0.8, 0.2, 1); }
                
                spacing: 10px;

                SyncCodeDialog {
                    confirmed(code) => {
                        root.show_connect_dialog = false;
                        root.connect_to_sync(code);
                    }
                    cancelled => {
                        root.show_connect_dialog = false;
                    }
                }
            }
        } // End Main Content Area"""

text = text.replace(main_content, new_main_content)

with open("ui/main.slint", "w") as f:
    f.write(text)
