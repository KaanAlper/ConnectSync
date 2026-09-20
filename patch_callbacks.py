with open('ui/main.slint', 'r') as f:
    content = f.read()

callbacks = """    callback open_sync_folder(string /*id*/);
    callback manual_sync_requested(string /*id*/);
    callback fully_quit_requested();
    callback remove_sync_folder(string /*id*/);
    
    callback autostart_toggled(bool);
    callback start_in_tray_toggled(bool);
    callback language_changed(string);
"""

content = content.replace("    callback open_sync_folder(string /*id*/);\n    callback manual_sync_requested(string /*id*/);\n    callback fully_quit_requested();\n    callback remove_sync_folder(string /*id*/);", callbacks)

# Ayarlar ekraninda switch toggled olunca callback tetikle
old_switch1 = "checked <=> root.setting_autostart;"
new_switch1 = """checked <=> root.setting_autostart;
                    toggled => { root.autostart_toggled(root.setting_autostart); }"""

old_switch2 = "checked <=> root.setting_start_in_tray;"
new_switch2 = """checked <=> root.setting_start_in_tray;
                    toggled => { root.start_in_tray_toggled(root.setting_start_in_tray); }"""

content = content.replace(old_switch1, new_switch1)
content = content.replace(old_switch2, new_switch2)

with open('ui/main.slint', 'w') as f:
    f.write(content)
