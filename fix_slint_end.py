with open("ui/main.slint", "r") as f:
    lines = f.readlines()

# Find the end of `HorizontalLayout` (Status bar)
# Then append the exact quit dialog string.
# Let's find the line `    }` which is the end of MainWindow's VerticalLayout
for i in range(len(lines)):
    if lines[i].strip() == "Rectangle {":
        if "width: 100px;" in lines[i+1]:
            # This is the corrupted quit dialog start!
            corrupt_start = i
            break

quit_dialog = """
        Rectangle {
            width: 100%;
            height: 100%;
            background: #11111be6;
            opacity: root.show_quit_dialog ? 1.0 : 0.0;
            visible: self.opacity > 0.01;
            animate opacity { duration: 200ms; easing: ease-out; }
            
            TouchArea {}

            Rectangle {
                width: 320px;
                height: 160px;
                background: #1e1e2e;
                border-radius: 12px;
                border-width: 1px;
                border-color: #313244;

                VerticalLayout {
                    padding: 20px;
                    spacing: 15px;
                    alignment: center;

                    Text { text: "Kapatmaktan emin misiniz?"; color: #cdd6f4; font-size: 1.1rem; font-weight: 600; horizontal-alignment: center; }
                    Text { text: "Uygulama tamamen sonlandırılacak."; color: #a6adc8; font-size: 0.85rem; horizontal-alignment: center; }

                    HorizontalLayout {
                        spacing: 15px;
                        alignment: center;
                        ModernButton { text: "İptal"; width: 100px; clicked => { root.show_quit_dialog = false; } }
                        Rectangle {
                            width: 100px;
                            height: 40px;
                            border-radius: 8px;
                            background: ta_quit.pressed ? #eba0ac : (ta_quit.has-hover ? #f38ba8 : #f38ba8);
                            Text { text: "Kapat"; color: #11111b; font-size: 0.95rem; font-weight: 600; horizontal-alignment: center; vertical-alignment: center; }
                            ta_quit := TouchArea { clicked => { root.fully_quit_requested(); } }
                        }
                    }
                }
            }
        }
}
"""

new_lines = lines[:corrupt_start]
with open("ui/main.slint", "w") as f:
    f.writelines(new_lines)
    f.write(quit_dialog)
