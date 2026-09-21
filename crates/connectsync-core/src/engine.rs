//! Senkronizasyon motoru: push (yerel -> Drive) ve pull (Drive -> yerel).
//!
//! Tasarım kuralları:
//! - Hiçbir hata sessizce yutulmaz. Bir dosyanın herhangi bir chunk'ı okunamaz,
//!   şifrelenemez veya yüklenemezse dosya manifest'e YAZILMAZ; bir sonraki turda
//!   yeniden denenir.
//! - Uzak manifest yalnızca "Drive'da hiç yok" durumunda sıfırdan oluşturulur.
//!   İndirme/çözme/ayrıştırma hatasında sync durur (yanlış sync code, ağ hatası vb.
//!   uzak manifest'i ezmemeli).
//! - Dosya sync sırasında değiştiyse manifest'e yazılmaz (tutarsız chunk listesi olmaz).
//! - Manifest periyodik olarak checkpoint'lenir; crash olursa tüm ilerleme kaybolmaz.
//! - Drive'dan silinmiş chunk'lar tespit edilir ve ilgili dosya yeniden yüklenir.
//! - Klasör boş/bağlı değil gibi durumlarda toplu silme koruması vardır.
//!
//! Bellek: dosya başına kanal (CHANNEL_CAP) + global yükleme izni (`Concurrency::uploads`) +
//! alıcıda bekleyen/chunker'da tutulan birer chunk. Chunk en fazla ~4 MiB olduğundan
//! üst sınır kabaca (files * (CHANNEL_CAP + 2) + uploads) * 4 MiB. Varsayılanda (4 thread)
//! ≈ 88 MiB, en yüksek ayarda (16 thread) ≈ 352 MiB.

use crate::limits::{clamp_threads, DEFAULT_THREADS};
use super::crypto::{decrypt_chunk, encrypt_chunk, hash_chunk_name, Keys};
use super::drive::DriveClient;
use super::manifest::{FileInfo, Manifest};
use super::progress::{self, PhaseGuard};
use fastcdc::v2020::StreamCDC;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};
use tokio::sync::Semaphore;
use tokio::task::{JoinHandle, JoinSet};
use walkdir::WalkDir;

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const MANIFEST_NAME: &str = "manifest.bin";

const CDC_MIN: usize = 256 * 1024;
const CDC_AVG: usize = 1024 * 1024;
const CDC_MAX: usize = 4 * 1024 * 1024;

/// Kullanıcı ayarındaki "eşzamanlı aktarım" sayısından türetilen paralellik sınırları.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Concurrency {
    /// Aynı anda işlenen dosya sayısı.
    files: usize,
    /// Tüm dosyalar toplamı, aynı anda süren chunk yüklemesi.
    uploads: usize,
}

impl Concurrency {
    /// `threads` önce izin verilen aralığa çekilir. Yükleme izni = threads; aynı anda
    /// işlenen dosya sayısı bunun ~3/4'ü (en az 1). Varsayılan 4 → 3 dosya / 4 yükleme.
    fn from_threads(threads: u32) -> Self {
        let t = clamp_threads(threads) as usize;
        Self { files: (t * 3 / 4).max(1), uploads: t }
    }
}

/// Chunker -> yükleyici kanal kapasitesi (dosya başına).
const CHANNEL_CAP: usize = 4;
/// Restore'da dosya başına önceden indirilen chunk sayısı.
const PREFETCH: usize = 4;

const CHECKPOINT_FILES: usize = 200;
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(60);

/// Toplu silme koruması: en az bu kadar dosya VE manifest'in yarısından fazlası.
const MASS_DELETE_MIN: usize = 20;

// ---------------------------------------------------------------------------
// Rapor / seçenek tipleri
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct PushOptions {
    /// Boş klasör veya toplu silme koruması devre dışı bırakılır.
    pub allow_mass_delete: bool,
}

#[derive(Debug, Default, Clone)]
pub struct SyncReport {
    pub scanned_files: usize,
    pub uploaded_files: usize,
    pub unchanged_files: usize,
    /// Drive'da chunk'ı eksik olduğu için yeniden yüklenmeye çalışılan dosyalar.
    pub repaired_files: usize,
    pub removed_from_manifest: usize,
    pub changed_during_sync: Vec<String>,
    pub failed_files: Vec<(String, String)>,
    pub scan_errors: usize,
    pub chunks_uploaded: u64,
    pub chunks_deduped: u64,
    pub bytes_uploaded: u64,
    pub manifest_revision: u32,
}

impl SyncReport {
    #[allow(dead_code)]
    pub fn is_clean(&self) -> bool {
        self.failed_files.is_empty() && self.scan_errors == 0
    }
}

#[derive(Debug, Default, Clone)]
pub struct PullReport {
    pub restored_files: usize,
    pub skipped_up_to_date: usize,
    /// Hedefte farklı bir dosya vardı ve `overwrite == false` idi.
    pub skipped_existing: Vec<String>,
    /// Güvensiz veya geçersiz yol (`..`, mutlak yol vb.)
    pub invalid_paths: Vec<String>,
    pub failed_files: Vec<(String, String)>,
    pub bytes_written: u64,
}

impl PullReport {
    #[allow(dead_code)]
    pub fn is_clean(&self) -> bool {
        self.failed_files.is_empty() && self.invalid_paths.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Motor
// ---------------------------------------------------------------------------

pub struct SyncEngine {
    pub drive_client: Arc<DriveClient>,
    pub keys: Arc<Keys>,
    pub drive_folder_id: String,
    pub local_folder_path: PathBuf,
    pub local_revision: tokio::sync::Mutex<u64>,
    sync_lock: tokio::sync::Mutex<()>,
    concurrency: Concurrency,
}

impl SyncEngine {
    #[allow(dead_code)]
    pub fn new(
        drive_client: DriveClient,
        keys: Keys,
        drive_folder_id: String,
        local_folder_path: PathBuf,
    ) -> Self {
        Self::new_with_arc(Arc::new(drive_client), keys, drive_folder_id, local_folder_path)
    }

    /// Arc<DriveClient> ile doğrudan oluşturmak için (drive paylaşımı gerektiğinde)
    pub fn new_with_arc(
        drive_client: Arc<DriveClient>,
        keys: Keys,
        drive_folder_id: String,
        local_folder_path: PathBuf,
    ) -> Self {
        // Restart sonrası rollback koruması için disk'ten revision oku
        let saved_revision: u64 = std::fs::read_to_string(
            local_folder_path.join(".connectsync-revision")
        )
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

        Self {
            drive_client,
            keys: Arc::new(keys),
            drive_folder_id,
            local_folder_path,
            sync_lock: tokio::sync::Mutex::new(()),
            local_revision: tokio::sync::Mutex::new(saved_revision),
            concurrency: Concurrency::from_threads(DEFAULT_THREADS),
        }
    }

    /// Eşzamanlı aktarım sayısını (ayarlardaki `concurrent_threads`) uygular.
    /// Aralık dışı değerler sınırlara çekilir.
    pub fn with_threads(mut self, threads: u32) -> Self {
        self.concurrency = Concurrency::from_threads(threads);
        self
    }

    pub async fn run_sync_push(&self) -> Res<SyncReport> {
        self.run_sync_push_with(PushOptions::default()).await
    }

    pub async fn run_sync_push_with(&self, opts: PushOptions) -> Res<SyncReport> {
        let _guard = self.sync_lock.lock().await;
        // Fonksiyondan nasıl çıkılırsa çıkılsın (başarı, `?` ile erken dönüş) ilerlemeyi sıfırlar.
        let _progress_guard = PhaseGuard(self.drive_client.progress.clone());
        self.drive_client.progress.begin(progress::PHASE_PREPARE, 0, 0);
        let mut report = SyncReport::default();

        println!("Drive index yenileniyor...");
        let count = self.drive_client.refresh_index(&self.drive_folder_id).await?;
        println!("Index yenilendi. Drive'da {count} dosya var.");

        let mut manifest = self.load_manifest().await?;

        // --- yerel tarama (bloklayıcı işler async thread'i tutmasın) ---
        let root = self.local_folder_path.clone();
        let scan = tokio::task::spawn_blocking(move || scan_local(&root))
            .await
            .map_err(|e| format!("tarama görevi çöktü: {e}"))??;
        report.scanned_files = scan.files.len();
        report.scan_errors = scan.errors;

        // --- silinen dosyalar ---
        // Tarama hatası varsa (izin, geçersiz ad) "yok" sonucuna güvenilemez.
        let removed: Vec<String> = if scan.errors == 0 {
            let present: HashSet<&str> = scan.files.iter().map(|f| f.rel_path.as_str()).collect();
            manifest
                .files
                .keys()
                .filter(|k| !present.contains(k.as_str()))
                .cloned()
                .collect()
        } else {
            println!(
                "{} tarama hatası var, silinen dosya tespiti bu turda atlandı.",
                scan.errors
            );
            Vec::new()
        };

        if !opts.allow_mass_delete && !removed.is_empty() {
            let total = manifest.files.len();
            let empty_scan = scan.files.is_empty();
            let mass = removed.len() >= MASS_DELETE_MIN && removed.len() * 2 > total;
            if empty_scan || mass {
                return Err(format!(
                    "Toplu silme engellendi: manifest'te {total} dosya var, yerelde {} tanesi bulunamadı \
                     (klasör boş veya bağlı olmayan bir disk olabilir). \
                     Bilerek yapıyorsan PushOptions {{ allow_mass_delete: true }} ile tekrar dene.",
                    removed.len()
                )
                .into());
            }
        }

        // --- iş listesi ---
        let mut work: Vec<LocalFile> = Vec::new();
        for lf in scan.files {
            match manifest.files.get(&lf.rel_path) {
                Some(ex) if ex.size == lf.size && ex.modified_at == lf.modified_ms => {
                    if ex.chunks.iter().all(|c| self.drive_client.has(&hex::encode(c))) {
                        report.unchanged_files += 1;
                    } else {
                        println!("Drive'da eksik chunk var, yeniden yüklenecek: {}", lf.rel_path);
                        report.repaired_files += 1;
                        work.push(lf);
                    }
                }
                _ => work.push(lf),
            }
        }

        let mut unsaved: usize = 0; // manifest'e henüz yazılmamış değişiklik sayısı
        for p in &removed {
            manifest.files.remove(p);
        }
        if !removed.is_empty() {
            report.removed_from_manifest = removed.len();
            unsaved += removed.len();
            println!("{} silinmiş dosya manifest'ten çıkarıldı.", removed.len());
        }

        // --- ilerleme: yüklenecek toplam bayt/dosya (arayüzdeki MB/GB çubuğu için) ---
        let total_push_bytes: u64 = work.iter().map(|f| f.size).sum();
        self.drive_client
            .progress
            .begin(progress::PHASE_PUSH, total_push_bytes, work.len() as u64);

        // --- yükleme hattı ---
        let ctx = Arc::new(PushCtx {
            client: self.drive_client.clone(),
            keys: self.keys.clone(),
            folder_id: self.drive_folder_id.clone(),
            upload_sem: Arc::new(Semaphore::new(self.concurrency.uploads)),
            inflight: InflightLocks::default(),
            chunks_uploaded: AtomicU64::new(0),
            chunks_deduped: AtomicU64::new(0),
            bytes_uploaded: AtomicU64::new(0),
        });

        let total_work = work.len();
        let mut queue = work.into_iter();
        let mut set: JoinSet<(String, Result<FileOutcome, String>)> = JoinSet::new();
        let mut done = 0usize;
        let mut last_checkpoint = Instant::now();

        loop {
            while set.len() < self.concurrency.files {
                let Some(lf) = queue.next() else { break };
                println!("Senkronize ediliyor: {}", lf.rel_path);
                self.drive_client.progress.set_current(&lf.rel_path);
                let ctx = ctx.clone();
                set.spawn(async move {
                    let rel = lf.rel_path.clone();
                    let res = sync_one_file(ctx, lf).await;
                    (rel, res)
                });
            }

            let Some(joined) = set.join_next().await else { break };
            done += 1;

            match joined {
                Ok((rel, Ok(FileOutcome::Synced(info)))) => {
                    println!("[{done}/{total_work}] Tamamlandı: {rel}");
                    manifest.files.insert(rel, info);
                    report.uploaded_files += 1;
                    unsaved += 1;
                    self.drive_client.progress.file_done();
                }
                Ok((rel, Ok(FileOutcome::ChangedDuringSync))) => {
                    println!("[{done}/{total_work}] Sync sırasında değişti/silindi, sonraya bırakıldı: {rel}");
                    report.changed_during_sync.push(rel);
                    self.drive_client.progress.file_done();
                }
                Ok((rel, Err(e))) => {
                    eprintln!("[{done}/{total_work}] HATA ({rel}): {e}");
                    report.failed_files.push((rel, e));
                    self.drive_client.progress.file_done();
                }
                Err(e) => {
                    // Görev çöktüğü için hangi dosya olduğu bilinmiyor; manifest'e
                    // yazılmadığından bir sonraki turda zaten yeniden denenir.
                    eprintln!("Dosya görevi çöktü: {e}");
                    report.failed_files.push(("<bilinmiyor>".into(), e.to_string()));
                    self.drive_client.progress.file_done();
                }
            }

            if unsaved > 0
                && (unsaved >= CHECKPOINT_FILES || last_checkpoint.elapsed() >= CHECKPOINT_INTERVAL)
            {
                match self.save_manifest(&mut manifest).await {
                    Ok(()) => {
                        println!("Checkpoint: manifest v{} kaydedildi.", manifest.revision);
                        unsaved = 0;
                    }
                    Err(e) => eprintln!("Checkpoint başarısız (devam ediliyor): {e}"),
                }
                last_checkpoint = Instant::now();
            }
        }

        // --- son manifest ---
        if unsaved > 0 {
            println!("Manifest güncelleniyor...");
            self.save_manifest(&mut manifest).await?;
        } else {
            println!("Manifest'te değişiklik yok, yüklenmedi.");
        }

        report.manifest_revision = manifest.revision as u32;
        report.chunks_uploaded = ctx.chunks_uploaded.load(Ordering::Relaxed);
        report.chunks_deduped = ctx.chunks_deduped.load(Ordering::Relaxed);
        report.bytes_uploaded = ctx.bytes_uploaded.load(Ordering::Relaxed);

        println!(
            "Sync bitti: {} yüklendi, {} değişmedi, {} hatalı, {} sonraya kaldı, {} silindi.",
            report.uploaded_files,
            report.unchanged_files,
            report.failed_files.len(),
            report.changed_during_sync.len(),
            report.removed_from_manifest
        );
        Ok(report)
    }

    /// Drive'daki manifest'e göre dosyaları `target_dir` altına geri yükler.
    pub async fn run_sync_pull(&self, target_dir: &Path, overwrite: bool) -> Res<PullReport> {
        let _guard = self.sync_lock.lock().await;
        let _progress_guard = PhaseGuard(self.drive_client.progress.clone());
        self.drive_client.progress.begin(progress::PHASE_PREPARE, 0, 0);
        let mut report = PullReport::default();

        let count = self.drive_client.refresh_index(&self.drive_folder_id).await?;
        println!("Index yenilendi. Drive'da {count} dosya var.");

        if !self.drive_client.has(MANIFEST_NAME) {
            return Err("Drive'da manifest bulunamadı; geri yüklenecek bir şey yok.".into());
        }
        let manifest = self.load_manifest().await?;

        tokio::fs::create_dir_all(target_dir).await?;

        let mut entries: Vec<(&String, &FileInfo)> = manifest.files.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));

        let mut jobs: Vec<(String, FileInfo, PathBuf)> = Vec::new();
        for (rel, info) in entries {
            let Some(dest) = safe_join(target_dir, rel) else {
                eprintln!("Güvensiz yol atlandı: {rel}");
                report.invalid_paths.push(rel.clone());
                continue;
            };

            match tokio::fs::metadata(&dest).await {
                Ok(md) if md.is_file() => {
                    if md.len() == info.size && mtime_ms(&md) == info.modified_at {
                        report.skipped_up_to_date += 1;
                        continue;
                    }
                    if !overwrite {
                        report.skipped_existing.push(rel.clone());
                        continue;
                    }
                }
                Ok(_) => {
                    report
                        .failed_files
                        .push((rel.clone(), "hedef yol bir dosya değil".into()));
                    continue;
                }
                Err(_) => {}
            }
            jobs.push((rel.clone(), info.clone(), dest));
        }

        let ctx = Arc::new(PullCtx {
            client: self.drive_client.clone(),
            keys: self.keys.clone(),
        });

        let total_pull_bytes: u64 = jobs.iter().map(|(_, info, _)| info.size).sum();
        self.drive_client
            .progress
            .begin(progress::PHASE_PULL, total_pull_bytes, jobs.len() as u64);

        let total = jobs.len();
        let mut queue = jobs.into_iter();
        let mut set: JoinSet<(String, Result<u64, String>)> = JoinSet::new();
        let mut done = 0usize;

        loop {
            while set.len() < self.concurrency.files {
                let Some((rel, info, dest)) = queue.next() else { break };
                println!("Geri yükleniyor: {rel}");
                self.drive_client.progress.set_current(&rel);
                let ctx = ctx.clone();
                set.spawn(async move {
                    let res = restore_one_file(ctx, info, dest).await;
                    (rel, res)
                });
            }

            let Some(joined) = set.join_next().await else { break };
            done += 1;

            match joined {
                Ok((rel, Ok(bytes))) => {
                    println!("[{done}/{total}] Geri yüklendi: {rel}");
                    report.restored_files += 1;
                    report.bytes_written += bytes;
                    self.drive_client.progress.file_done();
                }
                Ok((rel, Err(e))) => {
                    eprintln!("[{done}/{total}] HATA ({rel}): {e}");
                    report.failed_files.push((rel, e));
                    self.drive_client.progress.file_done();
                }
                Err(e) => {
                    eprintln!("Restore görevi çöktü: {e}");
                    report.failed_files.push(("<bilinmiyor>".into(), e.to_string()));
                    self.drive_client.progress.file_done();
                }
            }
        }

        println!(
            "Restore bitti: {} dosya, {} güncel, {} atlandı, {} hatalı.",
            report.restored_files,
            report.skipped_up_to_date,
            report.skipped_existing.len(),
            report.failed_files.len()
        );
        Ok(report)
    }

    // ----- manifest --------------------------------------------------------

    /// Yalnızca Drive'da manifest YOKSA yeni oluşturur. Diğer tüm hatalar sync'i durdurur.
    async fn load_manifest(&self) -> Res<Manifest> {
        if !self.drive_client.has(MANIFEST_NAME) {
            println!("Drive'da manifest yok, yeni oluşturulacak.");
            return Ok(Manifest::new());
        }

        let enc = self.drive_client.download_by_name(MANIFEST_NAME).await?;
        let dec = decrypt_chunk(&self.keys.enc_key, MANIFEST_NAME, &enc).map_err(|e| {
            format!("Manifest çözülemedi (sync code yanlış olabilir veya dosya bozuk), sync durduruldu: {e}")
        })?;
        let manifest = bincode::deserialize::<Manifest>(&dec)
            .map_err(|e| format!("Manifest parse hatası: {e}"))?;
            
        if manifest.schema_version != 1 {
            return Err(format!("Desteklenmeyen manifest sürümü: {}. Lütfen uygulamayı güncelleyin.", manifest.schema_version).into());
        }
            
        let mut known = self.local_revision.lock().await;
        if manifest.revision < *known {
            return Err(format!("Sunucudaki manifest eski (rollback algılandı). Uzak: {}, Yerel: {}", manifest.revision, *known).into());
        }
        *known = manifest.revision;
        Ok(manifest)
    }

    async fn save_manifest(&self, manifest: &mut Manifest) -> Res<()> {
        // Revizyonu yalnızca başarılı yüklemeden sonra commit et.
        // Önce klona yaz, upload başarılıysa orijinali güncelle.
        let next = manifest.revision.saturating_add(1);
        let mut candidate = manifest.clone();
        candidate.revision = next;

        let bin = bincode::serialize(&candidate).map_err(|e| format!("Manifest serialize hatası: {e}"))?;
        let enc = encrypt_chunk(&self.keys.enc_key, MANIFEST_NAME, &bin)
            .map_err(|e| format!("Manifest şifrelenemedi: {e}"))?;
        self.drive_client
            .upsert_file(&self.drive_folder_id, MANIFEST_NAME, &enc)
            .await?;

        // Upload başarılı — şimdi orijinali güncelle + yerel revizyonu kaydet
        manifest.revision = next;
        *self.local_revision.lock().await = next;

        // Revizyonu yerel diske de yaz (restart sonrası rollback koruması için)
        let rev_path = self.local_folder_path.join(".connectsync-revision");
        let _ = std::fs::write(&rev_path, next.to_string());

        Ok(())
    }

    // ----- vault (random salt) ---------------------------------------------

    const VAULT_NAME: &'static str = "vault.json";

    /// Drive'dan salt'ı okur. Yoksa yeni üretir ve kaydeder.
    /// Salt şifrelenmez — zaten public olması amaçlanmış (salt ≠ secret).
    pub async fn load_or_create_salt(&self) -> Res<[u8; 32]> {
        // En güncel durumu al
        self.drive_client.refresh_index(&self.drive_folder_id).await?;

        // Drive'da vault.json var mı kontrol et, varsa en eskisini indir (çakışma durumunda veri kaybını önler)
        if self.drive_client.has(Self::VAULT_NAME) {
            // "orderBy=createdTime asc" mantığıyla eski salt'ı bulmak en güvenlisi.
            // Fakat basitçe index üzerinden id'yi almak da çalışır (şu anda drive.rs en son yükleneni tutuyor olabilir, 
            // ama sıfırdan oluşturmayı kesinlikle engeller).
            let raw = self.drive_client.download_by_name(Self::VAULT_NAME).await?;
            let v: serde_json::Value = serde_json::from_slice(&raw)
                .map_err(|e| format!("vault.json parse hatası: {e}"))?;
            let hex_salt = v["salt"].as_str()
                .ok_or("vault.json 'salt' alanı eksik")?;
            let bytes = hex::decode(hex_salt)
                .map_err(|e| format!("vault.json salt hex decode hatası: {e}"))?;
            if bytes.len() != 32 {
                return Err(format!("vault.json salt boyutu hatalı: {}", bytes.len()).into());
            }
            let mut out = [0u8; 32];
            out.copy_from_slice(&bytes);
            return Ok(out);
        }

        // Yeni salt üret ve kaydet
        let mut salt = [0u8; 32];
        getrandom::fill(&mut salt).map_err(|e| format!("Salt üretilemedi: {e}"))?;
        let json = serde_json::json!({ "salt": hex::encode(salt) }).to_string();
        self.drive_client
            .upsert_file(&self.drive_folder_id, Self::VAULT_NAME, json.as_bytes())
            .await?;
        println!("Yeni vault salt üretildi ve Drive'a kaydedildi.");
        
        // Yeniden indexleyelim ki cache'e girsin
        self.drive_client.refresh_index(&self.drive_folder_id).await?;
        
        Ok(salt)
    }

    // ----- garbage collection -----------------------------------------------

    /// Drive'daki tüm dosyaları manifest'teki "canlı" chunk kümesiyle karşılaştırır.
    /// Manifest'te referans edilmeyen chunk'ları siler.
    /// Manifest ve vault.json'a dokunmaz.
    pub async fn run_gc(&self) -> Res<usize> {
        let _guard = self.sync_lock.lock().await;

        // Taze index al
        self.drive_client.refresh_index(&self.drive_folder_id).await?;

        // Manifest'teki canlı chunk kümesini hex adlarla oluştur
        let manifest = self.load_manifest().await?;
        let live: std::collections::HashSet<String> = manifest.files.values()
            .flat_map(|fi| fi.chunks.iter().map(hex::encode))
            .collect();

        // Drive'daki tüm dosyaları tara (name, id, created_time)
        let all = self.drive_client.all_files_info();
        let mut deleted = 0usize;

        let now = std::time::SystemTime::now();

        for (name, id, created_time_str) in all {
            // Manifest ve vault'a dokunma
            if name == MANIFEST_NAME || name == Self::VAULT_NAME {
                continue;
            }
            if !live.contains(&name) {
                // Orphan chunk bulundu. Yaş kontrolü yap.
                // Eğer chunk çok yeniyse (başka cihaz yüklüyor olabilir), SİLME.
                // 1 günden (86400 sn) eski orphanları sil.
                let mut should_delete = false;
                if let Some(ct) = created_time_str {
                    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(&ct) {
                        let sys_time: std::time::SystemTime = parsed.into();
                        if let Ok(dur) = now.duration_since(sys_time)
                            && dur.as_secs() > 86400 {
                                should_delete = true;
                            }
                    } else {
                        // Parse edilemediyse (normalde olmamalı), güvenli tarafta kalıp silelim mi?
                        // Hayır, silmeyelim.
                    }
                } else {
                    // createdTime yoksa (eski dosya), doğrudan sil
                    should_delete = true;
                }

                if should_delete {
                    match self.drive_client.delete_file(&id).await {
                        Ok(()) => {
                            deleted += 1;
                            println!("GC: silindi → {name}");
                        }
                        Err(e) => eprintln!("GC: silinemedi ({name}): {e}"),
                    }
                }
            }
        }

        println!("GC tamamlandı: {deleted} orphan chunk silindi.");
        Ok(deleted)
    }
}

// ---------------------------------------------------------------------------
// Push: tek dosya
// ---------------------------------------------------------------------------

struct PushCtx {
    client: Arc<DriveClient>,
    keys: Arc<Keys>,
    folder_id: String,
    upload_sem: Arc<Semaphore>,
    inflight: InflightLocks,
    chunks_uploaded: AtomicU64,
    chunks_deduped: AtomicU64,
    bytes_uploaded: AtomicU64,
}

enum FileOutcome {
    Synced(FileInfo),
    /// Dosya sync sırasında değişti veya silindi; manifest'e yazılmaz.
    ChangedDuringSync,
}

/// Bir dosyayı chunk'lar, şifreler ve yükler. Başarıda `FileInfo` döner;
/// herhangi bir chunk sorununda `Err` (dosya manifest'e yazılmaz).
async fn sync_one_file(ctx: Arc<PushCtx>, lf: LocalFile) -> Result<FileOutcome, String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<([u8; 32], Option<Vec<u8>>, u64)>(CHANNEL_CAP);
    let failed = Arc::new(AtomicBool::new(false));

    // Chunker: bloklayıcı thread'de okur, isimlendirir, (gerekirse) şifreler.
    let chunker = {
        let ctx = ctx.clone();
        let path = lf.abs_path.clone();
        tokio::task::spawn_blocking(move || -> Result<u64, String> {
            let file = fs::File::open(&path).map_err(|e| format!("dosya açılamadı: {e}"))?;
            let mut total = 0u64;

            for chunk in StreamCDC::new(file, CDC_MIN, CDC_AVG, CDC_MAX) {
                let chunk = chunk.map_err(|e| format!("okuma hatası: {e:?}"))?;
                total += chunk.data.len() as u64;

                let name = hash_chunk_name(&ctx.keys.hmac_key, &chunk.data);
                let plain_len = chunk.data.len() as u64;
                // Drive'da zaten varsa şifreleme ve kanal maliyetini atla.
                let enc = if ctx.client.has(&hex::encode(name)) {
                    None
                } else {
                    Some(
                        encrypt_chunk(&ctx.keys.enc_key, &hex::encode(name), &chunk.data)
                            .map_err(|e| format!("şifreleme hatası: {e}"))?,
                    )
                };

                if tx.blocking_send((name, enc, plain_len)).is_err() {
                    return Err("alıcı kapandı".into());
                }
            }
            Ok(total)
        })
    };

    // Alıcı: sırayı koruyarak hash listesini kurar, yüklemeleri paralel yürütür.
    let mut hashes: Vec<[u8; 32]> = Vec::new();
    let mut uploads: JoinSet<Result<(), String>> = JoinSet::new();

    while let Some((name, enc, plain_len)) = rx.recv().await {
        if failed.load(Ordering::Relaxed) {
            break; // bir yükleme zaten başarısız oldu, boşuna devam etme
        }
        hashes.push(name);

        let Some(enc) = enc else {
            // Chunk Drive'da zaten var; gerçek transfer yok. `plain_len` sayesinde ilerleme
            // çubuğu yine de bu kadar baytı "tamamlandı" sayar (aksi halde dedupe edilen
            // dosyalarda çubuk %100'e hiç ulaşmaz).
            ctx.chunks_deduped.fetch_add(1, Ordering::Relaxed);
            ctx.client.progress.add_bytes(plain_len);
            continue;
        };

        let permit = ctx
            .upload_sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| format!("yükleme semaforu kapandı: {e}"))?;

        let ctx2 = ctx.clone();
        let failed2 = failed.clone();
        let hex_name = hex::encode(name);
        uploads.spawn(async move {
            let _permit = permit; // görev bitene kadar izin tutulur
            let res = upload_chunk(&ctx2, &hex_name, enc).await;
            if res.is_err() {
                failed2.store(true, Ordering::Relaxed);
            }
            res
        });
    }
    drop(rx); // erken çıkıldıysa chunker'ın blocking_send'i hata alıp durur

    let mut first_err: Option<String> = None;
    while let Some(res) = uploads.join_next().await {
        let err = match res {
            Ok(Ok(())) => None,
            Ok(Err(e)) => Some(format!("chunk yüklenemedi: {e}")),
            Err(e) => Some(format!("yükleme görevi çöktü: {e}")),
        };
        if first_err.is_none() {
            first_err = err;
        }
    }
    let chunker_res = chunker.await;

    // Gerçek neden yükleme hatasıysa, chunker'ın "alıcı kapandı" hatası onu gölgelemesin.
    if let Some(e) = first_err {
        return Err(e);
    }
    let total = match chunker_res {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => return Err(e),
        Err(e) => return Err(format!("chunker görevi çöktü: {e}")),
    };

    // Sync sırasında değişti/silindi mi?
    match stat_file(&lf.abs_path) {
        Ok((size, mtime)) if size == lf.size && mtime == lf.modified_ms => {}
        _ => return Ok(FileOutcome::ChangedDuringSync),
    }

    if total != lf.size {
        return Err(format!(
            "okunan bayt sayısı ({total}) dosya boyutuyla ({}) uyuşmuyor",
            lf.size
        ));
    }

    Ok(FileOutcome::Synced(FileInfo {
        path: lf.rel_path,
        size: lf.size,
        modified_at: lf.modified_ms,
        chunks: hashes,
    }))
}

/// Aynı chunk'ı aynı anda iki görev yüklemesin (Drive aynı isimde çift dosya oluşturur):
/// isim başına kilit alınır, kilidi alan `ensure_chunk` içinde index'e bakar.
async fn upload_chunk(ctx: &PushCtx, name: &str, enc: Vec<u8>) -> Result<(), String> {
    let size = enc.len() as u64;
    let lock = ctx.inflight.get(name);
    let result = {
        let _guard = lock.lock().await;
        ctx.client.ensure_chunk(&ctx.folder_id, name, &enc).await
    };
    ctx.inflight.release(name, lock);

    match result {
        Ok(true) => {
            ctx.chunks_uploaded.fetch_add(1, Ordering::Relaxed);
            ctx.bytes_uploaded.fetch_add(size, Ordering::Relaxed);
            Ok(())
        }
        Ok(false) => {
            ctx.chunks_deduped.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// İsim başına async kilit tablosu; kullanılmayan girdiler temizlenir.
#[derive(Default)]
struct InflightLocks {
    map: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl InflightLocks {
    fn get(&self, name: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut m = self.map.lock().unwrap();
        m.entry(name.to_string()).or_default().clone()
    }

    /// `lock`, çağıranın elindeki kopya. Tabloda + çağıranda = 2 ise kimse beklemiyordur.
    fn release(&self, name: &str, lock: Arc<tokio::sync::Mutex<()>>) {
        let mut m = self.map.lock().unwrap();
        drop(lock);
        if let Some(arc) = m.get(name)
            && Arc::strong_count(arc) == 1 {
                m.remove(name);
            }
    }
}

// ---------------------------------------------------------------------------
// Pull: tek dosya
// ---------------------------------------------------------------------------

struct PullCtx {
    client: Arc<DriveClient>,
    keys: Arc<Keys>,
}

async fn restore_one_file(ctx: Arc<PullCtx>, info: FileInfo, dest: PathBuf) -> Result<u64, String> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("klasör oluşturulamadı: {e}"))?;
    }

    let tmp = tmp_path(&dest);
    match restore_into(&ctx, &info, &tmp).await {
        Ok(written) => match tokio::fs::rename(&tmp, &dest).await {
            Ok(()) => Ok(written),
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                Err(format!("dosya yerine taşınamadı: {e}"))
            }
        },
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            Err(e)
        }
    }
}

async fn restore_into(ctx: &Arc<PullCtx>, info: &FileInfo, tmp: &Path) -> Result<u64, String> {
    use tokio::io::AsyncWriteExt;

    let mut file = tokio::fs::File::create(tmp)
        .await
        .map_err(|e| format!("geçici dosya oluşturulamadı: {e}"))?;

    let mut written = 0u64;
    let mut pending: VecDeque<JoinHandle<Result<Vec<u8>, String>>> = VecDeque::new();
    let mut names = info.chunks.iter();

    let outcome = async {
        loop {
            // Sıra korunur: indirmeler öne alınır, yazma her zaman sırayla yapılır.
            while pending.len() < PREFETCH {
                let Some(name) = names.next() else { break };
                let ctx = ctx.clone();
                let name = *name;
                let hex_name = hex::encode(name);
                pending.push_back(tokio::spawn(async move { fetch_chunk(&ctx, &hex_name).await }));
            }
            let Some(handle) = pending.pop_front() else { break };

            let data = handle
                .await
                .map_err(|e| format!("indirme görevi çöktü: {e}"))??;
            file.write_all(&data)
                .await
                .map_err(|e| format!("yazma hatası: {e}"))?;
            written += data.len() as u64;
        }
        Ok::<(), String>(())
    }
    .await;

    if outcome.is_err() {
        for h in &pending {
            h.abort();
        }
    }
    outcome?;

    if written != info.size {
        return Err(format!(
            "geri yüklenen boyut ({written}) manifest'teki boyutla ({}) uyuşmuyor",
            info.size
        ));
    }

    file.flush().await.map_err(|e| format!("flush hatası: {e}"))?;
    file.sync_all()
        .await
        .map_err(|e| format!("fsync hatası: {e}"))?;

    // Değişiklik zamanını geri yükle (bir sonraki restore/push "güncel" görsün).
    let std_file = file.into_std().await;
    let _ = std_file.set_modified(UNIX_EPOCH + Duration::from_millis(info.modified_at));

    Ok(written)
}

async fn fetch_chunk(ctx: &PullCtx, name: &str) -> Result<Vec<u8>, String> {
    let enc = ctx
        .client
        .download_by_name(name)
        .await
        .map_err(|e| format!("chunk indirilemedi ({name}): {e}"))?;
    let plain = decrypt_chunk(&ctx.keys.enc_key, name, &enc).map_err(|e| format!("chunk çözülemedi ({name}): {e}"))?;

    // Chunk adı = HMAC(plaintext). Uyuşmuyorsa chunk takas edilmiş/bozulmuştur.
    if hex::encode(hash_chunk_name(&ctx.keys.hmac_key, &plain)) != name {
        return Err(format!("chunk bütünlük kontrolü başarısız ({name})"));
    }
    Ok(plain)
}

// ---------------------------------------------------------------------------
// Yerel dosya sistemi yardımcıları
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct LocalFile {
    rel_path: String,
    abs_path: PathBuf,
    size: u64,
    modified_ms: u64,
}

struct ScanResult {
    files: Vec<LocalFile>,
    errors: usize,
}

/// Bloklayıcı; `spawn_blocking` içinde çağır. Sembolik linkler takip edilmez/atlanır.
fn scan_local(root: &Path) -> Result<ScanResult, String> {
    if !root.is_dir() {
        return Err(format!("Senkronizasyon klasörü bulunamadı: {}", root.display()));
    }

    let mut files = Vec::new();
    let mut errors = 0usize;

    for entry in WalkDir::new(root) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Tarama hatası: {e}");
                errors += 1;
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }

        if entry.path().extension().is_some_and(|ext| ext == "connectsync-part") {
            continue;
        }
        // Dahili ConnectSync meta dosyaları — sync dışı
        let fname = entry.file_name();
        if fname == ".connectsync-revision" {
            continue;
        }

        let Some(rel_path) = rel_path_string(root, entry.path()) else {
            eprintln!("Geçersiz/UTF-8 olmayan yol atlandı: {}", entry.path().display());
            errors += 1;
            continue;
        };

        let md = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Metadata okunamadı ({rel_path}): {e}");
                errors += 1;
                continue;
            }
        };

        files.push(LocalFile {
            rel_path,
            abs_path: entry.path().to_path_buf(),
            size: md.len(),
            modified_ms: mtime_ms(&md),
        });
    }

    Ok(ScanResult { files, errors })
}

/// Yolu bileşenlerinden `/` ile birleştirir (platformdan bağımsız, lossy dönüşüm yok).
fn rel_path_string(root: &Path, full: &Path) -> Option<String> {
    let rel = full.strip_prefix(root).ok()?;
    let mut parts: Vec<&str> = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_str()?),
            _ => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

/// Manifest'ten gelen yolu `base` altında güvenle birleştirir; `..`, mutlak yol,
/// Windows sürücü öneki, boş bileşen gibi her şeyi reddeder.
fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = base.to_path_buf();
    for part in rel.split('/') {
        if cfg!(windows)
            && (part.contains(':')
                || part.eq_ignore_ascii_case("CON")
                || part.eq_ignore_ascii_case("PRN")
                || part.eq_ignore_ascii_case("AUX")
                || part.eq_ignore_ascii_case("NUL")
                || (part.len() == 4 && part[..3].eq_ignore_ascii_case("COM") && part.as_bytes()[3].is_ascii_digit())
                || (part.len() == 4 && part[..3].eq_ignore_ascii_case("LPT") && part.as_bytes()[3].is_ascii_digit()))
            {
                return None;
            }
        let mut comps = Path::new(part).components();
        match (comps.next(), comps.next()) {
            (Some(Component::Normal(_)), None) => out.push(part),
            _ => return None,
        }
    }
    Some(out)
}

fn stat_file(p: &Path) -> std::io::Result<(u64, u64)> {
    let md = fs::metadata(p)?;
    Ok((md.len(), mtime_ms(&md)))
}

/// Milisaniye çözünürlüğü (saniye çözünürlüğü aynı saniyedeki değişiklikleri kaçırırdı).
fn mtime_ms(md: &fs::Metadata) -> u64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn tmp_path(dest: &Path) -> PathBuf {
    let mut name = dest
        .file_name()
        .unwrap_or_else(|| OsStr::new("file"))
        .to_os_string();
    name.push(".connectsync-part");
    dest.with_file_name(name)
}

// ---------------------------------------------------------------------------
// Testler
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrency_default_matches_previous_constants() {
        // Ayar eklenmeden önceki sabitler: 3 dosya / 4 yükleme.
        let c = Concurrency::from_threads(crate::limits::DEFAULT_THREADS);
        assert_eq!(c, Concurrency { files: 3, uploads: 4 });
    }

    #[test]
    fn concurrency_clamps_out_of_range_values() {
        let low = Concurrency::from_threads(0);
        assert_eq!(low, Concurrency { files: 1, uploads: 1 });
        let high = Concurrency::from_threads(10_000);
        assert_eq!(high.uploads, crate::limits::MAX_THREADS as usize);
    }

    #[test]
    fn concurrency_never_starves_and_is_monotonic() {
        let mut prev = Concurrency::from_threads(crate::limits::MIN_THREADS);
        for t in crate::limits::MIN_THREADS..=crate::limits::MAX_THREADS {
            let c = Concurrency::from_threads(t);
            assert!(c.files >= 1 && c.uploads >= 1, "t={t}");
            assert!(c.files <= c.uploads, "t={t}");
            assert!(c.files >= prev.files && c.uploads >= prev.uploads, "t={t}");
            prev = c;
        }
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let base = Path::new("/tmp/restore");
        assert!(safe_join(base, "a/b.txt").is_some());
        assert!(safe_join(base, "../etc/passwd").is_none());
        assert!(safe_join(base, "a/../../x").is_none());
        assert!(safe_join(base, "/abs").is_none());
        assert!(safe_join(base, "a//b").is_none());
        assert!(safe_join(base, "./a").is_none());
        assert!(safe_join(base, "").is_none());
    }

    #[test]
    fn rel_path_uses_forward_slashes() {
        let rel = rel_path_string(Path::new("/r"), Path::new("/r/a/b.txt"));
        assert_eq!(rel.as_deref(), Some("a/b.txt"));
        assert!(rel_path_string(Path::new("/r"), Path::new("/r")).is_none());
    }

    #[test]
    fn inflight_locks_share_and_cleanup() {
        let locks = InflightLocks::default();
        let a = locks.get("x");
        let b = locks.get("x");
        assert!(Arc::ptr_eq(&a, &b));

        locks.release("x", b); // map + a = 2 -> tutulur
        assert_eq!(locks.map.lock().unwrap().len(), 1);

        locks.release("x", a); // sadece map'te kalıyor -> silinir
        assert_eq!(locks.map.lock().unwrap().len(), 0);
    }

    #[test]
    fn tmp_path_appends_suffix() {
        let t = tmp_path(Path::new("/x/y/file.txt"));
        assert_eq!(t, PathBuf::from("/x/y/file.txt.connectsync-part"));
    }
}
