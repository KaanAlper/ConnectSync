//! Eşzamanlı aktarım ayarının varsayılan/sınır değerleri. Motor (`engine`) ve uygulamaların ayar
//! ekranı aynı sınırları kullanır. Bellek üst sınırı için bkz. `engine.rs` başındaki not
//! (16'da ≈ 350 MiB).

pub const DEFAULT_THREADS: u32 = 4;
pub const MIN_THREADS: u32 = 1;
pub const MAX_THREADS: u32 = 16;

/// Kullanıcının girdiği/okunan değeri izin verilen aralığa çeker.
pub fn clamp_threads(threads: u32) -> u32 {
    threads.clamp(MIN_THREADS, MAX_THREADS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_to_the_allowed_range() {
        assert_eq!(clamp_threads(0), MIN_THREADS);
        assert_eq!(clamp_threads(4), 4);
        assert_eq!(clamp_threads(10_000), MAX_THREADS);
    }

    #[test]
    fn the_default_is_inside_the_range() {
        assert_eq!(clamp_threads(DEFAULT_THREADS), DEFAULT_THREADS);
    }
}
