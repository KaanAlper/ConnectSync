//! Sync ilerleme durumu: motor (drive/engine) yazar, arayüz zamanlayıcısı okur.
//! Kilitsiz sayaçlar (Atomic) kullanılır; arayüz thread'ini bloklamaz.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

pub const PHASE_IDLE: u8 = 0;
/// Index/manifest okuma, yerel tarama gibi boyut henüz bilinmeyen hazırlık aşaması.
pub const PHASE_PREPARE: u8 = 1;
pub const PHASE_PULL: u8 = 2;
pub const PHASE_PUSH: u8 = 3;

#[derive(Default)]
pub struct Progress {
    phase: AtomicU8,
    total_bytes: AtomicU64,
    done_bytes: AtomicU64,
    total_files: AtomicU64,
    done_files: AtomicU64,
    /// 0 = ağ sorunu yok; n>0 = üst üste n. yeniden deneme (bağlantı kopuk/ağ hatası olabilir)
    net_retry: AtomicU32,
    current_file: Mutex<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub phase: u8,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub total_files: u64,
    pub done_files: u64,
    pub net_retry: u32,
    pub current_file: String,
}

impl Progress {
    pub fn begin(&self, phase: u8, total_bytes: u64, total_files: u64) {
        self.total_bytes.store(total_bytes, Ordering::Relaxed);
        self.done_bytes.store(0, Ordering::Relaxed);
        self.total_files.store(total_files, Ordering::Relaxed);
        self.done_files.store(0, Ordering::Relaxed);
        self.phase.store(phase, Ordering::Relaxed);
    }

    pub fn finish(&self) {
        self.phase.store(PHASE_IDLE, Ordering::Relaxed);
        self.total_bytes.store(0, Ordering::Relaxed);
        self.done_bytes.store(0, Ordering::Relaxed);
        self.total_files.store(0, Ordering::Relaxed);
        self.done_files.store(0, Ordering::Relaxed);
        self.net_retry.store(0, Ordering::Relaxed);
        if let Ok(mut c) = self.current_file.lock() {
            c.clear();
        }
    }

    pub fn add_bytes(&self, n: u64) {
        self.done_bytes.fetch_add(n, Ordering::Relaxed);
    }

    pub fn file_done(&self) {
        self.done_files.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_current(&self, name: &str) {
        if let Ok(mut c) = self.current_file.lock() {
            c.clear();
            c.push_str(name);
        }
    }

    pub fn set_net_retry(&self, n: u32) {
        self.net_retry.store(n, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            phase: self.phase.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            done_bytes: self.done_bytes.load(Ordering::Relaxed),
            total_files: self.total_files.load(Ordering::Relaxed),
            done_files: self.done_files.load(Ordering::Relaxed),
            net_retry: self.net_retry.load(Ordering::Relaxed),
            current_file: self.current_file.lock().map(|c| c.clone()).unwrap_or_default(),
        }
    }
}

/// `?` ile erken dönüşlerde bile fazı/ilerlemeyi sıfırlar (RAII).
pub struct PhaseGuard(pub Arc<Progress>);

impl Drop for PhaseGuard {
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// Baytları insan-okur XDM/IDM tarzı biçime çevirir: "12.3 MB / 128.0 MB"
pub fn format_bytes_pair(done: u64, total: u64) -> String {
    format!("{} / {}", format_bytes(done), format_bytes(total))
}

pub fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let n = n as f64;
    if n >= GB {
        format!("{:.2} GB", n / GB)
    } else if n >= MB {
        format!("{:.1} MB", n / MB)
    } else if n >= KB {
        format!("{:.0} KB", n / KB)
    } else {
        format!("{n:.0} B")
    }
}
