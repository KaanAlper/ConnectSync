import re

with open('ui/main.slint', 'r') as f:
    content = f.read()

lang_ui = """
            HorizontalLayout {
                Text { text: "Uygulama Dili (Language)"; color: #a6adc8; vertical-alignment: center; }
                Rectangle { width: 10px; }
                Rectangle {
                    width: 100px; height: 30px; border-radius: 8px; background: #313244;
                    Text { text: "Türkçe (TR)"; color: #cdd6f4; horizontal-alignment: center; vertical-alignment: center; font-size: 0.9rem; }
                    TouchArea {
                        clicked => {
                            root.language_changed("en");
                        }
                    }
                }
            }
"""

if "Uygulama Dili" not in content:
    content = content.replace('Text { text: "Sistem & Güncelleme";', lang_ui + '\n            Rectangle { height: 5px; }\n            Text { text: "Sistem & Güncelleme";')

with open('ui/main.slint', 'w') as f:
    f.write(content)
