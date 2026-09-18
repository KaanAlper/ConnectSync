use notify_debouncer_full::{new_debouncer, notify::*, Debouncer, RecommendedCache};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc as tokio_mpsc;
use std::error::Error;

pub struct LocalWatcher {
    folder_path: String,
}

impl LocalWatcher {
    pub fn new(folder_path: &str) -> Self {
        Self {
            folder_path: folder_path.to_string(),
        }
    }

    /// İşletim sisteminin dosya değişikliklerini dinler, debounce eder ve kanala yollar.
    /// Çağıran tarafın döndürülen `Debouncer` nesnesini saklaması gerekir, yoksa izleme durur.
    pub fn start_watching(
        &self,
        tx: tokio_mpsc::Sender<String>,
    ) -> std::result::Result<Debouncer<RecommendedWatcher, RecommendedCache>, Box<dyn Error>> {
        let (std_tx, std_rx) = std::sync::mpsc::channel();

        // 2 saniye debounce süresi
        let mut debouncer = new_debouncer(Duration::from_secs(2), None, std_tx)?;

        debouncer.watch(Path::new(&self.folder_path), RecursiveMode::Recursive)?;

        println!("Sürekli dinleme başlatıldı (debounce aktif): {}", self.folder_path);

        // Ayrı bir thread içinde bloklanan std_rx'i okuyup tokio kanalına iletiyoruz.
        // Bu sayede tokio worker thread'ini kilitlenmekten kurtarıyoruz.
        std::thread::spawn(move || {
            for res in std_rx {
                match res {
                    Ok(events) => {
                        for ev in events {
                            if matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                                for path in &ev.paths {
                                    // .connectsync-part dosyaları geçici; bunları push'a sokarsak
                                    // sonsuz döngü çıkar (watcher kendi yazdığımız tmp'yi tetikler)
                                    if path.extension().and_then(|e| e.to_str()) == Some("connectsync-part") {
                                        continue;
                                    }
                                    // Dahili meta dosyalar
                                    if path.file_name().and_then(|n| n.to_str()) == Some(".connectsync-revision") {
                                        continue;
                                    }
                                    if let Some(path_str) = path.to_str() {
                                        let _ = tx.blocking_send(path_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("İzleme hatası: {:?}", e),
                }
            }
        });

        Ok(debouncer)
    }
}
