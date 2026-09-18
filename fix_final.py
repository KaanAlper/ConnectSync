with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# 1. Add DarkButton component at the top
dark_btn = """component DarkButton inherits Rectangle {
    in property <string> text;
    callback clicked();
    
    height: 40px;
    border-radius: 8px;
    border-width: 1px;
    border-color: ta.has-hover ? #89b4fa : #45475a;
    background: ta.pressed ? #1e1e2e : (ta.has-hover ? #313244 : #181825);
    
    Text {
        text: root.text;
        color: #cdd6f4;
        font-weight: 600;
        font-size: 0.95rem;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
    
    ta := TouchArea {
        clicked => { root.clicked() }
    }
}
"""
text = text.replace("component ModernButton inherits Rectangle {", dark_btn + "\ncomponent ModernButton inherits Rectangle {")

# 2. Change MainWindow background to #1e1e2e (lighter dark)
text = text.replace("background: #11111b;", "background: #1e1e2e;")

# 3. Change the first 2 buttons to DarkButton
buttons_search = r"""                        ModernButton \{\n                            text: "Yeni Bir Sync Klasörü Oluştur";\n                            clicked => \{ root\.create_new_sync\(\) \}\n                        \}\n\n                        ModernButton \{\n                            text: "Bir Sync Koduna Bağlan";\n                            clicked => \{ root\.show_connect_dialog = true; \}\n                        \}"""
buttons_replace = """                        DarkButton {
                            text: "Yeni Bir Sync Klasörü Oluştur";
                            clicked => { root.create_new_sync() }
                        }

                        DarkButton {
                            text: "Bir Sync Koduna Bağlan";
                            clicked => { root.show_connect_dialog = true; }
                        }"""
text = re.sub(buttons_search, buttons_replace, text)

# 4. Make Logo bigger
logo_search = r"""width: 280px;\n                        height: 133px;"""
logo_replace = """width: 360px;
                        height: 160px;"""
text = re.sub(logo_search, logo_replace, text)

with open("ui/main.slint", "w") as f:
    f.write(text)
