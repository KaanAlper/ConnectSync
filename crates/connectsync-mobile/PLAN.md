# ConnectSync Mobil (Android) — Deneme (Spike) Planı

**Amaç:** Ürünü yazmadan önce, mobilin en riskli 4–5 sorusuna *kanıtla* cevap vermek. Bu bir "yapılabilir
mi?" denemesidir; tam arayüz, çeviri, tema, güncelleyici, Play Store ve iOS **kapsam dışıdır**.

Masaüstü (Linux/Windows) **bu plandan etkilenmez**: kök paket `connect_sync` olduğu gibi kalır, CI'ın
`cargo build --release` komutu ve `target/release/connect_sync` yolu değişmez.

## 1. Şu anki düzen (çalışma alanı)

```
ConnectSync/
├─ Cargo.toml                    kök paket = masaüstü uygulaması (connect_sync) + [workspace]
├─ src/                          masaüstü: Slint arayüzü, tepsi, config, auth, watcher
├─ crates/
│  ├─ connectsync-core/          ORTAK çekirdek: crypto, manifest, drive, engine, progress, limits
│  └─ connectsync-mobile/        mobil giriş noktası — ŞİMDİLİK İSKELET (bu plan)
```

- `connectsync-core` arayüzden ve platformdan bağımsızdır (pencere, tepsi, anahtar zinciri, OAuth, dosya
  seçici, dosya izleyici **yok**). Android'e taşınabilir kısım budur.
- Masaüstüne özgü kalanlar: `config` (ProjectDirs + keyring), `auth` (tarayıcı OAuth + keyring), `watcher`
  (notify), `tray-icon`, `gtk`, `auto-launch`, `arboard`, `rfd`, `updater`.
- Varsayılan üye yalnızca kök pakettir: `cargo build` mobili derlemez. Mobil için `-p connectsync-mobile`.

## 2. Riskler (en büyükten küçüğe)

Etiketler: **[doğrulandı]** = kaynağı okundu; **[doğrulanacak]** = bilgi var ama spike'ta kanıtlanmalı.

### R1 — Google girişi (en büyük engel)
- **[doğrulandı]** Google'ın yerel uygulama OAuth belgesi: *"Custom URI schemes are no longer supported on
  Android"* ve *mobil uygulamalarda loopback IP yönlendirmesi DEPRECATED*. Masaüstündeki akış
  (`yup-oauth2` InstalledFlow: tarayıcı + yerel port) Android'de **kullanılamaz**.
- Gerekli: Google'ın Android kimlik API'si (Play Services, Authorization API) — bu **Java/Kotlin** koddur.
  Rust'tan JNI ile ya da küçük bir Kotlin yardımcı sınıfıyla çağrılacak. Salt-Rust çözüm yok.
- `drive.file` + `drive.appdata` kapsamları için erişim belirteci alınır; `DriveClient` zaten
  `TokenProvider` alıyor (belirteç bitince çağırır) → provider'ın içi JNI çağrısı olur.
- Google Cloud'da ayrı bir **Android** OAuth istemcisi (paket adı + imza SHA-1) gerekir. `client_secret.json`
  Android'de kullanılmaz.
- **[doğrulanacak]** Belirtecin sessizce yenilenip yenilenmediği (cihazda refresh token tutulmaz; her seferinde
  `authorize()` çağrısı sessiz dönüyor mu?).

### R2 — Yerel klasör erişimi (depolama)
- Android'de uygulama rastgele yola erişemez (scoped storage). Motor gerçek dosya yollarıyla çalışır
  (`walkdir`, `tokio::fs`); `content://` URI'leri `std::fs` ile açılamaz.
- Seçenekler (spike hangisinin uygulanabilir olduğunu ölçer):
  - **S-A** Uygulamanın özel dizini (`getExternalFilesDir`): izin gerekmez, `std::fs` çalışır. Kullanıcı
    kendi klasörünü senkronlayamaz, yalnızca uygulama içi bir klasörü. *Deneme için en kolayı.*
  - **S-B** SAF (klasör seçici + `DocumentFile`): kullanıcı klasör seçer; dosyalar JNI ile okunup yazılır ya
    da uygulama dizinine kopyalanır. Motorun yol varsayımı için bir soyutlama gerekir.
  - **S-C** "Tüm dosyalara erişim" izni: kolay ama Play'de kısıtlı; yalnızca APK ile dağıtımda anlamlı.

### R3 — Arka plan senkronu
- **[doğrulanacak]** Android arka plandaki süreci sonlandırır. Periyodik iş için WorkManager (en sık ~15 dk)
  ya da ön plan servisi (Android 14+ türü ve izni beyan edilmeli). Şu an sync döngüsü sürekli çalışan bir
  tokio görevi; mobilde "tur başına bir kez çalış" modeline uyarlanmalı (motorun `run_sync_pull/push`'u zaten
  tek turluk çağrılardır).

### R4 — Sır saklama
- **[doğrulandı]** `android-native-keyring-store` v1.0.0 (keyring-core için Android deposu) crates.io'da var.
- **[doğrulanacak]** Başlatma gereksinimleri (JNI/Context). Masaüstündeki `config`/`auth` doğrudan `keyring`
  kullanıyor; mobilde saklama katmanı bir arayüzün arkasına alınmalı.

### R5 — Slint Android desteği
- **[doğrulandı]** Slint 1.18'de `backend-android-activity-06` özelliği var. Slint belgesi: `[lib] crate-type =
  ["cdylib"]`, giriş noktası `#[unsafe(no_mangle)] fn android_main(app: AndroidApp)` içinde
  `slint::android::init(app)`; deneme: `cargo apk run --target aarch64-linux-android --lib`.
- Elimizdeki rehber (`Rust_Slint_Tokio_Android_Rehberi`) bu üç şeyi (cdylib, özellik, `android_main`)
  anlatmıyor; yalnızca `cargo-apk` ve izinleri anlatıyor.
- Tokio/ANR: masaüstünde `#[tokio::main]` içinde Slint döngüsü çalışıyor. Mobilde `Runtime`'ı açıkça kurup
  `rt.enter()` ile bağlamı korumak, mevcut ~24 `tokio::spawn` çağrısını yeniden yazmadan çalışır **[doğrulanacak]**.

### R6 — Arayüz
- Dikey 400×560 pencere zaten telefon oranına yakın (iyi başlangıç). Ama: `no-frame` + özel başlık çubuğu ve
  pencere sürükleme masaüstüne özgü; 30 px'lik ikon düğmeleri dokunmatik için küçük (≥ 48 dp); hover
  durumları yok; ekran klavyesi.

### R7 — Ağ/TLS ve derleme
- **[doğrulanacak]** `reqwest` 0.13'ün Android'de sertifika deposunu nasıl bulduğu.
- Argon2 (64 MiB bellek) ve `fastcdc` ARM'da derlenir mi / süre kabul edilebilir mi.
- NDK, `rustup target add aarch64-linux-android` (ve isteğe bağlı diğer 3 hedef), `cargo-apk` ya da `xbuild`.

## 3. Deneme adımları (her adımın çıkış ölçütü)

| # | Adım | Başarı ölçütü |
|---|------|---------------|
| S0 | SDK/NDK, hedefler, `cargo-apk`, emülatör/cihaz | Boş Slint penceresi APK olarak açılır |
| S1 | `connectsync-core`'u `aarch64-linux-android` için derle | Derleme geçer; cihazda `derive_keys` süresi ölçülür (Argon2 64 MiB) |
| S2 | Google girişi (Kotlin/JNI) → erişim belirteci → `DriveClient` | Telefonda `list_cloud_folders()` sonucu ekrana yazılır |
| S3 | Depolama (önce S-A): motorla bir klasör push/pull | Masaüstünde oluşan sync telefona iner; telefonda eklenen dosya masaüstüne çıkar |
| S4 | Arka plan: WorkManager/ön plan servisi ile tek tur sync | Uygulama kapalıyken bir tur tamamlanır |
| S5 | Sır saklama: `android-native-keyring-store` | Sync kodu/belirteç yeniden başlatmada okunur |
| S6 | Karar raporu | Her risk için yeşil/sarı/kırmızı + gerekçe + kabaca iş büyüklüğü |

Sıra bilinçli: **S2 ve S3 en riskli olanlardır** ve başarısız olurlarsa geri kalan işin anlamı değişir. S1'den
sonra bunlar önce denenmeli; S4/S5 sonra.

## 4. Kapsam dışı
Tam arayüz, çeviriler, tema, uygulama içi güncelleme (mobilde mağaza/APK kurulumu; `updater` zaten
Android'de "desteklenmiyor" döner), iOS, Play Store yayını.

## 5. Kararlar (spike başlamadan netleşmeli)
1. **Java/Kotlin kabul mü?** Google girişi için kaçınılmaz.
2. **Dağıtım:** yalnızca APK (yan yükleme) mi, Play Store mu? Depolama izni (S-C) ve Google doğrulaması bunu etkiler.
3. **Mobilde ne olacak?** Tam senkron istemcisi mi, yoksa yalnızca "buluttan dosya indir/yükle" mi? İkincisi
   R2/R3'ü büyük ölçüde hafifletir.
4. **Minimum Android sürümü:** rehber 24 diyor; Identity Services için Play Services gereksinimi ayrıca kontrol edilmeli.

## 6. Kaynaklar
- Google, *OAuth 2.0 for iOS & Desktop Apps*: Android'de custom URI şeması desteklenmiyor; mobilde loopback DEPRECATED.
- Slint belgesi, *Android* (`backend-android-activity-06`, `android_main`, `slint::android::init`).
- crates.io: `android-native-keyring-store` 1.0.0; `slint` 1.18.0 özellikleri.
