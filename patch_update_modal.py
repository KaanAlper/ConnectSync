with open('ui/main.slint', 'r') as f:
    content = f.read()

# 1. State değişkenini ekle
state_prop = "    in-out property <bool> show_settings: false;"
new_state_prop = state_prop + "\n    in-out property <bool> show_update_dialog: false;"
content = content.replace(state_prop, new_state_prop)

# 2. Ayarlar menüsündeki "Güncellemeleri Denetle" butonunu tetikleyici yapalim (Test amacli)
old_update_btn = """            DarkButton {
                text: "Güncellemeleri Denetle";
                clicked => { /* Check for updates */ }
            }"""
new_update_btn = """            DarkButton {
                text: "Güncellemeleri Denetle";
                clicked => { 
                    // TEST: Popup'i gostermek icin
                    root.show_update_dialog = true; 
                }
            }"""
content = content.replace(old_update_btn, new_update_btn)

# 3. Modal kodunu hazirla
modal_code = """
        // ── Update Ready Modal Dialog ─────────────────────────
        if show_update_dialog: TouchArea {
            width: 100%;
            height: 100%;
            
            Rectangle {
                width: 100%;
                height: 100%;
                background: #11111b99; // Karartma (Overlay)
            }
            
            Rectangle {
                width: 320px;
                height: 180px;
                background: #1e1e2e;
                border-radius: 12px;
                border-width: 1px;
                border-color: #313244;
                drop-shadow-blur: 15px;
                drop-shadow-color: #00000080;
                
                VerticalLayout {
                    padding: 24px;
                    spacing: 16px;
                    alignment: center;
                    
                    Text {
                        text: "🎉 Güncelleme Hazır!";
                        font-size: 1.2rem;
                        color: #cdd6f4;
                        font-weight: 700;
                        horizontal-alignment: center;
                    }
                    Text {
                        text: "Arka planda yeni sürüm başarıyla indirildi. Uygulamak için ConnectSync'i yeniden başlatmak ister misiniz?";
                        font-size: 0.9rem;
                        color: #a6adc8;
                        horizontal-alignment: center;
                        wrap: word-wrap;
                    }
                    HorizontalLayout {
                        spacing: 16px;
                        alignment: center;
                        height: 40px;
                        DarkButton {
                            text: "Daha Sonra";
                            width: 120px;
                            clicked => { root.show_update_dialog = false; }
                        }
                        ModernButton {
                            text: "Yeniden Başlat";
                            width: 130px;
                            clicked => { 
                                // root.fully_quit_requested(); // Veya restart event'i
                                root.show_update_dialog = false;
                            }
                        }
                    }
                }
            }
        }
"""

# MainWindow'un sonuna ekleyelim (en ustte cizilsin diye)
content = content.rsplit('}', 1)
new_content = content[0] + modal_code + "\n}\n"

with open('ui/main.slint', 'w') as f:
    f.write(new_content)
