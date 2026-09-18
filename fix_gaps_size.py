with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# 1. Increase Logo size by ~35%
logo_search = r"""                // --- Logo ---\n                spacing: 0px;\n                HorizontalLayout \{\n                    alignment: center;\n                    Image \{\n                        source: @image-url\("\.\./assets/logo\.png"\);\n                        width: 340px;\n                        height: 140px;\n                        image-fit: contain;\n                    \}\n                \}"""
logo_replace = """                // --- Logo ---
                spacing: 0px;
                HorizontalLayout {
                    alignment: center;
                    Image {
                        source: @image-url("../assets/logo.png");
                        width: 440px;
                        height: 190px;
                        image-fit: contain;
                    }
                }"""
text = re.sub(logo_search, logo_replace, text)

# 2. Add gap between "Serverless Folder Sync" and the buttons in Ana Ekran
# The structure is:
#                 Text { text: "Serverless Folder Sync"; ... }
#                 spacing: 10px;
#                 HorizontalLayout { alignment: center; VerticalLayout { width: 260px; spacing: 10px; DarkButton ...
# Wait, actually it's:
#                 Text { text: "Serverless Folder Sync"; ... }
#                 spacing: 10px;
#                 HorizontalLayout {
#                     alignment: center;
#                     VerticalLayout {
#                         width: 260px;
#                         spacing: 10px;
#                         DarkButton {

ana_ekran_search = r"""                Text \{\n                    text: "Serverless Folder Sync";\n                    font-size: 1\.1rem;\n                    color: #a6adc8;\n                    horizontal-alignment: center;\n                \}\n                spacing: 10px;\n                HorizontalLayout \{\n                    alignment: center;\n                    VerticalLayout \{\n                        width: 260px;\n                        spacing: 10px;\n\n                        DarkButton"""

ana_ekran_replace = """                Text {
                    text: "Serverless Folder Sync";
                    font-size: 1.1rem;
                    color: #a6adc8;
                    horizontal-alignment: center;
                }
                
                Rectangle { height: 25px; } // EXTRA GAP ADDED HERE
                
                HorizontalLayout {
                    alignment: center;
                    VerticalLayout {
                        width: 260px;
                        spacing: 10px;

                        DarkButton"""
text = re.sub(ana_ekran_search, ana_ekran_replace, text)


# 3. Change alignment back to center so it distributes evenly
login_search = r"""            // ── Giriş Ekranı\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: start; padding-top: 20px;"""
login_replace = """            // ── Giriş Ekranı
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;"""
text = re.sub(login_search, login_replace, text)

main_search = r"""            // ── Ana Ekran \(Menü\)\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: start; padding-top: 20px;"""
main_replace = """            // ── Ana Ekran (Menü)
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;"""
text = re.sub(main_search, main_replace, text)

with open("ui/main.slint", "w") as f:
    f.write(text)
