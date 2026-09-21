//! Sync döngülerinin yaşam döngüsü kaydı: hangi döngü "güncel", hangisi duraklatıldı.
//!
//! Neden gerekli:
//! - **Duraklat / devam et:** Silme popup'ı açılırken çalışan döngü durdurulur. Popup HANGİ yolla
//!   kapanırsa kapansın (İptal düğmesi, Esc) döngü yalnızca gerçekten duraklatılmışsa yeniden
//!   başlar; hiç durdurulmamış bir döngü gereksizce kesilmez.
//! - **"Sync durduruldu" mesajı:** Yeniden başlatılan bir döngünün eski kopyası geç kalıp yeni
//!   döngünün durumunu ezmesin diye her döngüye bir nesil numarası verilir; yalnızca hâlâ güncel
//!   ve duraklatılmamış döngü durdurulduğunu bildirir.

use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct SyncControl {
    next_generation: u64,
    current: HashMap<String, u64>,
    paused: HashSet<String>,
}

impl SyncControl {
    /// Yeni bir döngü kaydeder ve nesil numarasını döner. Aynı sync'in önceki döngüsü artık "eski"
    /// sayılır; yeni döngü duraklatılmış değildir.
    pub fn begin_loop(&mut self, code: &str) -> u64 {
        self.next_generation += 1;
        self.current.insert(code.to_string(), self.next_generation);
        self.paused.remove(code);
        self.next_generation
    }

    /// Bu nesil, sync'in hâlâ kayıtlı (yerine yenisi başlamamış) döngüsü mü?
    pub fn is_current(&self, code: &str, generation: u64) -> bool {
        self.current.get(code) == Some(&generation)
    }

    pub fn mark_paused(&mut self, code: &str) {
        self.paused.insert(code.to_string());
    }

    pub fn is_paused(&self, code: &str) -> bool {
        self.paused.contains(code)
    }

    /// Duraklatılmışsa işareti kaldırır ve `true` döner (döngü yeniden başlatılmalı).
    /// Duraklatılmamışsa `false`: çağıran hiçbir şey yapmamalı.
    pub fn take_paused(&mut self, code: &str) -> bool {
        self.paused.remove(code)
    }

    /// Durdurulan döngü "Sync durduruldu" yazmalı mı? Yalnızca hâlâ güncel ve duraklatılmamışsa.
    pub fn should_report_stop(&self, code: &str, generation: u64) -> bool {
        self.is_current(code, generation) && !self.is_paused(code)
    }

    /// Sync kaldırıldı: bu koda ait tüm kayıtları unut.
    pub fn forget(&mut self, code: &str) {
        self.current.remove(code);
        self.paused.remove(code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_loop_gets_a_new_generation() {
        let mut c = SyncControl::default();
        let a = c.begin_loop("s1");
        let b = c.begin_loop("s2");
        let a2 = c.begin_loop("s1");
        assert!(a < b && b < a2);
    }

    #[test]
    fn a_restarted_loop_makes_the_old_one_stale() {
        let mut c = SyncControl::default();
        let old = c.begin_loop("s1");
        let new = c.begin_loop("s1");
        assert!(!c.is_current("s1", old));
        assert!(c.is_current("s1", new));
        // Başka bir sync'in döngüsü etkilenmez
        let other = c.begin_loop("s2");
        assert!(c.is_current("s2", other) && c.is_current("s1", new));
    }

    #[test]
    fn resume_only_happens_for_a_loop_that_was_actually_paused() {
        let mut c = SyncControl::default();
        c.begin_loop("s1");
        // Hiç duraklatılmadı (ör. silme popup'ı satırdaki çöp kutusundan açıldı): devam ettirme yok
        assert!(!c.take_paused("s1"));
        c.mark_paused("s1");
        assert!(c.is_paused("s1"));
        assert!(c.take_paused("s1"));
        // İkinci kez (ör. hem İptal hem Esc tetiklendi): yine hiçbir şey yapma
        assert!(!c.take_paused("s1"));
    }

    #[test]
    fn starting_a_loop_clears_the_paused_flag() {
        let mut c = SyncControl::default();
        c.begin_loop("s1");
        c.mark_paused("s1");
        c.begin_loop("s1");
        assert!(!c.is_paused("s1"));
    }

    #[test]
    fn stop_is_reported_only_by_the_current_unpaused_loop() {
        let mut c = SyncControl::default();
        let old = c.begin_loop("s1");
        // Kullanıcı durdurdu: güncel ve duraklatılmamış → bildir
        assert!(c.should_report_stop("s1", old));
        // Duraklatıldı: kendi "Duraklatıldı" mesajını ezmemeli
        c.mark_paused("s1");
        assert!(!c.should_report_stop("s1", old));
        c.take_paused("s1");
        // Yeniden başlatıldı: eski döngü geç kalıp yeninin durumunu ezmemeli
        let _new = c.begin_loop("s1");
        assert!(!c.should_report_stop("s1", old));
    }

    #[test]
    fn forgetting_a_sync_drops_everything() {
        let mut c = SyncControl::default();
        let g = c.begin_loop("s1");
        c.mark_paused("s1");
        c.forget("s1");
        assert!(!c.is_paused("s1"));
        assert!(!c.is_current("s1", g));
        assert!(!c.take_paused("s1"));
    }
}
