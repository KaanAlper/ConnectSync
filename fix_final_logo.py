with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# Replace Logo block completely
logo_search = r"""                // --- Logo ---\n                spacing: 4px;\n                HorizontalLayout \{\n                    alignment: center;\n                    Rectangle \{\n                        width: 400px;\n                        height: 110px;\n                        clip: true;\n                        Image \{\n                            source: @image-url\("\.\./assets/logo\.png"\);\n                            width: 400px;\n                            height: 200px;\n                            image-fit: contain;\n                            y: -45px;\n                        \}\n                    \}\n                \}"""

logo_replace = """                // --- Logo ---
                spacing: 0px;
                HorizontalLayout {
                    alignment: center;
                    Image {
                        source: @image-url("../assets/logo.png");
                        width: 340px;
                        height: 140px;
                        image-fit: contain;
                    }
                }"""
text = re.sub(logo_search, logo_replace, text)

# Update padding-top for Giriş Ekranı
login_search = r"""            // ── Giriş Ekranı\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: start; padding-top: 50px;"""
login_replace = """            // ── Giriş Ekranı
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: start; padding-top: 20px;"""
text = re.sub(login_search, login_replace, text)

# Update padding-top for Ana Ekran
main_search = r"""            // ── Ana Ekran \(Menü\)\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: start; padding-top: 50px;"""
main_replace = """            // ── Ana Ekran (Menü)
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: start; padding-top: 20px;"""
text = re.sub(main_search, main_replace, text)

with open("ui/main.slint", "w") as f:
    f.write(text)
