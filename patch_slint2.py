import re

with open("ui/main.slint", "r") as f:
    content = f.read()

# Replace `if root.show_quit_dialog : Rectangle {` with animated version
old_code = "if root.show_quit_dialog : Rectangle {\n            width: 100%;\n            height: 100%;\n            background: #11111be6;"
new_code = """Rectangle {
            width: 100%;
            height: 100%;
            background: #11111be6;
            opacity: root.show_quit_dialog ? 1.0 : 0.0;
            visible: self.opacity > 0.01;
            animate opacity { duration: 200ms; easing: ease-out; }
            
            // TouchArea to block clicks when visible
            TouchArea {}"""

content = content.replace(old_code, new_code)

with open("ui/main.slint", "w") as f:
    f.write(content)
