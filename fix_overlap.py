with open("ui/main.slint", "r") as f:
    text = f.read()

# Remove the standalone Logo block
standalone_logo_pattern = r"""            // Logo for Login and Main Screen\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: center;\n                x: \(\!is_logged_in \|\| \(\!show_connect_dialog && \!show_my_syncs && active_sync_code == ""\)\) \? 0 : -self\.width;\n                animate x \{ duration: 350ms; easing: cubic-bezier\(0\.2, 0\.8, 0\.2, 1\); \}\n                \n                spacing: 10px;\n                HorizontalLayout \{\n                    alignment: center;\n                    Image \{\n                        source: @image-url\("\.\./assets/logo\.png"\);\n                        width: 280px;\n                        height: 133px;\n                        image-fit: contain;\n                    \}\n                \}\n                Text \{\n                    text: "ConnectSync";\n                    font-size: 1\.8rem;\n                    font-weight: 700;\n                    horizontal-alignment: center;\n                    color: white;\n                \}\n                Text \{\n                    text: "Serverless Folder Sync";\n                    font-size: 1\.1rem;\n                    color: #a6adc8;\n                    horizontal-alignment: center;\n                \}\n                Rectangle \{ height: 2px; \}\n            \}"""

import re
text = re.sub(standalone_logo_pattern, "", text)

# Now, we define a macro/component string to inject into both Giriş and Ana Ekran
logo_slint = """
                // --- Logo ---
                spacing: 10px;
                HorizontalLayout {
                    alignment: center;
                    Image {
                        source: @image-url("../assets/logo.png");
                        width: 280px;
                        height: 133px;
                        image-fit: contain;
                    }
                }
                Text {
                    text: "ConnectSync";
                    font-size: 1.8rem;
                    font-weight: 700;
                    horizontal-alignment: center;
                    color: white;
                }
                Text {
                    text: "Serverless Folder Sync";
                    font-size: 1.1rem;
                    color: #a6adc8;
                    horizontal-alignment: center;
                }
                Rectangle { height: 10px; }
                // ------------
"""

# Insert into Giriş Ekranı
login_screen_search = r"""            // ── Giriş Ekranı\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: center;\n                x: \(\!is_logged_in && \!is_logging_in\) \? 0 : -self\.width;\n                animate x \{ duration: 350ms; easing: cubic-bezier\(0\.2, 0\.8, 0\.2, 1\); \}\n                """
login_screen_replace = """            // ── Giriş Ekranı
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;
                x: (!is_logged_in && !is_logging_in) ? 0 : -self.width;
                animate x { duration: 350ms; easing: cubic-bezier(0.2, 0.8, 0.2, 1); }
                """ + logo_slint
text = re.sub(login_screen_search, login_screen_replace, text)

# Insert into Ana Ekran
main_screen_search = r"""            // ── Ana Ekran \(Menü\)\n            VerticalLayout \{\n                width: parent\.width; height: parent\.height; alignment: center;\n                x: \(is_logged_in && \!show_connect_dialog && \!show_my_syncs && active_sync_code == ""\) \? 0 : \(show_connect_dialog \|\| show_my_syncs \|\| active_sync_code \!\= "" \? -self\.width : self\.width\);\n                animate x \{ duration: 350ms; easing: cubic-bezier\(0\.2, 0\.8, 0\.2, 1\); \}\n                \n                spacing: 10px;"""
main_screen_replace = """            // ── Ana Ekran (Menü)
            VerticalLayout {
                width: parent.width; height: parent.height; alignment: center;
                x: (is_logged_in && !show_connect_dialog && !show_my_syncs && active_sync_code == "") ? 0 : (show_connect_dialog || show_my_syncs || active_sync_code != "" ? -self.width : self.width);
                animate x { duration: 350ms; easing: cubic-bezier(0.2, 0.8, 0.2, 1); }
                """ + logo_slint + """\n                spacing: 10px;"""
text = re.sub(main_screen_search, main_screen_replace, text)

with open("ui/main.slint", "w") as f:
    f.write(text)
