use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub schema_version: u32,
    pub revision: u64,
    pub files: HashMap<String, FileInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
    pub modified_at: u64,
    pub chunks: Vec<[u8; 32]>, // Raw HMAC bytes to save space
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            files: HashMap::new(),
        }
    }
}

impl Manifest {
    pub fn new() -> Self {
        Self::default()
    }
}
