with open('ui/main.slint', 'r') as f:
    content = f.read()

old_str = """export component MainWindow inherits Window {
    title: "ConnectSync";
    width: 400px;
    height: 560px;"""

new_str = """export component MainWindow inherits Window {
    title: "ConnectSync";
    min-width: 400px;
    min-height: 560px;"""

content = content.replace(old_str, new_str)

with open('ui/main.slint', 'w') as f:
    f.write(content)
