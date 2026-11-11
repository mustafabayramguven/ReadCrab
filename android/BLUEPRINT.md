# ReadCrab Android — BLUEPRINT

Kapsam: kökteki sunucu BLUEPRINT.md'sine dokunulmaz. Bu belge yalnız Android istemcisi, `crates/core` ve ilgili CI/workspace değişikliklerini tanımlar.
Dil: arayüz Türkçe (strings.xml), kod yorumları ve commit mesajları Türkçe. İşçiler derleyemez; bu yüzden her imza birebir verilmiştir. Bir imzadan sapmak yasaktır.

## 0. Kapalı kararlar (özet)

| Konu | Karar |
|---|---|
| Depo | Cargo workspace: `.` (sunucu `readcrab`, kökte kalır) + `crates/core` (`readcrab_core`). Gradle projesi `android/`. |
| Çekirdek HTTP | `ureq =2.12.1` (bloklayan, rustls+ring, webpki-roots). tokio YOK. Tüm UniFFI fonksiyonları bloklayan; Kotlin `Dispatchers.IO` ile çağırır. |
| Köprü | UniFFI `=0.29.5` proc-macro modu. Arayüzde yalnız `i32/i64/f64/bool/String/Option/Vec/record/enum/object` (işaretsiz tür YASAK, Kotlin `UInt` doğurur). |
| Kotlin paketi | Uygulama `io.github.mustafabayramguven.readcrab`; üretilen binding `io.github.mustafabayramguven.readcrab.core` (`uniffi.toml`). |
| Oturum | Çerez Kotlin'de DEĞİL, çekirdekte, `kv` tablosunda `token`. Elle `Cookie: krs_session=<token>` gönderilir, `Set-Cookie` elle ayrıştırılır. |
| Çakışma kuralı | Kitap başına en yeni zaman damgası kazanır (saniye, UTC). Eşitlikte yerel kazanır. Damga: yerel değişiklik anı, sunucu saatine `Date` başlığı farkıyla düzeltilmiş. Sunucuda küçük değişiklik gerekir (S1, bölüm 3). |
| Yer imleri | Sunucu kimliği (`server_id`) ile eşleşir; yerelde eklenen/silinen kuyruğa girer; sunucuda silinen yerelden de silinir. |
| Çevrimdışı | Bölüm/sayfa tek tek mevcut API'den indirilir; diskte `books/{id}/...`. Önbellek = indirme satırı olmayan kitapların dosyaları. |
| Ayarlar saklama | UI tercihleri `SharedPreferences("rc_prefs")` (Kotlin, `Prefs.kt`). Çekirdek yalnız `kv` tablosunu kullanır. DataStore YOK. |
| DI | YOK. `RcApp` (Application) tek örnek tutar: `core`, `prefs`, `connectivity`, `downloads`. ViewModel'ler `AndroidViewModel` ile `getApplication<RcApp>()` çağırır. |
| Gezinme | `navigation-compose`, string rotalar. |
| Simge | `web/favicon.svg` yolundan türetilmiş adaptive vector (bölüm 6.10). |
| TLS | Mozilla kök sertifikaları (webpki-roots). Özel CA / kendi imzalı sertifika bu sürümde DESTEKLENMEZ (CEO kararı); TLS hatası girişte anlaşılır Türkçe metinle gösterilir (`login_error_tls`). Düz `http://` LAN adresi desteklenir (istemci Rust olduğu için Android cleartext ayarı gerekmez). |
| Çıkış | `logout()` sunucuya çıkış dener, sonra yerel TÜM veriyi (db + dosyalar) siler. |

### 0.1 Doğrulanmış sürümler (hepsi gerçekten var; tek tek kontrol edildi)

| Bileşen | Sürüm |
|---|---|
| Rust araç zinciri | 1.89.0 (mevcut `rust-toolchain.toml`) |
| uniffi | =0.29.5 |
| ureq | =2.12.1 (features: `tls`, `json`; default-features kapalı) |
| rusqlite | =0.37.0 bundled (sunucuyla aynı) |
| serde / serde_json / sha2 / hex / rand / percent-encoding | sunucuyla aynı: =1.0.219 / =1.0.143 / =0.10.9 / =0.4.3 / =0.8.5 / =2.3.2 |
| httpdate | =1.0.3 |
| tiny_http (dev) | =0.12.0 |
| tempfile (dev) | =3.22.0 (sunucuyla aynı) |
| cargo-ndk | 4.1.2 (`cargo install cargo-ndk --version 4.1.2 --locked`) |
| NDK | 27.2.12479018 (r27c) |
| Android platform / build-tools | android-35 / 35.0.0 |
| JDK | Temurin 17 |
| Gradle | 8.13 (CI'de `gradle/actions/setup-gradle@v4` ile kurulur; wrapper YOK) |
| AGP | 8.13.2 |
| Kotlin + compose compiler eklentisi | 2.2.21 |
| Compose BOM | 2025.05.01 (compileSdk 35 ile uyumlu son hat; daha yeni BOM'lar compileSdk 36 ister) |
| activity-compose | 1.10.1 |
| navigation-compose | 2.8.9 |
| lifecycle-viewmodel-compose, lifecycle-runtime-compose | 2.9.0 |
| core-ktx | 1.15.0 |
| work-runtime-ktx | 2.10.1 |
| androidx.webkit | 1.12.1 |
| coil-compose (Coil 3) | 3.2.0 |
| JNA | 5.17.0 (`@aar`) |
| kotlinx-coroutines-android | 1.10.2 |
| junit | 4.13.2 |
| GitHub Actions | `actions/checkout@v4`, `actions/setup-java@v4`, `gradle/actions/setup-gradle@v4`, `actions/upload-artifact@v4`, `Swatinem/rust-cache@v2`, `dtolnay/rust-toolchain@master` |

## 1. Dosya ağacı

Etiket: N = yeni, M = değişen. Görev numaraları bölüm 8'dedir.

```
Cargo.toml                                   M  [G1] workspace tablosu eklenir, package.resolver kaldırılır
Dockerfile                                   M  [G1] `COPY crates ./crates` eklenir
.dockerignore                                M  [G1] `android/` eklenir
.gitignore                                   M  [G1] android/crates çıktıları eklenir
.github/workflows/ci.yml                     M  [G1] fmt satırı `-p readcrab`
.github/workflows/lock.yml                   N  [G2] `android` dalında Cargo.toml değişince Cargo.lock'u aynı dala commit eden iş akışı
.github/workflows/android.yml                N  [G11] çekirdek testi + APK
src/api/tracking.rs                          M  [G3] S1: client_ts + updated_at
tests/tracking.rs                            M  [G3] S1 testleri

crates/core/Cargo.toml                       N  [G1]
crates/core/uniffi.toml                      N  [G1]
crates/core/uniffi-bindgen.rs                N  [G1] bindgen ikilisi
crates/core/src/lib.rs                       N  [G1 yer tutucu, G5 final] kayıt/enum türleri, Core yapısı, new, session/login/logout
crates/core/src/error.rs                     N  [G4] CoreError + HErr eşlemesi
crates/core/src/util.rs                      N  [G4] fold_tr, now/offset, client_id, gövde ayıklama, img yeniden yazma
crates/core/src/migrations/0001_init.sql     N  [G4] şema
crates/core/src/db.rs                        N  [G4] bağlantı açma, göç, kv yardımcıları
crates/core/src/http.rs                      N  [G5] ureq istemcisi, oturum, DTO'lar
crates/core/src/library.rs                   N  [G6] kütüphane yenileme + yerel sorgular
crates/core/src/sync.rs                      N  [G6] ilerleme/durum/yer imi/ping itme-çekme + birleştirme
crates/core/src/download.rs                  N  [G7] indirme makinesi + depolama
crates/core/src/reader.rs                    N  [G7] kapak/sayfa/bölüm/içindekiler/arama
crates/core/tests/common/mod.rs              N  [G9] tiny_http sahte sunucu
crates/core/tests/auth.rs                    N  [G10]
crates/core/tests/sync.rs                    N  [G10]
crates/core/tests/download.rs                N  [G10]

android/settings.gradle.kts                  N  [G12]
android/build.gradle.kts                     N  [G12]
android/gradle.properties                    N  [G12]
android/app/build.gradle.kts                 N  [G12]
android/app/src/main/AndroidManifest.xml     N  [G12]
android/app/src/main/res/values/strings.xml  N  [G12] bölüm 6.11 listesi
android/app/src/main/res/values/themes.xml   N  [G12]
android/app/src/main/res/values/ic_launcher_background.xml N [G12]
android/app/src/main/res/drawable/ic_launcher_foreground.xml N [G12]
android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml N [G12]
android/app/src/main/res/mipmap-anydpi-v26/ic_launcher_round.xml N [G12]
android/app/src/main/assets/reader.html      N  [G18]
android/app/src/main/assets/reader.js        N  [G18]
android/app/src/test/kotlin/io/github/mustafabayramguven/readcrab/ThemeTest.kt N [G13]

(Kotlin kökü: android/app/src/main/kotlin/io/github/mustafabayramguven/readcrab/  = KÖK)
KÖK/RcApp.kt                                 N  [G14]
KÖK/MainActivity.kt                          N  [G15] + `object VolumeKeys`
KÖK/Prefs.kt                                 N  [G13]
KÖK/Connectivity.kt                          N  [G13]
KÖK/image/CoreImage.kt                       N  [G14] Coil modeli, keyer, fetcher
KÖK/work/Workers.kt                          N  [G14] DownloadWorker, SyncWorker, Scheduler
KÖK/work/DownloadTracker.kt                  N  [G14]
KÖK/ui/theme/Theme.kt                        N  [G13] renkler, şemalar, tipografi, Dimens, okuyucu renkleri
KÖK/ui/common/Components.kt                  N  [G13]
KÖK/ui/nav/AppNav.kt                         N  [G15]
KÖK/ui/login/LoginScreen.kt                  N  [G15] ekran + LoginViewModel
KÖK/ui/settings/SettingsScreen.kt            N  [G15] ekran + SettingsViewModel
KÖK/ui/library/LibraryScreen.kt              N  [G16] ekran + LibraryViewModel
KÖK/ui/series/SeriesScreen.kt                N  [G16] ekran + SeriesViewModel
KÖK/ui/reader/ReaderCommon.kt                N  [G17] ReadingTracker, ReaderWindowEffects, BookmarksSheet
KÖK/ui/reader/comic/ComicReaderScreen.kt     N  [G17] ekran + ComicReaderViewModel
KÖK/ui/reader/comic/ZoomableBox.kt           N  [G17]
KÖK/ui/reader/text/TextReaderScreen.kt       N  [G18] ekran + TextReaderViewModel + sheet'ler
KÖK/ui/reader/text/ReaderWebView.kt          N  [G18]
```

Üretilen/ignorelanan: `android/app/src/main/jniLibs/` (cargo-ndk), `android/app/build/` (Gradle, binding `build/generated/uniffi/kotlin`, `build/webAssets`).

## 2. Workspace dönüşümü (G1, G2)

### 2.1 Kök `Cargo.toml`
`[package]` içinden `resolver = "3"` satırı SİLİNİR; `[package]`'dan sonra şu tablo eklenir (bağımlılıklara dokunulmaz):
```toml
[workspace]
members = [".", "crates/core"]
resolver = "3"
```
Kökte `default-members` YOK: `cargo build/test/clippy` (bayraksız) yalnız `readcrab` paketine işler; mevcut CI komutları aynen çalışır. `src/app.rs` içindeki `include_str!("../web/...")` yolları değişmez (sunucu kökte kalıyor).

### 2.2 `Dockerfile`
`COPY src ./src` satırından hemen sonra ekle: `COPY crates ./crates` (workspace üyesinin manifesti diskte olmak zorunda). Başka değişiklik yok; `cargo build --release --locked` aynen kalır (yalnız `readcrab` derlenir).

### 2.3 `.dockerignore`
Mevcut içeriğe (`target/`, `data/`, `.git/`) satır ekle: `android/`.

### 2.4 `.gitignore`
Ekle:
```
android/.gradle/
android/build/
android/app/build/
android/app/src/main/jniLibs/
android/local.properties
*.iml
.idea/
```

### 2.5 `.github/workflows/ci.yml`
Yalnız şu satır değişir: `- run: cargo fmt --all -- --check` → `- run: cargo fmt -p readcrab -- --check` (çekirdek kodu yerelde biçimlenemediği için sunucu fmt denetimi dışında bırakılır; çekirdek `android.yml` içinde `cargo fmt -p readcrab_core` ile CI'da biçimlenir). Diğer adımlar aynı kalır.

### 2.6 Cargo.lock (CEO adımı, G1 itildikten sonra)
Bu makinede cargo yok; `Cargo.lock` yeni üyenin bağımlılıklarını içermez. Çalışma dalı `android`'dir; `workflow_dispatch` yalnız iş akışı dosyası varsayılan dalda (main) varken tetiklenebildiğinden `lock.yml` `push` tetikleyicisiyle çalışır. `lock.yml`:
```yaml
name: lock
on:
  push:
    branches: [android]
    paths:
      - "Cargo.toml"
      - "crates/core/Cargo.toml"
permissions:
  contents: write
jobs:
  lock:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: 1.89.0
      # metadata yalnız eksik girdileri ekler, mevcut sürümleri oynatmaz
      - run: cargo metadata --format-version 1 > /dev/null
      - run: |
          if git diff --quiet Cargo.lock; then echo "Cargo.lock güncel"; exit 0; fi
          git config user.name "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git add Cargo.lock
          git commit -m "Cargo.lock: crates/core bağımlılıkları eklendi"
          git pull --rebase origin "${GITHUB_REF_NAME}"
          git push origin "HEAD:${GITHUB_REF_NAME}"
```
Davranış (kesin):
- `GITHUB_TOKEN` ile atılan commit hiçbir iş akışını (kendisi dahil) tetiklemez; ayrıca `Cargo.lock` yolu filtrede olmadığı için döngü olmaz.
- Tetik: `android` dalına itilen bir itmede kök `Cargo.toml` veya `crates/core/Cargo.toml` değişmişse. K1 (G1) bunu sağlar. Sonradan bir bağımlılık eklenirse aynı iş akışı kilidi kendiliğinden günceller.
- K1 itildiği anda `ci.yml` (`test`, `image`) ve `android.yml` (`core`) AYNI ANDA başlar ve eski `Cargo.lock` yüzünden `--locked` ile BEKLENEN şekilde kırmızı olur; `lock.yml` kilidi `android` dalına commit eder. Bu bot commit'i yeni koşu tetiklemez; yani K1 için yeşil koşu olmaz. Kesinleşen sıra: CEO yalnız K1'i iter → `lock` iş akışının bitmesini bekler (`gh run watch`) → yerelde `git pull` → K2 ve sonrasını iter. K2 itişi `ci.yml` ve `android.yml`'i yeni kilitle tetikler; bundan sonra tüm koşular yeşil olmalıdır. `ci.yml` ve `android.yml` bu ara durum için DEĞİŞTİRİLMEZ (`--locked` kalır; kırmızı K1 koşusu kabul edilmiştir).
- Dockerfile `--locked` kalır: `image` işi K1'de kırmızıdır, K2'den itibaren yeşildir.
- `main`'e birleştirmeden önce `android` dalındaki son `Cargo.lock` zaten commit'lidir; ek adım yok.

### 2.7 `crates/core/Cargo.toml`
```toml
[package]
name = "readcrab_core"
version = "0.1.0"
edition = "2021"
rust-version = "1.89"
license = "GPL-3.0-only"

[lib]
crate-type = ["cdylib", "lib"]

[features]
cli = ["uniffi/cli"]

[[bin]]
name = "uniffi-bindgen"
path = "uniffi-bindgen.rs"
required-features = ["cli"]

[dependencies]
uniffi           = { version = "=0.29.5" }
ureq             = { version = "=2.12.1", default-features = false, features = ["tls", "json"] }
rusqlite         = { version = "=0.37.0", features = ["bundled"] }
serde            = { version = "=1.0.219", features = ["derive"] }
serde_json       = "=1.0.143"
sha2             = "=0.10.9"
hex              = "=0.4.3"
rand             = "=0.8.5"
percent-encoding = "=2.3.2"
httpdate         = "=1.0.3"

[dev-dependencies]
tiny_http = "=0.12.0"
tempfile  = "=3.22.0"
```
`crates/core/uniffi-bindgen.rs`: `fn main() { uniffi::uniffi_bindgen_main() }`.
`crates/core/uniffi.toml`:
```toml
[bindings.kotlin]
package_name = "io.github.mustafabayramguven.readcrab.core"
cdylib_name = "readcrab_core"
```
`crates/core/src/lib.rs` G1 yer tutucu: tek satır `uniffi::setup_scaffolding!();` (G5'te tamamen yeniden yazılır).

## 3. Sunucu değişikliği S1 (G3) — tek değişiklik, gerekçeli

Gerekçe: `PUT /api/books/{id}/progress` ve `PUT /api/books/{id}/status` `updated_at`'i her zaman SUNUCU `now` ile yazar ve `GET /api/books/{id}/progress` `updated_at` döndürmez. Bu yüzden çevrimdışı kuyruktan geç gönderilen eski bir konum, başka cihazdaki daha yeni konumu ezer ve "en yeni kazanır" kuralı uygulanamaz. Eksik zaman damgası alanı + istemci damgası kabulü gerekir. Eski istemciler (web) etkilenmez: `client_ts` isteğe bağlı, yanıta yalnız alan eklenir.
Diğer engeller denetlendi, SIFIR değişiklik: Origin başlığı yoksa CSRF denetimi geçer (`origin_and_content_type_guard`); yalnız POST/PUT/DELETE için `Content-Type: application/json` zorunludur (gövdesiz `DELETE /api/bookmarks/{id}` ve `POST /api/logout` dahil, istemci her zaman gönderir); çerez elle taşınır (`Secure` bayrağı önemsiz).

### 3.1 `src/api/tracking.rs` değişiklikleri

1. `ProgressRes` yapısına `updated_at: i64` eklenir. `get_progress` sorgusu `SELECT locator, percent, status, updated_at FROM progress ...`; kayıt yoksa varsayılan `updated_at: 0`.
2. `PutProgressReq` ve `PutStatusReq` içine `#[serde(default)] client_ts: Option<i64>`.
3. `PutProgressRes`: `status: String, percent: f64, applied: bool, updated_at: i64`. `PutStatusRes`: `status: String, applied: bool, updated_at: i64`.
4. Doğrulama (db çağrısından önce, iki işleyicide): `if matches!(req.client_ts, Some(t) if t < 1) { return Err(AppError::BadRequest("Geçersiz client_ts.")); }`
5. `put_progress` db kapanışı (yalnız değişen kısımlar; geri kalan mantık aynı):
```rust
let prev: Option<(String, Option<String>, i64, f64)> = conn
    .query_row(
        "SELECT status, finished_date, updated_at, percent
         FROM progress WHERE user_id = ?1 AND book_id = ?2",
        rusqlite::params![user_id, id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .optional()?;
// istemci damgası gelecekten gelemez; yoksa sunucu saati
let ts = client_ts.map_or(now, |t| t.min(now));
if let Some((ref st, _, prev_ts, prev_pct)) = prev {
    if prev_ts > ts {
        return Ok((st.clone(), prev_pct, false, prev_ts));
    }
}
// ... mevcut (new_status, new_finished_date) hesabı, desenler 4'lüye uyarlanır:
//     Some((ref st, Some(ref d), _, _)) if st == "finished"  ve  Some((ref st, ref fin_date, _, _))
// INSERT ... ON CONFLICT: updated_at olarak `now` yerine `ts` bağlanır
Ok((new_status, percent, true, ts))
```
   Kapanışın dönüş türü `(String, f64, bool, i64)`; yanıt `Json(PutProgressRes { status, percent, applied, updated_at })`.
6. `put_status` aynı desen: `prev` 3'lü `(String, Option<String>, i64)` (`status, finished_date, updated_at`) okunur, `ts` hesaplanır, `prev_ts > ts` ise `Ok((st.clone(), false, prev_ts))` döner (yazmaz), aksi halde `updated_at = ts` yazılır ve `Ok((status, true, ts))` döner; yanıt `PutStatusRes { status, applied, updated_at }`.

### 3.2 `tests/tracking.rs` — eklenecek testler (mevcut `call`, `login_as_admin`, `test_app`, `setup_two_books` yardımcıları)
Her test `let (app, dirs) = test_app().await; let cookie = login_as_admin(&app).await; let (cbz_id, _) = setup_two_books(&dirs).await;` ile başlar.
- `progress_get_returns_updated_at`: kayıt yokken GET `updated_at == 0`; PUT sonrası `updated_at > 0`.
- `progress_put_with_client_ts_applies_when_newer`: PUT `{"locator":"1","percent":0.5,"local_date":"2026-11-08","client_ts":1000}` → `applied == true`, `updated_at == 1000`; ardından `client_ts: 2000` ile `locator:"2"` → `applied == true`, GET `locator == "2"`, `updated_at == 2000`.
- `progress_put_with_stale_client_ts_is_ignored`: önce `client_ts: 2000, locator "5"`; sonra `client_ts: 1500, locator "1"` → `applied == false`, yanıt `updated_at == 2000`; GET `locator == "5"`.
- `progress_put_equal_client_ts_applies`: aynı `client_ts` iki kez → ikincisi `applied == true` (yeniden deneme idempotent).
- `progress_put_future_client_ts_is_clamped`: `client_ts: 99999999999` → 200, yanıt `updated_at <= şimdi + 5` (testte `SystemTime` ile hesapla).
- `progress_put_invalid_client_ts_rejected`: `client_ts: 0` → 400.
- `status_put_respects_client_ts`: `client_ts: 3000` ile `finished`; sonra `client_ts: 2000` ile `unread` → `applied == false`, GET status `finished`.
- `progress_put_without_client_ts_keeps_old_behavior`: `client_ts` yok → `applied == true`, `updated_at` ≈ şimdi.

## 4. Çekirdek (`crates/core`)

### 4.1 Mimari
- Tek UniFFI nesnesi `Core` (`Arc`). İçeride: `db: Mutex<rusqlite::Connection>`, `http: http::Http`, `root: PathBuf`, `stop: AtomicBool` (indirme), `cancel: Mutex<HashSet<i64>>`, `dl_running: AtomicBool`.
- Kural: HTTP çağrısı sırasında `db` kilidi TUTULMAZ. Önce ağdan topla, sonra tek işlemde (transaction) yaz.
- Paralellik: `std::thread::scope` ile en çok 4 iş parçacığı (kütüphane yenileme) / 3 (indirme). Yardımcı: `util::par_map<T: Send, R: Send>(items: Vec<T>, n: usize, f: impl Fn(T) -> R + Sync) -> Vec<R>` (sırayı korur).
- `data_dir` = Kotlin'in verdiği `filesDir/rc`. Düzen: `readcrab.db`, `books/{id}/p{n}` (çizgi roman sayfası, ham bayt), `books/{id}/c{n}.html` (bölüm gövdesi), `books/{id}/a/{ad}` (bölüm görselleri), `covers/{id}-{v}.jpg`. `Core::new` `books/` ve `covers/` dizinlerini oluşturur ve `downloads.state='running'` satırlarını `queued` yapar.
- Zaman: `util::now(&Http)` = `SystemTime` sn + `clock_offset` (son görülen `Date` başlığı farkı; yoksa 0).
- `local_date` Kotlin'den `YYYY-MM-DD` gelir (`LocalDate.now().toString()`); çekirdek biçimi doğrulamaz (sunucu doğrular, 400 gelirse işlem kuyruktan düşer).

### 4.2 SQLite şeması — `crates/core/src/migrations/0001_init.sql` (user_version 1)
Açılışta: `busy_timeout 5000`, `journal_mode=WAL`, `foreign_keys=OFF`. `db.rs` sunucudaki `Db::open` kalıbını izler: `user_version < 1` ise tek işlemde `execute_batch(MIGRATION_0001)` + `user_version=1`.
```sql
CREATE TABLE kv(
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
-- anahtarlar: server_url, username, user_id, is_admin('0'/'1'), token,
--             sync_last_ok_at, sync_last_error

CREATE TABLE series(
  id INTEGER PRIMARY KEY,
  ord INTEGER NOT NULL,                 -- sunucu sort=name sırasındaki sıra
  name TEXT NOT NULL,
  folded_name TEXT NOT NULL,
  cover_book_id INTEGER,
  cover_v INTEGER NOT NULL DEFAULT 0,
  sig TEXT NOT NULL DEFAULT ''          -- liste imzası, bölüm 4.5
);
CREATE INDEX idx_series_ord ON series(ord);

CREATE TABLE books(
  id INTEGER PRIMARY KEY,
  series_id INTEGER NOT NULL,
  ord INTEGER NOT NULL,                 -- seri içi sıra (sunucu sırası)
  title TEXT NOT NULL,
  folded_title TEXT NOT NULL,
  format TEXT NOT NULL,                 -- epub|mobi|cbz|zip|rar|cbr
  page_count INTEGER NOT NULL,          -- çizgi romanda sayfa, metinde bölüm sayısı
  size INTEGER NOT NULL,
  error TEXT,
  cover_v INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_books_series ON books(series_id, ord);

CREATE TABLE progress(
  book_id INTEGER PRIMARY KEY,
  locator TEXT NOT NULL DEFAULT '',
  percent REAL NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'unread' CHECK(status IN('unread','reading','finished','dropped')),
  updated_at INTEGER NOT NULL DEFAULT 0,  -- son yerel/sunucu değişiklik anı (sn, sunucu saatine göre)
  local_date TEXT NOT NULL DEFAULT '',
  p_dirty INTEGER NOT NULL DEFAULT 0,     -- 1: ilerleme sunucuya gitmedi
  s_dirty INTEGER NOT NULL DEFAULT 0,     -- 1: durum sunucuya gitmedi
  last_op TEXT NOT NULL DEFAULT '',       -- 'p' | 's': son yerel işlem
  seen_sig TEXT NOT NULL DEFAULT ''       -- sunucu liste imzası "durum|yüzde4", bölüm 4.5
);
CREATE INDEX idx_progress_continue ON progress(status, updated_at);

CREATE TABLE bookmarks(
  local_id INTEGER PRIMARY KEY AUTOINCREMENT,
  server_id INTEGER UNIQUE,               -- NULL: sunucuda henüz yok
  book_id INTEGER NOT NULL,
  locator TEXT NOT NULL,
  label TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0      -- 1: yerelde silindi, sunucuya DELETE gidecek
);
CREATE INDEX idx_bookmarks_book ON bookmarks(book_id, created_at);

CREATE TABLE pings(
  client_id TEXT PRIMARY KEY,             -- 32 küçük harf hex
  book_id INTEGER NOT NULL,
  active_seconds INTEGER NOT NULL DEFAULT 0,
  pages_read INTEGER NOT NULL DEFAULT 0,
  start_percent REAL NOT NULL,
  end_percent REAL NOT NULL,
  local_date TEXT NOT NULL,
  sent_seconds INTEGER NOT NULL DEFAULT -1, -- sunucuya en son giden active_seconds; -1: hiç gitmedi
  closed INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE toc(
  book_id INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  title TEXT NOT NULL,
  PRIMARY KEY(book_id, idx)
);

CREATE TABLE downloads(
  book_id INTEGER PRIMARY KEY,
  state TEXT NOT NULL CHECK(state IN('queued','running','failed','done')),
  parts_total INTEGER NOT NULL DEFAULT 0,
  parts_done INTEGER NOT NULL DEFAULT 0,
  bytes INTEGER NOT NULL DEFAULT 0,
  error TEXT,
  updated_at INTEGER NOT NULL
);

CREATE TABLE parts(                        -- indirilmiş/önbelleğe alınmış parça (sayfa veya bölüm)
  book_id INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  bytes INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(book_id, idx)
);
```
`parts` satırı yalnız parça dosyası (ve bölümün görselleri) diske tamamen yazıldıktan sonra eklenir. Çizgi romanda `idx` sayfa indeksi, metinde bölüm indeksi.

### 4.3 Hata tipi
```rust
#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreError {
    Network(String),      // bağlantı kurulamadı, zaman aşımı
    Tls(String),          // TLS el sıkışması/sertifika doğrulaması başarısız (özel CA, kendi imzalı, süresi dolmuş)
    Unauthorized,         // 401 veya oturum yok
    Server(String),       // diğer 4xx/5xx; metin sunucunun "message" alanı ya da "HTTP <kod>"
    Offline,              // dosya yerelde yok ve ağ gerekli (okuyucu çağrıları)
    NotFound,             // yerel kayıt yok ya da sunucu 404
    InvalidInput(String), // doğrulama
    Storage(String),      // SQLite / dosya sistemi
}
```
`impl std::fmt::Display` (Türkçe, Kotlin'de `message`): `Network(m)` → `"Sunucuya ulaşılamadı: {m}"`; `Tls(m)` → `"Güvenli bağlantı kurulamadı: sunucunun sertifikası doğrulanamadı. Özel veya kendi imzalı sertifikalar desteklenmez. ({m})"`; `Unauthorized` → `"Oturum açmanız gerekiyor."`; `Server(m)` → `m`; `Offline` → `"Bağlantı yok ve içerik indirilmemiş."`; `NotFound` → `"Bulunamadı."`; `InvalidInput(m)` → `m`; `Storage(m)` → `"Depolama hatası: {m}"`. `impl std::error::Error`. `From<rusqlite::Error>` ve `From<std::io::Error>` → `Storage`.
Kotlin'de sınıflar: `CoreException.Network`, `.Tls`, `.Unauthorized`, `.Server`, `.Offline`, `.NotFound`, `.InvalidInput`, `.Storage` (hepsi `message` taşır).

İç HTTP hatası (UniFFI'ye açılmaz): `http::HErr { Net(String), Status { code: u16, error: String, message: String, retry_after: Option<u64> }, Parse(String) }` ve `fn into_core(e: HErr) -> CoreError`: `Net(m)`→ `m` küçük harfe çevrilince `certificate`, `unknownissuer`, `invalid peer`, `handshake`, `invalid dnsname` veya `tls` içeriyorsa `Tls(m)`, aksi halde `Network(m)`; `Status{401}`→`Unauthorized`; `Status{404}`→`NotFound`; diğer `Status`→`Server(message veya "HTTP kod")`; `Parse`→`Server("Sunucu yanıtı anlaşılamadı.")`.

### 4.4 UniFFI yüzeyi — `crates/core/src/lib.rs` (birebir)

```rust
uniffi::setup_scaffolding!();

mod db; mod download; mod error; mod http; mod library; mod reader; mod sync; mod util;
pub use error::CoreError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ReadStatus { Unread, Reading, Finished, Dropped }

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum DownloadState { None, Queued, Running, Failed, Done }

#[derive(Clone, Debug, uniffi::Record)]
pub struct SessionInfo { pub server_url: String, pub username: String, pub user_id: i64, pub is_admin: bool }

#[derive(Clone, Debug, uniffi::Record)]
pub struct DownloadInfo { pub book_id: i64, pub state: DownloadState, pub parts_done: i32, pub parts_total: i32, pub bytes: i64, pub error: Option<String> }

#[derive(Clone, Debug, uniffi::Record)]
pub struct SeriesItem {
    pub id: i64, pub name: String, pub book_count: i32, pub read_count: i32, pub reading_count: i32,
    pub downloaded_count: i32, pub percent: f64, pub cover_book_id: Option<i64>, pub cover_v: i64,
    pub last_read_at: Option<i64>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ContinueItem { pub book_id: i64, pub title: String, pub series_id: i64, pub series_name: String, pub is_comic: bool, pub percent: f64, pub cover_v: i64 }

#[derive(Clone, Debug, uniffi::Record)]
pub struct BookItem {
    pub id: i64, pub series_id: i64, pub title: String, pub format: String, pub is_comic: bool,
    pub page_count: i32, pub size: i64, pub error: Option<String>, pub cover_v: i64,
    pub status: ReadStatus, pub percent: f64, pub download: DownloadInfo,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SeriesDetail { pub id: i64, pub name: String, pub books: Vec<BookItem> }

#[derive(Clone, Debug, uniffi::Record)]
pub struct BookDetail {
    pub book: BookItem, pub series_name: String, pub locator: String,
    pub next_book_id: Option<i64>, pub next_book_title: Option<String>, pub prev_book_id: Option<i64>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ProgressInfo { pub locator: String, pub percent: f64, pub status: ReadStatus }

#[derive(Clone, Debug, uniffi::Record)]
pub struct BookmarkItem { pub local_id: i64, pub locator: String, pub label: String, pub created_at: i64, pub pending: bool }

#[derive(Clone, Debug, uniffi::Record)]
pub struct TocEntry { pub index: i32, pub title: String }

#[derive(Clone, Debug, uniffi::Record)]
pub struct TextInfo { pub chapter_count: i32, pub toc: Vec<TocEntry> }

#[derive(Clone, Debug, uniffi::Record)]
pub struct SearchHitItem { pub ch: i32, pub p: f64, pub before: String, pub hit: String, pub after: String }

#[derive(Clone, Debug, uniffi::Record)]
pub struct SearchResults { pub hits: Vec<SearchHitItem>, pub truncated: bool }

#[derive(Clone, Debug, uniffi::Record)]
pub struct SyncReport { pub pushed: i32, pub pulled_books: i32 }

#[derive(Clone, Debug, uniffi::Record)]
pub struct SyncStatus { pub pending_ops: i32, pub last_ok_at: Option<i64>, pub last_error: Option<String> }

#[derive(Clone, Debug, uniffi::Record)]
pub struct DownloadRun { pub finished: i32, pub failed: i32 }

#[derive(Clone, Debug, uniffi::Record)]
pub struct StorageUsage { pub downloads_bytes: i64, pub cache_bytes: i64, pub covers_bytes: i64, pub db_bytes: i64 }

#[derive(uniffi::Object)]
pub struct Core { /* bölüm 4.1 alanları */ }

#[uniffi::export]
impl Core {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<std::sync::Arc<Core>, CoreError>;

    // --- oturum ---
    pub fn session(&self) -> Option<SessionInfo>;                       // kv'den; ağ yok
    pub fn login(&self, server_url: String, username: String, password: String) -> Result<SessionInfo, CoreError>;
    pub fn logout(&self) -> Result<(), CoreError>;                      // sunucuya POST /api/logout dener (hata yutulur), sonra TÜM yerel veri silinir

    // --- eşitleme ---
    pub fn sync_now(&self) -> Result<SyncReport, CoreError>;            // push + kütüphane yenile + ilerleme/yer imi çek
    pub fn sync_push(&self) -> Result<i32, CoreError>;                  // yalnız giden kuyruk; itilen işlem sayısı
    pub fn sync_status(&self) -> SyncStatus;
    pub fn pull_book(&self, book_id: i64) -> Result<(), CoreError>;     // tek kitabın ilerleme + yer imi; istek başına 5 sn zaman aşımı

    // --- kütüphane (yalnız yerel veri; ağ yok) ---
    pub fn list_series(&self, query: String, only_downloaded: bool) -> Result<Vec<SeriesItem>, CoreError>;
    pub fn continue_reading(&self) -> Result<Vec<ContinueItem>, CoreError>;
    pub fn get_series(&self, series_id: i64) -> Result<SeriesDetail, CoreError>;
    pub fn get_book(&self, book_id: i64) -> Result<BookDetail, CoreError>;

    // --- ilerleme (yalnız yerel yazma; kuyruğa girer) ---
    pub fn get_progress(&self, book_id: i64) -> Result<ProgressInfo, CoreError>;
    pub fn save_progress(&self, book_id: i64, locator: String, percent: f64, local_date: String) -> Result<(), CoreError>;
    pub fn set_status(&self, book_id: i64, status: ReadStatus, local_date: String) -> Result<(), CoreError>;

    // --- okuma süresi ---
    pub fn ping_start(&self, book_id: i64, start_percent: f64, local_date: String) -> Result<String, CoreError>; // client_id
    pub fn ping_update(&self, client_id: String, active_seconds: i64, pages_read: i64, end_percent: f64) -> Result<(), CoreError>;
    pub fn ping_close(&self, client_id: String) -> Result<(), CoreError>;

    // --- yer imleri (yerel yazma; kuyruğa girer) ---
    pub fn list_bookmarks(&self, book_id: i64) -> Result<Vec<BookmarkItem>, CoreError>;
    pub fn add_bookmark(&self, book_id: i64, locator: String, label: String) -> Result<BookmarkItem, CoreError>;
    pub fn delete_bookmark(&self, local_id: i64) -> Result<(), CoreError>;

    // --- indirme ---
    pub fn download_enqueue(&self, book_ids: Vec<i64>) -> Result<(), CoreError>;
    pub fn download_remove(&self, book_ids: Vec<i64>) -> Result<(), CoreError>;   // çalışan işi iptal eder, dosya+satır siler (önbellek dosyaları dahil)
    pub fn download_infos(&self) -> Result<Vec<DownloadInfo>, CoreError>;        // yalnız satırı olan kitaplar
    pub fn run_downloads(&self) -> Result<DownloadRun, CoreError>;               // BLOKLAR; kuyruk boşalana dek
    pub fn download_stop(&self);                                                 // çalışan run_downloads'ı nazikçe durdurur
    pub fn storage_usage(&self) -> Result<StorageUsage, CoreError>;
    pub fn clear_cache(&self) -> Result<i64, CoreError>;                         // serbest kalan bayt

    // --- okuyucu ---
    pub fn cover_path(&self, book_id: i64, version: i64) -> Result<String, CoreError>;
    pub fn page_path(&self, book_id: i64, page: i32) -> Result<String, CoreError>;
    pub fn text_info(&self, book_id: i64) -> Result<TextInfo, CoreError>;
    pub fn ensure_chapter(&self, book_id: i64, chapter: i32) -> Result<(), CoreError>;
    pub fn search_text(&self, book_id: i64, query: String) -> Result<SearchResults, CoreError>;
}
```
Dosya bölüşümü: `#[uniffi::export] impl Core` bloğu birden çok olabilir (UniFFI destekler). `lib.rs`: tüm kayıt/enum türleri, `Core` yapısı (alanlar `pub(crate)`), `new`, `session`, `login`, `logout`. `library.rs`: `list_series`, `continue_reading`, `get_series`, `get_book` (+ `refresh`). `sync.rs`: `sync_now`, `sync_push`, `sync_status`, `pull_book`, `get_progress`, `save_progress`, `set_status`, `ping_*`, yer imi fonksiyonları. `download.rs`: `download_*`, `run_downloads`, `storage_usage`, `clear_cache`. `reader.rs`: `cover_path`, `page_path`, `text_info`, `ensure_chapter`, `search_text`. Ortak yardımcı: `Core::with_db<T>(&self, f: impl FnOnce(&mut rusqlite::Connection) -> Result<T, CoreError>) -> Result<T, CoreError>` (kilidi alır; zehirli kilidi `into_inner` ile aşar). `db.rs`: `open(path:&Path)->rusqlite::Result<Connection>`, `kv_get(&Connection,&str)->Option<String>`, `kv_set(&Connection,&str,&str)`, `kv_del(&Connection,&str)`, `wipe_all(&Connection)`.

Davranış sözleşmeleri:
- `login`: URL normalize: kırp, `://` yoksa başına `http://`, sonundaki `/` kırpılır; boşsa `InvalidInput("Sunucu adresi gerekli.")`. `POST /api/login`. Başarıda `kv`'ye `server_url, username(sunucudan dönen), user_id, is_admin, token` yazılır. Önceki `server_url` veya `user_id` farklıysa önce `wipe_all()` (tüm tablolar + `books/` + `covers/` boşaltılır; `kv` sıfırlanır). Aynıysa veri korunur, yalnız `token` yenilenir. 401 → `Unauthorized`, 429 → `Server(sunucu mesajı)`.
- `logout`: `POST /api/logout` (gövde `{}`), hata yutulur; ardından `wipe_all()`.
- `sync_now` ve `sync_push` oturum yoksa `Unauthorized`. 401 görürse `Unauthorized` döner ve yerel veri/kuyruk KORUNUR.
- `list_series`: `folded_name LIKE %fold_tr(q)%` VEYA serinin herhangi bir kitabının `folded_title`'ında içerme; sıralama `ORDER BY ord`. `only_downloaded`: serinin en az bir kitabında `downloads.state='done'`.
- `continue_reading`: `progress.status='reading'` ve `books.error IS NULL`, `ORDER BY progress.updated_at DESC, books.id DESC LIMIT 20`.
- `BookItem.percent` = `CASE WHEN status='finished' THEN 1.0 ELSE percent END`. `BookItem.download` kitabın `downloads` satırı; yoksa `state=None` ve sıfırlar.
- `get_book.next_book_id/title`: aynı seride `ord` büyük olan ilk kitap `error IS NULL`; `prev_book_id` küçük olan ilk (azalan) `error IS NULL`.
- `save_progress`: `percent` sonlu değilse veya `[0,1]` dışıysa `InvalidInput`; `locator.len() > 200` ise `InvalidInput`. Yerel durum türetmesi sunucuyla aynı: `percent >= 0.9999` → `finished`; değilse önceki `finished` ise `finished` kalır; değilse `reading`. Satırı upsert eder: `locator, percent, status, updated_at=now, local_date, p_dirty=1, last_op='p'`.
- `set_status`: yalnız `status`, `updated_at=now`, `local_date`, `s_dirty=1`, `last_op='s'` günceller (locator/percent aynen kalır; satır yoksa `locator='', percent=0` ile oluşur).
- `ping_start`: 16 rastgele bayttan 32 hex `client_id`; `pings` satırı eklenir (`active_seconds=0`, `sent_seconds=-1`). `ping_update`: `active_seconds=MAX(mevcut, verilen)`, `pages_read=MAX`, `end_percent` yazılır; değerler sunucu sınırlarına kırpılır (`0..=86400`, `0..=100000`, `0..=1`). `ping_close`: `closed=1`.
- `add_bookmark`: `locator` boş/`>200` bayt, `label` kırpılmış boş/`>120` karakter ise `InvalidInput`; kitap başına (silinmemiş) 100 sınırı → `InvalidInput("Yer imi sınırına ulaşıldı.")`. `created_at=now`.
- `delete_bookmark`: `server_id IS NULL` ise satır fiziksel silinir; değilse `deleted=1`.
- `download_enqueue`: her kitap için `books.error IS NULL` değilse atlanır; `state='done'` ise atlanır; diğerleri `state='queued', error=NULL` (satır yoksa `parts_total=parts_done=bytes=0` ile oluşur).
- `cover_path(book_id, version)`: `covers/{id}-{version}.jpg` varsa yolunu döner; yoksa `GET /api/books/{id}/cover` indirir (önce `.tmp`, sonra `rename`), aynı `id` için eski sürüm dosyalarını siler. Ağ yoksa `Offline`, 404 → `NotFound`.
- `page_path(book_id, page)`: `books/{id}/p{page}` varsa döner; yoksa ağdan indirir, `parts`'a ekler (kitabın `downloads` satırı varsa `parts_done` da güncellenir). Ağ hatası → `Offline`; 404 → `NotFound`.
- `text_info`: `toc` tablosu doluysa ondan; boşsa `GET /api/books/{id}/toc` çeker ve yazar. `chapter_count = books.page_count` (0 ise `toc.len()`; ikisi de 0 ise 1). Ağ yok ve `toc` boşsa varsayılan: `chapter_count` kadar `Bölüm {i+1}` girdisi üretilir (ağ gerekmez).
- `ensure_chapter`: `books/{id}/c{n}.html` varsa hiçbir şey yapmaz; yoksa bölüm + görselleri indirip yazar (bölüm 4.6). Ağ yok → `Offline`.
- `search_text`: yalnız çevrimiçi; `GET /api/books/{id}/search?q=<yüzde-kodlu>`; ağ yoksa `Network`; `q` kırpılıp boşluklar tekilleştirilir, 2..=64 karakter dışı `InvalidInput`.

### 4.5 HTTP sözleşmesi (çekirdeğin çağırdığı her uç; sunucu kodundan okundu)

Ortak: `Cookie: krs_session=<token>` (login hariç). POST/PUT/DELETE: `Content-Type: application/json` HER ZAMAN (gövdesiz DELETE ve `POST /api/logout` dahil). `Origin` ASLA gönderilmez. `User-Agent: ReadCrab-Android/0.1`. Zaman aşımı: bağlantı 8 sn, okuma/yazma 30 sn; `pull_book` ve `search_text` isteği 5 sn / 20 sn toplam. Gövde sınırı 64 MiB. Her başarılı yanıtta `Date` başlığı varsa `httpdate::parse_http_date` ile `clock_offset = sunucu_sn - yerel_sn` güncellenir. Hata gövdesi: `{"error":"<kod>","message":"<metin>"}`; 429'da `Retry-After`.

| Uç | İstek | Başarılı yanıt |
|---|---|---|
| `POST /api/login` | `{"username","password"}` | 200 `{"id":i64,"username","is_admin":bool}` + `Set-Cookie: krs_session=<tok>; ...` (`Secure` olabilir; yalnız `krs_session=` değeri alınır). 401 yanlış parola, 429 kilit |
| `POST /api/logout` | `{}` | 204 |
| `GET /api/series?limit=200&offset=<n>&sort=name` | — | 200 `{"total":i64,"items":[{"id","name","book_count","read_count","reading_count","percent":f64,"cover_book_id":i64\|null,"cover_v":i64\|null,"last_read_at":i64\|null}]}` |
| `GET /api/series/{id}` | — | 200 `{"id","name","books":[{"id","title","format","page_count","size","error":str\|null,"status","percent":f64,"cover_v":i64}]}`; 404 |
| `GET /api/books/{id}/progress` | — | 200 `{"locator":str,"percent":f64,"status":str,"updated_at":i64}` (`updated_at` S1; yoksa `#[serde(default)]` 0) |
| `PUT /api/books/{id}/progress` | `{"locator","percent","local_date","client_ts":i64}` | 200 `{"status","percent","applied":bool,"updated_at":i64}` (`applied` yoksa `true` say) |
| `PUT /api/books/{id}/status` | `{"status","local_date","client_ts":i64}` | 200 `{"status","applied","updated_at"}` |
| `POST /api/reading/ping` | `{"client_id","book_id","active_seconds","pages_read","start_percent","end_percent","local_date"}` | 204; 404 kitap yok; 400 doğrulama |
| `GET /api/books/{id}/bookmarks` | — | 200 `{"items":[{"id","locator","label","created_at"}]}` |
| `POST /api/books/{id}/bookmarks` | `{"locator","label"}` | 201 `{"id","locator","label","created_at"}`; 409 sınır; 400 |
| `DELETE /api/bookmarks/{id}` | (gövdesiz, CT json) | 204; 404 |
| `GET /api/books/{id}/pages` | — | 200 `{"count":i64}` (yalnız çizgi roman) |
| `GET /api/books/{id}/pages/{n}` | — | 200 görsel bayt (n 0 tabanlı); 404 aralık dışı; 422 bozuk |
| `GET /api/books/{id}/toc` | — | 200 `{"chapters":[{"index":usize,"title"}]}` (yalnız metin) |
| `GET /api/books/{id}/chapters/{n}` | — | 200 `text/html` belge; 404; 422 `corrupt_book`/`drm_protected` |
| `GET /api/books/{id}/asset?p=<yüzde-kodlu>` | — | 200 bayt, `Content-Type` görsel; 404 |
| `GET /api/books/{id}/cover` | — | 200 `image/jpeg`; 404 kapak yok |
| `GET /api/books/{id}/search?q=<kodlu>` | — | 200 `{"results":[{"ch","p","before","hit","after"}],"truncated":bool}`; 400; 429 `too_many_attempts` |

`Set-Cookie` ayrıştırma (`http::parse_session_cookie(headers: &[&str]) -> Option<String>`): her başlık değerinde `krs_session=` ile başlayan çiftin `;`ye kadar kısmı; boşsa `None`.

### 4.6 Bölüm kaydı (indirme + isteğe bağlı okuma ortak yolu) — `reader::fetch_chapter(core, book_id, n)`
1. `GET /api/books/{id}/chapters/{n}` metnini al.
2. `util::extract_body(doc) -> &str`: sunucunun `frame_document` çıktısı sabittir (`<body>\n{gövde}\n</body>`): `doc.find("<body>\n")` → +7 ile `doc.rfind("\n</body>")` arası; bulunamazsa `HErr::Parse`.
3. `util::asset_refs(body, book_id) -> Vec<String>`: `src="/api/books/{book_id}/asset?p=` ile başlayıp sonraki `"` ye kadar olan `p` değerleri (zaten yüzde-kodlu, `&` içermez), tekilleştirilmiş.
4. Her `p` için `GET /api/books/{id}/asset?p={p}`; dosya adı `format!("{}.{}", &hex(sha256(p))[..16], ext)`; `ext`: `Content-Type` `image/png→png`, `image/jpeg→jpg`, `image/gif→gif`, `image/webp→webp`, `image/svg+xml→svg`, diğer `img`. `books/{id}/a/{ad}` yazılır. 404 → bu görsel atlanır (src boşaltılır `src=""`); ağ hatası → parça başarısız.
5. Gövdede `src="/api/books/{id}/asset?p={p}"` → `src="/book/{id}/a/{ad}"` yeniden yazılır (`util::rewrite_assets(body, book_id, &map) -> String`).
6. Yeniden yazılmış gövde `books/{id}/c{n}.html.tmp`'ye yazılıp `rename` edilir; `parts(book_id, n, bytes=gövde+görseller)` eklenir.

### 4.7 Kütüphane yenileme — `library::refresh(core) -> Result<(), CoreError>` (`sync_now` adım 2)
1. `GET /api/series?limit=200&offset=k&sort=name` döngüsü, `k += 200` ta ki toplanan öğe sayısı `total` olana kadar. Öğe `i` için `ord = k_toplam_indeks`.
2. İmza: `sig = format!("{}|{}|{}|{:.4}|{}|{}|{}|{}", name, book_count, read_count, reading_count, percent, last_read_at.unwrap_or(0), cover_book_id.unwrap_or(0), cover_v.unwrap_or(0))`.
3. Detayı çekilecek seriler: yerelde yok, `sig` farklı VEYA o serinin kitapları yerelde yok. `par_map(.., 4, |id| GET /api/series/{id})`.
4. Tek `transaction`: seri upsert (`name, folded_name=fold_tr(name), cover_book_id, cover_v, ord, sig`); listede olmayan seriler ve onların kitap/`progress`/`bookmarks`/`toc`/`parts`/`downloads` satırları + `books/{id}` dizinleri silinir; çekilen detaylar için kitap upsert (`ord` = dizideki indeks) ve o serinin listede olmayan kitapları aynı şekilde silinir (dosyalar dahil).
5. İlerleme çekme adayları (yalnız detayı yeni çekilen kitaplar): kitap için liste imzası `lsig = format!("{}|{:.4}", status, percent)`. Yerel `progress` satırı yoksa: `status=="unread" && percent==0.0` ise satır `seen_sig=lsig` ile oluşturulur ve ÇEKİLMEZ; aksi halde aday. Satır varsa `seen_sig != lsig` ise aday.
6. Adaylar için `par_map(.., 4, GET /api/books/{id}/progress)`; sonuç `sync::merge_progress` (bölüm 5.3) ile uygulanır; her durumda `seen_sig = lsig` yazılır.
7. `kv.sync_last_ok_at = now`.

### 4.8 İndirme durum makinesi (`download.rs`)

Durumlar (`downloads.state`): `queued`, `running`, `failed`, `done`; satır yok = `None`.
| Geçiş | Tetik |
|---|---|
| (yok) → queued | `download_enqueue` |
| failed → queued | `download_enqueue` (yeniden dene; tamamlanmış parçalar atlanır) |
| queued → running | `run_downloads` döngüsü en eski `updated_at`'li `queued` satırı seçer |
| running → done | tüm parçalar tamam ve kapak indirildi |
| running → failed | bir parça 3 denemeden sonra başarısız (bekleme 1 sn, 2 sn; yalnız ağ hatası ve 5xx yeniden denenir; 4xx kalıcı); `error` Türkçe metin |
| running → queued | `download_stop`, süreç ölümü (`Core::new` sıfırlar), `Unauthorized` |
| herhangi → (yok) | `download_remove` (dosya + `downloads` + `parts` + `toc` satırları silinir; çalışan iş `cancel` kümesini görüp durur) |
`run_downloads` algoritması: `dl_running` zaten true ise `Ok(DownloadRun{0,0})` hemen döner. Döngü: sıradaki `queued` kitabı seç → `running` yap → hazırlık: çizgi romanda `parts_total = books.page_count` (0 ise `GET .../pages`.count); metinde `parts_total = books.page_count` (0 ise toc uzunluğu) ve `toc` yoksa çek-yaz; kapağı `cover_path` ile indir (hata yutulur) → eksik parçaları (`parts`'ta olmayan `idx`) `par_map(.., 3, ...)` ile indir (çizgi roman: `GET pages/{n}` → `p{n}.tmp` → rename → `parts`; metin: bölüm 4.6); her parçadan sonra `downloads.parts_done = COUNT(parts)`, `bytes = SUM(parts.bytes)`, `updated_at=now` güncellenir; her parçadan önce `stop` ve `cancel` denetlenir. `Unauthorized` görürse kitap `queued`'a döner ve fonksiyon `Err(Unauthorized)` döner. Kuyruk boşalınca `Ok(DownloadRun{finished, failed})`; `dl_running` her çıkışta (Drop korumasıyla) false yapılır.

Önbellek tanımı: `downloads` satırı OLMAYAN kitapların `parts` satırları ve dosyaları. `clear_cache`: bu kitapların `books/{id}` dizinlerini, `parts` ve `toc` satırlarını siler, silinen bayt toplamını döner. İndirme satırı olan (queued/running/failed/done) kitaplara dokunulmaz. `storage_usage`: `downloads_bytes = SUM(parts.bytes)` (satırı olan kitaplar), `cache_bytes = SUM(parts.bytes)` (satırı olmayanlar), `covers_bytes` = `covers/` dosya boyutları toplamı, `db_bytes` = `readcrab.db` boyutu.

## 5. ReadSync algoritması

### 5.1 Çakışma kuralı (kesin)
Bir kitabın ilerleme/durumu için tek kural: **en büyük `updated_at` (saniye) kazanır; eşitlikte yerel kazanır.** `updated_at` yerelde = değişiklik anında `util::now()`; sunucuda = S1 ile gelen `client_ts` (en çok sunucu `now`). Saat farkı `Date` başlığıyla düzeltilir. Yer imleri çakışmaz: ekleme/silme kümesi birleşimi (bölüm 5.4). Okuma süresi: toplanır, `client_id` ile idempotent (sunucu `MAX` alır).

### 5.2 Giden kuyruk — `sync::push(core) -> Result<i32, CoreError>`
Oturum yoksa `Unauthorized`. Sıra: ilerleme/durum → yer imleri → ping. İlk ağ hatasında (`Net`) durur ve `Network` döner (kalanlar kuyrukta kalır). `Status 401` → `Unauthorized` (kuyruk korunur). Her başarılı işlemde sayaç artar.
1. `progress` satırları `WHERE p_dirty=1 OR s_dirty=1 ORDER BY updated_at`. Her satır için işlem sırası: ikisi de kirliyse `last_op='p'` ise ÖNCE durum sonra ilerleme, `last_op='s'` ise ÖNCE ilerleme sonra durum; tek kirli ise o. `client_ts = satır.updated_at`.
   - İlerleme: `PUT /api/books/{id}/progress {locator, percent, local_date, client_ts}`.
   - Durum: `PUT /api/books/{id}/status {status, local_date, client_ts}`.
   - 2xx: bayrak ancak satırın `updated_at`'i okuma anındakiyle aynıysa temizlenir (`UPDATE ... SET p_dirty=0 WHERE book_id=? AND updated_at=?`). Yanıtta `applied == false` ise: bayraklar temizlenir, `seen_sig=''` yapılır ve kitap `pull_progress` listesine alınır (sunucu daha yeni).
   - 404 veya 400: bayraklar temizlenir (kalıcı hata, düşer). 429/5xx: kuyruk korunur, `last_error` yazılır, bu tur sonlanır ve `Server(mesaj)` döner.
2. `bookmarks`: (a) `deleted=1 AND server_id NOT NULL` → `DELETE /api/bookmarks/{server_id}`; 204 veya 404 → satır fiziksel silinir. (b) `server_id IS NULL AND deleted=0 ORDER BY created_at` → `POST /api/books/{book_id}/bookmarks {locator,label}`; 201 → `server_id` yazılır; 409 veya 400 → satır silinir (`last_error` yazılır).
3. `pings`: `active_seconds > sent_seconds` olan satırlar → `POST /api/reading/ping` (`client_id, book_id, active_seconds, pages_read, start_percent, end_percent, local_date`). 204 → `sent_seconds=active_seconds`; ardından `closed=1 AND sent_seconds=active_seconds` olan satır silinir. 404/400 → satır silinir. `active_seconds == 0` satırlar gönderilmez; `closed=1` ise silinir.

### 5.3 Çekme ve birleştirme
`sync_now` adımları: (1) `push`; (2) `library::refresh` (ilerleme çekme dahil, bölüm 4.7); (3) yer imi çekme: `downloads.state='done'` VEYA `progress.status IN('reading')` VEYA yerel yer imi olan kitaplar için `par_map(.., 4, pull_bookmarks)`; (4) `push` (yeni üretilen bir şey kaldıysa); (5) `kv.sync_last_ok_at=now`, `sync_last_error` temizlenir. Hata olursa `sync_last_error` Türkçe mesajla yazılır ve hata döner. `SyncReport.pushed` iki push toplamı, `pulled_books` birleşmede sunucu verisi uygulanan kitap sayısı.

Saf birleştirme (birim testli), `sync.rs`:
```rust
pub struct LocalProg { pub updated_at: i64, pub pending: bool }       // pending = p_dirty||s_dirty
pub struct ServerProg { pub updated_at: i64, pub has_row: bool }
pub enum Merge { TakeServer, KeepLocal }
pub fn merge_progress(local: Option<&LocalProg>, srv: &ServerProg) -> Merge
```
Kurallar (sırayla): (1) `!srv.has_row` (sunucuda kayıt yok: `updated_at==0` ve `status=='unread'` ve `locator==''`) → `KeepLocal`. (2) yerel satır yok → `TakeServer`. (3) `local.pending`: `srv.updated_at > local.updated_at` ise `TakeServer` (bekleyen yerel işlem iptal edilir: bayraklar 0), aksi `KeepLocal` (eşitlikte de yerel). (4) `pending` değil → `TakeServer`. `TakeServer` uygulaması: `locator, percent, status, updated_at=srv.updated_at` yazılır, bayraklar 0, `last_op=''`. Eski sunucu (S1 yok, `updated_at==0` ama satır var): `has_row` true sayılır; (3)'te `0 > local.updated_at` yanlış olur → yerel kazanır, bekleyen yoksa sunucu kazanır — kabul edilen bozulma.

Yer imleri (`pull_bookmarks(book_id)`): `GET /api/books/{id}/bookmarks`; saf fonksiyon
```rust
pub struct LocalBm { pub local_id: i64, pub server_id: Option<i64>, pub locator: String, pub label: String, pub deleted: bool }
pub struct SrvBm { pub id: i64, pub locator: String, pub label: String, pub created_at: i64 }
pub enum BmOp { Insert(SrvBm), Adopt { local_id: i64, server_id: i64 }, Update { local_id: i64, locator: String, label: String, created_at: i64 }, Remove { local_id: i64 } }
pub fn merge_bookmarks(local: &[LocalBm], server: &[SrvBm]) -> Vec<BmOp>
```
Kurallar: her `s` için: `server_id==s.id` olan yerel varsa → silinmişse işlem YOK (DELETE bekliyor), değilse `Update`; yoksa `server_id IS NULL AND !deleted` ve `(locator,label)` eşit ilk yerel varsa → `Adopt` (aynı eşleme iki sunucu kaydına verilmez); yoksa `Insert`. Sunucu kümesinde olmayan `server_id`'li yerel her satır → `Remove` (`deleted` olsun olmasın). `server_id IS NULL` satırlara dokunulmaz.

### 5.4 Okuma ekranı tetikleri
Okuyucu açılışı: çevrimiçiyse `pull_book(id)` (en çok 4 sn beklenir, sonuç yoksa yerel konumla devam); okuma sırasında her sayfa/konum değişiminde yalnız `save_progress` (yerel); okuyucu açıkken her 30 sn ve çıkışta `sync_push()` (hata yutulur); çıkışta `Scheduler.kickSync()`. Arka planda: `SyncWorker` periyodik (1 saat, ağ gerekli) ve `kickSync` tek seferlik (5 sn gecikme, ağ gerekli, `REPLACE`).

## 6. Android

### 6.1 Gradle dosyaları (G12)

Wrapper YOK. CI `gradle/actions/setup-gradle@v4` ile Gradle 8.13'ü kurar ve `gradle` komutunu doğrudan çağırır (`gradlew`/`gradle-wrapper.jar` depoya girmez; yerelde de `gradle` 8.13 ile çalışılır).

`android/settings.gradle.kts`:
```kotlin
pluginManagement {
    repositories { google(); mavenCentral(); gradlePluginPortal() }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories { google(); mavenCentral() }
}
rootProject.name = "ReadCrab"
include(":app")
```
`android/build.gradle.kts`:
```kotlin
plugins {
    id("com.android.application") version "8.13.2" apply false
    id("org.jetbrains.kotlin.android") version "2.2.21" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.21" apply false
}
```
`android/gradle.properties`:
```
org.gradle.jvmargs=-Xmx3g -Dfile.encoding=UTF-8
android.useAndroidX=true
android.nonTransitiveRClass=true
kotlin.code.style=official
```
`android/app/build.gradle.kts`:
```kotlin
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "io.github.mustafabayramguven.readcrab"
    compileSdk = 35
    defaultConfig {
        applicationId = "io.github.mustafabayramguven.readcrab"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    sourceSets.getByName("main") {
        // cargo-ndk çıktısı src/main/jniLibs'e gider (varsayılan konum)
        java.srcDir(layout.buildDirectory.dir("generated/uniffi/kotlin")) // CI'da üretilen UniFFI Kotlin binding
        assets.srcDir(layout.buildDirectory.dir("webAssets"))             // web/ yazı tipleri ve CSS (kopya depoya girmez)
    }
}

kotlin { compilerOptions { jvmTarget.set(JvmTarget.JVM_17) } }

// Okuyucu için web/ altından yalnız gerekenler assets köküne (=> /static/...) gider
val syncWebAssets by tasks.registering(Sync::class) {
    from(rootProject.projectDir.resolve("../web")) {
        include("frame.css", "reader-fonts.css", "fonts/**")
        exclude("fonts/roboto-flex-*")
    }
    into(layout.buildDirectory.dir("webAssets"))
}
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("Assets") }
    .configureEach { dependsOn(syncWebAssets) }

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.05.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.navigation:navigation-compose:2.8.9")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.0")
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.work:work-runtime-ktx:2.10.1")
    implementation("androidx.webkit:webkit:1.12.1")
    implementation("io.coil-kt.coil3:coil-compose:3.2.0")
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
    testImplementation("junit:junit:4.13.2")
}
```
Not: `syncWebAssets` yalnız `frame.css`, `reader-fonts.css` ve 7 okuyucu yazı tipinin `fonts/*.woff2` + lisans dosyalarını kopyalar; `reader.html/reader.js` `src/main/assets/` içindedir ve aynı asset köküne birleşir. WebView'de `/static/` yolu asset köküne eşlenir; böylece `reader-fonts.css`'in `/static/fonts/...` adresleri DEĞİŞMEDEN çalışır.

### 6.2 `AndroidManifest.xml` (G12)
```xml
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:tools="http://schemas.android.com/tools">
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
    <uses-permission android:name="android.permission.POST_NOTIFICATIONS" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />
    <application
        android:name=".RcApp"
        android:allowBackup="false"
        android:icon="@mipmap/ic_launcher"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:label="@string/app_name"
        android:supportsRtl="true"
        android:theme="@style/Theme.ReadCrab">
        <activity
            android:name=".MainActivity"
            android:exported="true"
            android:windowSoftInputMode="adjustResize"
            android:configChanges="orientation|screenSize|smallestScreenSize|screenLayout|keyboardHidden|uiMode">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
        <service
            android:name="androidx.work.impl.foreground.SystemForegroundService"
            android:foregroundServiceType="dataSync"
            tools:node="merge" />
    </application>
</manifest>
```
`themes.xml`: `<style name="Theme.ReadCrab" parent="android:Theme.Material.NoActionBar"><item name="android:windowBackground">@android:color/black</item><item name="android:navigationBarColor">@android:color/transparent</item><item name="android:statusBarColor">@android:color/transparent</item></style>` (Compose temayı çizer; başlangıç beyaz çakmasını önlemek için siyah).

### 6.3 Tema (G13) — `ui/theme/Theme.kt`

Tüm renkler sabittir (Material 3 baseline, tohum #6750A4).

Açık şema (`lightColorScheme`):
| belirteç | hex |
|---|---|
| primary / onPrimary | #6750A4 / #FFFFFF |
| primaryContainer / onPrimaryContainer | #EADDFF / #21005D |
| secondary / onSecondary | #625B71 / #FFFFFF |
| secondaryContainer / onSecondaryContainer | #E8DEF8 / #1D192B |
| tertiary / onTertiary | #7D5260 / #FFFFFF |
| tertiaryContainer / onTertiaryContainer | #FFD8E4 / #31111D |
| error / onError | #B3261E / #FFFFFF |
| errorContainer / onErrorContainer | #F9DEDC / #410E0B |
| background / onBackground | #FEF7FF / #1D1B20 |
| surface / onSurface | #FEF7FF / #1D1B20 |
| surfaceVariant / onSurfaceVariant | #E7E0EC / #49454F |
| outline / outlineVariant | #79747E / #CAC4D0 |
| scrim | #000000 |
| inverseSurface / inverseOnSurface / inversePrimary | #322F35 / #F5EFF7 / #D0BCFF |
| surfaceDim / surfaceBright | #DED8E1 / #FEF7FF |
| surfaceContainerLowest / Low / (default) / High / Highest | #FFFFFF / #F7F2FA / #F3EDF7 / #ECE6F0 / #E6E0E9 |

Koyu şema (`darkColorScheme`):
| belirteç | hex |
|---|---|
| primary / onPrimary | #D0BCFF / #381E72 |
| primaryContainer / onPrimaryContainer | #4F378B / #EADDFF |
| secondary / onSecondary | #CCC2DC / #332D41 |
| secondaryContainer / onSecondaryContainer | #4A4458 / #E8DEF8 |
| tertiary / onTertiary | #EFB8C8 / #492532 |
| tertiaryContainer / onTertiaryContainer | #633B48 / #FFD8E4 |
| error / onError | #F2B8B5 / #601410 |
| errorContainer / onErrorContainer | #8C1D18 / #F9DEDC |
| background / onBackground | #141218 / #E6E0E9 |
| surface / onSurface | #141218 / #E6E0E9 |
| surfaceVariant / onSurfaceVariant | #49454F / #CAC4D0 |
| outline / outlineVariant | #938F99 / #49454F |
| scrim | #000000 |
| inverseSurface / inverseOnSurface / inversePrimary | #E6E0E9 / #322F35 / #6750A4 |
| surfaceDim / surfaceBright | #141218 / #3B383E |
| surfaceContainerLowest / Low / (default) / High / Highest | #0F0D13 / #1D1B20 / #211F26 / #2B2930 / #36343B |

Saf siyah (AMOLED) — koyu şemanın üstüne uygulanan tek fonksiyon (yalnız koyu etkinken; dinamik koyu şema dahil):
```kotlin
fun amoled(s: ColorScheme): ColorScheme = s.copy(
    background = Color.Black, surface = Color.Black, surfaceDim = Color.Black,
    surfaceBright = Color.Black, surfaceVariant = Color.Black,
    surfaceContainerLowest = Color.Black, surfaceContainerLow = Color.Black,
    surfaceContainer = Color.Black, surfaceContainerHigh = Color.Black,
    surfaceContainerHighest = Color.Black,
    outlineVariant = Color(0xFF3A363E), // siyah yüzeyde kart/ayırıcı çizgisi görünsün
)
```
AMOLED'de kartlar `OutlinedCard`/`outlineVariant` kenarlığıyla ayrışır (dolgu rengi siyah). Seçili/vurgu kapsayıcıları (`primaryContainer`, `secondaryContainer`) yüzey sayılmaz, koyu şemadaki gibi kalır.

```kotlin
enum class ThemeMode { SYSTEM, LIGHT, DARK }

@Composable
fun ReadCrabTheme(prefs: UiPrefs, content: @Composable () -> Unit) {
    val dark = when (prefs.themeMode) {
        ThemeMode.SYSTEM -> isSystemInDarkTheme()
        ThemeMode.LIGHT -> false
        ThemeMode.DARK -> true
    }
    val ctx = LocalContext.current
    val dynamic = prefs.dynamicColor && Build.VERSION.SDK_INT >= 31
    val base = when {
        dynamic && dark -> dynamicDarkColorScheme(ctx)
        dynamic -> dynamicLightColorScheme(ctx)
        dark -> DarkScheme
        else -> LightScheme
    }
    val scheme = if (dark && prefs.amoled) amoled(base) else base
    MaterialTheme(colorScheme = scheme, typography = RcTypography, shapes = RcShapes, content = content)
}
fun isEffectiveDark(prefs: UiPrefs, systemDark: Boolean): Boolean   // ReaderTheme "auto" çözümü için
```
Tipografi `RcTypography` = `Typography()` varsayılan M3 ölçeği, FontFamily varsayılan (sistem). Değişen satırlar: `titleLarge = 22.sp/28.sp, FontWeight.Medium`; `titleMedium = 16.sp/24.sp, Medium, letterSpacing 0.15.sp`; `labelLarge = 14.sp/20.sp, Medium`; `labelSmall = 11.sp/16.sp`; `bodyMedium = 14.sp/20.sp`. `RcShapes = Shapes(small = RoundedCornerShape(8.dp), medium = RoundedCornerShape(12.dp), large = RoundedCornerShape(16.dp), extraLarge = RoundedCornerShape(28.dp))`.

Boşluk/ölçü sabitleri (`object Dimens`): `s1=4.dp, s2=8.dp, s3=12.dp, s4=16.dp, s5=24.dp, s6=32.dp`; `gutter=16.dp` (ekran kenar boşluğu); `coverRadius=8.dp`; `coverAspect=2f/3f`; `gridMin=112.dp`; `gridGap=12.dp`; `barHeight=56.dp`; `progressTrack=3.dp`; `bookRowCover=56.dp` genişlik (yükseklik 84.dp); `continueCardWidth=104.dp`.

Okuyucu renkleri (`ReaderColors`), WebView için `bg/fg/link`:
| tema | bg | fg | link |
|---|---|---|---|
| light | #FEF7FF | #1D1B20 | #6750A4 |
| dark | #141218 | #E6E0E9 | #D0BCFF |
| sepia | #F4ECD8 | #433422 | #7A4A12 |
| black | #000000 | #E6E0E9 | #D0BCFF |
Çizgi roman okuyucu arka planı HER ZAMAN `#000000`. Okuyucu üst/alt çubuk dolgusu `MaterialTheme.colorScheme.surface.copy(alpha = 0.94f)` (AMOLED'de tam siyah).

`ThemeTest.kt` (JUnit4, saf JVM): (1) `amoled(DarkScheme)` için `background, surface, surfaceDim, surfaceBright, surfaceVariant, surfaceContainerLowest/Low/(default)/High/Highest` hepsi `Color.Black`; (2) `amoled(DarkScheme).primary == DarkScheme.primary`; (3) `DarkScheme.surface == Color(0xFF141218)`; (4) `readerColors("sepia").bg == "#F4ECD8"` (okuyucu renk tablosunu bir `fun readerColors(theme: String): ReaderPalette` veri sınıfı `ReaderPalette(bg: String, fg: String, link: String)` döndürür).

### 6.4 Tercihler (G13) — `Prefs.kt`
```kotlin
data class UiPrefs(
    val themeMode: ThemeMode = ThemeMode.SYSTEM,
    val amoled: Boolean = false,
    val dynamicColor: Boolean = false,
    val wifiOnly: Boolean = true,
    val immersive: Boolean = true,
    val keepAwake: Boolean = true,
    val tapZones: Boolean = true,
    val volumeKeys: Boolean = false,
    val readerTheme: String = "auto",     // auto | light | dark | sepia | black
    val textFont: String = "literata",    // literata | merriweather | source-serif | eb-garamond | inter | atkinson | lexend
    val textSize: Int = 18,               // 14..28, adım 2
    val lineHeight: Float = 1.7f,         // 1.2..2.2, adım 0.1
    val margin: Int = 0,                  // 0..20, adım 1 (genişliğin yüzdesi)
    val align: String = "left",           // left | justify
    val textMode: String = "scroll",      // scroll | page
    val comicMode: String = "ltr",        // ltr | rtl | webtoon (varsayılan; kitap başına ezilebilir)
    val needsRelogin: Boolean = false,
)

class Prefs(ctx: Context) {
    val state: StateFlow<UiPrefs>
    fun update(f: (UiPrefs) -> UiPrefs)            // bellek + SharedPreferences("rc_prefs"); anahtar adları alan adlarıdır
    fun comicModeFor(bookId: Long): String          // anahtar "cmode.$bookId", yoksa state.value.comicMode
    fun setComicModeFor(bookId: Long, mode: String)
}
```
Yazı tipi/boyut sınırları web `calc.js normalizePrefs` ile aynıdır; `update` değerleri bu sınırlara kırpar ve geçersiz dizgiyi varsayılana çevirir. Yazı tipi adları marka adıdır: `strings.xml`'e GİRMEZ, Kotlin'de sabittir. `Prefs.kt` (üst düzey, birebir): `val TEXT_FONTS: List<Pair<String, String>> = listOf("literata" to "Literata", "merriweather" to "Merriweather", "source-serif" to "Source Serif 4", "eb-garamond" to "EB Garamond", "inter" to "Inter", "atkinson" to "Atkinson Hyperlegible", "lexend" to "Lexend")` (kimlik to etiket). Ayarlar ve metin okuyucu yazı tipi seçicileri etiketi buradan alır.

### 6.5 Uygulama kabuğu

**`RcApp.kt`** (G14): `class RcApp : Application(), SingletonImageLoader.Factory`. Alanlar (`lateinit`/`by lazy`): `val prefs: Prefs`, `val core: Core` (`Core(File(filesDir, "rc").absolutePath)`), `val connectivity: Connectivity`, `val downloads: DownloadTracker`. `onCreate`: bildirim kanalı (`id "indirmeler"`, ad `@string/notif_channel_downloads`, `IMPORTANCE_LOW`); oturum varsa `Scheduler.schedulePeriodicSync(this)`. `newImageLoader(context: PlatformContext): ImageLoader = ImageLoader.Builder(context).components { add(RcKeyer()); add(RcFetcher.Factory(core)) }.diskCache(null).build()`.

**`Connectivity.kt`** (G13): `class Connectivity(ctx: Context) { val online: StateFlow<Boolean> }` — `ConnectivityManager.registerDefaultNetworkCallback`; `onAvailable`→true (yalnız `NET_CAPABILITY_INTERNET` bakılır, VALIDATED aranmaz: LAN sunucuları için), `onLost`→false; başlangıç değeri `activeNetwork != null`.

**`image/CoreImage.kt`** (G14): Coil 3 özel model/fetcher.
```kotlin
sealed interface RcImage {
    data class Cover(val bookId: Long, val version: Long) : RcImage
    data class Page(val bookId: Long, val page: Int) : RcImage
}
class RcKeyer : Keyer<RcImage> {
    override fun key(data: RcImage, options: Options): String = when (data) {
        is RcImage.Cover -> "rc-cover-${data.bookId}-${data.version}"
        is RcImage.Page -> "rc-page-${data.bookId}-${data.page}"
    }
}
class RcFetcher(private val data: RcImage, private val core: Core) : Fetcher {
    override suspend fun fetch(): FetchResult {
        val path = withContext(Dispatchers.IO) {
            when (data) {
                is RcImage.Cover -> core.coverPath(data.bookId, data.version)
                is RcImage.Page -> core.pagePath(data.bookId, data.page)
            }
        }
        return SourceFetchResult(
            source = ImageSource(file = File(path).toOkioPath(), fileSystem = FileSystem.SYSTEM),
            mimeType = null,
            dataSource = DataSource.DISK,
        )
    }
    class Factory(private val core: Core) : Fetcher.Factory<RcImage> {
        override fun create(data: RcImage, options: Options, imageLoader: ImageLoader): Fetcher = RcFetcher(data, core)
    }
}
```
İçe aktarmalar: `coil3.ImageLoader, coil3.PlatformContext, coil3.SingletonImageLoader, coil3.decode.DataSource, coil3.decode.ImageSource, coil3.fetch.FetchResult, coil3.fetch.Fetcher, coil3.fetch.SourceFetchResult, coil3.key.Keyer, coil3.request.Options, okio.FileSystem, okio.Path.Companion.toOkioPath`. Çizgi roman sayfa isteği `ImageRequest.Builder(ctx).data(RcImage.Page(..)).size(2000, 2000).build()`; kapak isteği kutu boyutunda (varsayılan).

**`work/DownloadTracker.kt`** (G14):
```kotlin
class DownloadTracker(private val app: RcApp, private val scope: CoroutineScope) {
    val infos: StateFlow<Map<Long, DownloadInfo>>      // bookId -> info (yalnız satırı olanlar)
    fun refresh()                                      // tek seferlik core.downloadInfos() (IO)
    fun enqueue(bookIds: List<Long>)                   // core.downloadEnqueue + Scheduler.scheduleDownload(app, prefs.wifiOnly) + polling başlat
    fun remove(bookIds: List<Long>)                    // core.downloadRemove + refresh
}
```
Polling: iş sürerken (`queued|running` var) her 700 ms `refresh`; yoksa durur. `RcApp.onCreate` bir kez `refresh()` çağırır, aktif iş varsa polling'i başlatır.

**`work/Workers.kt`** (G14):
```kotlin
object Scheduler {
    fun scheduleDownload(ctx: Context, wifiOnly: Boolean)   // unique "downloads", ExistingWorkPolicy.APPEND_OR_REPLACE, OneTimeWorkRequest<DownloadWorker>, kısıt UNMETERED (wifiOnly) veya CONNECTED, backoff EXPONENTIAL 30 sn
    fun schedulePeriodicSync(ctx: Context)                  // unique "sync-periodic", KEEP, PeriodicWorkRequest 1 saat, kısıt CONNECTED
    fun kickSync(ctx: Context)                              // unique "sync-kick", REPLACE, OneTimeWorkRequest<SyncWorker>, initialDelay 5 sn, kısıt CONNECTED
    fun cancelAll(ctx: Context)                             // çıkışta tüm işleri iptal
}
class DownloadWorker(ctx: Context, p: WorkerParameters) : CoroutineWorker(ctx, p) {
    override suspend fun doWork(): Result
}
class SyncWorker(ctx: Context, p: WorkerParameters) : CoroutineWorker(ctx, p) {
    override suspend fun doWork(): Result
}
```
`DownloadWorker.doWork`: `try { setForeground(ForegroundInfo(1, bildirim, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)) } catch (_: Exception) {}` (arka plandan başlatma yasağında sessiz devam). Bildirim: kanal `indirmeler`, başlık `@string/notif_downloading`, `setOngoing(true)`, `setSmallIcon(android.R.drawable.stat_sys_download)`. Sonra `try { withContext(Dispatchers.IO) { core.runDownloads() }; Result.success() } catch (e: CoreException.Unauthorized) { prefs.update { it.copy(needsRelogin = true) }; Result.failure() } catch (e: CoreException.Network) { Result.retry() } catch (e: CoreException.Tls) { Result.failure() } catch (e: CancellationException) { core.downloadStop(); throw e }`. Diğer `CoreException` → `Result.failure()`.
`SyncWorker.doWork`: `core.session()` yoksa `Result.success()`; `try { withContext(IO) { core.syncNow() }; Result.success() } catch (Unauthorized) { needsRelogin = true; Result.failure() } catch (Network|Server) { Result.retry() } catch (Tls) { Result.failure() }`.
`core`/`prefs` her ikisi `(applicationContext as RcApp)` üzerinden alınır.

**`MainActivity.kt`** (G15): `class MainActivity : ComponentActivity()`. `onCreate`: `enableEdgeToEdge()`; `setContent { val app = application as RcApp; val prefs by app.prefs.state.collectAsStateWithLifecycle(); ReadCrabTheme(prefs) { AppNav(app) } }`. Ses tuşları:
```kotlin
object VolumeKeys { @Volatile var handler: ((forward: Boolean) -> Boolean)? = null }
override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
    if (keyCode == KeyEvent.KEYCODE_VOLUME_DOWN || keyCode == KeyEvent.KEYCODE_VOLUME_UP) {
        val h = VolumeKeys.handler
        if (h != null && h(keyCode == KeyEvent.KEYCODE_VOLUME_DOWN)) return true
    }
    return super.onKeyDown(keyCode, event)
}
override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean {
    if ((keyCode == KeyEvent.KEYCODE_VOLUME_DOWN || keyCode == KeyEvent.KEYCODE_VOLUME_UP) && VolumeKeys.handler != null) return true
    return super.onKeyUp(keyCode, event)
}
```
(Ses AŞAĞI = ileri sayfa, ses YUKARI = geri.) Okuyucular `prefs.volumeKeys` açıkken `VolumeKeys.handler`'ı `DisposableEffect` ile kurar/temizler.

**`ui/nav/AppNav.kt`** (G15) — rotalar:
| rota | ekran |
|---|---|
| `giris` | `LoginScreen` |
| `kutuphane` | `LibraryScreen` |
| `seri/{seriesId}` (Long) | `SeriesScreen` |
| `oku/{bookId}` (Long) | `ReaderRoute`: `core.getBook` ile `isComic` bakar → `ComicReaderScreen` veya `TextReaderScreen` |
| `ayarlar` | `SettingsScreen` |
`AppNav.kt` TAM KOD (birebir; içe aktarmalar: `androidx.navigation.NavType`, `androidx.navigation.compose.{NavHost, composable, rememberNavController}`, `androidx.navigation.navArgument`, `androidx.lifecycle.compose.collectAsStateWithLifecycle`, `kotlinx.coroutines.{Dispatchers, withContext}`):
```kotlin
@Composable
fun AppNav(app: RcApp) {
    val nav = rememberNavController()
    val prefs by app.prefs.state.collectAsStateWithLifecycle()
    val start = remember {
        if (app.core.session() == null || app.prefs.state.value.needsRelogin) "giris" else "kutuphane"
    }
    val toLogin: () -> Unit = { nav.navigate("giris") { popUpTo(0) } }
    // oturum başka yerde (işçi, ekran) düşünce her yerden girişe dön
    LaunchedEffect(prefs.needsRelogin) {
        val route = nav.currentDestination?.route
        if (prefs.needsRelogin && route != null && route != "giris") toLogin()
    }
    // okuyucudan geri: yığın boşsa kütüphaneye; seriesId kullanılmaz
    val readerBack: (Long) -> Unit = { _ ->
        if (!nav.popBackStack()) nav.navigate("kutuphane") { popUpTo(0) }
    }
    // sonraki cilt: mevcut okuyucunun yerine geçer, geri tuşu seriye döner
    val nextBook: (Long) -> Unit = { id ->
        nav.navigate("oku/$id") { popUpTo("oku/{bookId}") { inclusive = true } }
    }
    NavHost(navController = nav, startDestination = start) {
        composable("giris") {
            LoginScreen(
                onLoggedIn = {
                    nav.navigate("kutuphane") { popUpTo("giris") { inclusive = true } }
                },
            )
        }
        composable("kutuphane") {
            LibraryScreen(
                onOpenSeries = { id -> nav.navigate("seri/$id") },
                onOpenBook = { id -> nav.navigate("oku/$id") },
                onOpenSettings = { nav.navigate("ayarlar") },
                onSessionExpired = toLogin,
            )
        }
        composable(
            route = "seri/{seriesId}",
            arguments = listOf(navArgument("seriesId") { type = NavType.LongType }),
        ) { entry ->
            SeriesScreen(
                seriesId = entry.arguments!!.getLong("seriesId"),
                onBack = { nav.popBackStack() },
                onOpenBook = { id -> nav.navigate("oku/$id") },
                onSessionExpired = toLogin,
            )
        }
        composable(
            route = "oku/{bookId}",
            arguments = listOf(navArgument("bookId") { type = NavType.LongType }),
        ) { entry ->
            ReaderRoute(
                app = app,
                bookId = entry.arguments!!.getLong("bookId"),
                onBack = readerBack,
                onNextBook = nextBook,
                onSessionExpired = toLogin,
            )
        }
        composable("ayarlar") {
            SettingsScreen(
                onBack = { nav.popBackStack() },
                onLoggedOut = toLogin,
                onSessionExpired = toLogin,
            )
        }
    }
}

@Composable
private fun ReaderRoute(
    app: RcApp, bookId: Long,
    onBack: (Long) -> Unit, onNextBook: (Long) -> Unit, onSessionExpired: () -> Unit,
) {
    // 0 = yükleniyor, 1 = çizgi roman, 2 = metin, 3 = hata
    val kind by produceState(0, bookId) {
        value = withContext(Dispatchers.IO) {
            runCatching { if (app.core.getBook(bookId).book.isComic) 1 else 2 }.getOrDefault(3)
        }
    }
    when (kind) {
        0 -> Box(Modifier.fillMaxSize().background(Color.Black), Alignment.Center) { CircularProgressIndicator() }
        1 -> ComicReaderScreen(bookId = bookId, onBack = onBack, onNextBook = onNextBook, onSessionExpired = onSessionExpired)
        2 -> TextReaderScreen(bookId = bookId, onBack = onBack, onNextBook = onNextBook, onSessionExpired = onSessionExpired)
        else -> LaunchedEffect(Unit) { onBack(0L) }
    }
}
```
Ekran imzaları (rotalardaki çağrılarla BİREBİR uyumlu): `LoginScreen(onLoggedIn: () -> Unit, vm: LoginViewModel = viewModel())`; `LibraryScreen(onOpenSeries, onOpenBook, onOpenSettings, onSessionExpired, vm)`; `SeriesScreen(seriesId, onBack, onOpenBook, onSessionExpired, vm)`; `SettingsScreen(onBack, onLoggedOut, onSessionExpired, vm)`; `ComicReaderScreen/TextReaderScreen(bookId, onBack: (Long) -> Unit, onNextBook: (Long) -> Unit, onSessionExpired, vm)`. Çıkışta (`onLoggedOut`) ve oturum düşünce (`onSessionExpired`) `toLogin` kullanılır. Geçiş animasyonu: varsayılan.

### 6.6 Ekranlar — durum/olay tipleri

Ortak kalıp: her ViewModel `class XViewModel(app: Application) : AndroidViewModel(app)`; `val state: StateFlow<XState>`; `fun onEvent(e: XEvent)`; tek seferlik etkiler `val effects: SharedFlow<XEffect>` (yalnız belirtilenlerde). Çekirdek çağrıları `viewModelScope.launch(Dispatchers.IO)`. `CoreException.Unauthorized` yakalanınca `prefs.update { it.copy(needsRelogin = true) }` ve ekran `onSessionExpired()` çağırır.

**Giriş** (G15) — `LoginViewModel`
```kotlin
data class LoginState(val server: String = "", val username: String = "", val password: String = "",
    val loading: Boolean = false, val error: String? = null, val relogin: Boolean = false, val showPassword: Boolean = false)
sealed interface LoginEvent {
    data class Server(val v: String) : LoginEvent
    data class Username(val v: String) : LoginEvent
    data class Password(val v: String) : LoginEvent
    data object TogglePassword : LoginEvent
    data object Submit : LoginEvent
}
sealed interface LoginEffect { data object LoggedIn : LoginEffect }
```
Başlangıçta `core.session()` varsa `server`/`username` önceden dolu, `relogin = prefs.needsRelogin`. `Submit`: `core.login(...)`; başarıda `needsRelogin=false`, `Scheduler.schedulePeriodicSync`, `effects.emit(LoggedIn)`; ayrıca ilk eşitleme `Scheduler.kickSync` ile başlar. Hata eşleme: `Unauthorized` → `login_error_credentials`; `Network` → `login_error_network`; `Tls` → `login_error_tls`; `Server(m)`/`InvalidInput(m)` → `m`. Ekran imzası (birebir): `@Composable fun LoginScreen(onLoggedIn: () -> Unit, vm: LoginViewModel = viewModel())`; `vm.effects` `LaunchedEffect(Unit)` içinde toplanır, `LoginEffect.LoggedIn` gelince `onLoggedIn()` çağrılır (yönlendirme `AppNav`'dadır). Düzen: ortalanmış sütun, logo `Image(painter = painterResource(R.drawable.ic_launcher_foreground), contentDescription = null, modifier = Modifier.size(72.dp).clip(RoundedCornerShape(16.dp)).background(Color(0xFF6750A4)))` (foreground vektörü saydam zeminde beyaz glif olduğundan açık temada görünmesi için mor zemin şarttır), başlık `titleLarge`, 3 `OutlinedTextField` (sunucu `KeyboardType.Uri`, parola `PasswordVisualTransformation` + göz ikonu), tam genişlik `Button` (yükleme sırasında `CircularProgressIndicator` 20.dp), hata `error` renginde `bodyMedium`.

**Kütüphane** (G16) — `LibraryViewModel`
```kotlin
data class LibraryState(
    val query: String = "", val onlyDownloaded: Boolean = false,
    val series: List<SeriesItem> = emptyList(), val continueItems: List<ContinueItem> = emptyList(),
    val refreshing: Boolean = false, val online: Boolean = true, val message: String? = null,
    val loaded: Boolean = false,
)
sealed interface LibraryEvent {
    data class Query(val q: String) : LibraryEvent
    data class OnlyDownloaded(val on: Boolean) : LibraryEvent
    data object Refresh : LibraryEvent
    data object MessageShown : LibraryEvent
}
```
Her `Query/OnlyDownloaded` ve her eşitleme/indirme değişiminden sonra `core.listSeries(query, onlyDownloaded)` + `core.continueReading()` yeniden yüklenir (`downloads.infos` akışı değişince de). Açılışta yerel veri hemen gösterilir, ardından (çevrimiçiyse) otomatik `syncNow` çalışır. `Refresh` = `core.syncNow()`; hata `message` olur (`library_refresh_failed`). Ekran: `Scaffold` + `TopAppBar` (başlık `library_title`, eylemler: arama alanı açılır, ayarlar ikonu); çevrimdışıysa üstte `library_offline` şeridi (`tertiaryContainer`); süzgeç satırı `FilterChip(library_only_downloaded)`; gövde `PullToRefreshBox` içinde `LazyVerticalGrid(GridCells.Adaptive(Dimens.gridMin), contentPadding = PaddingValues(Dimens.gutter), horizontalArrangement/verticalArrangement = spacedBy(Dimens.gridGap))`. İlk öğe (`span = { GridItemSpan(maxLineSpan) }`, yalnız sorgu boşken ve liste doluyken): "Okumaya devam" başlığı (`titleMedium`) + `LazyRow` (kart genişliği `Dimens.continueCardWidth`, kapak 2:3, altında `LinearProgressIndicator` ve 2 satır başlık). Seri kartı: `CoverImage` (2:3, `coverRadius`), altında ad (`labelLarge`, 2 satır), `book_count` metni (`labelSmall`), `percent > 0` ise ince ilerleme çizgisi (`progressTrack`), `downloadedCount == bookCount` ise sağ üstte `Icons.Filled.DownloadDone` rozeti. Boş durumlar: `library_empty` / `library_empty_downloaded` / `library_empty_search`. Tıklama → `onOpenSeries(id)`; "devam" kartı → `onOpenBook(bookId)`.
İmza: `@Composable fun LibraryScreen(onOpenSeries: (Long) -> Unit, onOpenBook: (Long) -> Unit, onOpenSettings: () -> Unit, onSessionExpired: () -> Unit, vm: LibraryViewModel = viewModel())`.

**Seri** (G16) — `SeriesViewModel` (`SavedStateHandle` yerine ekran `seriesId` parametresini `fun load(id: Long)` ile verir)
```kotlin
data class SeriesState(
    val loaded: Boolean = false, val detail: SeriesDetail? = null, val online: Boolean = true,
    val message: String? = null, val busy: Boolean = false,
)
sealed interface SeriesEvent {
    data class Download(val bookIds: List<Long>) : SeriesEvent
    data class RemoveDownload(val bookIds: List<Long>) : SeriesEvent
    data class MarkStatus(val bookIds: List<Long>, val status: ReadStatus) : SeriesEvent
    data object MessageShown : SeriesEvent
}
```
Öğeler: üst çubukta seri adı + geri; eylem menüsü (`series_download_all` → hata/DRM olmayan ve `download.state != DONE` tüm kitaplar; `series_delete_all`; `series_mark_all_read`; `series_mark_all_unread`). Genişletilmiş FAB: ilk `status != FINISHED` kitap (yoksa ilk kitap) için `series_continue` (okuma yüzdesi varsa) veya `series_read`. `LazyColumn` satırı `BookRow`: sol 56x84 kapak (`CoverImage`), orta: başlık (`bodyLarge`, 2 satır), meta (`labelMedium`: `book_meta_comic`/`book_meta_text` + biçim büyük harf + `Formatter.formatShortFileSize`), durum (`book_status_*`) + `LinearProgressIndicator(progress = percent)`; sağ: indirme düğmesi — `NONE`→`Icons.Outlined.Download` (tıkla: indir); `QUEUED/RUNNING`→ `CircularProgressIndicator(progress = partsDone/partsTotal)` 24.dp + altında `download_running` (tıkla: iptal=`RemoveDownload`); `DONE`→`Icons.Filled.CheckCircle` (tıkla: menü `book_delete_download`); `FAILED`→ `Icons.Filled.ErrorOutline` (tıkla: yeniden dene). Durum metni eşlemesi (`BookRow`, birebir): `when (book.status) { ReadStatus.UNREAD -> stringResource(R.string.book_status_unread); ReadStatus.READING -> stringResource(R.string.book_status_reading, formatPercent(book.percent)); ReadStatus.FINISHED -> stringResource(R.string.book_status_finished); ReadStatus.DROPPED -> stringResource(R.string.book_status_dropped) }`. Uzun basma/üç nokta menüsü: `book_mark_read`, `book_mark_unread`, indir/sil. `error != null` olan satır soluk (alpha 0.5) ve tıklayınca `book_unavailable` snackbar. Çevrimdışıyken `download.state != DONE` satırlar soluk; tıklayınca `book_needs_network`. Tıklama → `onOpenBook(id)`. İlk indirmede (API 33+) `POST_NOTIFICATIONS` izni `rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission())` ile bir kez istenir; reddedilse de indirme sürer. `MarkStatus` her kitap için `core.setStatus(id, status, LocalDate.now().toString())` çağırır ve `Scheduler.kickSync`.
İmza: `@Composable fun SeriesScreen(seriesId: Long, onBack: () -> Unit, onOpenBook: (Long) -> Unit, onSessionExpired: () -> Unit, vm: SeriesViewModel = viewModel())`.

**Ayarlar** (G15) — `SettingsViewModel`
```kotlin
data class SettingsState(
    val prefs: UiPrefs, val session: SessionInfo?, val sync: SyncStatus?, val storage: StorageUsage?,
    val syncing: Boolean = false, val message: String? = null, val confirmLogout: Boolean = false,
)
sealed interface SettingsEvent {
    data class Patch(val f: (UiPrefs) -> UiPrefs) : SettingsEvent
    data object SyncNow : SettingsEvent
    data object ClearCache : SettingsEvent
    data object AskLogout : SettingsEvent
    data object CancelLogout : SettingsEvent
    data object ConfirmLogout : SettingsEvent
    data object MessageShown : SettingsEvent
}
sealed interface SettingsEffect { data object LoggedOut : SettingsEffect }
```
Bölümler (`SettingsSection` başlıkları `labelLarge`, `primary`): Görünüm (tema `SingleChoiceSegmentedButtonRow` Sistem/Açık/Koyu; AMOLED `Switch` — koyu etkin değilse de açıktır ama etkisi koyuda görünür; dinamik renk `Switch` yalnız SDK ≥ 31'de görünür), Okuyucu (okuyucu teması: `Otomatik/Açık/Koyu/Sepya/Saf siyah` `FilterChip` satırı; yazı tipi/boyut/satır aralığı/kenar boşluğu/hizalama/okuma biçimi varsayılanları; çizgi roman yönü `ltr/rtl/webtoon`; tam ekran, ekran açık, dokunma bölgeleri, ses tuşları anahtarları), İndirmeler (`wifiOnly`, depolama üç satırı + `settings_clear_cache` düğmesi), Eşitleme (son eşitleme zamanı `DateUtils.getRelativeTimeSpanString`, bekleyen işlem sayısı, son hata, `settings_sync_now` düğmesi; çalışırken `LinearProgressIndicator`), Hesap (sunucu, kullanıcı, `settings_logout`). `ConfirmLogout`: `core.syncStatus().pendingOps > 0` ise `logout_confirm_pending` eklenir; onayda `core.logout()`, `Scheduler.cancelAll`, `downloads.refresh()`, `LoggedOut`.
İmza: `@Composable fun SettingsScreen(onBack: () -> Unit, onLoggedOut: () -> Unit, onSessionExpired: () -> Unit, vm: SettingsViewModel = viewModel())`.

**Ortak bileşenler** (G13, `ui/common/Components.kt`): 
- `@Composable fun CoverImage(bookId: Long?, version: Long, modifier: Modifier = Modifier, radius: Dp = Dimens.coverRadius)` — `bookId == null` ise `surfaceContainerHigh` zemin + `Icons.Outlined.MenuBook`; aksi halde `AsyncImage(model = RcImage.Cover(bookId, version), contentScale = ContentScale.Crop)`, `aspectRatio(Dimens.coverAspect)`, kırpma `RoundedCornerShape(radius)`, `contentDescription = null`.
- `@Composable fun EmptyState(icon: ImageVector, text: String, modifier: Modifier = Modifier)`.
- `@Composable fun OfflineBanner(text: String)`.
- `@Composable fun ConfirmDialog(title: String, body: String, confirmLabel: String, onConfirm: () -> Unit, onDismiss: () -> Unit)`.
- `@Composable fun SettingsSwitchRow(title: String, desc: String? = null, checked: Boolean, onChange: (Boolean) -> Unit)`.
- `fun formatPercent(p: Double): Int = (p * 100).roundToInt().coerceIn(0, 100)`.

### 6.7 Okuyucu ortak parçalar (G17) — `ui/reader/ReaderCommon.kt`

```kotlin
class ReadingTracker(private val app: RcApp, private val scope: CoroutineScope) {
    var foreground: Boolean = true          // ekran LifecycleEventEffect ile ON_RESUME/ON_PAUSE'ta günceller
    fun start(bookId: Long, startPercent: Double)   // core.pingStart (IO) -> clientId; 1 sn'lik sayaç başlar
    fun touch()                              // son girdi anı = şimdi
    fun addPage(key: String)                 // benzersiz sayfa kümesine ekler, touch() çağırır
    fun setPercent(p: Double)                // endPercent
    fun stop()                               // son pingUpdate + pingClose + Scheduler.kickSync(app)
}
```
Sayaç: her saniye `foreground && now - lastInput <= 60_000` ise `active++` (web `tracker.js` ile aynı kural). Her 15 sn `core.pingUpdate(clientId, active, pages.size, endPercent)`; her 30 sn `core.syncPush()` (IO, hata yutulur). `pagesRead = pages.size`, `endPercent`/`startPercent` `[0,1]`'e kırpılır.

```kotlin
@Composable
fun ReaderWindowEffects(prefs: UiPrefs, onVolume: ((forward: Boolean) -> Boolean)?)
```
- `prefs.immersive`: `WindowCompat.getInsetsController(window, view)`; `hide(WindowInsetsCompat.Type.systemBars())`, `systemBarsBehavior = BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE`; `onDispose` → `show(systemBars())`.
- `prefs.keepAwake`: `window.addFlags(FLAG_KEEP_SCREEN_ON)`, `onDispose` → `clearFlags`.
- `prefs.volumeKeys && onVolume != null`: `VolumeKeys.handler = onVolume`, `onDispose` → `null`.
`window` = `(LocalContext.current as Activity).window`.

```kotlin
@Composable
fun BookmarksSheet(items: List<BookmarkItem>, onOpen: (BookmarkItem) -> Unit,
                   onDelete: (Long) -> Unit, onDismiss: () -> Unit)   // ModalBottomSheet; boşsa reader_bookmarks_empty
```
Satır: etiket (`bodyLarge`), `pending` ise küçük `Icons.Outlined.CloudOff`, silme ikonu (`localId`).

### 6.8 Çizgi roman okuyucu (G17)

`ComicReaderViewModel`
```kotlin
data class ComicState(
    val loading: Boolean = true, val error: String? = null, val title: String = "", val seriesId: Long = 0,
    val pageCount: Int = 0, val page: Int = 0, val mode: String = "ltr",       // ltr | rtl | webtoon
    val barsVisible: Boolean = true, val showBookmarks: Boolean = false,
    val bookmarks: List<BookmarkItem> = emptyList(),
    val nextBookId: Long? = null, val nextBookTitle: String? = null, val prefs: UiPrefs = UiPrefs(),
)
sealed interface ComicEvent {
    data class Load(val bookId: Long) : ComicEvent
    data class PageChanged(val page: Int) : ComicEvent        // pager/webtoon yerleşince
    data class SeekTo(val page: Int) : ComicEvent              // kaydırıcı bırakıldı / yer imi
    data object ToggleBars : ComicEvent
    data class SetMode(val mode: String) : ComicEvent
    data object ToggleBookmark : ComicEvent
    data object ShowBookmarks : ComicEvent
    data object HideBookmarks : ComicEvent
    data class OpenBookmark(val b: BookmarkItem) : ComicEvent
    data class DeleteBookmark(val localId: Long) : ComicEvent
    data object Retry : ComicEvent
}
sealed interface ComicEffect { data class ScrollTo(val page: Int) : ComicEffect }
```
- `Load`: `core.getBook(id)`; sayfa sayısı `book.page_count`; çevrimiçiyse `withTimeoutOrNull(4000) { core.pullBook(id) }`; sonra `core.getProgress(id).locator.toIntOrNull()` (aralık dışıysa 0) başlangıç sayfası; `mode = prefs.comicModeFor(id)`; yer imleri `core.listBookmarks`; `tracker.start(id, (page+1)/count)`.
- `PageChanged(p)`: `state.page=p`; `core.saveProgress(id, p.toString(), (p+1).toDouble()/count, LocalDate.now().toString())`; `tracker.addPage(p.toString())`; `tracker.setPercent(...)`.
- `ToggleBookmark`: mevcut sayfanın yer imi (`locator == page.toString()`) varsa `deleteBookmark`, yoksa `addBookmark(id, page.toString(), "Sayfa ${page+1}")`; ardından `Scheduler.kickSync`.
- `SetMode(m)`: `prefs.setComicModeFor(id, m)`; mod değişince geçerli sayfaya `ScrollTo` etkisi.
- Çıkışta (`onCleared` ve ekran `DisposableEffect`): `tracker.stop()`.
- Sonraki cilt: son sayfada (`page == pageCount-1`) ve `nextBookId != null` ise altta kart (`reader_next_volume`, `reader_open` düğmesi → `onNextBook(id)`).

`ComicReaderScreen`:
```kotlin
@Composable fun ComicReaderScreen(bookId: Long, onBack: (seriesId: Long) -> Unit, onNextBook: (Long) -> Unit,
                                  onSessionExpired: () -> Unit, vm: ComicReaderViewModel = viewModel())
```
- Arka plan `Color.Black`. Üst çubuk (başlık, geri, yer imi anahtarı `Icons.Filled.Bookmark/BookmarkBorder`, yer imleri listesi, mod menüsü: `comic_dir_ltr/rtl/webtoon`) ve alt çubuk (sayfa göstergesi `reader_page_of` + `Slider`), `AnimatedVisibility(barsVisible)`.
- `ltr`/`rtl`: `HorizontalPager(state, reverseLayout = (mode == "rtl"), beyondViewportPageCount = 1, key = { it })`; her sayfa `ZoomableBox(onTap = ...) { AsyncImage(model = ImageRequest.Builder(ctx).data(RcImage.Page(bookId, it)).size(2000, 2000).build(), contentDescription = null, contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize()) }`. Dokunma bölgeleri (`prefs.tapZones`): x/genişlik `< 1/3` → sol, `> 2/3` → sağ, orta → `ToggleBars`. LTR'de sol=geri sağ=ileri; RTL'de tersi. Sayfa değişimi `pagerState.animateScrollToPage`. `snapshotFlow { pagerState.settledPage }.distinctUntilChanged()` → `PageChanged`. Slider: `Slider(value = page.toFloat(), valueRange = 0f..(count-1).toFloat(), steps = (count-2).coerceAtLeast(0), onValueChange = yerel, onValueChangeFinished = SeekTo)`; RTL'de slider `CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Rtl)` içinde.
- `webtoon`: `LazyColumn(state = LazyListState(firstVisibleItemIndex = page))`, `items(pageCount) { AsyncImage(model = RcImage.Page(bookId, it), contentScale = ContentScale.FillWidth, modifier = Modifier.fillMaxWidth().heightIn(min = 320.dp)) }`; dokunma → `ToggleBars`; geçerli sayfa = `firstVisibleItemIndex` (yerleşince `PageChanged`). Webtoon'da yakınlaştırma YOK (kapsam kararı).
- Önbellek ısıtma: `settledPage` değişince `imageLoader.enqueue(...)` ile `page+1` ve `page+2` (RTL'de de +1 yönü mantıksal ileridir).
- `ReaderWindowEffects(prefs, onVolume = { forward -> scope.launch { pagerState.animateScrollToPage(pagerState.currentPage + if (forward) 1 else -1) }; true })` (`scope = rememberCoroutineScope()`; webtoon modunda `listState.animateScrollBy` ±ekran yüksekliğinin %90'ı); ileri/geri yönü RTL'de mantıksaldır (ileri = sonraki sayfa).
- Yükleme sırasında `CircularProgressIndicator`; hata `EmptyState(Icons.Outlined.ErrorOutline, error)` + `Retry`.

`ZoomableBox.kt` (birebir):
```kotlin
@Composable
fun ZoomableBox(onTap: (Offset, IntSize) -> Unit, content: @Composable () -> Unit) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    var size by remember { mutableStateOf(IntSize.Zero) }
    fun clamp(o: Offset, s: Float): Offset {
        val mx = size.width * (s - 1f) / 2f
        val my = size.height * (s - 1f) / 2f
        return Offset(o.x.coerceIn(-mx, mx), o.y.coerceIn(-my, my))
    }
    Box(
        Modifier
            .fillMaxSize()
            .onSizeChanged { size = it }
            .pointerInput(Unit) {
                detectTapGestures(
                    onTap = { onTap(it, size) },
                    onDoubleTap = { p ->
                        if (scale > 1f) { scale = 1f; offset = Offset.Zero }
                        else {
                            val c = Offset(size.width / 2f, size.height / 2f)
                            scale = 2.5f
                            offset = clamp((c - p) * scale, scale)
                        }
                    },
                )
            }
            .pointerInput(Unit) {
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    do {
                        val event = awaitPointerEvent()
                        val pressed = event.changes.count { it.pressed }
                        if (pressed >= 2 || scale > 1f) {
                            val ns = (scale * event.calculateZoom()).coerceIn(1f, 4f)
                            offset = if (ns == 1f) Offset.Zero else clamp(offset + event.calculatePan(), ns)
                            scale = ns
                            event.changes.forEach { if (it.positionChanged()) it.consume() }
                        }
                    } while (event.changes.any { it.pressed })
                }
            }
            .graphicsLayer { scaleX = scale; scaleY = scale; translationX = offset.x; translationY = offset.y },
    ) { content() }
}
```
(Tek parmak ve `scale == 1` iken olay tüketilmez, böylece `HorizontalPager` kaydırması çalışır.) İçe aktarmalar: `androidx.compose.foundation.gestures.{awaitEachGesture, awaitFirstDown, calculatePan, calculateZoom, detectTapGestures}`, `androidx.compose.ui.input.pointer.{pointerInput, positionChanged}`.

### 6.9 Metin okuyucu (G18)

Mimari: Compose üstünde `AndroidView(WebView)`; WebView tek bir kabuk sayfasını (`reader.html`) açar, bölüm gövdesi `fetch` ile yerel dosyadan alınıp `document.body.innerHTML`'e basılır. Bölüm dosyalarını çekirdek yazar (`ensureChapter`), WebView `WebViewAssetLoader` ile okur:
- `https://appassets.androidplatform.net/static/...` → `AssetsPathHandler` (`reader.html`, `reader.js`, `frame.css`, `reader-fonts.css`, `fonts/*.woff2`).
- `https://appassets.androidplatform.net/book/{id}/c{n}.html` ve `/book/{id}/a/{ad}` → `InternalStoragePathHandler(ctx, File(ctx.filesDir, "rc/books"))`.
Böylece yazı tipleri, `frame.css` ve bölüm HTML'i sunucudaki web okuyucuyla BİREBİR aynı kaynaklardan gelir; web'den kopyalanan tek şey derleme anında `syncWebAssets` ile asset köküne giren dosyalardır.

`ReaderWebView.kt` (birebir iskelet):
```kotlin
sealed interface WebCommand {
    data class SetPrefs(val json: String) : WebCommand
    data class Load(val path: String, val ch: Int, val ratio: Double) : WebCommand   // path: "/book/{id}/c{n}.html"
    data class Page(val dir: Int) : WebCommand                                      // +1 ileri, -1 geri
}

@Composable
fun ReaderWebView(commands: Flow<WebCommand>, bgColor: Int, onMessage: (String) -> Unit, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    val loader = remember {
        WebViewAssetLoader.Builder()
            .setDomain("appassets.androidplatform.net")
            .addPathHandler("/static/", WebViewAssetLoader.AssetsPathHandler(ctx))
            .addPathHandler("/book/", WebViewAssetLoader.InternalStoragePathHandler(ctx, File(ctx.filesDir, "rc/books")))
            .build()
    }
    var web by remember { mutableStateOf<WebView?>(null) }
    AndroidView(
        modifier = modifier,
        factory = { c ->
            WebView(c).apply {
                settings.javaScriptEnabled = true
                settings.allowFileAccess = false
                settings.allowContentAccess = false
                settings.domStorageEnabled = false
                settings.setSupportZoom(false)
                settings.textZoom = 100
                isVerticalScrollBarEnabled = false
                isHorizontalScrollBarEnabled = false
                overScrollMode = View.OVER_SCROLL_NEVER
                webViewClient = object : WebViewClient() {
                    override fun shouldInterceptRequest(v: WebView, r: WebResourceRequest) = loader.shouldInterceptRequest(r.url)
                    override fun shouldOverrideUrlLoading(v: WebView, r: WebResourceRequest) = true   // dış gezinme yok
                }
                addJavascriptInterface(object { @JavascriptInterface fun post(json: String) = onMessage(json) }, "Android")
                loadUrl("https://appassets.androidplatform.net/static/reader.html")
                web = this
            }
        },
        update = { it.setBackgroundColor(bgColor) },
    )
    LaunchedEffect(web) {
        val w = web ?: return@LaunchedEffect
        commands.collect { c ->
            val js = when (c) {
                is WebCommand.SetPrefs -> "RC.setPrefs(${c.json})"
                is WebCommand.Load -> "RC.load(${JSONObject.quote(c.path)}, ${c.ch}, ${c.ratio})"
                is WebCommand.Page -> "RC.page(${c.dir})"
            }
            w.evaluateJavascript(js, null)   // ana iş parçacığı: collect Main'de çalışır
        }
    }
    DisposableEffect(Unit) { onDispose { web?.destroy() } }
}
```
`onMessage` JS iş parçacığından gelir; ViewModel `viewModelScope.launch` ile işler.

**JS → Kotlin mesajları** (`Android.post(JSON.stringify(obj))`):
| `t` | alanlar | anlam / Kotlin tepkisi |
|---|---|---|
| `ready` | — | kabuk hazır: VM `SetPrefs` + geçerli konumu `Load` olarak yollar |
| `loaded` | `ch:int, pages:int` | bölüm yerleşti; VM `loading=false`, sonraki bölümü arka planda `ensureChapter` |
| `pos` | `ch:int, p:float, page:int, pages:int` | konum değişti: VM durumu + `saveProgress` (500 ms birleştirme) + `tracker.addPage` |
| `tap` | — | `ToggleBars` (sayfa modunda kenar üçlüleri JS içinde işlenir, `tap` yalnız orta/kaydırma modu) |
| `edge` | `d:int` (+1/-1) | sayfa/ekran sınırı aşıldı: `d=+1` → sonraki bölüm (`ratio 0`), `d=-1` → önceki bölüm (`ratio 1`); yoksa sonraki cilt kartı denetimi |
| `error` | `msg:string` | VM `error = msg` + yeniden dene |
**Kotlin → JS** (`RC.*`): `RC.setPrefs({bg,fg,link,font,fs,lh,align,mode,mg,zones})`, `RC.load(path, ch, ratio)`, `RC.page(dir)`.
`SetPrefs` alanları: `bg/fg/link` okuyucu teması renkleri (`"#RRGGBB"`), `font` kimlik (`literata`...), `fs` Int px, `lh` Float, `align` `left|justify`, `mode` `page|scroll`, `mg` Int (0..20), `zones` Boolean (`prefs.tapZones`).

`assets/reader.html` (birebir):
```html
<!doctype html>
<html lang="tr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'">
<link rel="stylesheet" href="/static/reader-fonts.css">
<link rel="stylesheet" href="/static/frame.css">
<script src="/static/reader.js" defer></script>
</head>
<body></body>
</html>
```
`assets/reader.js` (birebir; `web/reader-text.js` `relayout/commitPage/scheduleSnap/onScrollMode` mantığının iframe'siz uyarlaması; `frame.css` değişmeden kullanılır):
```js
(function () {
  'use strict';
  var FONTS = {
    literata: '"Literata Variable", Georgia, "Noto Serif", serif',
    merriweather: '"Merriweather Variable", Georgia, "Noto Serif", serif',
    'source-serif': '"Source Serif 4 Variable", Georgia, "Noto Serif", serif',
    'eb-garamond': '"EB Garamond Variable", Georgia, "Noto Serif", serif',
    inter: '"Inter Variable", system-ui, sans-serif',
    atkinson: '"Atkinson Hyperlegible", system-ui, sans-serif',
    lexend: '"Lexend Variable", system-ui, sans-serif'
  };
  var BAR_V = 176;                       // frame.css: üst 80 + alt 96 dolgu
  var root = document.documentElement;
  var P = { mode: 'scroll', mg: 0, fs: 18, zones: true };
  var ch = 0, ratio = 0, pageIdx = 0, pageTotal = 1;
  var ready = false, restoring = false, deferred = false, lastScrollAt = 0;
  var snapT = null, relT = null, down = null;

  function post(o) { try { Android.post(JSON.stringify(o)); } catch (e) {} }
  function clamp01(v) { v = Number(v); return !isFinite(v) ? 0 : v < 0 ? 0 : v > 1 ? 1 : v; }
  function se() { return document.scrollingElement || root; }
  function paged() { return P.mode === 'page'; }
  function report() { post({ t: 'pos', ch: ch, p: Number(ratio.toFixed(6)), page: pageIdx, pages: pageTotal }); }

  function applyPrefs() {
    var s = root.style;
    s.setProperty('--bg', P.bg); s.setProperty('--fg', P.fg); s.setProperty('--link', P.link);
    s.setProperty('--fs', P.fs + 'px'); s.setProperty('--lh', String(P.lh));
    s.setProperty('--ws', '0em'); s.setProperty('--ls', '0em');
    s.setProperty('--font', FONTS[P.font] || FONTS.literata);
    s.setProperty('--align', P.align); s.setProperty('--hyph', P.align === 'justify' ? 'auto' : 'manual');
    root.classList.toggle('paged', paged());
  }

  function relayout() {
    if (!ready) return;
    var pw = window.innerWidth, ph = window.innerHeight;
    var m = Math.max(16 + (pw * P.mg) / 100, (pw - 38 * P.fs) / 2);
    var st = root.style;
    st.setProperty('--pw', pw + 'px'); st.setProperty('--ph', ph + 'px'); st.setProperty('--m', m + 'px');
    st.setProperty('--bh', Math.max(0, ph - BAR_V) + 'px');
    st.setProperty('--mx', Math.round((pw * P.mg) / 100) + 'px');
    var e = se();
    restoring = true;
    if (paged()) {
      var range = document.createRange();
      range.selectNodeContents(document.body);
      var right = range.getBoundingClientRect().right + e.scrollLeft;
      pageTotal = Math.max(1, Math.ceil((right - 1) / pw));
      pageIdx = pageTotal <= 1 ? 0 : Math.round(ratio * (pageTotal - 1));
      window.scrollTo({ left: pageIdx * pw, top: 0, behavior: 'auto' });
    } else {
      pageTotal = 1; pageIdx = 0;
      e.scrollTop = Math.round(ratio * Math.max(0, e.scrollHeight - e.clientHeight));
    }
    requestAnimationFrame(function () {
      requestAnimationFrame(function () {
        restoring = false;
        if (deferred) { deferred = false; onScroll(); }
      });
    });
    var single = paged() ? pageTotal <= 1 : e.scrollHeight - e.clientHeight <= 2;
    if (single && ratio < 1) ratio = 1;      // tek ekranlık bölüm okunmuş sayılır
    report();
  }
  function relayoutSoon() {
    clearTimeout(relT); clearTimeout(snapT);
    relT = setTimeout(function () { relT = null; relayout(); }, 100);
  }

  function snapSoon() {
    clearTimeout(snapT);
    snapT = setTimeout(function () {
      if (relT || !paged()) return;
      var pw = window.innerWidth, e = se();
      var i = Math.max(0, Math.min(pageTotal - 1, Math.round(e.scrollLeft / pw)));
      if (Math.abs(e.scrollLeft - i * pw) > 1) window.scrollTo({ left: i * pw, top: 0, behavior: 'auto' });
      if (i !== pageIdx) { pageIdx = i; ratio = pageTotal <= 1 ? 0 : i / (pageTotal - 1); report(); }
    }, 120);
  }

  function onScroll() {
    lastScrollAt = Date.now();
    if (restoring) { deferred = true; return; }
    if (paged()) { snapSoon(); return; }
    var e = se(), max = Math.max(0, e.scrollHeight - e.clientHeight);
    var r = max > 0 ? clamp01(e.scrollTop / max) : 0;
    if (e.scrollTop + e.clientHeight >= e.scrollHeight - 2) r = 1;
    ratio = r; report();
  }
  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', relayoutSoon);
  document.addEventListener('load', relayoutSoon, true);            // görseller yüklendikçe düzen değişir
  if (document.fonts && document.fonts.addEventListener) document.fonts.addEventListener('loadingdone', relayoutSoon);

  document.addEventListener('pointerdown', function (e) { down = { x: e.clientX, y: e.clientY }; }, { passive: true });
  document.addEventListener('click', function (e) {
    var a = e.target.closest && e.target.closest('a');
    if (a) e.preventDefault();                                       // dış bağlantı yok
    var moved = down && (Math.abs(e.clientX - down.x) > 10 || Math.abs(e.clientY - down.y) > 10);
    down = null;
    if (moved || Date.now() - lastScrollAt < 250) return;
    if (paged() && P.zones) {
      var z = e.clientX / window.innerWidth;
      if (z < 1 / 3) { RC.page(-1); return; }
      if (z > 2 / 3) { RC.page(1); return; }
    }
    post({ t: 'tap' });
  });

  var RC = (window.RC = {});
  RC.setPrefs = function (p) { P = p; applyPrefs(); relayoutSoon(); };
  RC.load = function (path, c, r) {
    ready = false; ch = c; ratio = clamp01(r);
    fetch(path).then(function (res) {
      if (!res.ok) throw new Error('HTTP ' + res.status);
      return res.text();
    }).then(function (html) {
      document.body.innerHTML = html;
      ready = true; applyPrefs(); relayout();
      post({ t: 'loaded', ch: ch, pages: pageTotal });
    }).catch(function (err) { post({ t: 'error', msg: String(err) }); });
  };
  RC.page = function (d) {
    if (!ready) return;
    var e = se();
    if (paged()) {
      var i = pageIdx + d;
      if (i >= pageTotal) { post({ t: 'edge', d: 1 }); return; }
      if (i < 0) { post({ t: 'edge', d: -1 }); return; }
      pageIdx = i; ratio = pageTotal <= 1 ? 0 : i / (pageTotal - 1);
      window.scrollTo({ left: i * window.innerWidth, top: 0, behavior: 'auto' });
      report();
    } else {
      if (d > 0 && e.scrollTop + e.clientHeight >= e.scrollHeight - 2) { post({ t: 'edge', d: 1 }); return; }
      if (d < 0 && e.scrollTop <= 0) { post({ t: 'edge', d: -1 }); return; }
      e.scrollTop += d * Math.max(40, Math.floor(e.clientHeight * 0.9));
    }
  };
  post({ t: 'ready' });
})();
```

`TextReaderViewModel`
```kotlin
enum class TextSheet { NONE, TOC, BOOKMARKS, SEARCH, SETTINGS }
data class SearchUi(val query: String = "", val loading: Boolean = false,
                    val hits: List<SearchHitItem> = emptyList(), val truncated: Boolean = false,
                    val error: String? = null, val searched: Boolean = false)
data class TextState(
    val loading: Boolean = true, val error: String? = null, val title: String = "", val seriesId: Long = 0,
    val chapterCount: Int = 1, val toc: List<TocEntry> = emptyList(),
    val chapter: Int = 0, val ratio: Double = 0.0, val page: Int = 0, val pages: Int = 1, val percent: Double = 0.0,
    val barsVisible: Boolean = true, val sheet: TextSheet = TextSheet.NONE,
    val bookmarks: List<BookmarkItem> = emptyList(), val isBookmarked: Boolean = false,
    val search: SearchUi = SearchUi(), val online: Boolean = true,
    val nextBookId: Long? = null, val nextBookTitle: String? = null, val showNextCard: Boolean = false,
    val prefs: UiPrefs = UiPrefs(),
)
sealed interface TextEvent {
    data class Load(val bookId: Long) : TextEvent
    data class Web(val json: String) : TextEvent                 // JS mesajı
    data object ToggleBars : TextEvent
    data class OpenSheet(val sheet: TextSheet) : TextEvent
    data object CloseSheet : TextEvent
    data class GoToc(val index: Int) : TextEvent                 // ratio 0
    data class GoChapter(val delta: Int) : TextEvent             // -1/+1, ratio 0
    data object ToggleBookmark : TextEvent
    data class OpenBookmark(val b: BookmarkItem) : TextEvent
    data class DeleteBookmark(val localId: Long) : TextEvent
    data class SearchQuery(val q: String) : TextEvent            // 350 ms bekle, ≥2 karakter, yalnız online
    data class OpenHit(val h: SearchHitItem) : TextEvent
    data class Page(val dir: Int) : TextEvent                    // ses tuşları / düğme
    data object Retry : TextEvent
}
val commands: SharedFlow<WebCommand>      // ekran ReaderWebView'a verir
```
- `Load`: `core.getBook`; `core.textInfo` → `chapterCount`, `toc`; çevrimiçiyse `withTimeoutOrNull(4000){ core.pullBook }`; konum `core.getProgress(id).locator`: JSON `{"ch":N,"p":R}` (`org.json.JSONObject` ile; hatalıysa 0/0.0; `ch` aralık dışıysa 0); `tracker.start(id, percent)`; `ready` mesajı gelince `SetPrefs` + `loadChapter(ch, ratio)`.
- `loadChapter(ch, ratio)`: `core.ensureChapter(id, ch)` (IO) → `Offline` ise `error = "Bölüm indirilmemiş ve bağlantı yok"` (string kaynağı `reader_error`), aksi halde `commands.emit(Load("/book/$id/c$ch.html", ch, ratio))`.
- `pos` mesajı: `percent = (ch + p) / chapterCount`; `core.saveProgress(id, "{\"ch\":$ch,\"p\":${"%.6f".format(Locale.ROOT, p)}}", percent, LocalDate.now().toString())` (500 ms birleştirme); `isBookmarked` = aynı `ch` ve `|p - ratio| <= max(0.005, 0.5/pages)`; `tracker.addPage("$ch:${if (paged) page else (p*20).toInt()}")`; `showNextCard = ch == chapterCount-1 && nextBookId != null && (paged ? page >= pages-1 : p >= 0.98)`.
- Yer imi: `locator` = aynı JSON biçimi (web ile uyumlu), `label = "Bölüm ${ch+1} · %${percent*100 yuvarlak}"` (`reader_chapter_percent`). `OpenBookmark`: JSON'u ayrıştırıp (`ch`, `p`) aynı bölümdeyse `RC.load` yerine `loadChapter(ch, p)` yine yeniden yükler (basitlik: her zaman `loadChapter`).
- Arama: `core.searchText` (IO); çevrimdışıysa `search.error = reader_search_offline`. `OpenHit(h)` → `loadChapter(h.ch, h.p)` + sheet kapanır.
- Prefs değişince (`app.prefs.state`) `commands.emit(SetPrefs(json))`; `readerTheme == "auto"` ise `isEffectiveDark` ve `amoled`'e göre `black|dark|light`.
- Çıkış: `tracker.stop()`.

`TextReaderScreen`
```kotlin
@Composable fun TextReaderScreen(bookId: Long, onBack: (seriesId: Long) -> Unit, onNextBook: (Long) -> Unit,
                                 onSessionExpired: () -> Unit, vm: TextReaderViewModel = viewModel())
```
Düzen: `Box`: `ReaderWebView(...)` tam ekran (immersive değilse `windowInsetsPadding(WindowInsets.systemBars)`), üst çubuk (geri, başlık tek satır, yer imi anahtarı, ara, içindekiler, ayarlar), alt çubuk (`LinearProgressIndicator(progress = percent)` + metin `reader_chapter_percent` + sayfa modunda `reader_page_of`, önceki/sonraki bölüm ikonları), her ikisi `AnimatedVisibility(barsVisible)` ve `surface.copy(alpha=0.94f)`. Sonraki cilt kartı (`showNextCard`): altta `Card` (`reader_next_volume` + `reader_open`). Sheet'ler `ModalBottomSheet`: İçindekiler (`LazyColumn`, geçerli bölüm vurgulu), Yer imleri (`BookmarksSheet`), Arama (alan + sonuç listesi: `before + **hit** + after` `AnnotatedString`, durum metinleri `reader_search_*`), Ayarlar (yazı tipi `FilterChip` satırı 7 seçenek, boyut/satır aralığı/kenar boşluğu `Slider`, hizalama ve okuma biçimi `SingleChoiceSegmentedButtonRow`, okuyucu teması 5 `FilterChip`; değişiklik `app.prefs.update`). `ReaderWindowEffects(prefs, onVolume = { f -> vm.onEvent(TextEvent.Page(if (f) 1 else -1)); true })`. WebView arka planı `Color.parseColor(readerColors(...).bg)`.

### 6.10 Simge (G12)
`web/favicon.svg` içindeki tek `<path>`'in `d` değeri BİREBİR kopyalanır (Material "menu_book", 960'lık koordinat, y negatif). Mavi/mor zemin `#6750A4`.
`res/values/ic_launcher_background.xml`: `<resources><color name="ic_launcher_background">#6750A4</color></resources>`.
`res/drawable/ic_launcher_foreground.xml`:
```xml
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp" android:height="108dp" android:viewportWidth="108" android:viewportHeight="108">
    <group android:translateX="30" android:translateY="30" android:scaleX="0.05" android:scaleY="0.05">
        <group android:translateY="960">
            <path android:fillColor="#FFFFFF" android:pathData="<favicon.svg path d>"/>
        </group>
    </group>
</vector>
```
(960 birim x 0.05 = 48 birim, 108'lik kanvasın ortasında; güvenli bölge 66 birimin içinde glif ~42 birim.)
`mipmap-anydpi-v26/ic_launcher.xml` ve `ic_launcher_round.xml` (aynı içerik): `<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android"><background android:drawable="@color/ic_launcher_background"/><foreground android:drawable="@drawable/ic_launcher_foreground"/></adaptive-icon>`. Min SDK 26 olduğundan ek raster simge gerekmez.

### 6.11 `strings.xml` (G12) — TAM LİSTE (`res/values/strings.xml`)
Biçim dizgileri `%1$d` biçimindedir; `%` işareti için `%%`.
```
app_name=ReadCrab
action_back=Geri | action_close=Kapat | action_cancel=Vazgeç | action_ok=Tamam | action_retry=Yeniden dene
action_delete=Sil | action_search=Ara | action_settings=Ayarlar | action_more=Diğer işlemler
login_title=ReadCrab'e giriş | login_server=Sunucu adresi | login_server_hint=http://192.168.1.10:3435
login_username=Kullanıcı adı | login_password=Parola | login_button=Giriş yap | login_show_password=Parolayı göster
login_error_credentials=Kullanıcı adı veya parola hatalı. | login_error_network=Sunucuya ulaşılamadı. Adresi ve bağlantıyı denetleyin.
login_error_tls=Güvenli bağlantı kurulamadı: sunucunun sertifikası doğrulanamadı. Özel CA veya kendi imzalı sertifikalar desteklenmez; genel bir CA sertifikası (ör. Let's Encrypt) ya da http:// adresi kullanın.
login_relogin=Oturumunuzun süresi doldu. Yeniden giriş yapın.
library_title=Kütüphane | library_continue=Okumaya devam | library_search_hint=Seri ara
library_only_downloaded=Yalnız indirilenler | library_empty=Henüz seri yok. Sunucuda tarama yapıldığında burada görünür.
library_empty_downloaded=İndirilmiş kitap yok. | library_empty_search=Eşleşen seri yok.
library_offline=Çevrimdışısınız. Yalnız indirilenler açılabilir. | library_refresh_failed=Eşitlenemedi: %1$s
series_books_count=%1$d kitap
series_continue=Devam et | series_read=Oku | series_download_all=Tümünü indir | series_delete_all=Tüm indirmeleri sil
series_mark_all_read=Tümünü okundu işaretle | series_mark_all_unread=Tümünü okunmadı işaretle
book_download=İndir | book_delete_download=İndirmeyi sil | book_mark_read=Okundu işaretle | book_mark_unread=Okunmadı işaretle
book_status_unread=Okunmadı | book_status_reading=Okunuyor · %%%1$d | book_status_finished=Okundu | book_status_dropped=Bırakıldı
book_meta_comic=%1$d sayfa | book_meta_text=%1$d bölüm
download_queued=Kuyrukta | download_running=İndiriliyor %1$d/%2$d | download_failed=İndirilemedi. Yeniden denemek için dokunun. | download_done=İndirildi
book_unavailable=Bu kitap açılamıyor: %1$s | book_needs_network=Çevrimdışısınız ve bu kitap indirilmemiş.
reader_loading=Yükleniyor… | reader_error=Bölüm yüklenemedi. İndirilmemiş ve bağlantı yok olabilir.
reader_bookmark_add=Yer imi ekle | reader_bookmark_remove=Yer imini kaldır | reader_bookmarks=Yer imleri | reader_bookmarks_empty=Yer imi yok.
reader_toc=İçindekiler | reader_search=Kitapta ara | reader_search_hint=En az 2 karakter | reader_search_offline=Arama için bağlantı gerekir.
reader_search_none=Sonuç yok. | reader_search_truncated=Sonuçlar kısaltıldı. | reader_settings=Okuma ayarları
reader_next_volume=Sonraki cilt: %1$s | reader_open=Aç | reader_page_of=%1$d / %2$d | reader_chapter_percent=Bölüm %1$d · %%%2$d
reader_font=Yazı tipi | reader_font_size=Yazı boyutu | reader_line_height=Satır aralığı | reader_margin=Kenar boşluğu
reader_align=Hizalama | reader_align_left=Sola | reader_align_justify=İki yana | reader_mode=Okuma biçimi
reader_mode_page=Sayfa | reader_mode_scroll=Kaydırma | reader_theme=Okuyucu teması
reader_theme_auto=Otomatik | reader_theme_light=Açık | reader_theme_dark=Koyu | reader_theme_sepia=Sepya | reader_theme_black=Saf siyah
comic_dir_ltr=Soldan sağa | comic_dir_rtl=Sağdan sola | comic_dir_webtoon=Dikey (webtoon) | comic_direction=Okuma yönü
settings_title=Ayarlar | settings_section_appearance=Görünüm | settings_theme=Tema
theme_system=Sistem | theme_light=Açık | theme_dark=Koyu
settings_amoled=Saf siyah (AMOLED) | settings_amoled_desc=Koyu modda tüm yüzeyler #000000 olur.
settings_dynamic=Dinamik renk | settings_dynamic_desc=Duvar kağıdından renk al (Android 12+).
settings_section_reader=Okuyucu | settings_immersive=Tam ekran | settings_keep_awake=Ekran açık kalsın
settings_tap_zones=Dokunma bölgeleri | settings_tap_zones_desc=Kenarlara dokunarak sayfa çevir.
settings_volume_keys=Ses tuşlarıyla sayfa çevir
settings_section_downloads=İndirmeler | settings_wifi_only=Yalnız Wi-Fi'da indir | settings_storage=Depolama
storage_downloads=İndirilenler: %1$s | storage_cache=Önbellek: %1$s | storage_covers=Kapaklar: %1$s
settings_clear_cache=Önbelleği temizle | settings_cache_cleared=Önbellek temizlendi.
settings_section_sync=Eşitleme (ReadSync) | settings_sync_now=Şimdi eşitle | sync_last_ok=Son eşitleme: %1$s
sync_never=Henüz eşitlenmedi | sync_pending=Bekleyen değişiklik: %1$d | sync_error=Son hata: %1$s | sync_done=Eşitlendi.
settings_section_account=Hesap | settings_server=Sunucu: %1$s | settings_user=Kullanıcı: %1$s | settings_logout=Çıkış yap
logout_confirm_title=Çıkış yapılsın mı? | logout_confirm_body=Bu cihazdaki indirmeler ve önbellek silinir.
logout_confirm_pending=%1$d eşitlenmemiş değişiklik kaybolacak.
notif_channel_downloads=İndirmeler | notif_downloading=Kitaplar indiriliyor…
```
(`|` yalnız bu listede ayraçtır; her girdi ayrı `<string name="...">` olur.) Ekranlar yalnız bu anahtarları kullanır; yeni metin gerekirse listeye eklenir.

## 7. CI — `.github/workflows/android.yml` (G11)

```yaml
name: android

on:
  push:
    paths:
      - "crates/**"
      - "android/**"
      - "web/fonts/**"
      - "web/frame.css"
      - "web/reader-fonts.css"
      - "web/favicon.svg"
      - "Cargo.toml"
      - "Cargo.lock"
      - "rust-toolchain.toml"
      - ".github/workflows/android.yml"
  pull_request:
  workflow_dispatch:

permissions:
  contents: read

jobs:
  core:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: 1.89.0
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt -p readcrab_core            # kod yerelde biçimlenemediği için CI biçimler (yalnız bu iş içinde)
      - run: cargo clippy --locked -p readcrab_core --all-targets   # uyarılar başarısızlık sayılmaz
      - run: cargo test --locked -p readcrab_core

  apk:
    needs: core
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: 1.89.0
          targets: aarch64-linux-android,armv7-linux-androideabi,x86_64-linux-android
      - uses: Swatinem/rust-cache@v2
      - uses: actions/setup-java@v4
        with:
          distribution: temurin
          java-version: "17"
      - name: Android SDK/NDK
        run: |
          yes | "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --licenses > /dev/null || true
          "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" "platforms;android-35" "build-tools;35.0.0" "ndk;27.2.12479018"
          echo "ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.2.12479018" >> "$GITHUB_ENV"
      - run: cargo install cargo-ndk --version 4.1.2 --locked
      - name: Rust çekirdeği (3 ABI)
        run: |
          cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o android/app/src/main/jniLibs \
            build --locked --release -p readcrab_core
      - name: UniFFI Kotlin binding
        run: |
          cargo run --locked -p readcrab_core --features cli --bin uniffi-bindgen -- generate \
            --library android/app/src/main/jniLibs/arm64-v8a/libreadcrab_core.so \
            --language kotlin --no-format \
            --out-dir android/app/build/generated/uniffi/kotlin
      - uses: gradle/actions/setup-gradle@v4
        with:
          gradle-version: "8.13"
      - name: Debug APK + birim testleri
        working-directory: android
        run: gradle --no-daemon assembleDebug testDebugUnitTest
      - uses: actions/upload-artifact@v4
        with:
          name: readcrab-debug-apk
          path: android/app/build/outputs/apk/debug/app-debug.apk
          if-no-files-found: error
```
`cargo ndk ... -o` hedef dizinde `<abi>/libreadcrab_core.so` üretir. `cargo test` yerel (host) hedefte çalışır; cdylib+lib çıktısı olduğundan `crate-type` ikisini de içerir.

## 8. Görev listesi

İşçiler HİÇBİR git komutu çalıştırmaz (add/commit/push/pull dahil); yalnız dosya yazar. Commit ve push CEO'dadır (bölüm 8.1).

Sıra bağımlılık sırasıdır. Aynı numaralı satırlar tek görevdir. "Paralel" = ayrık dosyalar. İki işçi AYNI dosyaya dokunmaz; ortak dosya yoktur (lib.rs yalnız G1→G5 sıralı).

| # | İşçi | Görev | Dokunulan dosyalar | Bağımlılık |
|---|---|---|---|---|
| G1 | İŞÇİ-2 | Workspace dönüşümü (bölüm 2.1-2.5, 2.7) | `Cargo.toml`, `Dockerfile`, `.dockerignore`, `.gitignore`, `.github/workflows/ci.yml`, `crates/core/Cargo.toml`, `crates/core/uniffi.toml`, `crates/core/uniffi-bindgen.rs`, `crates/core/src/lib.rs` (yer tutucu) | — |
| G2 | İŞÇİ-2 | `lock.yml` (bölüm 2.6, `push` tetikli) | `.github/workflows/lock.yml` | G1 |
| G3 | İŞÇİ-1 | Sunucu S1 + testleri (bölüm 3) | `src/api/tracking.rs`, `tests/tracking.rs` | — (G1 ile paralel) |
| G4 | İŞÇİ-1 | Çekirdek temel: hata, yardımcılar, şema, db | `crates/core/src/error.rs`, `util.rs`, `db.rs`, `migrations/0001_init.sql` (+ birim testleri `util.rs` içinde: `fold_tr`, `extract_body`, `asset_refs`, `rewrite_assets`, `par_map` sırası) | G1 |
| G5 | İŞÇİ-1 | HTTP istemcisi + `lib.rs` (türler, `Core`, `new`, `session`, `login`, `logout`) | `crates/core/src/http.rs` (birim testleri: `parse_session_cookie`, URL normalize, `into_core` eşleme, `Date` ofseti), `crates/core/src/lib.rs` | G4 |
| G6 | İŞÇİ-1 | Kütüphane + ReadSync | `crates/core/src/library.rs`, `crates/core/src/sync.rs` (birim testleri: `merge_progress`, `merge_bookmarks`, yerel durum türetme) | G5 |
| G7 | İŞÇİ-1 | İndirme makinesi + okuyucu uçları | `crates/core/src/download.rs`, `crates/core/src/reader.rs` | G6 |
| G9 | İŞÇİ-2 | Sahte HTTP sunucusu (bölüm 9.2) | `crates/core/tests/common/mod.rs` | G5 ile paralel |
| G10 | İŞÇİ-1 | Entegrasyon testleri (bölüm 9.3) | `crates/core/tests/auth.rs`, `sync.rs`, `download.rs` | G7, G9 |
| G11 | İŞÇİ-1 | Android CI | `.github/workflows/android.yml` | G1 |
| G12 | İŞÇİ-2 | Gradle iskeleti, manifest, kaynaklar, simge, strings | `android/settings.gradle.kts`, `android/build.gradle.kts`, `android/gradle.properties`, `android/app/build.gradle.kts`, `android/app/src/main/AndroidManifest.xml`, `res/values/{strings,themes,ic_launcher_background}.xml`, `res/drawable/ic_launcher_foreground.xml`, `res/mipmap-anydpi-v26/ic_launcher{,_round}.xml` | — (G1 ile paralel) |
| G13 | İŞÇİ-2 | Tema, tercihler, bağlantı durumu, ortak bileşenler, tema testi | `KÖK/ui/theme/Theme.kt`, `KÖK/Prefs.kt`, `KÖK/Connectivity.kt`, `KÖK/ui/common/Components.kt`, `src/test/kotlin/.../ThemeTest.kt` | G12 |
| G14 | İŞÇİ-1 | Uygulama sınıfı, Coil fetcher, işçiler, indirme izleyici | `KÖK/RcApp.kt`, `KÖK/image/CoreImage.kt`, `KÖK/work/Workers.kt`, `KÖK/work/DownloadTracker.kt` | G13 (Prefs) |
| G15 | İŞÇİ-2 | Etkinlik, gezinme, giriş, ayarlar | `KÖK/MainActivity.kt`, `KÖK/ui/nav/AppNav.kt`, `KÖK/ui/login/LoginScreen.kt`, `KÖK/ui/settings/SettingsScreen.kt` | G13, G14 |
| G16 | İŞÇİ-2 | Kütüphane + seri ekranları | `KÖK/ui/library/LibraryScreen.kt`, `KÖK/ui/series/SeriesScreen.kt` | G13, G14 |
| G17 | İŞÇİ-1 | Okuyucu ortak + çizgi roman okuyucu | `KÖK/ui/reader/ReaderCommon.kt`, `KÖK/ui/reader/comic/ComicReaderScreen.kt`, `KÖK/ui/reader/comic/ZoomableBox.kt` | G14, G15 (`VolumeKeys`) |
| G18 | İŞÇİ-1 | Metin okuyucu (WebView, JS köprüsü) | `android/app/src/main/assets/reader.html`, `reader.js`, `KÖK/ui/reader/text/ReaderWebView.kt`, `KÖK/ui/reader/text/TextReaderScreen.kt` | G17 (`ReaderCommon`) |
| G19 | CEO | K1'i `android` dalına it, `lock.yml` koşusunun `Cargo.lock`'u commit etmesini bekle (`gh run watch`), `git pull`, kalan commit'leri it; elle doğrulama (bölüm 9.6). Tüm commit/push işlemlerini yalnız CEO yapar | — | G1-G18 |

Paralel planı: [G1, G3, G12] eşzamanlı → [G2, G4, G13 (G12 sonrası), G9] → G5 → G6 → G7; Kotlin tarafı: G13 → G14 → [G15 (İŞÇİ-2) ∥ G17 (İŞÇİ-1)] → [G16 (İŞÇİ-2) ∥ G18 (İŞÇİ-1)]. Sayım: İŞÇİ-1 = 10 görev (G3, G4, G5, G6, G7, G10, G11, G14, G17, G18); İŞÇİ-2 = 7 görev (G1, G2, G9, G12, G13, G15, G16); CEO = 1 (G19). Toplam 18 görev (G8 numarası kullanılmaz).

### 8.1 Commit grupları (commit'leri YALNIZ CEO atar; işçiler hiçbir git komutu çalıştırmaz. Türkçe mesaj; her mesaj `Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>` satırıyla biter)
| Grup | Görevler | Mesaj |
|---|---|---|
| K1 | G1, G2 | `Cargo çalışma alanı: crates/core iskeleti, Dockerfile/CI yolları ve lock iş akışı` (ilk itiş; bölüm 2.6 sırası) |
| K2 | G3 | `Sunucu: ilerleme uçlarına client_ts ve updated_at eklendi (ReadSync çakışma kuralı)` |
| K3 | G4, G5 | `Çekirdek: SQLite şeması, HTTP istemcisi ve oturum` |
| K4 | G6 | `Çekirdek: kütüphane önbelleği, ReadSync kuyruğu ve birleştirme` |
| K5 | G7 | `Çekirdek: indirme yöneticisi ve okuyucu uçları` |
| K6 | G9, G10 | `Çekirdek: sahte sunuculu entegrasyon testleri` |
| K7 | G11 | `CI: Android derleme iş akışı (cargo-ndk, UniFFI, debug APK)` |
| K8 | G12, G13 | `Android: Gradle iskeleti, kaynaklar, simge, tema ve tercihler` |
| K9 | G14, G15, G16 | `Android: uygulama kabuğu, giriş, kütüphane, seri ve ayarlar ekranları` |
| K10 | G17 | `Android: çizgi roman okuyucu` |
| K11 | G18 | `Android: metin okuyucu (WebView, yer imi, arama)` |
| K12 | — | (otomatik, `lock.yml` botu; K1'den hemen sonra gelir) `Cargo.lock: crates/core bağımlılıkları eklendi` |

## 9. Testler

### 9.1 Çekirdek birim testleri (CI `core` işinde `cargo test -p readcrab_core`)
- `util`: `fold_tr("İSTANBUL ıı") == "istanbul ii"`; `extract_body` doğru aralığı keser, `<body>` yoksa hata; `asset_refs` yinelenenleri tekilleştirir; `rewrite_assets` yalnız ilgili kitabın `src` değerlerini değiştirir; `par_map` sırayı korur.
- `http`: `parse_session_cookie` (çoklu başlık, boş değer, `Secure` ekli); `Date` başlığından ofset; URL normalize (`192.168.1.2:3435` → `http://192.168.1.2:3435`, sondaki `/`, boş → `InvalidInput`); `into_core` eşlemeleri (`Net("invalid peer certificate: UnknownIssuer")` → `Tls`, `Net("Connection refused")` → `Network`).
- `sync::merge_progress` tablo testi: kayıt yok→KeepLocal; yerel yok→TakeServer; pending+sunucu daha yeni→TakeServer; pending+eşit→KeepLocal; pending+yerel daha yeni→KeepLocal; pending değil→TakeServer.
- `sync::merge_bookmarks`: Insert; Update; Adopt (aynı locator+label, ikinci sunucu kaydına tekrar verilmez); sunucuda olmayan `server_id`'li → Remove; `deleted=1` yerel satıra sunucu kaydı → işlem yok; `server_id IS NULL` satıra dokunulmaz.
- `sync`: `save_progress` yerel durum türetmesi (0.5→reading, 1.0→finished, finished iken 0.3→finished kalır); `set_status` bayrakları; `last_op` sırası (iki bayrak kirliyken `push` isteklerinin sırası mock testinde doğrulanır).
- `download` durum makinesi (sahte `Http` gerekmez, yalnız db): `enqueue` (hata kitabı atlanır, done atlanır), `Core::new` `running→queued`, `download_remove` satır+dosyaları siler, `clear_cache` yalnız satırı olmayan kitapları siler.

### 9.2 `tests/common/mod.rs` — sahte sunucu sözleşmesi (G9)
```rust
pub struct Req { pub method: String, pub path: String /* sorgu dahil */, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
pub struct Reply { pub status: u16, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
pub fn json(status: u16, v: &serde_json::Value) -> Reply;           // Content-Type: application/json
pub fn bytes(status: u16, content_type: &str, body: Vec<u8>) -> Reply;
pub struct Mock { pub base: String, pub log: std::sync::Arc<std::sync::Mutex<Vec<Req>>> }
impl Mock {
    pub fn start(handler: impl Fn(&Req) -> Reply + Send + Sync + 'static) -> Mock;      // 127.0.0.1:0
    pub fn start_on(port: u16, handler: impl Fn(&Req) -> Reply + Send + Sync + 'static) -> Mock;
    pub fn port(&self) -> u16;
    pub fn stop(self);                      // dinleyiciyi kapatır (çevrimdışı simülasyonu)
    pub fn requests(&self) -> Vec<Req>;     // log kopyası
}
```
Uygulama: `tiny_http::Server::http(addr)`, `Arc<Server>` + işçi iş parçacığı `recv_timeout(100ms)` döngüsü ve `AtomicBool` durdurma; her istek `log`'a eklenir; her yanıta `Date` başlığı eklenir (`httpdate::fmt_http_date(SystemTime::now())` — tests için `httpdate` dev-dependency DEĞİL, normal bağımlılık olduğundan kullanılabilir). Test yardımcıları (birebir):
```rust
pub const TOKEN: &str = "testtoken123";
pub fn core_in(dir: &tempfile::TempDir) -> std::sync::Arc<readcrab_core::Core> {
    readcrab_core::Core::new(dir.path().to_string_lossy().into_owned()).unwrap()
}
// Sunucunun gerçek POST /api/login 200 yanıtı: gövde UserRes {id, username, is_admin}, Set-Cookie build_session_cookie biçimi
pub fn login_ok() -> Reply {
    let mut r = json(200, &serde_json::json!({"id": 1, "username": "admin", "is_admin": true}));
    r.headers.push((
        "Set-Cookie".to_string(),
        format!("krs_session={TOKEN}; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000"),
    ));
    r
}
pub fn login_fail() -> Reply {   // sunucunun gerçek 401 gövdesi
    json(401, &serde_json::json!({"error": "unauthorized", "message": "Oturum açmanız gerekiyor."}))
}
pub fn is_login(req: &Req) -> bool { req.method == "POST" && req.path == "/api/login" }
pub fn has_cookie(req: &Req) -> bool {
    req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("cookie") && v == &format!("krs_session={TOKEN}"))
}
```
Dönüş tipleri: `login_ok() -> Reply`, `login_fail() -> Reply`, `is_login(&Req) -> bool`, `has_cookie(&Req) -> bool`, `core_in(&TempDir) -> Arc<Core>`. Test işleyicileri `if is_login(req) { return login_ok(); }` ile başlar; diğer uçlarda `has_cookie(req)` yoksa `login_fail()` döner. Giriş testleri `core.login(mock.base.clone(), "admin".into(), "password1234".into())` çağırır; beklenen `session()`: `server_url == mock.base`, `username == "admin"`, `user_id == 1`, `is_admin == true`.

### 9.3 Entegrasyon testleri (G10)
`tests/auth.rs`: (1) `login` başarılı → sonraki istekte `Cookie: krs_session=tok` var; `POST /api/login` isteğinde `Content-Type: application/json` var ve `Origin` YOK; `session()` döner. (2) yanlış parola 401 → `Unauthorized`. (3) `logout` isteği gövde `{}` ve CT json ile gider, ardından `session() == None` ve `list_series` boş. (4) farklı sunucu/kullanıcıyla `login` yerel veriyi siler. (5) 401 gelen `sync_now` → `Unauthorized`, yerel `list_series` verisi duruyor.
`tests/sync.rs`: (1) ilk `sync_now`: sahte `/api/series` (toplam 2, `limit=200`), `/api/series/{id}`, `/api/books/{id}/progress` → `list_series`, `get_series`, `continue_reading` beklenen; `progress` yalnız `seen_sig` farklı kitaplar için çağrıldı (log sayımı). (2) ikinci `sync_now` imza aynıyken detay/ilerleme çağrısı YOK. (3) çevrimdışı kuyruk: sunucu durmuşken `save_progress`, `set_status`, `add_bookmark`, `ping_*` → `sync_status().pending_ops` doğru; `sync_now` → `Err(Network)`; sunucu `start_on(aynı port)` ile döner → `sync_now` → `PUT /progress` gövdesi `client_ts` ve `local_date` içerir, `pending_ops == 0`. (4) `applied:false` yanıtı → ardından `GET /progress` çağrılır ve yerel değer sunucununkine döner. (5) sunucu daha yeni `updated_at` ile pull → yerel bekleyen işlem iptal; yerel daha yeni → korunur ve itilir. (6) yer imi: çevrimdışı ekle → `POST` sonra `server_id` atanır; `delete_bookmark` → `DELETE /api/bookmarks/{id}` (CT json); sunucu listesinde olmayan kayıt yerelden silinir. (7) ping: aynı `client_id` ile iki `ping_update` → tek satır, `POST` gövdesinde son `active_seconds`; `ping_close` + gönderim sonrası satır silinir. (8) sıra: `last_op='s'` iken önce progress sonra status istekleri.
`tests/download.rs`: (1) çizgi roman: `download_enqueue` → `run_downloads` → `download_infos` `Done`, `parts_done == parts_total`, `page_path` sunucu `stop()` sonrası da çalışır. (2) bir sayfa 404 → `Failed` + `error`; sayfa düzelince `download_enqueue` → yalnız eksik sayfa istenir (log). (3) metin: bölüm HTML'inde `<img src="/api/books/7/asset?p=OEBPS%2Fimg%2Fa%2Epng">` → `books/7/c0.html` içinde `src="/book/7/a/<16hex>.png"` ve dosya var; 404 görsel → `src=""`. (4) `cover_path` ikinci çağrıda ağa gitmez; sürüm değişince eski dosya silinir. (5) `clear_cache` yalnız önbelleği siler (indirilen kitap kalır); `storage_usage` değerleri tutarlı. (6) `download_stop` çalışan `run_downloads`'ı durdurur ve satırlar `Queued` olur. (7) `run_downloads` 401 → `Err(Unauthorized)`, kitap `Queued`.

### 9.4 Sunucu testleri
G3 testleri (bölüm 3.2) + MEVCUT tüm sunucu testleri değişmeden geçmeli (`cargo test --locked` kökte; web/JS testleri ve e2e/Docker işleri aynen).

### 9.5 CI'de koşanlar
`ci.yml`: fmt (`-p readcrab`), clippy -D warnings (sunucu), JS denetimleri, sunucu testleri, Docker imajı + smoke, e2e. `android.yml`: `cargo fmt`+`clippy`+`cargo test -p readcrab_core`; APK işi: cargo-ndk (3 ABI) + UniFFI binding + `gradle assembleDebug testDebugUnitTest` (ThemeTest) + `readcrab-debug-apk` artifact'ı.

### 9.6 Elle doğrulama listesi (cihaz/emülatör; debug APK)
1. Yanlış parola hata gösterir; doğru parola kütüphaneyi açar; uygulama yeniden açılınca giriş istemez.
2. Seri ızgarası kapaklı gelir; "Okumaya devam" rafı yalnız okunan kitapları gösterir; arama ada ve kitap başlığına göre süzer; "yalnız indirilenler" doğru süzer.
3. Seri sayfasında bir çizgi roman ve bir EPUB indir: ilerleme halkası/`n/m` artar; bitince onay simgesi. Uçak modu aç: kütüphane açılır, indirilen kitaplar tamamen açılır (sayfa/bölüm, görseller dahil), indirilmeyenler soluk ve açılmaz.
4. "İndirmeyi sil" dosyaları siler (Ayarlar > Depolama düşer). Önbelleği temizle indirilenlere dokunmaz.
5. Çizgi roman: soldan-sağa, sağdan-sola (kaydırma ve dokunma bölgeleri ters), webtoon; pinch ve çift dokunma yakınlaştırır, yakınlaştırılmış halde sayfa kaymaz; kaydırıcı sayfaya atlar; son sayfada sonraki cilt kartı.
6. Metin: sayfa ve kaydırma modu; 7 yazı tipi, boyut, satır aralığı, kenar boşluğu, hizalama, 4 okuyucu teması anında uygulanır; içindekiler bölüme atlar; yer imi ekle/kaldır/atla; kitap içi arama (çevrimdışıyken uyarı); ilerleme yüzdesi; döndürmede konum korunur; son bölüm sonunda sonraki cilt kartı.
7. Tam ekran, ekran açık kalsın, dokunma bölgeleri kapat/aç, ses tuşları (açıkken sayfa çevirir ve ses penceresi çıkmaz, kapalıyken normal ses).
8. Tema: Sistem/Açık/Koyu; AMOLED açıkken koyu modda TÜM yüzeyler #000000 (kütüphane, seri, ayarlar, alt sayfalar); Android 12+ dinamik renk anahtarı çalışır, AMOLED üstüne uygulanır.
9. ReadSync: telefonda oku → web arayüzünde (`/#/seri/..`) aynı konum; webde ilerle → telefonda "Şimdi eşitle" sonrası konum gelir; uçak modunda oku → bağlantı gelince web'de görünür; iki cihazda çevrimdışı farklı konum → saati yeni olan kazanır; yer imi ekle/sil iki yönlü; okuma süresi sunucu geçmişinde görünür.
10. Okundu/okunmadı işareti sunucuya gider; "Tümünü okundu" tüm kitapları değiştirir.
11. Yalnız Wi-Fi'da indir açıkken mobil veride indirme beklemede kalır, Wi-Fi'da başlar; bildirim "Kitaplar indiriliyor…" görünür.
12. Oturum süresi dolunca (sunucuda oturum silinince) giriş ekranı "Oturumunuzun süresi doldu" ile gelir ve bekleyen değişiklikler giriş sonrası gönderilir.
13. Çıkış: onay sorar, bekleyen değişiklik sayısı yazar; sonra yerel veri silinir.

## 10. Ponytail notları (bilinçli kısayollar)
- Webtoon'da yakınlaştırma yok; sayfa kaydırıcısı mantıksal sayfaya göre.
- Kütüphane yenileme imza farkıyla seçici; çok büyük (binlerce seri) kütüphanede ilk eşitleme uzun sürer (4 paralel istek).
- Bölüm arama/çekme yalnız bölüm başına tam HTML; MOBI görselleri sunucudaki `asset?p=<n>` yoluyla aynı şekilde işlenir (yeniden yazma kuralı aynı).
- Yer imi etiketi/konumu düzenlenemez (sunucu da düzenlemeye izin vermez).
- Dış bağlantılar okuyucuda açılmaz.
- Okuyucu yazı tipi adları (Literata, Merriweather, ...) marka adıdır; yerelleştirilmez, `TEXT_FONTS` sabitinde (Prefs.kt) durur, `strings.xml`'de yoktur.
- TLS: yalnız Mozilla kökleri (webpki-roots); özel CA/kendi imzalı sertifika yok. Bu durumda `CoreError::Tls` ve `login_error_tls` metni gösterilir; destek gerekirse `rustls-platform-verifier` + JNI başlatma sonraki sürümde.
- `ureq` bloklayan: iptal, hazır isteğin bitmesini bekler (en çok 30 sn).

## CEO'YA SORULAR: yok
