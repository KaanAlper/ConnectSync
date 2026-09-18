import sys

with open("ui_top.slint", "r") as f:
    top = f.read()

# Find the end of properties and callbacks inside MainWindow
idx = top.find("    VerticalLayout {\n        padding: 15px;")
if idx == -1:
    print("Could not find VerticalLayout inside MainWindow!")
    sys.exit(1)

top = top[:idx]

with open("ui/main.slint", "r") as f:
    current = f.read()

# Find where the VerticalLayout starts in current ui/main.slint
body_idx = current.find("    VerticalLayout {\n        padding: 15px;")
if body_idx == -1:
    print("Could not find body in ui/main.slint!")
    sys.exit(1)

body = current[body_idx:]

with open("ui/main.slint", "w") as f:
    f.write(top)
    f.write(body)
