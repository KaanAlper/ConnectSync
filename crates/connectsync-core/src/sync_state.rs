//! Cihaz başına "son eşitlenen durum" ve üç yönlü pull kararı.
//!
//! **Sorun:** Motor yalnızca uzak manifeste bakıyordu. Pull, yerel dosya manifestle birebir aynı değilse
//! üzerine yazıyordu; döngü her turda ÖNCE pull yaptığı için, henüz yüklenmemiş bir yerel düzenleme eski
//! uzak sürümle EZİLEBİLİRDİ (veri kaybı). Yerel değişiklik, uzak değişiklik ve çakışma ayırt edilemiyordu.
//!
//! **Çözüm:** Her dosya için son başarılı eşitlemedeki (boyut, değişme zamanı) bu cihazda saklanır
//! (`base`). Üç değer karşılaştırılır: yerel (`local`), son eşitlenen (`base`), uzak (`remote`).
//! Yerel dosya `base`'ten farklıysa kullanıcının henüz eşitlenmemiş bir değişikliği vardır ve ASLA sessizce
//! ezilmez.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// Sync klasörünün köküne yazılan durum dosyası (taramadan dışlanır; bkz. `engine::scan_local`).
pub const STATE_FILE: &str = ".connectsync-state.json";

/// Bir dosyanın kimliği: boyut + değişme zamanı (ms). Motorun "aynı dosya" ölçütüyle aynıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStat {
    pub size: u64,
    pub modified_ms: u64,
}

/// Bu cihazdaki dosyaların son başarılı eşitlemedeki durumu.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncState {
    #[serde(default)]
    files: HashMap<String, FileStat>,
}

impl SyncState {
    pub fn get(&self, rel: &str) -> Option<FileStat> {
        self.files.get(rel).copied()
    }

    pub fn set(&mut self, rel: &str, stat: FileStat) {
        self.files.insert(rel.to_string(), stat);
    }

    pub fn remove(&mut self, rel: &str) {
        self.files.remove(rel);
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    fn path(dir: &Path) -> PathBuf {
        dir.join(STATE_FILE)
    }

    /// Yoksa ya da bozuksa BOŞ döner. Boş durum "hiçbir dosyanın tabanı yok" demektir: motor farklı dosyaları
    /// ezmek yerine çakışma olarak sorar (güvenli taraf).
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(Self::path(dir))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Atomik yazar (geçici dosya + rename): yarım yazılmış durum dosyası bırakmaz. Geçici dosya
    /// `.connectsync-part` uzantısı taşır, yani taramada da dışlanır.
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        let json = serde_json::to_vec(self).map_err(io::Error::other)?;
        let tmp = dir.join(format!("{STATE_FILE}.connectsync-part"));
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, Self::path(dir))
    }
}

// ---------------------------------------------------------------------------
// Pull kararı
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullDecision {
    /// Yerel dosya uzakla birebir aynı.
    UpToDate,
    /// Uzak sürümü yerel dosyanın yerine yaz (yerel dosya yok ya da yalnızca uzak değişti).
    Restore,
    /// Yerel dosya değişmiş, uzak değişmemiş: dokunma, push yükleyecek.
    KeepLocal,
    /// Yerel de uzak da değişmiş (ya da taban bilinmiyor): dokunma, kullanıcıya sor.
    Conflict,
}

/// `local`: bu cihazdaki dosya (yoksa `None`); `base`: son eşitlemedeki durum; `remote`: manifestteki durum.
/// `overwrite == false` ise mevcut farklı dosya asla ezilmez.
pub fn decide_pull(
    local: Option<FileStat>,
    base: Option<FileStat>,
    remote: FileStat,
    overwrite: bool,
) -> PullDecision {
    let Some(local) = local else {
        return PullDecision::Restore; // yerelde yok: indir
    };
    if local == remote {
        return PullDecision::UpToDate;
    }
    if !overwrite {
        return PullDecision::KeepLocal;
    }
    match base {
        // Taban yok (durum dosyası yok/bozuk ya da bu dosya hiç eşitlenmedi): hangisinin yeni olduğu
        // bilinemez. Varsayılan "uzak kazanır" olsaydı yerel düzenleme kaybolurdu → sor.
        None => PullDecision::Conflict,
        // Yerel dosya son eşitlemeden beri değişmemiş, uzak değişmiş: güvenle güncelle.
        Some(base) if base == local => PullDecision::Restore,
        // Yerel değişmiş ve uzak, son eşitlemedeki haliyle duruyor: yalnızca yerel değişti.
        Some(base) if base == remote => PullDecision::KeepLocal,
        // İkisi de değişmiş.
        Some(_) => PullDecision::Conflict,
    }
}

// ---------------------------------------------------------------------------
// Yedek dosya adı
// ---------------------------------------------------------------------------

/// `a.txt` + `2026-09-22 01-30-05` → `a (yerel yedek 2026-09-22 01-30-05).txt`. Uzantı korunur (dosya
/// hâlâ doğru programla açılır). Çakışma kontrolü çağıranda: aynı ad varsa `unique_backup_path`.
pub fn backup_file_name(file_name: &str, stamp: &str) -> String {
    let path = Path::new(file_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(file_name);
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{stem} (yerel yedek {stamp}).{ext}"),
        None => format!("{stem} (yerel yedek {stamp})"),
    }
}

/// `dest`'in yanında henüz var olmayan bir yedek yolu üretir (aynı saniyede ikinci yedek `… (2)` alır).
pub fn unique_backup_path(dest: &Path, stamp: &str) -> PathBuf {
    let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("dosya");
    let dir = dest.parent().unwrap_or_else(|| Path::new("."));
    let first = dir.join(backup_file_name(name, stamp));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(backup_file_name(name, &format!("{stamp} ({n})"))))
        .find(|p| !p.exists())
        .expect("sonsuz aralıkta boş ad bulunur")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(size: u64, mtime: u64) -> FileStat {
        FileStat { size, modified_ms: mtime }
    }

    // ── pull kararı ─────────────────────────────────────────────────────────

    #[test]
    fn a_missing_local_file_is_restored() {
        assert_eq!(decide_pull(None, None, st(1, 1), true), PullDecision::Restore);
        assert_eq!(decide_pull(None, Some(st(1, 1)), st(1, 1), true), PullDecision::Restore);
    }

    #[test]
    fn an_identical_file_is_up_to_date_regardless_of_base() {
        let f = st(10, 100);
        for base in [None, Some(f), Some(st(9, 9))] {
            assert_eq!(decide_pull(Some(f), base, f, true), PullDecision::UpToDate);
        }
    }

    #[test]
    fn a_remote_only_change_updates_the_untouched_local_file() {
        // local == base (bu cihaz değiştirmedi), remote yeni → güvenle güncelle
        let base = st(10, 100);
        assert_eq!(decide_pull(Some(base), Some(base), st(20, 200), true), PullDecision::Restore);
    }

    #[test]
    fn a_local_only_edit_is_never_overwritten() {
        // BU, düzeltilen hata: yerel düzenleme (local != base) ve uzak sabit (remote == base).
        // Eskiden pull bunu eski uzak sürümle eziyordu; push'a hiç sıra gelmiyordu.
        let base = st(10, 100);
        let edited = st(15, 300);
        assert_eq!(decide_pull(Some(edited), Some(base), base, true), PullDecision::KeepLocal);
    }

    #[test]
    fn both_sides_changed_is_a_conflict() {
        let base = st(10, 100);
        let local = st(15, 300);
        let remote = st(20, 250);
        assert_eq!(decide_pull(Some(local), Some(base), remote, true), PullDecision::Conflict);
    }

    #[test]
    fn an_unknown_base_with_differing_files_is_a_conflict_not_an_overwrite() {
        // Durum dosyası yok (ilk çalıştırma/bozuk): kim yeni bilinemez → sor, ezme.
        assert_eq!(decide_pull(Some(st(15, 300)), None, st(20, 250), true), PullDecision::Conflict);
    }

    #[test]
    fn without_overwrite_an_existing_different_file_is_left_alone() {
        assert_eq!(decide_pull(Some(st(1, 1)), Some(st(1, 1)), st(2, 2), false), PullDecision::KeepLocal);
        assert_eq!(decide_pull(Some(st(1, 1)), None, st(2, 2), false), PullDecision::KeepLocal);
    }

    #[test]
    fn the_decision_table_never_overwrites_a_locally_changed_file() {
        // Özellik testi: yerel dosya base'ten farklıysa sonuç ASLA Restore olmamalı.
        let stats = [st(1, 1), st(2, 2), st(3, 3), st(1, 9)];
        for &local in &stats {
            for &base in &stats {
                for &remote in &stats {
                    let d = decide_pull(Some(local), Some(base), remote, true);
                    if local != base && local != remote {
                        assert_ne!(d, PullDecision::Restore, "local={local:?} base={base:?} remote={remote:?}");
                    }
                }
            }
        }
    }

    // ── durum dosyası ───────────────────────────────────────────────────────

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("connectsync-state-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn state_survives_a_save_load_round_trip() {
        let dir = temp_dir("rt");
        let mut s = SyncState::default();
        s.set("a.txt", st(10, 100));
        s.set("dir/b.bin", st(20, 200));
        s.save(&dir).unwrap();
        let back = SyncState::load(&dir);
        assert_eq!(back, s);
        assert_eq!(back.get("dir/b.bin"), Some(st(20, 200)));
        assert!(!dir.join(format!("{STATE_FILE}.connectsync-part")).exists(), "geçici dosya kalmamalı");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_or_corrupt_state_file_loads_as_empty() {
        let dir = temp_dir("bad");
        assert!(SyncState::load(&dir).is_empty());
        std::fs::write(dir.join(STATE_FILE), b"{ bozuk json").unwrap();
        assert!(SyncState::load(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_get_remove_work() {
        let mut s = SyncState::default();
        assert_eq!(s.get("x"), None);
        s.set("x", st(1, 2));
        assert_eq!((s.get("x"), s.len()), (Some(st(1, 2)), 1));
        s.set("x", st(3, 4)); // üzerine yazar
        assert_eq!(s.get("x"), Some(st(3, 4)));
        s.remove("x");
        assert!(s.is_empty());
    }

    // ── yedek adı ───────────────────────────────────────────────────────────

    #[test]
    fn backup_names_keep_the_extension() {
        let t = "2026-09-22 01-30-05";
        assert_eq!(backup_file_name("a.txt", t), "a (yerel yedek 2026-09-22 01-30-05).txt");
        assert_eq!(backup_file_name("rapor", t), "rapor (yerel yedek 2026-09-22 01-30-05)");
        assert_eq!(backup_file_name("archive.tar.gz", t), "archive.tar (yerel yedek 2026-09-22 01-30-05).gz");
        assert_eq!(backup_file_name(".hidden", t), ".hidden (yerel yedek 2026-09-22 01-30-05)");
        assert_eq!(backup_file_name("Müzik ve Şarkı.mp3", t), "Müzik ve Şarkı (yerel yedek 2026-09-22 01-30-05).mp3");
    }

    #[test]
    fn a_second_backup_in_the_same_second_gets_a_distinct_name() {
        let dir = temp_dir("bk");
        let dest = dir.join("a.txt");
        let first = unique_backup_path(&dest, "T");
        std::fs::write(&first, b"1").unwrap();
        let second = unique_backup_path(&dest, "T");
        assert_ne!(first, second);
        assert!(!second.exists());
        std::fs::write(&second, b"2").unwrap();
        let third = unique_backup_path(&dest, "T");
        assert!(![first, second].contains(&third));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
