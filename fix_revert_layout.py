with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# 1. Restore the alignments to center, no padding-top!
login_search = r"""            // ── Giriş Ekranı\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: start; padding-top: 10px;"""
login_replace = """            // ── Giriş Ekranı
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;"""
text = re.sub(login_search, login_replace, text)

main_search = r"""            // ── Ana Ekran \(Menü\)\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: start; padding-top: 10px;"""
main_replace = """            // ── Ana Ekran (Menü)
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;"""
text = re.sub(main_search, main_replace, text)


# 2. Revert logo size to something beautiful (360x160 is a 30% bump from 280x133)
logo_search = r"""                // --- Logo ---\n                spacing: 0px;\n                HorizontalLayout \{\n                    alignment: center;\n                    Image \{\n                        source: @image-url\("\.\./assets/logo\.png"\);\n                        width: 400px;\n                        height: 190px;\n                        image-fit: contain;\n                    \}\n                \}"""
logo_replace = """                // --- Logo ---
                spacing: 10px;
                HorizontalLayout {
                    alignment: center;
                    Image {
                        source: @image-url("../assets/logo.png");
                        width: 360px;
                        height: 160px;
                        image-fit: contain;
                    }
                }"""
text = re.sub(logo_search, logo_replace, text)

# 3. Reduce the crazy 40px gap I added, let's make it 25px again
gap_search = r"""                Rectangle \{ height: 40px; \} // EXTRA GAP ADDED HERE"""
gap_replace = """                Rectangle { height: 25px; }"""
text = re.sub(gap_search, gap_replace, text)

with open("ui/main.slint", "w") as f:
    f.write(text)
