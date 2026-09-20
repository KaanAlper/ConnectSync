import re

with open('ui/main.slint', 'r') as f:
    content = f.read()

# 1. Component ekleyelim (dosya basina)
switch_component = """
component Switch inherits Rectangle {
    in-out property <bool> checked: false;
    in property <bool> enabled: true;
    callback toggled();
    
    width: 40px;
    height: 20px;
    border-radius: 10px;
    background: !enabled ? #31324480 : (checked ? #a6e3a1 : #45475a);
    
    Rectangle {
        width: 16px;
        height: 16px;
        border-radius: 8px;
        background: root.enabled ? #11111b : #181825;
        x: checked ? root.width - self.width - 2px : 2px;
        y: 2px;
        animate x { duration: 150ms; easing: ease-in-out; }
    }
    
    TouchArea {
        clicked => { 
            if (root.enabled) { 
                root.checked = !root.checked; 
                root.toggled(); 
            } 
        }
    }
}
"""

if "component Switch inherits" not in content:
    # insert after the imports
    parts = content.split("export struct SyncFolderItem", 1)
    content = parts[0] + switch_component + "\nexport struct SyncFolderItem" + parts[1]

# 2. Add properties
props = """    in-out property <bool> setting_autostart: false;
    in-out property <bool> setting_start_in_tray: true;"""
if "setting_autostart" not in content:
    content = content.replace("in-out property <bool> show_settings: false;", "in-out property <bool> show_settings: false;\n" + props)

# 3. Ayarlar (Settings) kismina ekle
startup_ui = """
            Text { text: "Başlangıç"; color: #89b4fa; font-size: 1rem; font-weight: 700; }
            HorizontalLayout {
                Text { text: "Bilgisayar açıldığında otomatik başlat"; color: #cdd6f4; vertical-alignment: center; }
                Rectangle { width: 10px; }
                Switch { 
                    checked <=> root.setting_autostart; 
                }
            }
            HorizontalLayout {
                opacity: root.setting_autostart ? 1.0 : 0.5;
                Text { text: "Arka planda (Sistem tepsisinde) başlat"; color: #a6adc8; vertical-alignment: center; }
                Rectangle { width: 10px; }
                Switch { 
                    enabled: root.setting_autostart;
                    checked <=> root.setting_start_in_tray; 
                }
            }
            Rectangle { height: 10px; }
"""

if "Başlangıç" not in content:
    content = content.replace('Text { text: "Performans"; color: #89b4fa; font-size: 1rem; font-weight: 700; }', startup_ui + '\n            Text { text: "Performans"; color: #89b4fa; font-size: 1rem; font-weight: 700; }')

with open('ui/main.slint', 'w') as f:
    f.write(content)
