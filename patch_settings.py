import re

with open('ui/main.slint', 'r') as f:
    content = f.read()

# 1. Settings layout'unu guncelle
old_settings_top = """            HorizontalLayout {
                alignment: start;
                IconButton {
                    text: "←";
                    clicked => { root.show_settings = false; }
                }
                Text { text: "  Ayarlar"; font-size: 1.4rem; color: #cdd6f4; font-weight: 700; vertical-alignment: center; }
            }"""

new_settings_top = """            HorizontalLayout {
                height: 40px;
                spacing: 10px;

                VerticalLayout {
                    alignment: center;
                    IconButton {
                        icon: @image-url("back.svg");
                        clicked => { root.show_settings = false; }
                    }
                }

                Text {
                    text: "Ayarlar";
                    color: white;
                    font-size: 1.15rem;
                    font-weight: 700;
                    vertical-alignment: center;
                    overflow: elide;
                    horizontal-stretch: 1;
                }
            }"""

content = content.replace(old_settings_top, new_settings_top)

# 2. Esc key handler (FocusScope) ekle
# `MainWindow inherits Window {` altina ekleyecegiz
focus_scope = """
    forward-focus: fs;
    fs := FocusScope {
        width: 100%; height: 100%;
        key-pressed(event) => {
            if (event.text == "\\u{001b}") {
                if (root.show_settings) {
                    root.show_settings = false;
                    return accept;
                }
                if (root.show_my_syncs) {
                    root.show_my_syncs = false;
                    return accept;
                }
                if (root.show_connect_dialog) {
                    root.show_connect_dialog = false;
                    return accept;
                }
            }
            return reject;
        }
    }
"""

if "forward-focus: fs;" not in content:
    content = content.replace('default-font-family: "Segoe UI";', 'default-font-family: "Segoe UI";\n' + focus_scope)

with open('ui/main.slint', 'w') as f:
    f.write(content)
