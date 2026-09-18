    // ----- Keys registry in appDataFolder -----
    pub async fn get_sync_keys(&self) -> Result<std::collections::HashMap<String, String>, DriveError> {
        let q = "name = 'connectsync_keys.json' and 'appDataFolder' in parents and trashed = false";
        let url = format!(
            "{API}/files?q={}&spaces=appDataFolder&fields={}",
            urlencoding::encode(q),
            urlencoding::encode("files(id)")
        );
        let list: FileList = self.send(|c| c.get(&url)).await?.json().await?;
        if let Some(f) = list.files.into_iter().next() {
            let data = self.download_chunk(&f.id).await?;
            if let Ok(json) = serde_json::from_slice(&data) {
                return Ok(json);
            }
        }
        Ok(std::collections::HashMap::new())
    }

    pub async fn save_sync_key(&self, folder_id: &str, hex_key: &str) -> Result<(), DriveError> {
        let mut keys = self.get_sync_keys().await.unwrap_or_default();
        keys.insert(folder_id.to_string(), hex_key.to_string());
        
        let q = "name = 'connectsync_keys.json' and 'appDataFolder' in parents and trashed = false";
        let url = format!(
            "{API}/files?q={}&spaces=appDataFolder&fields={}",
            urlencoding::encode(q),
            urlencoding::encode("files(id)")
        );
        let list: FileList = self.send(|c| c.get(&url)).await?.json().await?;
        let file_id = list.files.into_iter().next().map(|f| f.id);
        
        let data = serde_json::to_vec(&keys).unwrap();
        
        if let Some(fid) = file_id {
            // Update
            let url = format!("https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media", fid);
            self.send(|c| c.patch(&url).body(data)).await?;
        } else {
            // Create
            let meta = json!({
                "name": "connectsync_keys.json",
                "parents": ["appDataFolder"]
            });
            let url = format!("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart");
            
            // For multipart we need a proper body, but we can also use simple upload + patch metadata, or multipart.
            // Let's just use multipart boundary manually since it's small.
            let boundary = "foo_bar_baz";
            let body = format!(
                "--{0}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{1}\r\n--{0}\r\nContent-Type: application/json\r\n\r\n{2}\r\n--{0}--",
                boundary,
                serde_json::to_string(&meta).unwrap(),
                serde_json::to_string(&keys).unwrap()
            );
            
            self.send(|c| {
                c.post(&url)
                 .header("Content-Type", format!("multipart/related; boundary={}", boundary))
                 .body(body)
            }).await?;
        }
        Ok(())
    }
