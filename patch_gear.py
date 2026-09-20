with open('ui/main.slint', 'r') as f:
    content = f.read()

gear_button = """                IconButton {
                    text: "⚙";
                    clicked => {
                        // root.show_settings = true;
                    }
                }
"""

# Find the minimize button and insert gear before it
target_minimize = """                IconButton {
                    text: "_";
                    clicked => { root.minimize_requested() }
                }"""

new_content = content.replace(target_minimize, gear_button + target_minimize)

with open('ui/main.slint', 'w') as f:
    f.write(new_content)
