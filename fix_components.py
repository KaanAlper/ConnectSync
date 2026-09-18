with open("ui/main.slint", "r") as f:
    text = f.read()

# Fix SmoothSpinner
# We replace the whole SmoothSpinner component
old_spinner = """component SmoothSpinner inherits Rectangle {
    width: 30px;
    height: 30px;
    property <angle> rot: 0deg;
    
    Rectangle {
        width: 100%;
        height: 100%;
        border-radius: parent.width / 2;
        border-width: 3px;
        border-color: #313244;
    }
    
    Path {
        width: 100%;
        height: 100%;
        stroke: #89b4fa;
        stroke-width: 3px;
        viewbox-width: 100;
        viewbox-height: 100;
        MoveTo { x: 50; y: 0; }
        ArcTo { x: 100; y: 50; radius-x: 50; radius-y: 50; sweep: true; }
        
        rotation-angle: rot;
        rotation-origin-x: 50px;
        rotation-origin-y: 50px;
    }
    
    Timer {
        interval: 16ms;
        running: true;
        triggered => { root.rot += 6deg; }
    }
}"""

import re
text = re.sub(r'component SmoothSpinner inherits Rectangle \{.*?    \}\n\}', """component SmoothSpinner inherits Rectangle {
    width: 30px;
    height: 30px;
    property <angle> rot: 0deg;
    
    Timer {
        interval: 16ms;
        running: true;
        triggered => { root.rot += 8deg; }
    }
    
    Image {
        width: 100%;
        height: 100%;
        source: @image-url("spinner.svg");
        transform-rotation: root.rot;
    }
}""", text, flags=re.DOTALL)

# Fix TouchArea has-focus
text = text.replace("ta_input.has-focus", "ta_input.pressed")

with open("ui/main.slint", "w") as f:
    f.write(text)
