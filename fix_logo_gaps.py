with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# 1. Update the logo block
logo_search = r"""                // --- Logo ---\n                spacing: 10px;\n                HorizontalLayout \{\n                    alignment: center;\n                    Image \{\n                        source: @image-url\("\.\./assets/logo\.png"\);\n                        width: 360px;\n                        height: 160px;\n                        image-fit: contain;\n                    \}\n                \}"""

logo_replace = """                // --- Logo ---
                spacing: 4px;
                HorizontalLayout {
                    alignment: center;
                    Rectangle {
                        width: 400px;
                        height: 110px;
                        clip: true;
                        Image {
                            source: @image-url("../assets/logo.png");
                            width: 400px;
                            height: 200px;
                            image-fit: contain;
                            y: -45px;
                        }
                    }
                }"""

text = re.sub(logo_search, logo_replace, text)

# 2. Update alignment for Giriş Ekranı to move it up
login_search = r"""            // ── Giriş Ekranı\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: center;"""
login_replace = """            // ── Giriş Ekranı
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: start; padding-top: 50px;"""
text = re.sub(login_search, login_replace, text)

# 3. Update alignment for Ana Ekran to move it up
main_search = r"""            // ── Ana Ekran \(Menü\)\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: center;"""
main_replace = """            // ── Ana Ekran (Menü)
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: start; padding-top: 50px;"""
text = re.sub(main_search, main_replace, text)


with open("ui/main.slint", "w") as f:
    f.write(text)
