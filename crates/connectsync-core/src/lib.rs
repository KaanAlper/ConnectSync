//! ConnectSync çekirdeği: uçtan uca şifreli senkronun arayüzden bağımsız kısmı.
//!
//! Bu crate MASAÜSTÜ ya da MOBİL'e özgü hiçbir şey içermez (pencere, tepsi, anahtar zinciri, OAuth
//! akışı, dosya seçici, dosya izleyici yok). Onlar uygulama crate'lerinde kalır:
//! - `connect_sync` (masaüstü, kök paket): `config`, `auth`, `watcher` + Slint arayüzü.
//! - `connectsync-mobile`: mobil giriş noktası.
//!
//! Modüller:
//! - [`crypto`]: anahtar türetme (Argon2id + HKDF), chunk şifreleme/adlandırma.
//! - [`manifest`]: dosya → chunk listesi kaydı.
//! - [`drive`]: Google Drive REST istemcisi (yeniden deneme, ilerleme, anahtar kaydı).
//! - [`engine`]: içerik tabanlı parçalama, push/pull, GC.
//! - [`progress`]: motorun yazdığı, arayüzün okuduğu kilitsiz ilerleme sayaçları.
//! - [`limits`]: eşzamanlı aktarım ayarı sınırları.
//! - [`scopes`]: Google Drive yetki (OAuth kapsamı) profilleri.

pub mod crypto;
pub mod drive;
pub mod engine;
pub mod limits;
pub mod manifest;
pub mod progress;
pub mod scopes;
