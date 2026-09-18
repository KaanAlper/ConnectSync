import re

with open("ui/main.slint", "r") as f:
    content = f.read()

# We need to find the end of the `for folder in root.sync_folders : Rectangle { ... }` block
# The block ends after the Sil/Durdur Butonu Rectangle

cloud_section = """
                        if root.cloud_folders.length > 0 : Text {
                            text: "Buluttaki Diğer Sync'ler (Eşitlenmiyor)";
                            color: #a6adc8;
                            font-size: 0.85rem;
                            font-weight: 600;
                            padding-top: 10px;
                            padding-bottom: 4px;
                        }

                        for folder in root.cloud_folders : Rectangle {
                            background: #1e1e2e;
                            border-radius: 8px;
                            border-width: 1px;
                            border-color: #313244;
                            height: 50px;
                            HorizontalLayout {
                                padding: 10px;
                                spacing: 10px;
                                VerticalLayout {
                                    alignment: center;
                                    Text { text: "Bulut Klasörü: " + folder.name; font-size: 0.95rem; color: #a6adc8; font-weight: 500; overflow: elide; }
                                }
                                Rectangle { horizontal-stretch: 1; }
                                ModernButton {
                                    text: "Bağlan";
                                    width: 100px;
                                    clicked => { root.show_connect_dialog = true; } // Ideally we prepopulate the code
                                }
                            }
                        }
"""

# Insert it after the `ta_remove` block
target = """ta_remove := TouchArea {
                                            clicked => { root.remove_sync_folder(folder.id) }
                                        }
                                    }
                                }
                            }
                        }"""
content = content.replace(target, target + "\n" + cloud_section)

with open("ui/main.slint", "w") as f:
    f.write(content)
