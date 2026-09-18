with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# Increase logo size exactly in place
logo_search = r"""                // --- Logo ---\n                spacing: 10px;\n                HorizontalLayout \{\n                    alignment: center;\n                    Image \{\n                        source: @image-url\("\.\./assets/logo\.png"\);\n                        width: 360px;\n                        height: 160px;\n                        image-fit: contain;\n                    \}\n                \}"""
logo_replace = """                // --- Logo ---
                spacing: 10px;
                HorizontalLayout {
                    alignment: center;
                    Image {
                        source: @image-url("../assets/logo.png");
                        width: 400px;
                        height: 180px;
                        image-fit: contain;
                    }
                }"""
text = re.sub(logo_search, logo_replace, text)

with open("ui/main.slint", "w") as f:
    f.write(text)
