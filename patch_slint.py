import re

with open("ui/main.slint", "r") as f:
    content = f.read()

# Add properties
content = content.replace(
    'in-out property <bool> show_connect_dialog: false;',
    'in-out property <bool> show_connect_dialog: false;\n    in-out property <bool> show_quit_dialog: false;'
)

content = content.replace(
    'callback manual_sync_requested(string /*id*/);',
    'callback manual_sync_requested(string /*id*/);\n    callback fully_quit_requested();\n    callback remove_sync_folder(string /*id*/);'
)

# Update height logic
content = re.sub(
    r'height: !root\.is_logged_in \? 380px : \(\n\s*root\.show_connect_dialog \? 560px : \(\n\s*root\.sync_folders\.length > 0 \? 540px : 450px\n\s*\)\n\s*\);',
    'height: !root.is_logged_in ? 380px : (root.show_quit_dialog ? 380px : (root.show_connect_dialog ? 450px : (root.sync_folders.length > 0 ? 560px : 450px)));',
    content
)

# Replace close button X
content = re.sub(
    r'text: "✕";\s*clicked => { root\.close_requested\(\) }',
    'text: "✕";\n                    clicked => { root.show_quit_dialog = true; }',
    content
)

# Fix icon sizes
content = content.replace('width: 24px;\n                                        height: 24px;', 'width: 32px;\n                                        height: 32px;')
content = content.replace('font-size: 0.9rem;', 'font-size: 1.2rem;')

# Add Remove button
remove_btn = """
                                    // Sil/Durdur Butonu
                                    Rectangle {
                                        width: 32px;
                                        height: 32px;
                                        border-radius: 4px;
                                        background: ta_remove.pressed ? #f38ba8 : (ta_remove.has-hover ? #eba0ac : transparent);
                                        Text { text: "✖"; vertical-alignment: center; horizontal-alignment: center; font-size: 1.2rem; color: ta_remove.has-hover ? #11111b : #f38ba8; }
                                        ta_remove := TouchArea {
                                            clicked => { root.remove_sync_folder(folder.id) }
                                        }
                                    }
"""
content = content.replace(
    'clicked => { root.manual_sync_requested(folder.id) }\n                                        }\n                                    }',
    'clicked => { root.manual_sync_requested(folder.id) }\n                                        }\n                                    }' + remove_btn
)

# Add Quit overlay at the end before final }
quit_overlay = """
        if root.show_quit_dialog : Rectangle {
            width: 100%;
            height: 100%;
            background: #11111be6;

            Rectangle {
                width: 320px;
                height: 160px;
                background: #1e1e2e;
                border-radius: 12px;
                border-width: 1px;
                border-color: #313244;

                VerticalLayout {
                    padding: 20px;
                    spacing: 15px;
                    alignment: center;

                    Text {
                        text: "Kapatmaktan emin misiniz?";
                        color: #cdd6f4;
                        font-size: 1.1rem;
                        font-weight: 600;
                        horizontal-alignment: center;
                    }

                    Text {
                        text: "Uygulama tamamen sonlandırılacak.";
                        color: #a6adc8;
                        font-size: 0.85rem;
                        horizontal-alignment: center;
                    }

                    HorizontalLayout {
                        spacing: 15px;
                        alignment: center;
                        ModernButton {
                            text: "İptal";
                            width: 100px;
                            clicked => { root.show_quit_dialog = false; }
                        }
                        Rectangle {
                            width: 100px;
                            height: 40px;
                            border-radius: 8px;
                            background: ta_quit.pressed ? #eba0ac : (ta_quit.has-hover ? #f38ba8 : #f38ba8);
                            Text {
                                text: "Kapat";
                                color: #11111b;
                                font-size: 0.95rem;
                                font-weight: 600;
                                horizontal-alignment: center;
                                vertical-alignment: center;
                            }
                            ta_quit := TouchArea {
                                clicked => { root.fully_quit_requested(); }
                            }
                        }
                    }
                }
            }
        }
"""

# Insert right before the last closing brace
last_brace_idx = content.rfind('}')
content = content[:last_brace_idx] + quit_overlay + '\n}'

with open("ui/main.slint", "w") as f:
    f.write(content)
