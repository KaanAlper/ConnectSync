//! Uygulamanın (masaüstü) senkron katmanı.
//!
//! Arayüzden/platformdan bağımsız kısımlar `connectsync-core` crate'inde yaşar ve buradan yeniden
//! dışa aktarılır: böylece `sync_core::engine::SyncEngine` gibi mevcut yollar değişmeden çalışır.
//! Burada yalnızca MASAÜSTÜNE özgü olanlar kalır:
//! - `config`: ayar dosyası (ProjectDirs) + anahtar zinciri (keyring).
//! - `auth`: Google OAuth akışı (tarayıcı + yerel yönlendirme) + anahtar zinciri.
//! - `watcher`: yerel klasör değişiklik izleyici.

pub use connectsync_core::{crypto, drive, engine, progress};

pub mod auth;
pub mod config;
pub mod watcher;
