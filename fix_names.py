with open("ui/main.slint", "r") as f:
    text = f.read()

text = text.replace("struct SyncFolder {", "struct SyncFolderItem {")
text = text.replace("<[SyncFolder]> sync_folders: [];", "<[SyncFolderItem]> sync_folders: [];")
text = text.replace("for folder in root.sync_folders :", "for folder in root.sync_folders :")

# Add cloud_folders
# in-out property <[SyncFolderItem]> sync_folders: [];
text = text.replace("in-out property <[SyncFolderItem]> sync_folders: [];", "in-out property <[SyncFolderItem]> sync_folders: [];\n    in-out property <[SyncFolderItem]> cloud_folders: [];")

with open("ui/main.slint", "w") as f:
    f.write(text)
