with open("ui/main.slint", "r") as f:
    content = f.read()

import re

old_icon_btn = """component IconButton inherits Rectangle {
    in-out property <image> icon;
    callback clicked;
    width: 30px;
    height: 30px;
    border-radius: 5px;
    background: ta.pressed ? #45475a : (ta.has-hover ? #313244 : transparent);
    
    ta := TouchArea {
        clicked => { root.clicked() }
    }
    Image {
        source: root.icon;
        width: 16px;
        height: 16px;
        colorize: white;
    }
}"""

new_icon_btn = """component IconButton inherits Rectangle {
    in-out property <string> text: "";
    in-out property <image> icon;
    callback clicked;
    width: 30px;
    height: 30px;
    border-radius: 5px;
    background: ta.pressed ? #45475a : (ta.has-hover ? #313244 : transparent);
    
    ta := TouchArea {
        clicked => { root.clicked() }
    }
    if root.text != "": Text {
        text: root.text;
        color: white;
        horizontal-alignment: center;
        vertical-alignment: center;
        font-size: 1.2rem;
    }
    if root.text == "": Image {
        source: root.icon;
        width: 16px;
        height: 16px;
        colorize: white;
    }
}"""

content = content.replace(old_icon_btn, new_icon_btn)

with open("ui/main.slint", "w") as f:
    f.write(content)
