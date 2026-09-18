import re

with open("ui/main.slint", "r") as f:
    content = f.read()

# Add cloud_folders property
content = content.replace(
    'in-out property <[SyncFolderItem]> sync_folders;',
    'in-out property <[SyncFolderItem]> sync_folders;\n    in-out property <[SyncFolderItem]> cloud_folders;'
)

# Replace the single ScrollView loop with two sections
old_scroll = """                if root.sync_folders.length > 0: ScrollView {
                    width: 100%;
                    VerticalLayout {
                        spacing: 8px;
                        padding-top: 5px;
                        for folder in root.sync_folders : Rectangle {"""

new_scroll = """                if root.sync_folders.length > 0 || root.cloud_folders.length > 0: ScrollView {
                    width: 100%;
                    VerticalLayout {
                        spacing: 8px;
                        padding-top: 5px;
                        
                        if root.sync_folders.length > 0 : Text {
                            text: "Sistemdeki Aktif Sync'ler";
                            color: #a6adc8;
                            font-size: 0.85rem;
                            font-weight: 600;
                            padding-bottom: 4px;
                        }

                        for folder in root.sync_folders : Rectangle {"""

content = content.replace(old_scroll, new_scroll)

# Add cloud folders section at the end of the scroll view
# Find where the local folders loop ends
# It's right before `Rectangle { vertical-stretch: 1; }` (if it exists inside ScrollView, wait, the loop ends with } for the Rectangle item)

# I'll just use regex to insert the cloud folders section after the local folders loop.
