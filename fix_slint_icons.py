with open("ui/main.slint", "r") as f:
    content = f.read()

import re

# Add copy_feedback_id property
content = re.sub(
    r'in-out property <bool> show_my_syncs: false;',
    r'in-out property <bool> show_my_syncs: false;\n    in-out property <string> copy_feedback_id: "";',
    content
)

# Update copy_to_clipboard signature to take ID as well
content = re.sub(
    r'callback copy_to_clipboard\(string\);',
    r'callback copy_to_clipboard(string, string);',
    content
)

# Replace IconButton
old_icon_btn = """component IconButton inherits Rectangle {
    in-out property <string> text;
    callback clicked;
    width: 30px;
    height: 30px;
    border-radius: 5px;
    background: ta.pressed ? #45475a : (ta.has-hover ? #313244 : transparent);
    
    ta := TouchArea {
        clicked => { root.clicked() }
    }
    Text {
        text: root.text;
        color: white;
        horizontal-alignment: center;
        vertical-alignment: center;
        font-size: 1.2rem;
    }
}"""

new_icon_btn = """component IconButton inherits Rectangle {
    in-out property <image> icon;
    callback clicked;
    width: 30px;
    height: 30px;
    border-radius: 5px;
    background: ta.pressed ? #45475a : (ta.has-hover ? #313244 : transparent);
    
    ta := TouchArea {
        clicked => { root.clicked() }
    }
    Image {
        source: root.icon;
        width: 16px;
        height: 16px;
        colorize: white;
    }
}"""

content = content.replace(old_icon_btn, new_icon_btn)

# Fix back button
content = content.replace('text: "←";', 'icon: @image-url("back.svg");')

# Fix Active Sync Copy button
old_active_copy = """                HorizontalLayout {
                    alignment: center;
                    spacing: 10px;
                    Text { text: "Kod:"; color: #a6adc8; vertical-alignment: center; }
                    Text { text: root.active_sync_code; color: #a6e3a1; font-weight: 700; vertical-alignment: center; font-size: 1.2rem; }
                    IconButton {
                        text: "📋";
                        clicked => { root.copy_to_clipboard(root.active_sync_code) }
                    }
                }"""

new_active_copy = """                VerticalLayout {
                    alignment: center;
                    spacing: 5px;
                    HorizontalLayout {
                        alignment: center;
                        spacing: 10px;
                        ModernButton {
                            text: "Kodu Kopyala";
                            width: 160px;
                            clicked => { root.copy_to_clipboard(root.active_sync_code, "active"); }
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
                }"""

content = content.replace(old_active_copy, new_active_copy)

# Fix List Copy button and remove code text
old_list_copy = """                                    if folder.is_syncing: SmoothSpinner { width: 16px; height: 16px; }
                                    
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
                                    }"""

new_list_copy = """                                    if folder.is_syncing: SmoothSpinner { width: 16px; height: 16px; }
                                    
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
                                    }"""

content = content.replace(old_list_copy, new_list_copy)

with open("ui/main.slint", "w") as f:
    f.write(content)
