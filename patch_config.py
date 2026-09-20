import re

with open('src/sync_core/config.rs', 'r') as f:
    content = f.read()

old_struct = """pub struct AppConfig {
    pub sync_interval_minutes: u32, 
    pub auto_start_enabled: bool,
    pub sync_folders: Vec<SyncFolder>,
}"""

new_struct = """pub struct AppConfig {
    pub sync_interval_minutes: u32, 
    pub auto_start_enabled: bool,
    pub start_in_tray: bool,
    pub concurrent_threads: u32,
    pub language: String,
    pub sync_folders: Vec<SyncFolder>,
}"""

old_default = """    fn default() -> Self {
        Self {
            sync_interval_minutes: 5,
            auto_start_enabled: false,
            sync_folders: Vec::new(),
        }
    }"""

new_default = """    fn default() -> Self {
        Self {
            sync_interval_minutes: 5,
            auto_start_enabled: false,
            start_in_tray: true,
            concurrent_threads: 4,
            language: "tr".to_string(),
            sync_folders: Vec::new(),
        }
    }"""

content = content.replace(old_struct, new_struct)
content = content.replace(old_default, new_default)

with open('src/sync_core/config.rs', 'w') as f:
    f.write(content)
