# Google `drive` yetkisi için başvuru rehberi

**Amaç:** Farklı Google hesapları arasında sync koduyla paylaşımı mümkün kılmak (arkadaşın kodunu verir,
sen kendi hesabınla bağlanırsın). Bunun için ConnectSync'in `drive` (tüm Drive) yetkisini istemesi gerekir.

**Etiketler:** ✅ = Google'ın resmi belgesinde okundu · ⚠️ = bilgi var ama doğrulanmadı, Cloud Console'da / Google'dan teyit et.

## 0. Neden gerekli (kısa)

- Şu an uygulama `drive.file` istiyor. Google belgesi (✅): *"Create new Drive files, or modify existing files,
  that you open with an app or that the user shares with an app while using the Google Picker API or the app's
  file picker."* Başka hesabın klasörüne kod yapıştırarak erişmek bu kapsamla **mümkün değil**; API `404` döner.
- Klasördeki "linki olan herkes" izni yalnızca **tarayıcı** erişimi sağlar, uygulamayı kurtarmaz.
- `drive` yetkisi bunu çözer ama Google bunu **kısıtlı (restricted) kapsam** sayar (✅) ve uygulama kullanıcının
  **tüm Drive'ına** erişir.

## 1. Kodda hazır olanlar

- **Ayarlar → Gelişmiş → "Başkalarıyla paylaşım için tüm Drive erişimi"** anahtarı. Varsayılan **kapalı**
  (`drive.file`); mevcut kullanıcılar etkilenmez.
- Her yetki profilinin belirteci **ayrı** saklanır (`google_token_v2` / `google_token_v3_full`). Böylece kapsam
  değişince eski dar belirteç yanlışlıkla yeniden kullanılmaz; anahtar açılınca yeniden giriş istenir, eski
  belirteç silinmez (geri dönünce tekrar giriş gerekmez).
- Profil tanımı: `crates/connectsync-core/src/scopes.rs` (testli).

## 2. Doğrulama sürerken ne olur? (yayın durumun: **In production**)

Projenin yayın durumu **"In production"** olduğu için "Test users" listesi ve 7 günlük belirteç ömrü **geçerli değil**
(✅ 7 günlük ömür yalnızca "Testing" durumuna aittir). Bunun yerine:

1. Cloud Console → **OAuth consent screen / Data access** → `https://www.googleapis.com/auth/drive` kapsamını ekle.
   Mevcut kapsamlar (`drive.file`, `drive.appdata`) etkilenmez; yeni kapsam **"doğrulama gerekli"** olarak işaretlenir.
2. Doğrulama bitene kadar kapsamı isteyen kullanıcı Google'ın **"Bu uygulama doğrulanmadı"** ekranını görür; *Gelişmiş →
   devam et* ile geçebilir. ⚠️ Doğrulanmamış uygulamalar için toplam kullanıcı sınırı vardır (bildiğim kadarıyla 100);
   teyit et. Yani sen ve arkadaşların için pratikte çalışır, geniş dağıtım için doğrulama şart.
3. Uygulamada anahtarı aç → yeniden giriş → arkadaşının koduyla bağlan.

⚠️ Bu davranışları (doğrulanmamış kapsamın uyarı ekranıyla kullanılabilmesi ve kullanıcı sınırı) Google'ın belgesinden
okumadım; ilk denemede ekranı gör ve gerekirse Cloud Console'daki "Verification Center" mesajını oku.

## 3. Üretim doğrulaması (kalıcı çözüm)

### 3.1 Uygunluk
✅ Drive için kısıtlı kapsam yalnızca şu türlere verilir: **Backup and sync** ("Platform-specific and web apps
that provide local sync or automatic backup of users' Drive files"), Productivity & education, Reporting &
security. **ConnectSync "Backup and sync" kategorisine girer.**

⚠️ Aynı belge `drive.file` + Picker'ı tavsiye ediyor. İnceleyici "neden Picker değil?" diye sorabilir; cevap
aşağıdaki gerekçe metninde (masaüstü uygulamasında Picker yok; kullanıcılar klasör kimliğini sync koduyla paylaşıyor).

### 3.2 Güvenlik değerlendirmesi
✅ Yıllık üçüncü taraf güvenlik değerlendirmesi, kısıtlı veriye **üçüncü taraf sunucu üzerinden** erişen uygulamalar
içindir. ConnectSync'in sunucusu **yok** (veri masaüstünden doğrudan Google Drive'a gider), bu yüzden büyük olasılıkla
gerekmez. ⚠️ Başvuruda Google net söyler; **"sunucumuz yok"** ifadesini açıkça yaz.

### 3.3 Marka doğrulaması
✅ ~2–3 iş günü. Gerekenler: uygulama adı, logo, kullanıcı destek e-postası, **ana sayfa**, **gizlilik politikası**
ve (varsa) hizmet şartları bağlantısı, yetkili alan adları. ⚠️ `github.io` alt alan adının "yetkili alan adı" olarak
kabul edilip edilmediği belirsiz; kendi alan adın varsa onu kullan.

### 3.4 Kapsam gerekçesi + demo videosu
✅ Demo videosu **YouTube'a "Unlisted" (liste dışı)** yüklenir, bağlantısı forma yazılır. Aşağıdaki hazır metinleri kullan.

### 3.5 Gizlilik politikası (Google bunu kodla karşılaştırır)
Politika şunları **doğru** söylemeli (önceki inceleme mevcut politikanın kodla çeliştiğini gösterdi):
- Hangi kapsamlar: `drive.file` **ve** `drive.appdata` (ve anahtar açıkken `drive`). "Sadece drive.file" yanlış.
- `drive.appdata` ne için: kullanıcının kendi Drive'ındaki gizli uygulama alanında `connectsync_keys.json`
  (sync kimliği → şifre anahtarı) tutulur; diğer bilgisayarlarda sync'leri bulmak içindir. ⚠️ Bu kayıt **şifrelenmeden**
  duruyor: "uçtan uca şifreli / Zero-Knowledge" ifadesi bununla çelişir, politikada ve README'de düzeltilmeli.
- `drive` (tüm Drive) ne için: yalnızca kullanıcının **eklediği sync klasörlerine** erişim; başka dosyalara
  bakılmaz/okunmaz/indirilmez. Bunu kodun gerçekten yaptığından emin ol (bkz. §5).
- Veri nerede: yalnızca kullanıcının cihazı ve kendi Google Drive'ı; **ConnectSync'in sunucusu yoktur**, geliştiriciye
  veri gönderilmez, üçüncü taraflarla paylaşılmaz.
- ⚠️ Kısıtlı kapsamlar için Google'ın **API Services User Data Policy / Limited Use** ifadesinin politikada yer alması
  beklenir; güncel metni Google belgesinden al.
- İletişim e-postası ve silme/iptal yolu (Google hesap izinlerinden erişimi kaldırma).

## 4. Yapıştırmaya hazır metinler (İngilizce)

### 4.1 Scope justification — `https://www.googleapis.com/auth/drive`
Cloud Console formundaki **"How will the scopes be used?"** alanı **en fazla 1000 karakter**. Aşağıdaki metin 896
karakter; olduğu gibi yapıştır:

> ConnectSync is a desktop file-sync client (local sync): it keeps a local folder in sync with a folder in the user's own Google Drive. All content is encrypted on the user's device before upload, and the developer runs no server. Users can share a sync code with another person so both sync the same encrypted folder from their own Google accounts. Joining a folder created by another account requires opening it by ID. drive.file cannot do this: it only exposes files the app created or that the user picked in Google Picker, a web widget that is unavailable in a native desktop client. We therefore need drive, used only to read, create, update and delete the encrypted chunk files inside sync folders the user explicitly adds, and to look up those folders by ID. The scope is off by default and requested only when the user turns on a setting. No other Drive files are listed, read or modified.

### 4.2 Scope justification — `https://www.googleapis.com/auth/drive.appdata`
> ConnectSync stores a small registry file (`connectsync_keys.json`) in the app's hidden Application Data folder in
> the user's own Drive. It maps sync-folder IDs to the sync codes so the user's other computers can rediscover their
> syncs. It is only readable by ConnectSync for that user.

### 4.3 How the data is handled
> ConnectSync has no backend server. Data flows directly between the user's device and Google Drive over HTTPS.
> The developer receives no user data. Data is not shared with third parties, not used for advertising, and not
> used to train models. Use of information received from Google APIs adheres to the Google API Services User Data
> Policy, including the Limited Use requirements.

## 5. Başvurudan ÖNCE koddan doğrulanması gerekenler

Gerekçede "başka dosyalara bakmıyoruz" diyeceksek kod bunu doğru yapmalı:
- [ ] `list_cloud_folders` sorgusu `name contains 'ConnectSync_'` ile **tüm Drive'da** arıyor. `drive` yetkisiyle bu,
      kullanıcıyla paylaşılan **başkalarının** klasörlerini de döndürebilir. Yalnızca anahtar kaydında bulunanları
      göstermek (mevcut davranış) doğru; ama arama kapsamının bilinçli olduğu kodda yorumlanmalı.
- [ ] `get_or_create_folder(name)` **ada göre** arıyor: tüm Drive'da aynı adlı bir klasörü eşleştirebilir. Yeni
      yetkiyle yanlış klasöre katılmamak için ada değil **kimliğe/anahtara** dayanmalı ya da eşleşme doğrulanmalı.
- [ ] Silme akışı (`delete_drive_sync`) yalnızca kullanıcının seçtiği sync klasörünü silmeli.

## 5b. Cloud Console "Restricted scopes" formu (yapıştırdığın ekrana göre)

- **Kapsam:** `.../auth/drive — See, edit, create, and delete all of your Google Drive files` ("Approval required").
- **"What features will you use?":** Seçenekleri bilmediğim için tahmin etmiyorum. Google'ın kategorisi **Backup and sync**
  (✅ belgede); seçenekler arasında buna en yakın olan ve dosyaları okuma/oluşturma/güncelleme/silmeyi kapsayanları seç.
- **Gerekçe:** §4.1 (896/1000 karakter).
- **Demo videosu:** YouTube bağlantısı (Unlisted). Formun notları:
  - **"Canlı uygulamada kaydetme, doğrulanmamış kapsamı üretim trafiğine açma"** (kullanıcı kotanı tüketir). ConnectSync'te
    kapsam **varsayılan kapalı** bir ayarın arkasında, yani normal kullanıcılar bu kapsamı hiç istemez ✓. Kaydı
    **yerelde geliştirme sürümüyle** (`cargo run --release`) ve **ayrı bir test Google hesabıyla** yap; yayına çıkmış
    sürümü kullanma.
  - **"Doğrulanmamış uygulama" ekranı test hesabında çıkacak; bu beklenen ve videoda GÖSTERİLMELİ** (*Gelişmiş → devam*).
  - **"Video, projeye atanmış TÜM OAuth istemcilerini içermeli":** Credentials sayfasında kaç OAuth istemcisi olduğunu
    kontrol et. Uygulama tek bir "installed" (masaüstü) istemci kullanıyor (`client_secret.json`); başka istemci varsa
    ya kullanılmayanları sil ya da hepsini videoda göster.

## 6. Demo videosu

### 6.0 Önce: video gerçekten gerekli mi?
✅ Google'ın "doğrulama gerekmeyen" durumları arasında **kişisel kullanım** var: uygulamayı yalnızca sen ya da **kişisel
olarak tanıdığın az sayıda kullanıcı** kullanıyorsa, doğrulanmamış uygulama ekranından geçip hesaplarına izin
verebilirsiniz. Yani **sen + arkadaşın için video ve inceleme olmadan** `drive` kapsamı çalışır (doğrulanmamış
kullanıcı kotası vardır). Video yalnızca uygulamayı **herkese** açmak istediğinde şart.

### 6.1 Google'ın resmi şartları (✅ restricted-scope-verification sayfası)
1. OAuth izin sürecini kullanıcının göreceği gibi, **İngilizce** göster (izin akışı dahil).
2. İzin ekranının **uygulama adını** (ConnectSync) doğru gösterdiğini göster.
3. İzin ekranında tarayıcı **adres çubuğunun OAuth istemci kimliğini** (`client_id=…`) içerdiğini göster.
4. İstediğin **her kısıtlı kapsamın** sağladığı işlevi göster (burada `drive`).
5. Birden fazla OAuth istemcin varsa **her biri için** veri erişimini göster.
- Video **YouTube'a "Unlisted"** yüklenir, bağlantı forma yazılır.
- Test hesabında çıkan **"doğrulanmamış uygulama" ekranı gösterilmelidir** (form da söylüyor).

### 6.2 Çekim öncesi hazırlık
- [ ] **İki Google test hesabı**: A (klasörü oluşturan) ve B (kodla bağlanan, `drive` kapsamını veren). B, arkadaşının hesabı da olabilir.
- [ ] Tarayıcı ve Google hesap dili **İngilizce**; uygulama dili de İngilizce (Ayarlar → dil).
- [ ] Anahtarlı sürümü yerelde çalıştır (`cargo run --release`), **yayındaki sürümü değil**.
- [ ] Credentials sayfasında **kaç OAuth istemcisi** olduğuna bak (uygulama yalnızca `client_secret.json`'daki masaüstü
      istemcisini kullanır). Birden fazla varsa kullanılmayanları sil ya da hepsini göster.
- [ ] Kişisel bilgi/gerçek dosya yok: demo klasörü ve örnek dosyalar kullan.

### 6.3 Çekim planı (~3–4 dk) — her adımı yavaş yap, önemli anlarda 2 sn dur
| # | Ne göster | Söyleyeceğin (İngilizce, kısa) |
|---|-----------|--------------------------------|
| 1 | Uygulama ana ekranı (İngilizce) | "This is ConnectSync, a desktop file-sync client." |
| 2 | **A hesabıyla** yeni sync oluştur (demo klasörü), sync kodunu kopyala | "User A creates an encrypted sync folder and gets a sync code." |
| 3 | Çıkış yap. **Ayarlar → Gelişmiş → "Full Drive access"** anahtarını aç (ipucu metni görünsün) | "To join a folder created by another Google account, the user turns on full Drive access." |
| 4 | **Giriş** → tarayıcı açılır. **Adres çubuğunu yakınlaştır: `client_id=1045169667398-…` görünsün.** Uygulama adını ve "unverified app" ekranını göster (*Advanced → Go to ConnectSync*) | "The consent screen shows the app name and our OAuth client ID in the address bar." |
| 5 | İzin ekranındaki kapsam listesini oku: "See, edit, create, and delete all of your Google Drive files" | "The app requests the drive scope so it can open a folder created by another account." |
| 6 | İzni ver, uygulamaya dön → **B hesabıyla, A'nın koduyla bağlan**: klasör bulunur, gerçek adı görünür, yerel klasör seç, dosyalar iner | "User B pastes User A's code; the app opens that folder by ID and downloads the files." |
| 7 | B'de yeni bir dosya ekle → eşitle → **tarayıcıda Drive'ı aç**: klasörde şifreli (okunamayan, hex adlı) parçalar görünsün | "Only encrypted chunks are stored; the content cannot be read in Drive." |
| 8 | Drive'ın kök dizinini göster: **başka hiçbir dosya değişmedi** | "The app only touches the sync folders the user adds; no other Drive files are read or modified." |
| 9 | Ayarlarda anahtarı kapat (isteğe bağlı) | "The setting is off by default." |

### 6.4 Kayıt aracı ve ipuçları
- Kayıt: OBS Studio (PipeWire ekran yakalama) ya da `wf-recorder -g "$(slurp)" -f demo.mp4` (Hyprland/wlroots). Çözünürlük 1080p.
- Tarayıcı penceresi **tam ekran olmasın**: adres çubuğu ve `client_id` okunabilir kalmalı. Gerekirse yakınlaştır.
- Konuşmak yerine **altyazı/ekran metni** de olur; ama izin akışı İngilizce olmalı.
- Süre uzun olmasın; her kısıtlı kapsamın ne yaptığı net olsun.

## 7. Senden gerekenler
- [x] Gizlilik politikası: `kaanalper.github.io/public/promo/connectsync/privacy.html` (iletişim e-postası ekli). `drive` için isteğe bağlı üçüncü kapsam ve Limited Use ifadesi yerelde eklendi; **yayınlamak için o repoda commit + push gerekir**.
- [x] Cloud projesinin yayın durumu: **In production**.
- [ ] Ana sayfa / alan adı (kendi alan adın var mı?).
- [ ] Destek/iletişim e-postası (herkese açık sayfaya yazılacağı için sen seç).
- [ ] Karar: kısıtlı kapsam reddedilirse **B planı** = yalnızca okuma (API anahtarıyla, ek izin gerektirmez) — bkz. sohbet.

## 8. Riskler
- Google "least privilege" ilkesiyle `drive` isteğini reddedebilir (`drive.file` + Picker öneriyor). Gerekçe metni
  bunu baştan cevaplıyor ama garanti yok.
- `drive` yetkisi "şifreli, izole tünel" mesajıyla gerilim yaratır: uygulama teknik olarak tüm Drive'a erişebilir.
  Bunu kullanıcıya açıkça söylemek (anahtarın ipucu metni bunu yapıyor) hem dürüst hem Google için gerekli.
- Yeni sürümlerde bu kapsam ya da kapsam kullanımı genişlerse yeniden doğrulama gerekir.
