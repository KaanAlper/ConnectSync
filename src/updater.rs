//! Uygulama içi güncelleme.
//!
//! Akış: GitHub Releases'teki en son sürümü bul → yeniyse indir (ilerleme sayaçlarıyla) →
//! SHA-256 doğrula → çalışan ikiliyi yerinde değiştir → istenince yeniden başlat.
//!
//! Güvenlik/sağlamlık kuralları:
//! - `.sha256` dosyası olmayan sürüm reddedilir (doğrulanamayan ikili çalıştırılmaz).
//! - İndirme, hedefle AYNI dizindeki gizli bir dosyaya yapılır; doğrulanmadan asla `exe`'ye
//!   dokunulmaz. Değiştirme tek bir `rename` (Linux'ta atomik; çalışan süreç eski inode'la sürer).
//! - `cargo` derleme dizinindeki (target/debug|release) ikili ASLA değiştirilmez: geliştirme
//!   derlemesi kazara CI sürümüyle ezilmesin.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub const REPO: &str = "KaanAlper/ConnectSync";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// (major, minor, patch): demet karşılaştırması sürüm sırasıyla aynıdır.
pub type Version = (u64, u64, u64);

// ---------------------------------------------------------------------------
// Sürüm
// ---------------------------------------------------------------------------

/// "v1.0.12", "1.0.12", "1.2.3-beta+abc" → (1,0,12) / (1,2,3). Geçersizse `None`.
pub fn parse_version(s: &str) -> Option<Version> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let version = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(version)
}

pub fn format_version(v: Version) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

pub fn current_version() -> Version {
    parse_version(CURRENT_VERSION).unwrap_or((0, 0, 0))
}

// ---------------------------------------------------------------------------
// Hata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    Network(String),
    Release(String),
    NoAsset(String),
    NoChecksum(String),
    Checksum,
    Io(String),
    Unsupported,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(e) => write!(f, "GitHub'a ulaşılamadı: {e}"),
            Self::Release(e) => write!(f, "Sürüm bilgisi okunamadı: {e}"),
            Self::NoAsset(name) => write!(f, "Bu sürümde bu sistem için dosya yok ({name})"),
            Self::NoChecksum(name) => {
                write!(f, "{name} için .sha256 dosyası yok; doğrulanamadığından indirme reddedildi")
            }
            Self::Checksum => write!(f, "İndirilen dosya bozuk (SHA-256 uyuşmuyor)"),
            Self::Io(e) => write!(f, "Dosya hatası: {e}"),
            Self::Unsupported => write!(f, "Bu sistemde uygulama içi güncelleme desteklenmiyor"),
        }
    }
}

impl std::error::Error for UpdateError {}

fn net(e: reqwest::Error) -> UpdateError {
    UpdateError::Network(e.to_string())
}

fn io(e: std::io::Error) -> UpdateError {
    UpdateError::Io(e.to_string())
}

// ---------------------------------------------------------------------------
// Sürüm bilgisi (GitHub Releases)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub version: Version,
    pub asset_name: String,
    pub asset_url: String,
    pub sha256_url: Option<String>,
    pub size: u64,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Bu sistem için yayınlanan ham ikili dosyanın adı (release.yml ile aynı olmalı).
pub fn platform_asset_name() -> Option<&'static str> {
    if cfg!(target_os = "linux") {
        Some("ConnectSync-Linux")
    } else if cfg!(target_os = "windows") {
        Some("ConnectSync-Windows-Portable.exe")
    } else {
        None
    }
}

/// `releases/latest` JSON'unu bu sistemin dosyasına indirger.
pub fn release_from_json(json: &str, asset_name: &str) -> Result<Release, UpdateError> {
    let gh: GhRelease = serde_json::from_str(json).map_err(|e| UpdateError::Release(e.to_string()))?;
    let version = parse_version(&gh.tag_name)
        .ok_or_else(|| UpdateError::Release(format!("anlaşılmayan sürüm etiketi: {}", gh.tag_name)))?;
    let asset = gh
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| UpdateError::NoAsset(asset_name.to_string()))?;
    let sha_name = format!("{asset_name}.sha256");
    let sha256_url = gh
        .assets
        .iter()
        .find(|a| a.name == sha_name)
        .map(|a| a.browser_download_url.clone());
    Ok(Release {
        tag: gh.tag_name,
        version,
        asset_name: asset.name.clone(),
        asset_url: asset.browser_download_url.clone(),
        sha256_url,
        size: asset.size,
    })
}

fn http_client() -> Result<reqwest::Client, UpdateError> {
    reqwest::Client::builder()
        .user_agent(format!("ConnectSync/{CURRENT_VERSION}"))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(net)
}

/// En son sürümü sorgular; yalnızca çalışan sürümden YENİYSE `Some` döner.
pub async fn check_latest() -> Result<Option<Release>, UpdateError> {
    let asset = platform_asset_name().ok_or(UpdateError::Unsupported)?;
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = http_client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(net)?
        .error_for_status()
        .map_err(net)?
        .text()
        .await
        .map_err(net)?;
    let release = release_from_json(&body, asset)?;
    Ok((release.version > current_version()).then_some(release))
}

// ---------------------------------------------------------------------------
// İndirme + doğrulama
// ---------------------------------------------------------------------------

/// İndirme ilerlemesi: indirme görevi yazar, arayüz zamanlayıcısı okur (kilitsiz).
#[derive(Default)]
pub struct DownloadProgress {
    pub downloaded: AtomicU64,
    pub total: AtomicU64,
}

impl DownloadProgress {
    /// (indirilen, toplam) bayt.
    pub fn snapshot(&self) -> (u64, u64) {
        (self.downloaded.load(Ordering::Relaxed), self.total.load(Ordering::Relaxed))
    }
}

/// "<64 hex>  dosya" (sha256sum çıktısı) ya da yalın hash → küçük harfli hash.
pub fn parse_sha256(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?.trim_start_matches('*');
    (token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit())).then(|| token.to_ascii_lowercase())
}

/// Hedefle aynı dizindeki gizli geçici dosya (aynı dosya sisteminde → `rename` atomik olur).
pub fn staging_path(exe: &Path) -> PathBuf {
    let name = exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    exe.with_file_name(format!(".{name}.update"))
}

/// Sürümü `dest`'e indirir ve SHA-256'sını doğrular. Hata olursa `dest` silinir.
pub async fn download_verified(
    release: &Release,
    dest: &Path,
    progress: &DownloadProgress,
) -> Result<(), UpdateError> {
    let result = download_inner(release, dest, progress).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(dest).await;
    }
    result
}

async fn download_inner(
    release: &Release,
    dest: &Path,
    progress: &DownloadProgress,
) -> Result<(), UpdateError> {
    let sha_url = release
        .sha256_url
        .as_deref()
        .ok_or_else(|| UpdateError::NoChecksum(release.asset_name.clone()))?;
    let client = http_client()?;

    let sha_text = client
        .get(sha_url)
        .send()
        .await
        .map_err(net)?
        .error_for_status()
        .map_err(net)?
        .text()
        .await
        .map_err(net)?;
    let expected = parse_sha256(&sha_text)
        .ok_or_else(|| UpdateError::NoChecksum(format!("{} (biçim geçersiz)", release.asset_name)))?;

    let mut resp = client
        .get(&release.asset_url)
        .send()
        .await
        .map_err(net)?
        .error_for_status()
        .map_err(net)?;
    progress.downloaded.store(0, Ordering::Relaxed);
    progress.total.store(resp.content_length().unwrap_or(release.size), Ordering::Relaxed);

    let mut file = tokio::fs::File::create(dest).await.map_err(io)?;
    let mut hasher = Sha256::new();
    while let Some(chunk) = resp.chunk().await.map_err(net)? {
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(io)?;
        progress.downloaded.fetch_add(chunk.len() as u64, Ordering::Relaxed);
    }
    file.flush().await.map_err(io)?;
    drop(file);

    if hex::encode(hasher.finalize()) == expected {
        Ok(())
    } else {
        Err(UpdateError::Checksum)
    }
}

// ---------------------------------------------------------------------------
// Kurulum + yeniden başlatma
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    Replaced,
    /// `cargo` derleme dizinindeki ikili: dosya bilerek değiştirilmedi.
    SkippedDevBuild,
}

/// `.../target/debug/...` ya da `.../target/<hedef>/release/...` gibi bir yol mu?
pub fn is_cargo_target_dir(exe: &Path) -> bool {
    let parts: Vec<_> = exe.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    parts
        .iter()
        .position(|p| p == "target")
        .is_some_and(|i| parts[i + 1..].iter().any(|p| p == "debug" || p == "release"))
}

/// Doğrulanmış `staged` dosyasını çalışan `exe`'nin yerine koyar.
pub fn install(staged: &Path, exe: &Path) -> Result<InstallOutcome, UpdateError> {
    if is_cargo_target_dir(exe) {
        let _ = std::fs::remove_file(staged);
        return Ok(InstallOutcome::SkippedDevBuild);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(staged, std::fs::Permissions::from_mode(0o755)).map_err(io)?;
    }
    if cfg!(windows) {
        // Windows çalışan .exe'nin üzerine yazmaz ama adını değiştirmeye izin verir.
        let old = exe.with_extension("old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(exe, &old).map_err(io)?;
        if let Err(e) = std::fs::rename(staged, exe) {
            let _ = std::fs::rename(&old, exe); // geri al
            return Err(io(e));
        }
    } else {
        std::fs::rename(staged, exe).map_err(io)?;
    }
    Ok(InstallOutcome::Replaced)
}

/// Linux'ta üzerine yazılmış çalışan ikilinin yolu "… (deleted)" ile biter; eki at.
pub fn clean_exe_path(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_suffix(" (deleted)")) {
        Some(clean) => PathBuf::from(clean),
        None => p,
    }
}

/// Çalışan ikilinin yolu. Kurulumdan ÖNCE alınmalı (sonra Linux'ta "(deleted)" eki gelir).
pub fn current_exe_path() -> Result<PathBuf, UpdateError> {
    std::env::current_exe().map(clean_exe_path).map_err(io)
}

/// Yeni ikiliyi başlatır; çağıran hemen ardından çıkmalıdır. Linux'ta eski süreç çıkıp tepsi
/// simgesini bırakabilsin diye 1 sn gecikmeli başlar.
pub fn restart(exe: &Path) -> Result<(), UpdateError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cmd;
    if cfg!(unix) {
        cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg("sleep 1; exec \"$0\" \"$@\"").arg(exe).args(&args);
    } else {
        cmd = std::process::Command::new(exe);
        cmd.args(&args);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(io)
}

/// Önceki güncellemeden kalan geçici/eski dosyaları temizler (en iyi çaba).
pub fn cleanup_stale_files() {
    if let Ok(exe) = current_exe_path() {
        let _ = std::fs::remove_file(staging_path(&exe));
        let _ = std::fs::remove_file(exe.with_extension("old"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    // ── sürüm ────────────────────────────────────────────────────────────────

    #[test]
    fn parses_release_tags_and_plain_versions() {
        assert_eq!(parse_version("v1.0.12"), Some((1, 0, 12)));
        assert_eq!(parse_version("1.0.12"), Some((1, 0, 12)));
        assert_eq!(parse_version(" V0.2.0 "), Some((0, 2, 0)));
        assert_eq!(parse_version("1.2.3-beta+build5"), Some((1, 2, 3)));
    }

    #[test]
    fn rejects_malformed_versions() {
        for bad in ["", "v", "1.0", "1.0.0.0", "a.b.c", "1..2", "latest"] {
            assert_eq!(parse_version(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn versions_compare_numerically_not_lexically() {
        // 1.0.9 < 1.0.10 (metin sırasıyla tersi çıkardı)
        assert!(parse_version("v1.0.10") > parse_version("v1.0.9"));
        assert!(parse_version("v1.1.0") > parse_version("v1.0.99"));
        assert!(parse_version("v2.0.0") > parse_version("v1.99.99"));
    }

    #[test]
    fn crate_version_is_parseable() {
        assert!(parse_version(CURRENT_VERSION).is_some(), "{CURRENT_VERSION}");
    }

    // ── GitHub JSON ─────────────────────────────────────────────────────────

    const RELEASE_JSON: &str = r#"{
        "tag_name": "v1.0.42",
        "assets": [
            {"name": "ConnectSync-Linux", "browser_download_url": "https://x/ConnectSync-Linux", "size": 1234},
            {"name": "ConnectSync-Linux.sha256", "browser_download_url": "https://x/ConnectSync-Linux.sha256", "size": 90},
            {"name": "ConnectSync-Windows-Portable.exe", "browser_download_url": "https://x/win.exe", "size": 999}
        ]
    }"#;

    #[test]
    fn picks_the_asset_and_checksum_for_this_platform() {
        let r = release_from_json(RELEASE_JSON, "ConnectSync-Linux").unwrap();
        assert_eq!(r.version, (1, 0, 42));
        assert_eq!(r.tag, "v1.0.42");
        assert_eq!(r.asset_url, "https://x/ConnectSync-Linux");
        assert_eq!(r.sha256_url.as_deref(), Some("https://x/ConnectSync-Linux.sha256"));
        assert_eq!(r.size, 1234);
    }

    #[test]
    fn checksum_is_optional_in_the_json_but_flagged_missing() {
        let r = release_from_json(RELEASE_JSON, "ConnectSync-Windows-Portable.exe").unwrap();
        assert_eq!(r.sha256_url, None);
    }

    #[test]
    fn missing_asset_and_bad_json_are_errors() {
        assert_eq!(
            release_from_json(RELEASE_JSON, "ConnectSync-MacOS").unwrap_err(),
            UpdateError::NoAsset("ConnectSync-MacOS".into())
        );
        assert!(matches!(release_from_json("{", "x"), Err(UpdateError::Release(_))));
        assert!(matches!(
            release_from_json(r#"{"tag_name": "nightly", "assets": []}"#, "x"),
            Err(UpdateError::Release(_))
        ));
    }

    // ── sha256 satırı ───────────────────────────────────────────────────────

    #[test]
    fn parses_sha256sum_output() {
        let h = "a".repeat(64);
        assert_eq!(parse_sha256(&format!("{h}  ConnectSync-Linux\n")), Some(h.clone()));
        assert_eq!(parse_sha256(&format!("{}  x", h.to_uppercase())), Some(h.clone()));
        assert_eq!(parse_sha256(&format!("{h} *ConnectSync-Linux")), Some(h.clone()));
        assert_eq!(parse_sha256(&h), Some(h));
    }

    #[test]
    fn rejects_malformed_sha256() {
        assert_eq!(parse_sha256(""), None);
        assert_eq!(parse_sha256("deadbeef  x"), None);
        assert_eq!(parse_sha256(&format!("{}  x", "g".repeat(64))), None);
    }

    // ── yollar ──────────────────────────────────────────────────────────────

    #[test]
    fn strips_the_deleted_suffix_linux_adds_after_replacement() {
        assert_eq!(
            clean_exe_path(PathBuf::from("/home/u/.local/bin/connectsync (deleted)")),
            PathBuf::from("/home/u/.local/bin/connectsync")
        );
        assert_eq!(clean_exe_path(PathBuf::from("/a/b")), PathBuf::from("/a/b"));
    }

    #[test]
    fn detects_cargo_build_directories_only() {
        assert!(is_cargo_target_dir(Path::new("/home/u/ConnectSync/target/debug/connect_sync")));
        assert!(is_cargo_target_dir(Path::new("/home/u/ConnectSync/target/release/connect_sync")));
        assert!(is_cargo_target_dir(Path::new("/w/target/x86_64-unknown-linux-gnu/release/app")));
        assert!(!is_cargo_target_dir(Path::new("/home/u/.local/bin/connectsync")));
        assert!(!is_cargo_target_dir(Path::new("/home/u/target/notes/app"))); // debug/release yok
        assert!(!is_cargo_target_dir(Path::new("/opt/release/connectsync"))); // "target" yok
    }

    #[test]
    fn staging_file_sits_next_to_the_exe() {
        let s = staging_path(Path::new("/home/u/.local/bin/connectsync"));
        assert_eq!(s, PathBuf::from("/home/u/.local/bin/.connectsync.update"));
    }

    // ── kurulum ─────────────────────────────────────────────────────────────

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("connectsync-updater-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    #[test]
    fn install_replaces_the_binary_and_makes_it_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("install");
        let exe = dir.join("connectsync");
        std::fs::write(&exe, b"old").unwrap();
        let staged = staging_path(&exe);
        std::fs::write(&staged, b"new").unwrap();

        assert_eq!(install(&staged, &exe), Ok(InstallOutcome::Replaced));
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert!(!staged.exists());
        assert_eq!(std::fs::metadata(&exe).unwrap().permissions().mode() & 0o111, 0o111);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_never_touches_a_cargo_build_binary() {
        let dir = temp_dir("dev");
        let build = dir.join("target").join("debug");
        std::fs::create_dir_all(&build).unwrap();
        let exe = build.join("connect_sync");
        std::fs::write(&exe, b"dev build").unwrap();
        let staged = staging_path(&exe);
        std::fs::write(&staged, b"release").unwrap();

        assert_eq!(install(&staged, &exe), Ok(InstallOutcome::SkippedDevBuild));
        assert_eq!(std::fs::read(&exe).unwrap(), b"dev build");
        assert!(!staged.exists(), "geçici dosya temizlenmeli");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── indirme (yerel sahte sunucu) ────────────────────────────────────────

    /// Yol → gövde tablosuyla HTTP/1.1 yanıtlayan minimal sunucu. Taban URL döner.
    fn serve(routes: Vec<(&'static str, Vec<u8>)>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { break };
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                match routes.iter().find(|(p, _)| *p == path) {
                    Some((_, body)) => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = s.write_all(head.as_bytes());
                        let _ = s.write_all(body);
                    }
                    None => {
                        let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
            }
        });
        format!("http://{addr}")
    }

    fn release_at(base: &str, with_checksum: bool) -> Release {
        Release {
            tag: "v1.0.99".into(),
            version: (1, 0, 99),
            asset_name: "ConnectSync-Linux".into(),
            asset_url: format!("{base}/asset"),
            sha256_url: with_checksum.then(|| format!("{base}/asset.sha256")),
            size: 0,
        }
    }

    fn sha_of(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    // ── gerçek GitHub'a karşı (elle: cargo test -- --ignored live) ───────────

    #[tokio::test]
    #[ignore = "ağ gerektirir"]
    async fn live_check_finds_a_release_for_this_platform() {
        let release = check_latest().await.expect("GitHub sorgusu").expect("yerel 0.x sürümü 1.x'ten eski olmalı");
        assert!(release.version > current_version());
        assert!(release.sha256_url.is_some(), "release'te .sha256 yok: {release:?}");
        assert_eq!(Some(release.asset_name.as_str()), platform_asset_name());
    }

    #[tokio::test]
    #[ignore = "ağ gerektirir, ~37 MB indirir"]
    async fn live_download_of_the_real_release_verifies() {
        let release = check_latest().await.unwrap().unwrap();
        let dir = temp_dir("live");
        let dest = dir.join("real-download");
        let progress = DownloadProgress::default();

        download_verified(&release, &dest, &progress).await.expect("indirme+doğrulama");

        let (done, total) = progress.snapshot();
        assert!(done > 1_000_000 && done == total, "{done}/{total}");
        assert_eq!(std::fs::metadata(&dest).unwrap().len(), done);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn downloads_verifies_and_reports_progress() {
        let body = b"pretend this is a 30 MB binary".to_vec();
        let base = serve(vec![
            ("/asset", body.clone()),
            ("/asset.sha256", format!("{}  ConnectSync-Linux\n", sha_of(&body)).into_bytes()),
        ]);
        let dir = temp_dir("dl-ok");
        let dest = dir.join("out");
        let progress = DownloadProgress::default();

        download_verified(&release_at(&base, true), &dest, &progress).await.unwrap();

        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert_eq!(progress.snapshot(), (body.len() as u64, body.len() as u64));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_corrupted_download_is_rejected_and_deleted() {
        let base = serve(vec![
            ("/asset", b"tampered".to_vec()),
            ("/asset.sha256", format!("{}  ConnectSync-Linux\n", sha_of(b"original")).into_bytes()),
        ]);
        let dir = temp_dir("dl-bad");
        let dest = dir.join("out");

        let err = download_verified(&release_at(&base, true), &dest, &DownloadProgress::default())
            .await
            .unwrap_err();

        assert_eq!(err, UpdateError::Checksum);
        assert!(!dest.exists(), "doğrulanamayan dosya diskte kalmamalı");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_release_without_checksum_is_refused_before_downloading() {
        let dir = temp_dir("dl-nosha");
        let dest = dir.join("out");
        let err = download_verified(&release_at("http://127.0.0.1:1", false), &dest, &DownloadProgress::default())
            .await
            .unwrap_err();
        assert!(matches!(err, UpdateError::NoChecksum(_)), "{err:?}");
        assert!(!dest.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_missing_asset_is_a_network_error_and_leaves_no_file() {
        let body = b"x".to_vec();
        let base = serve(vec![(
            "/asset.sha256",
            format!("{}  f\n", sha_of(&body)).into_bytes(),
        )]); // /asset → 404
        let dir = temp_dir("dl-404");
        let dest = dir.join("out");

        let err = download_verified(&release_at(&base, true), &dest, &DownloadProgress::default())
            .await
            .unwrap_err();

        assert!(matches!(err, UpdateError::Network(_)), "{err:?}");
        assert!(!dest.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
