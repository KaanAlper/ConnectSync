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

use super::progress::Progress;

const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_API: &str = "https://www.googleapis.com/upload/drive/v3";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
const BOUNDARY: &str = "connectsync_9f2b7c41d8e5a360";
const MAX_RETRIES: u32 = 6;
/// İnternet bağlantısı tamamen kopmuşken (timeout/connect hataları) çok daha sabırlı ol:
/// ~kısa aralıklarla toplam birkaç dakika dener, sadece işlemi başarısız saymak yerine
/// arayüzde "bağlantı bekleniyor" durumunu gösterir.
const NET_MAX_RETRIES: u32 = 120;
/// Drive klasörüne kullanıcının verdiği gerçek adı saklamak için appProperties anahtarı.
/// (Drive'daki klasör adı "ConnectSync_<hex>" olarak kalır; görünen ad burada tutulur.)
const NAME_PROP: &str = "cs_name";

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
    // Bazı sorgular sadece `files(id)` ister; bu yüzden name yoksa boş kalsın
    #[serde(default)]
    name: String,
    #[serde(rename = "createdTime")]
    created_time: Option<String>,
}

#[derive(Deserialize)]
struct Created {
    id: String,
}

#[derive(Deserialize)]
struct FolderList {
    #[serde(default)]
    files: Vec<FolderEntry>,
}

#[derive(Deserialize)]
struct FolderEntry {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(rename = "appProperties", default)]
    app_properties: Option<HashMap<String, String>>,
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
    /// Arayüzün okuduğu ilerleme (MB/GB, dosya sayısı) ve ağ durumu (kopukluk/yeniden deneme).
    pub progress: Arc<Progress>,
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
            progress: Arc::new(Progress::default()),
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
                Ok(resp) if resp.status().is_success() => {
                    // Sunucudan cevap geldi: ağ çalışıyor, kopukluk uyarısını temizle.
                    self.progress.set_net_retry(0);
                    return Ok(resp);
                }
                Ok(resp) => {
                    // Sunucu cevap verdiyse ağ bağlantısı vardır (401/429/5xx olsa da).
                    self.progress.set_net_retry(0);
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
                    // Sunucuya hiç ulaşılamadı: muhtemelen internet kesildi. Kısa aralıklarla
                    // uzun süre dene (NET_MAX_RETRIES) ve her denemede arayüze bildir.
                    let transient = e.is_timeout() || e.is_connect() || e.is_request();
                    if transient && attempt < NET_MAX_RETRIES {
                        self.progress.set_net_retry(attempt + 1);
                        tokio::time::sleep(backoff(attempt.min(5))).await;
                        attempt += 1;
                        continue;
                    }
                    self.progress.set_net_retry(0);
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

    /// Drive'daki ConnectSync klasörlerini (id, görünen ad) olarak döndürür.
    /// Görünen ad, kullanıcının klasör oluştururken seçtiği gerçek isimdir
    /// (appProperties'te saklanır); eski/adsız klasörlerde Drive adına geri düşülür.
    pub async fn list_cloud_folders(&self) -> Result<Vec<(String, String)>, DriveError> {
        let q = "name contains 'ConnectSync_' and mimeType = 'application/vnd.google-apps.folder' and trashed = false";
        let url = format!(
            "{API}/files?q={}&spaces=drive&pageSize=100&fields={}",
            urlencoding::encode(q),
            urlencoding::encode("files(id,name,appProperties)")
        );
        let list: FolderList = self.send(|c| c.get(&url)).await?.json().await?;
        let mut result = Vec::new();
        for f in list.files {
            let display = f
                .app_properties
                .as_ref()
                .and_then(|p| p.get(NAME_PROP))
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or(f.name);
            result.push((f.id, display));
        }
        Ok(result)
    }

    /// Klasörün kullanıcı dostu adını Drive'a appProperties olarak yazar (yalnızca bu
    /// uygulama görür, başka bir yerde görünmez). Klasör oluşturulduktan hemen sonra
    /// bir kez çağrılması beklenir; var olan bir değerin üzerine de yazar.
    pub async fn set_folder_display_name(&self, folder_id: &str, name: &str) -> Result<(), DriveError> {
        let patch_url = format!(
            "{API}/files/{}?fields=id",
            urlencoding::encode(folder_id)
        );
        let body = json!({ "appProperties": { "cs_name": name } });
        self.send(|c| c.patch(&patch_url).json(&body)).await?;
        Ok(())
    }

    /// Klasör hâlâ Drive'da mı? 404 (kalıcı silinmiş) ya da çöp kutusunda ise `false`.
    /// Ağ/yetki gibi başka hatalar `Err` döner: bunlar "silinmiş" SAYILMAZ, aksi halde geçici bir
    /// kesinti kullanıcıya yanlışlıkla "verin silinmiş" dedirtirdi.
    pub async fn folder_exists(&self, folder_id: &str) -> Result<bool, DriveError> {
        Ok(self.folder_info(folder_id).await?.is_some_and(|f| !f.trashed))
    }

    /// Oturum açmış Google hesabının e-postası. Yalnızca TEŞHİS içindir (ör. "bu kod başka bir hesaba ait
    /// olabilir" durumunda kullanıcı hangi hesapla bağlı olduğunu görsün).
    pub async fn signed_in_email(&self) -> Result<Option<String>, DriveError> {
        let url = format!("{API}/about?fields={}", urlencoding::encode("user(emailAddress)"));
        let about: About = self.send(|c| c.get(&url)).await?.json().await?;
        Ok(about.user.and_then(|u| u.email_address).filter(|e| !e.is_empty()))
    }

    /// Klasörün ad/çöp kutusu/gerçek-ad bilgisi. `None` = Drive "dosya yok" (404) dedi: kalıcı olarak
    /// silinmiş YA DA bu hesap/uygulama klasörü göremiyor (`drive.file` kapsamı yalnızca uygulamanın
    /// oluşturduğu/açtığı dosyaları gösterir). Ağ/yetki hataları `Err` döner.
    pub async fn folder_info(&self, folder_id: &str) -> Result<Option<FolderInfo>, DriveError> {
        let url = format!(
            "{API}/files/{}?fields={}",
            urlencoding::encode(folder_id),
            urlencoding::encode("name,trashed,appProperties")
        );
        match self.send(|c| c.get(&url)).await {
            Ok(resp) => Ok(Some(resp.json().await?)),
            Err(e) if is_not_found(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn get_or_create_folder(&self, 
        name: &str,
        parent_id: Option<&str>,
        can_others_write: bool,
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
        let role = if can_others_write { "writer" } else { "reader" };
        let perm_body = serde_json::json!({ "type": "anyone", "role": role });
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
            let url = "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart".to_string();
            
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
    /// Anahtar kaydından tek bir klasörün anahtarını siler.
    pub async fn remove_sync_key(&self, folder_id: &str) -> Result<(), DriveError> {
        let mut keys = self.get_sync_keys().await?;
        if keys.remove(folder_id).is_none() {
            return Ok(());
        }
        let q = "name = 'connectsync_keys.json' and 'appDataFolder' in parents and trashed = false";
        let url = format!(
            "{API}/files?q={}&spaces=appDataFolder&fields={}",
            urlencoding::encode(q),
            urlencoding::encode("files(id)")
        );
        let list: FileList = self.send(|c| c.get(&url)).await?.json().await?;
        if let Some(f) = list.files.into_iter().next() {
            let data = serde_json::to_vec(&keys).unwrap();
            let url = format!("https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media", f.id);
            self.send(|c| c.patch(&url).body(data.clone())).await?;
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
            self.progress.add_bytes(data.len() as u64);
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
        self.progress.add_bytes(data.len() as u64);
        Ok(created.id)
    }

    // ----- indirme -------------------------------------------------------------

    pub async fn download_chunk(&self, file_id: &str) -> Result<Vec<u8>, DriveError> {
        let url = format!("{API}/files/{}?alt=media", urlencoding::encode(file_id));
        let mut attempts = 0;
        loop {
            // `send()` zaten bağlantı/timeout hatalarında uzun süre yeniden dener
            // (internet kesilmesi dahil); burada yalnızca gövde okuma hatasını ele alıyoruz.
            let resp = self.send(|c| c.get(&url)).await?;
            match resp.bytes().await {
                Ok(b) => {
                    self.progress.add_bytes(b.len() as u64);
                    return Ok(b.to_vec());
                }
                Err(e) if attempts < 5 => {
                    eprintln!("Body okuma hatası, tekrar deneniyor ({attempts}/5): {e}");
                    self.progress.set_net_retry(attempts + 1);
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                Err(e) => return Err(e.into()),
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

/// Drive "dosya yok" (404) mu? Silinmiş klasörü ağ hatasından ayırmak için.
pub fn is_not_found(e: &DriveError) -> bool {
    matches!(e, DriveError::Api { status: 404, .. })
}

#[derive(Deserialize)]
struct About {
    user: Option<AboutUser>,
}

#[derive(Deserialize)]
struct AboutUser {
    #[serde(rename = "emailAddress")]
    email_address: Option<String>,
}

/// `folder_info` yanıtı.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FolderInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub trashed: bool,
    #[serde(rename = "appProperties", default)]
    pub app_properties: Option<HashMap<String, String>>,
}

/// `folder_info` sonucunun kullanıcıya anlamı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderAccess {
    /// Klasör var ve çöp kutusunda değil.
    Available,
    /// Klasör Drive'ın çöp kutusunda (geri yüklenebilir).
    Trashed,
    /// Drive "dosya yok" (404) dedi: kalıcı olarak silinmiş YA DA bu hesap/uygulama göremiyor. `drive.file`
    /// kapsamı yalnızca uygulamanın oluşturduğu ya da Picker ile açılan dosyaları gösterdiği için, başka bir
    /// Google hesabının oluşturduğu klasör (herkese açık olsa bile) bu duruma düşer.
    NotVisible,
}

pub fn folder_access(info: Option<&FolderInfo>) -> FolderAccess {
    match info {
        Some(f) if !f.trashed => FolderAccess::Available,
        Some(_) => FolderAccess::Trashed,
        None => FolderAccess::NotVisible,
    }
}

impl FolderInfo {
    /// Kullanıcıya gösterilecek ad: sync oluşturulurken yazılan gerçek ad (appProperties), yoksa
    /// Drive'daki ad ("ConnectSync_<hex>" öneki atılır; aksi halde rastgele bir hex görünürdü).
    pub fn display_name(&self) -> String {
        self.app_properties
            .as_ref()
            .and_then(|p| p.get(NAME_PROP))
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| self.name.strip_prefix("ConnectSync_").unwrap_or(&self.name).to_string())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn info(json: &str) -> FolderInfo {
        serde_json::from_str(json).expect("geçerli JSON")
    }

    #[test]
    fn the_real_name_written_at_creation_wins() {
        let f = info(r#"{"name":"ConnectSync_938891ca","appProperties":{"cs_name":"Müzik"}}"#);
        assert_eq!(f.display_name(), "Müzik");
    }

    #[test]
    fn falls_back_to_the_drive_name_without_the_random_prefix() {
        assert_eq!(info(r#"{"name":"ConnectSync_938891ca"}"#).display_name(), "938891ca");
        // Önek yoksa olduğu gibi
        assert_eq!(info(r#"{"name":"Belgelerim"}"#).display_name(), "Belgelerim");
    }

    #[test]
    fn a_blank_real_name_is_ignored() {
        let f = info(r#"{"name":"ConnectSync_abcd1234","appProperties":{"cs_name":"   "}}"#);
        assert_eq!(f.display_name(), "abcd1234");
    }

    #[test]
    fn missing_fields_default_to_a_live_unnamed_folder() {
        let f = info("{}");
        assert!(!f.trashed);
        assert_eq!(f.display_name(), "");
    }

    #[test]
    fn trashed_is_parsed() {
        assert!(info(r#"{"name":"x","trashed":true}"#).trashed);
    }

    #[test]
    fn about_response_yields_the_email_or_nothing() {
        let email = |json: &str| {
            serde_json::from_str::<About>(json).unwrap().user.and_then(|u| u.email_address)
        };
        assert_eq!(email(r#"{"user":{"emailAddress":"a@b.com"}}"#), Some("a@b.com".into()));
        assert_eq!(email(r#"{"user":{}}"#), None);
        assert_eq!(email("{}"), None);
    }

    #[test]
    fn folder_access_tells_live_trashed_and_invisible_folders_apart() {
        let live = info(r#"{"name":"x"}"#);
        let trashed = info(r#"{"name":"x","trashed":true}"#);
        assert_eq!(folder_access(Some(&live)), FolderAccess::Available);
        assert_eq!(folder_access(Some(&trashed)), FolderAccess::Trashed);
        assert_eq!(folder_access(None), FolderAccess::NotVisible);
    }

    #[test]
    fn only_a_404_counts_as_not_found() {
        let api = |status| DriveError::Api { status, body: String::new() };
        assert!(is_not_found(&api(404)));
        for other in [401, 403, 429, 500] {
            assert!(!is_not_found(&api(other)), "{other}");
        }
        assert!(!is_not_found(&DriveError::Auth("x".into())));
    }
}
