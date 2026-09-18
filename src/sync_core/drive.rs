//! Google Drive istemcisi.
//!
//! - Her HTTP yanıtı kontrol edilir (401/403/429/5xx artık "başarılı" sayılmaz)
//! - 429, 5xx, rate-limit 403'leri ve ağ hataları için exponential backoff
//! - 401'de token yenileme (tek seferde, paralel isteklerde çakışmadan)
//! - Klasör içeriği `isim -> file_id` cache'i olarak tutulur (var mı kontrolü + restore)
//! - Drive'ın beklediği `multipart/related` gövdesi elle kurulur

use reqwest::{Client, RequestBuilder, Response, StatusCode, header};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_API: &str = "https://www.googleapis.com/upload/drive/v3";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
const BOUNDARY: &str = "connectsync_9f2b7c41d8e5a360";
const MAX_RETRIES: u32 = 6;

// ---------------------------------------------------------------------------
// Hata tipi (Send + Sync, engine'deki `Box<dyn Error>`'a `?` ile dönüşür)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum DriveError {
    Http(reqwest::Error),
    Api { status: u16, body: String },
    Auth(String),
    Protocol(String),
}

impl fmt::Display for DriveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DriveError::Http(e) => write!(f, "HTTP hatası: {e}"),
            DriveError::Api { status, body } => write!(f, "Drive API hatası ({status}): {body}"),
            DriveError::Auth(m) => write!(f, "Yetkilendirme hatası: {m}"),
            DriveError::Protocol(m) => write!(f, "Beklenmeyen yanıt: {m}"),
        }
    }
}

impl Error for DriveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DriveError::Http(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for DriveError {
    fn from(e: reqwest::Error) -> Self {
        DriveError::Http(e)
    }
}

// ---------------------------------------------------------------------------
// Token sağlayıcı: 401 alınca yeni token almak için çağrılır
// ---------------------------------------------------------------------------

pub type TokenFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
pub type TokenProvider = Arc<dyn Fn() -> TokenFuture + Send + Sync>;

// ---------------------------------------------------------------------------
// API yanıt tipleri
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct FileList {
    #[serde(default)]
    files: Vec<FileEntry>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct FileEntry {
    id: String,
    name: String,
    #[serde(rename = "createdTime")]
    created_time: Option<String>,
}

#[derive(Deserialize)]
struct Created {
    id: String,
}

// ---------------------------------------------------------------------------
// İstemci
// ---------------------------------------------------------------------------

pub struct DriveClient {
    client: Client,
    token: RwLock<String>,
    provider: Option<TokenProvider>,
    refresh_lock: tokio::sync::Mutex<()>,
    /// Hedef klasördeki `dosya adı -> FileEntry` haritası
    index: Mutex<HashMap<String, FileEntry>>,
}

impl DriveClient {
    pub fn new(access_token: String, provider: Option<TokenProvider>) -> Result<Self, DriveError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .build()?;

        Ok(Self {
            client,
            token: RwLock::new(access_token),
            provider,
            refresh_lock: tokio::sync::Mutex::new(()),
            index: Mutex::new(HashMap::new()),
        })
    }

    // ----- düşük seviye: retry + token yenileme --------------------------------

    /// İsteği `build` ile her denemede yeniden kurar (multipart gövdeler tekrar kullanılamaz).
    async fn send<F>(&self, build: F) -> Result<Response, DriveError>
    where
        F: Fn(&Client) -> RequestBuilder,
    {
        let mut attempt: u32 = 0;
        let mut refreshed = false;

        loop {
            let token = self.token.read().unwrap().clone();
            let result = build(&self.client).bearer_auth(&token).send().await;

            match result {
                Ok(resp) if resp.status().is_success() => return Ok(resp),
                Ok(resp) => {
                    let status = resp.status();
                    let retry_after = parse_retry_after(&resp);
                    let body = resp.text().await.unwrap_or_default();

                    if status == StatusCode::UNAUTHORIZED && !refreshed && self.provider.is_some() {
                        refreshed = true;
                        self.refresh_token(&token).await?;
                        continue;
                    }

                    if is_retryable(status, &body) && attempt < MAX_RETRIES {
                        tokio::time::sleep(retry_after.unwrap_or_else(|| backoff(attempt))).await;
                        attempt += 1;
                        continue;
                    }

                    return Err(DriveError::Api {
                        status: status.as_u16(),
                        body: body.chars().take(500).collect(),
                    });
                }
                Err(e) => {
                    let transient = e.is_timeout() || e.is_connect() || e.is_request();
                    if transient && attempt < MAX_RETRIES {
                        tokio::time::sleep(backoff(attempt)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    }

    /// Paralel isteklerde tek yenileme yapılır: kilidi alan, token hâlâ eskiyse yeniler.
    async fn refresh_token(&self, stale: &str) -> Result<(), DriveError> {
        let provider = self
            .provider
            .as_ref()
            .ok_or_else(|| DriveError::Auth("token yenileyici tanımlı değil".into()))?;

        let _guard = self.refresh_lock.lock().await;

        let current = self.token.read().unwrap().clone();
        if current != stale {
            return Ok(()); // başka bir görev zaten yeniledi
        }

        let fresh = provider().await.map_err(DriveError::Auth)?;
        *self.token.write().unwrap() = fresh;
        Ok(())
    }

    // ----- klasör --------------------------------------------------------------

    /// Klasör oluşturur veya varsa ID'sini döndürür.
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

    pub async fn get_or_create_folder(&self, 
        name: &str,
        parent_id: Option<&str>,
    ) -> Result<String, DriveError> {
        let mut q = format!(
            "name = '{}' and mimeType = '{FOLDER_MIME}' and trashed = false",
            escape_q(name)
        );
        if let Some(p) = parent_id {
            q.push_str(&format!(" and '{}' in parents", escape_q(p)));
        }

        let url = format!(
            "{API}/files?q={}&spaces=drive&pageSize=10&fields={}",
            urlencoding::encode(&q),
            urlencoding::encode("files(id,name)")
        );

        let list: FileList = self.send(|c| c.get(&url)).await?.json().await?;
        if let Some(f) = list.files.into_iter().next() {
            return Ok(f.id);
        }

        let mut body = json!({ "name": name, "mimeType": FOLDER_MIME });
        if let Some(p) = parent_id {
            body["parents"] = json!([p]);
        }

        let create_url = format!("{API}/files?fields=id");
        let created: Created = self
            .send(|c| c.post(&create_url).json(&body))
            .await?
            .json()
            .await?;

        // Paylaşım ayarını herkese açık yazılabilir yap (dışardan bağlanabilmesi için)
        let perm_url = format!("{API}/files/{}/permissions", created.id);
        let perm_body = serde_json::json!({ "type": "anyone", "role": "writer" });
        let _ = self.send(|c| c.post(&perm_url).json(&perm_body)).await;

        Ok(created.id)
    }

    /// Klasördeki tüm dosyaları sayfalayarak listeler ve cache'i yeniler.
    /// Aynı isimde birden fazla dosya varsa ilki kullanılır.
    /// Sync başında bir kez çağır. Döndürdüğü değer dosya sayısıdır.
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
            self.send(|c| c.patch(&url).body(data.clone())).await?;
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
                 .body(body.clone())
            }).await?;
        }
        Ok(())
    }
    pub async fn refresh_index(&self, folder_id: &str) -> Result<usize, DriveError> {
        let q = format!("'{}' in parents and trashed = false", escape_q(folder_id));
        let fields = urlencoding::encode("nextPageToken,files(id,name,createdTime)").into_owned();

        let mut map: HashMap<String, FileEntry> = HashMap::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut url = format!(
                "{API}/files?q={}&spaces=drive&pageSize=1000&fields={fields}&orderBy=modifiedTime%20desc",
                urlencoding::encode(&q)
            );
            if let Some(t) = &page_token {
                url.push_str(&format!("&pageToken={}", urlencoding::encode(t)));
            }

            let list: FileList = self.send(|c| c.get(&url)).await?.json().await?;
            for f in list.files {
                let name = f.name.clone();
                map.entry(name).or_insert(f);
            }

            match list.next_page_token {
                Some(t) => page_token = Some(t),
                None => break,
            }
        }

        let count = map.len();
        *self.index.lock().unwrap() = map;
        Ok(count)
    }

    // ----- cache sorguları -----------------------------------------------------

    pub fn has(&self, name: &str) -> bool {
        self.index.lock().unwrap().contains_key(name)
    }

    pub fn file_id(&self, name: &str) -> Option<String> {
        self.index.lock().unwrap().get(name).map(|f| f.id.clone())
    }

    // ----- yükleme -------------------------------------------------------------

    /// Chunk yükler. Cache'te varsa yüklemez. `true` = gerçekten yüklendi.
    /// Not: Şifrelemeden ÖNCE `has()` ile bakmak daha ucuzdur.
    pub async fn ensure_chunk(
        &self,
        folder_id: &str,
        name: &str,
        data: &[u8],
    ) -> Result<bool, DriveError> {
        if self.has(name) {
            return Ok(false);
        }
        let id = self.create_file(folder_id, name, data).await?;
        self.index
            .lock()
            .unwrap()
            .entry(name.to_string())
            .or_insert(FileEntry {
                id,
                name: name.to_string(),
                created_time: None, // GC için önemli değil, yeni yüklenmiş
            });
        Ok(true)
    }

    /// Manifest gibi güncellenen dosyalar için: varsa içeriğini değiştirir, yoksa oluşturur.
    pub async fn upsert_file(
        &self,
        folder_id: &str,
        name: &str,
        data: &[u8],
    ) -> Result<String, DriveError> {
        if let Some(id) = self.file_id(name) {
            let url = format!(
                "{UPLOAD_API}/files/{}?uploadType=media&fields=id",
                urlencoding::encode(&id)
            );
            self.send(|c| {
                c.patch(&url)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .body(data.to_vec())
            })
            .await?;
            return Ok(id);
        }

        let id = self.create_file(folder_id, name, data).await?;
        self.index
            .lock()
            .unwrap()
            .insert(name.to_string(), FileEntry {
                id: id.clone(),
                name: name.to_string(),
                created_time: None,
            });
        Ok(id)
    }

    /// Koşulsuz yeni dosya oluşturur (multipart/related).
    async fn create_file(
        &self,
        folder_id: &str,
        name: &str,
        data: &[u8],
    ) -> Result<String, DriveError> {
        let metadata = json!({ "name": name, "parents": [folder_id] });
        let body = multipart_related(&metadata.to_string(), data);
        let content_type = format!("multipart/related; boundary={BOUNDARY}");
        let url = format!("{UPLOAD_API}/files?uploadType=multipart&fields=id");

        let created: Created = self
            .send(|c| {
                c.post(&url)
                    .header(header::CONTENT_TYPE, content_type.as_str())
                    .body(body.clone())
            })
            .await?
            .json()
            .await?;

        if created.id.is_empty() {
            return Err(DriveError::Protocol("yükleme yanıtında id boş".into()));
        }
        Ok(created.id)
    }

    // ----- indirme -------------------------------------------------------------

    pub async fn download_chunk(&self, file_id: &str) -> Result<Vec<u8>, DriveError> {
        let url = format!("{API}/files/{}?alt=media", urlencoding::encode(file_id));
        let mut attempts = 0;
        loop {
            match self.send(|c| c.get(&url)).await {
                Ok(resp) => {
                    match resp.bytes().await {
                        Ok(b) => return Ok(b.to_vec()),
                        Err(e) if attempts < 3 => {
                            eprintln!("Body okuma hatası, tekrar deneniyor ({attempts}/3): {e}");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
                Err(e) if attempts < 3 => {
                    eprintln!("İndirme başlatılamadı, tekrar deneniyor ({attempts}/3): {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                Err(e) => return Err(e),
            }
            attempts += 1;
        }
    }

    /// Cache'teki isme göre indirir (restore için hash -> içerik).
    pub async fn download_by_name(&self, name: &str) -> Result<Vec<u8>, DriveError> {
        let id = self
            .file_id(name)
            .ok_or_else(|| DriveError::Protocol(format!("Drive'da bulunamadı: {name}")))?;
        self.download_chunk(&id).await
    }

    /// Drive'dan bir dosyayı ID ile kalıcı olarak siler. GC için kullanılır.
    pub async fn delete_file(&self, file_id: &str) -> Result<(), DriveError> {
        let url = format!("{API}/files/{}", urlencoding::encode(file_id));
        self.send(|c| c.delete(&url)).await?;
        Ok(())
    }

    /// Klasördeki tüm (isim, id) çiftlerini döndürür. GC için tam liste gerekir.
    #[allow(dead_code)]
    pub fn all_names(&self) -> Vec<(String, String)> {
        self.index.lock().unwrap().iter()
            .map(|(k, v)| (k.clone(), v.id.clone()))
            .collect()
    }

    /// GC için dosya id, ad ve oluşturulma zamanını döndürür: (name, id, created_time)
    pub fn all_files_info(&self) -> Vec<(String, String, Option<String>)> {
        self.index.lock().unwrap().iter()
            .map(|(k, v)| (k.clone(), v.id.clone(), v.created_time.clone()))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Yardımcılar
// ---------------------------------------------------------------------------

/// Drive sorgu string'i içinde `\` ve `'` kaçırılır.
fn escape_q(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

fn multipart_related(metadata_json: &str, data: &[u8]) -> Vec<u8> {
    let head = format!(
        "--{BOUNDARY}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata_json}\r\n--{BOUNDARY}\r\nContent-Type: application/octet-stream\r\n\r\n"
    );
    let tail = format!("\r\n--{BOUNDARY}--");

    let mut body = Vec::with_capacity(head.len() + data.len() + tail.len());
    body.extend_from_slice(head.as_bytes());
    body.extend_from_slice(data);
    body.extend_from_slice(tail.as_bytes());
    body
}

fn is_retryable(status: StatusCode, body: &str) -> bool {
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return true;
    }
    // Drive rate-limit'i 403 ile de döndürür (rateLimitExceeded / userRateLimitExceeded)
    status == StatusCode::FORBIDDEN && body.contains("ateLimitExceeded")
}

fn parse_retry_after(resp: &Response) -> Option<Duration> {
    let secs = resp
        .headers()
        .get(header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()?;
    Some(Duration::from_secs(secs.min(120)))
}

/// 1s, 2s, 4s, 8s, 16s, 32s + jitter
fn backoff(attempt: u32) -> Duration {
    let base_ms = (1u64 << attempt.min(5)) * 1000;
    let jitter_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.subsec_nanos() % 1000) as u64)
        .unwrap_or(0);
    Duration::from_millis(base_ms + jitter_ms)
}
