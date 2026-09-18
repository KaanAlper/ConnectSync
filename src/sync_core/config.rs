use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncFolder {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(skip)]
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    pub sync_interval_minutes: u32, 
    pub auto_start_enabled: bool,
    pub sync_folders: Vec<SyncFolder>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sync_interval_minutes: 5,
            auto_start_enabled: false,
            sync_folders: Vec::new(),
        }
    }
}

impl AppConfig {
    fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "ConnectSync", "ConnectSync")
            .map(|proj_dirs| {
                let dir = proj_dirs.config_dir();
                fs::create_dir_all(dir).ok();
                dir.join("config.json")
            })
    }

    pub fn load() -> Self {
        let mut config = Self::default();
        if let Some(path) = Self::config_path() {
            if let Ok(data) = fs::read_to_string(path) {
                if let Ok(loaded) = serde_json::from_str::<Self>(&data) {
                    config = loaded;
                }
            }
        }
        
        // Load codes from keyring
        if let Ok(entry) = keyring::Entry::new("ConnectSync", "sync_codes_v2") {
            if let Ok(data) = entry.get_password() {
                if let Ok(codes_map) = serde_json::from_str::<std::collections::HashMap<String, String>>(&data) {
                    for f in &mut config.sync_folders {
                        if let Some(code) = codes_map.get(&f.id) {
                            f.code = code.clone();
                        }
                    }
                }
            }
        }
        
        config
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(path) = Self::config_path() {
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let temp_path = path.with_extension("json.tmp");
                if fs::write(&temp_path, json).is_ok() {
                    let _ = fs::rename(temp_path, path);
                }
            }
        }
        
        if let Ok(entry) = keyring::Entry::new("ConnectSync", "sync_codes_v2") {
            let mut codes_map = std::collections::HashMap::new();
            for f in &self.sync_folders {
                codes_map.insert(f.id.clone(), f.code.clone());
            }
            if let Ok(json) = serde_json::to_string(&codes_map) {
                let _ = entry.set_password(&json);
            } else {
                let _ = entry.delete_credential();
            }
        }
        
        Ok(())
    }
}
