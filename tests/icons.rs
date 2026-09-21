//! Windows ikon dosyaları GERÇEK .ico olmalı. `rc.exe` (winres) ve NSIS, uzantısı .ico yapılmış bir
//! PNG'yi reddeder ve Windows derlemesini düşürür (RC2175 "not in 3.00 format"). CI `cargo test`
//! çalıştırmadığı için bu hata ancak Windows işi kırılınca fark ediliyordu; bu test yerelde yakalar.

use std::fs;
use std::path::Path;

fn u16_at(b: &[u8], i: usize) -> usize {
    u16::from_le_bytes([b[i], b[i + 1]]) as usize
}

fn u32_at(b: &[u8], i: usize) -> usize {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize
}

#[test]
fn every_ico_asset_is_a_real_icon_file() {
    let mut checked = Vec::new();
    for entry in fs::read_dir("assets").expect("assets/ okunamadı") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ico") {
            continue;
        }
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() >= 6, "{path:?}: çok kısa");
        // ICONDIR: rezerve=0, tür=1 (ICO), görsel sayısı. PNG dosyası 89 50 4E 47 ile başlar.
        assert_eq!(
            (u16_at(&bytes, 0), u16_at(&bytes, 2)),
            (0, 1),
            "{path:?} gerçek bir ICO değil (uzantısı değiştirilmiş bir PNG olabilir)"
        );
        let count = u16_at(&bytes, 4);
        assert!(count >= 1, "{path:?}: görsel yok");
        // Her ICONDIRENTRY (16 bayt) dosyanın içinde kalan bir veri bloğunu göstermeli.
        for i in 0..count {
            let e = 6 + 16 * i;
            assert!(e + 16 <= bytes.len(), "{path:?}: giriş tablosu kesik");
            let (size, offset) = (u32_at(&bytes, e + 8), u32_at(&bytes, e + 12));
            assert!(size > 0 && offset + size <= bytes.len(), "{path:?}: giriş {i} dosya dışına taşıyor");
        }
        checked.push(path);
    }
    // build.rs (winres) ve installer.nsi bu dosyaya başvurur.
    assert!(
        checked.iter().any(|p| p == Path::new("assets/app_icon.ico")),
        "assets/app_icon.ico bulunamadı ya da .ico değil: {checked:?}"
    );
}
