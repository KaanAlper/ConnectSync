with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# 1. Fix Logo size so it centers properly
logo_search = r"""                // --- Logo ---\n                spacing: 0px;\n                HorizontalLayout \{\n                    alignment: center;\n                    Image \{\n                        source: @image-url\("\.\./assets/logo\.png"\);\n                        width: 440px;\n                        height: 190px;\n                        image-fit: contain;\n                    \}\n                \}"""
logo_replace = """                // --- Logo ---
                spacing: 0px;
                HorizontalLayout {
                    alignment: center;
                    Image {
                        source: @image-url("../assets/logo.png");
                        width: 400px;
                        height: 190px;
                        image-fit: contain;
                    }
                }"""
text = re.sub(logo_search, logo_replace, text)

# 2. Fix alignments (to push it up)
login_search = r"""            // ── Giriş Ekranı\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: center;"""
login_replace = """            // ── Giriş Ekranı
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: start; padding-top: 10px;"""
text = re.sub(login_search, login_replace, text)

main_search = r"""            // ── Ana Ekran \(Menü\)\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: center;"""
main_replace = """            // ── Ana Ekran (Menü)
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: start; padding-top: 10px;"""
text = re.sub(main_search, main_replace, text)

# 3. Increase the button gap
gap_search = r"""                Rectangle \{ height: 25px; \} // EXTRA GAP ADDED HERE"""
gap_replace = """                Rectangle { height: 40px; } // EXTRA GAP ADDED HERE"""
text = re.sub(gap_search, gap_replace, text)

with open("ui/main.slint", "w") as f:
    f.write(text)
