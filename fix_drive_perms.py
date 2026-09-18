with open("src/sync_core/drive.rs", "r") as f:
    content = f.read()

old_code = """        let created: Created = self
            .send(|c| c.post(&create_url).json(&body))
            .await?
            .json()
            .await?;

        Ok(created.id)"""

new_code = """        let created: Created = self
            .send(|c| c.post(&create_url).json(&body))
            .await?
            .json()
            .await?;

        // Paylaşım ayarını herkese açık yazılabilir yap (dışardan bağlanabilmesi için)
        let perm_url = format!("{API}/files/{}/permissions", created.id);
        let perm_body = serde_json::json!({ "type": "anyone", "role": "writer" });
        let _ = self.send(|c| c.post(&perm_url).json(&perm_body)).await;

        Ok(created.id)"""

content = content.replace(old_code, new_code)
with open("src/sync_core/drive.rs", "w") as f:
    f.write(content)
