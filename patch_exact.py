with open('ui/main.slint', 'r') as f:
    content = f.read()

old_str = """export component MainWindow inherits Window {
    title: "ConnectSync";
    background: #1e1e2e;"""

new_str = """export component MainWindow inherits Window {
    title: "ConnectSync";
    width: 400px;
    height: 560px;
    background: #1e1e2e;"""

content = content.replace(old_str, new_str)

with open('ui/main.slint', 'w') as f:
    f.write(content)
