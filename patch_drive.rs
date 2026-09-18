    pub async fn list_cloud_folders(&self) -> Result<Vec<(String, String)>, DriveError> {
        let q = "name contains 'ConnectSync_' and mimeType = 'application/vnd.google-apps.folder' and trashed = false";
        let url = format!(
            "{API}/files?q={}&spaces=drive&pageSize=100&fields={}",
            urlencoding::encode(q),
            urlencoding::encode("files(id,name)")
        );

        let list: FileList = self.send(|c| c.get(&url)).await?.json().await?;
        let mut result = Vec::new();
        for f in list.files {
            result.push((f.id, f.name));
        }
        Ok(result)
    }
