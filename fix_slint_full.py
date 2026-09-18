import re

with open("ui/main.slint", "r") as f:
    content = f.read()

# We need to find the start of VerticalLayout { padding: 15px; inside MainWindow
start_idx = content.find("    VerticalLayout {\n        padding: 15px;")
if start_idx == -1:
    print("Could not find start!")
    exit(1)

# Find the end of it
# We know it ends before the dummy Window or at the end.
end_idx = content.rfind("        Rectangle {") # The quit dialog
if end_idx == -1:
    end_idx = len(content)

new_layout = """    VerticalLayout {
        padding: 15px;

        // ── Custom Titlebar ──────────────────────────────────────
        HorizontalLayout {
            height: 30px;
            alignment: space-between;

            TouchArea {
                width: parent.width - 70px;
            }

            HorizontalLayout {
                spacing: 8px;
                alignment: end;
                IconButton {
                    text: "—";
                    clicked => { root.minimize_requested() }
                }
                IconButton {
                    text: "✕";
                    clicked => { root.show_quit_dialog = true; }
                }
            }
        }

        // ── Dynamic Top Bar (For lists) ─────────────────────────
        if is_logged_in && show_my_syncs: HorizontalLayout {
            alignment: start;
            height: 40px;
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

        // ── Main Content Area ────────────────────────────────────
        VerticalLayout {
            vertical-stretch: 1;
            alignment: center;
            padding-left: 25px;
            padding-right: 25px;

            // Logo for Login and Main Screen
            if (!is_logged_in) || (is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code == ""): VerticalLayout {
                alignment: center;
                spacing: 5px;
                Image {
                    source: @image-url("../assets/logo.png");
                    width: 140px;
                    height: 70px;
                    image-fit: contain;
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
            if !is_logged_in && !is_logging_in: VerticalLayout {
                alignment: center;
                ModernButton {
                    text: "Google Drive ile Giriş Yap";
                    clicked => { root.login_requested() }
                }
            }

            if !is_logged_in && is_logging_in: HorizontalLayout {
                alignment: center;
                spacing: 20px;
                SmoothSpinner { width: 30px; height: 30px; }
                ModernButton {
                    text: "Tekrar Dene";
                    width: 150px;
                    clicked => { root.login_requested() }
                }
            }

            // ── Ana Ekran (Menü)
            if is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code == "": VerticalLayout {
                spacing: 10px;
                alignment: center;
                width: 260px;

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

            // ── Aktif Sync Ekranı
            if is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code != "": VerticalLayout {
                spacing: 20px;
                alignment: center;
                
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
            if is_logged_in && !show_connect_dialog && show_my_syncs: VerticalLayout {
                spacing: 10px;
                
                if root.sync_folders.length == 0: VerticalLayout {
                    alignment: center;
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
            if is_logged_in && show_connect_dialog: VerticalLayout {
                alignment: center;
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
        } // End Main Content

        // ── Hata barı (Absolute at bottom if needed)
        if root.error_text != "": Rectangle {
            background: #f38ba820;
            border-radius: 4px;
            height: 28px;
            Text {
                text: root.error_text;
                color: #f38ba8;
                font-size: 0.78rem;
                horizontal-alignment: center;
                vertical-alignment: center;
                overflow: elide;
            }
        }

        // ── Absolute Bottom Bar ─────────────────────────────────
        HorizontalLayout {
            height: 20px;
            alignment: space-between;

            // Left side: Çıkış Yap
            HorizontalLayout {
                alignment: start;
                if is_logged_in: Text {
                    text: "Çıkış Yap";
                    color: ta_logout.has-hover ? #f38ba8 : #7f849c;
                    font-size: 0.85rem;
                    vertical-alignment: center;
                    ta_logout := TouchArea { clicked => { root.logout_requested() } }
                }
            }

            // Right side: Status
            HorizontalLayout {
                alignment: end;
                spacing: 8px;
                
                if !root.is_logged_in : HorizontalLayout {
                    spacing: 3px;
                    alignment: center;
                    Rectangle { width: 5px; height: 15px; Rectangle { width: 5px; height: 5px; border-radius: 2.5px; background: white; y: 5px + sin(animation-tick() / 1s * 360deg) * 4px; } }
                    Rectangle { width: 5px; height: 15px; Rectangle { width: 5px; height: 5px; border-radius: 2.5px; background: white; y: 5px + sin(animation-tick() / 1s * 360deg - 60deg) * 4px; } }
                    Rectangle { width: 5px; height: 15px; Rectangle { width: 5px; height: 5px; border-radius: 2.5px; background: white; y: 5px + sin(animation-tick() / 1s * 360deg - 120deg) * 4px; } }
                }

                if root.is_logged_in && !root.is_syncing : Text {
                    text: "✓";
                    color: white;
                    font-size: 1.1rem;
                    font-weight: 800;
                    vertical-alignment: center;
                }

                if root.is_logged_in && root.is_syncing : Rectangle {
                    width: 8px;
                    height: 8px;
                    border-radius: 4px;
                    background: #f9e2af;
                    y: 6px;
                }
                Text {
                    text: root.is_logged_in
                        ? (root.sync_folders.length > 0
                            ? (root.is_syncing ? "Eşitleniyor" : "Drive Bağlı")
                            : "Drive Bağlı")
                        : (root.is_logging_in ? "Bağlanıyor..." : "Giriş Bekleniyor");
                    color: #a6adc8;
                    font-size: 0.85rem;
                    vertical-alignment: center;
                }
            }
        }
    }
"""

new_content = content[:start_idx] + new_layout + "\n" + content[end_idx:]

with open("ui/main.slint", "w") as f:
    f.write(new_content)
