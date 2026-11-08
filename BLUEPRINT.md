# BLUEPRINT: kavita-rs

Bu belge kavita-rs için tek referanstır. Planlayıcı dosya yazamadığı için metin son yanıt olarak döndü. CEO bunu `/home/mustafa/kavita-rs/BLUEPRINT.md` olarak kaydeder.

Üstünde çalışılan sürüm sabitleri 2026-11-08 tarihinde crates.io API ile (HTTP 200) doğrulandı. `mobi 0.8.0` API'si GitHub kaynağından okundu.

---

## 0. Kapalı kararlar (özet)

| Konu | Karar |
|---|---|
| Dil/çatı | Rust 1.89.0, edition 2021, `resolver = "3"`, `rust-version = "1.89"`. MSRV'ye duyarlı çözümleyici, 1.89 ile derlenemeyecek yeni transitif sürümleri seçmez. |
| Web | axum 0.8.4 (yol parametresi sözdizimi `/{id}`), tokio. |
| DB | rusqlite (bundled SQLite), tek bağlantı `Arc<std::sync::Mutex<Connection>>`, WAL, `foreign_keys=ON`, `busy_timeout=5000`. Havuz yok. Tüm DB işi `spawn_blocking` içinde yapılır. |
| Migrasyon | `PRAGMA user_version` + `src/migrations/NNNN_*.sql` dosyaları (`include_str!`). Harici migrasyon crate'i yok. |
| Frontend | Elle yazılmış HTML/CSS/vanilla JS, `include_str!` ile binary'ye gömülü, statik `match` tablosu. Kütüphane yok, CDN yok. |
| Router | Hash tabanlı SPA (`#/...`). |
| Grafikler | `document.createElementNS` ile kütüphanesiz SVG. |
| Zaman dilimi | Sunucu saat dilimi bilmez. İstemci her istekte kendi yerel tarihini (`local_date`, YYYY-MM-DD) gönderir. Gün sınırı = oturumun BAŞLADIĞI yerel gün. Gece yarısını aşan oturum başladığı güne yazılır. |
| Kalıcı kimlik | `books.id` sabittir, satır asla silinmez. Dosya kaybolunca `missing=1` olur. Aynı içerik özetine (`content_id`) sahip yeni dosya bulunursa eski satır canlandırılır. |
| Oturum çerezi | `krs_session`, HttpOnly, SameSite=Strict, Path=/, Max-Age 30 gün. `Secure` bayrağı YALNIZCA `COOKIE_SECURE=1` ortam değişkeniyle eklenir. Varsayılan eklenmez (LAN'da HTTP çalışır). |
| CSRF | SameSite=Strict + durum değiştiren metotlarda `Origin` başlığı VARSA `Host` ile eşleşmek zorunda (aksi 403 `csrf`) + `Content-Type: application/json` zorunlu. sendBeacon, Blob tipi `application/json` ile gönderir. |
| Giriş sınırlama | Bellek içi. Kullanıcı adı başına 5 başarısız deneme / 15 dk, IP başına 20 başarısız / 15 dk. Aşınca 15 dk kilit, 429 + `Retry-After`. `X-Forwarded-For` güvenilmez. |
| Güvenli içerik | EPUB/MOBI bölümleri `ammonia` ile temizlenir, `<iframe sandbox="allow-same-origin">` içinde gösterilir (`allow-scripts` YOK). Çerçeve yanıtı CSP başlığı taşır (bkz. 6.3). |
| RAR/CBR | `bsdtar` alt süreç, glob kaçışlı, `--` ayırıcı, 30 sn zaman aşımı, 64 MiB çıktı sınırı. |
| MOBI | `mobi = "=0.8.0"` yeterlidir (başlık, metin, `image_records()`, `encryption()`). Bölümler `<mbp:pagebreak>` ile bölünür. |

---

## 1. Crate listesi (hepsi tam sürüm, `=` ile sabit)

`[dependencies]`:

```
axum            = { version = "=0.8.4", default-features = false, features = ["http1","json","query","tokio","macros"] }
tokio           = { version = "=1.47.1", features = ["rt-multi-thread","macros","process","time","net","io-util","signal","sync"] }
tower-http      = { version = "=0.6.6", default-features = false, features = ["catch-panic"] }
rusqlite        = { version = "=0.37.0", features = ["bundled"] }
argon2          = "=0.5.3"            # default features: password-hash + rand_core + std
rand            = "=0.8.5"
sha2            = "=0.10.9"
hex             = "=0.4.3"
base64          = "=0.22.1"
serde           = { version = "=1.0.219", features = ["derive"] }
serde_json      = "=1.0.143"
zip             = { version = "=2.4.2", default-features = false, features = ["deflate"] }
quick-xml       = "=0.37.5"
image           = { version = "=0.25.8", default-features = false, features = ["jpeg","png","gif","webp"] }
mobi            = "=0.8.0"
ammonia         = "=4.1.2"
percent-encoding= "=2.3.2"
tracing         = "=0.1.41"
tracing-subscriber = { version = "=0.3.20", features = ["env-filter"] }
```

`[dev-dependencies]`:

```
tower           = { version = "=0.5.2", features = ["util"] }
http-body-util  = "=0.1.3"
tempfile        = "=3.22.0"
tar             = "=0.4.44"
```

Kurallar:
- `Cargo.toml`'daki sürümler yalnız `=` biçiminde yazılır.
- Yukarıdaki sürümler crates.io'da var (HTTP 200 ile doğrulandı).
- Crate API'si derlemede uyuşmazsa işçi docs.rs'i okuyup KODU düzeltir. Sürümü yükseltmez.
- Sürüm yükseltmek CEO kararıdır.
- `axum` bağımlılığındaki `ConnectInfo` için `axum::serve(...into_make_service_with_connect_info::<SocketAddr>())` kullanılır.
- `tar` yalnız test fixture'ı içindir (bsdtar tar'ı da okur).

### 1.1 Cargo.lock stratejisi (kesin)

1. Depoya ilk commit'te `Cargo.lock` yoktur.
2. CI'nin ilk adımı `[ -f Cargo.lock ] || cargo generate-lockfile` çalıştırır. Ardından `Cargo.lock`'u `actions/upload-artifact@v4` ile `cargo-lock` adıyla yükler.
3. Sonraki tüm cargo komutları `--locked` ile çalışır.
4. Görev 4 (bkz. bölüm 11), ilk yeşil koşuda `gh run download --name cargo-lock` ile dosyayı indirir ve `Cargo.lock` olarak commit'ler.
5. Bundan sonra `Cargo.lock` depodadır. Dockerfile `COPY Cargo.lock` yapar. Docker işi `if: hashFiles('Cargo.lock') != ''` ile korunur.
6. Bağımlılık değişirse işçi ilgili CI'nin ürettiği lock'u aynı yöntemle commit'ler: dosyayı siler, push eder, artifact'tan geri alır.

---

## 2. Yapılandırma (ortam değişkenleri)

| Değişken | Varsayılan | Anlam |
|---|---|---|
| `PORT` | `3435` | Dinleme portu, `0.0.0.0` |
| `DATA_DIR` | `/data` | `kavita.db`, `covers/` |
| `BOOKS_DIR` | `/books` | Salt okunur kütüphane |
| `COOKIE_SECURE` | `0` | `1` ise çereze `Secure` eklenir |
| `RUST_LOG` | `info` | tracing filtresi |

`Config::from_env()` geçersiz `PORT` değerinde panik yapmaz. `process::exit(2)` ile Türkçe mesaj basar.

---

## 3. Dosya ağacı ve sorumluluklar

```
kavita-rs/
├─ BLUEPRINT.md                 bu belge
├─ LICENSE                      GPL-3.0 tam metni (gnu.org/licenses/gpl-3.0.txt)
├─ README.md                    Türkçe: çalıştırma, ilk kullanıcı, ortam değişkenleri
├─ Cargo.toml / Cargo.lock      Cargo.lock'u CI üretir (1.1)
├─ rust-toolchain.toml          channel = "1.89.0", components = rustfmt, clippy
├─ .gitignore  .dockerignore    target/, data/
├─ Dockerfile                   multi-stage (bölüm 10)
├─ docker-compose.yml           3435:3435, ./books:/books:ro, kavita-data:/data
├─ scripts/smoke.sh             imaj smoke testi (bölüm 10.3)
├─ .github/workflows/ci.yml
├─ web/
│  ├─ index.html                kabuk (script/stil yalnız harici dosya)
│  ├─ style.css                 uygulama teması, mobil+masaüstü, odak halkaları
│  ├─ frame.css                 EPUB/MOBI çerçevesi içeriği stili (tema/yazı boyutu CSS değişkenleriyle)
│  ├─ favicon.svg               kendi çizimimiz, Kavita logosu DEĞİL
│  ├─ app.js                    router, fetch sarmalayıcı (JSON hata → toast), login/setup/kütüphane/seri/admin sayfaları
│  ├─ tracker.js                okuma oturumu izleyici (boşta algılama, ping, sendBeacon), progress kaydı
│  ├─ reader-comic.js           görsel okuyucu
│  ├─ reader-text.js            metin okuyucu
│  └─ stats.js                  geçmiş sayfası, istatistik, hedef, SVG grafikler
├─ src/
│  ├─ main.rs                   CLI: `serve` (varsayılan) ve `healthcheck`
│  ├─ lib.rs                    pub mod'lar + `pub fn build_app(state)->Router` (testler kullanır)
│  ├─ config.rs                 Config
│  ├─ error.rs                  AppError, IntoResponse, JSON hata gövdesi
│  ├─ db.rs                     Db sarmalayıcı, open(), migrate(), `run()` yardımcısı
│  ├─ migrations/0001_init.sql
│  ├─ auth.rs                   argon2, oturum, auth middleware, origin kontrolü, giriş sınırlayıcı
│  ├─ app.rs                    AppState, router montajı, güvenlik başlıkları, statik sunum
│  ├─ scan.rs                   tarayıcı, ScanStatus, kimlik, canlandırma
│  ├─ safepath.rs               /books içinde güvenli yol çözümü
│  ├─ formats/
│  │  ├─ mod.rs                 Format enum, FormatError, sınırlar (sabitler)
│  │  ├─ natural.rs             doğal sıralama karşılaştırıcısı
│  │  ├─ comic.rs               CBZ/ZIP/RAR/CBR sayfa listesi + okuma (zip + bsdtar)
│  │  ├─ epub.rs                EPUB ayrıştırma
│  │  ├─ mobi.rs                MOBI bölümleri/görselleri
│  │  ├─ sanitize.rs            ammonia yapılandırması + yol yeniden yazımı + çerçeve belgesi
│  │  └─ cover.rs               kapak çıkarma + küçültme + önbellek
│  └─ api/
│     ├─ mod.rs                 router() birleştirme
│     ├─ auth.rs                setup, login, logout, me
│     ├─ admin.rs               kullanıcı yönetimi, tarama
│     ├─ library.rs             seriler, kitap detayı, kapak
│     ├─ reader.rs              sayfalar, toc, bölüm çerçevesi, asset
│     ├─ tracking.rs            progress, status, ping, history
│     └─ stats.rs               stats, goal
└─ tests/
   ├─ common/mod.rs             fixture üreticileri + test uygulaması kurulumu
   ├─ auth_flow.rs  library.rs  reader.rs  tracking.rs  security.rs  formats.rs
   └─ fixtures/                 (sample.rar CEO tarafından, bkz. sorular)
```

---

## 4. SQLite şeması (`0001_init.sql`, `user_version` = 1)

```sql
CREATE TABLE users(
  id INTEGER PRIMARY KEY,
  username TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash TEXT NOT NULL,
  is_admin INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL              -- unix saniye
);
CREATE TABLE sessions(
  token_hash TEXT PRIMARY KEY,             -- sha256(token) hex; düz token DB'de yok
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  last_seen INTEGER NOT NULL
);
CREATE INDEX idx_sessions_user ON sessions(user_id);

CREATE TABLE series(
  id INTEGER PRIMARY KEY,
  rel_path TEXT NOT NULL UNIQUE,           -- /books'a göre klasör; kökteki dosyalar için ''
  name TEXT NOT NULL,                      -- klasör adı; iç içe ise "A / B"; kök için 'Diğer'
  added_at INTEGER NOT NULL
);
CREATE TABLE books(
  id INTEGER PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES series(id),
  rel_path TEXT NOT NULL UNIQUE,           -- /books'a göre, '/' ayraçlı, UTF-8
  title TEXT NOT NULL,
  format TEXT NOT NULL CHECK(format IN('epub','mobi','cbz','zip','rar','cbr')),
  size INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  content_id TEXT NOT NULL,                -- bkz. 7.2
  page_count INTEGER NOT NULL DEFAULT 0,   -- görsel: sayfa; metin: bölüm sayısı
  has_cover INTEGER NOT NULL DEFAULT 0,
  sort_index INTEGER NOT NULL DEFAULT 0,   -- seri içi doğal sıra
  error TEXT,                              -- 'corrupt' | 'drm' | NULL
  missing INTEGER NOT NULL DEFAULT 0,
  added_at INTEGER NOT NULL
);
CREATE INDEX idx_books_series ON books(series_id, missing, sort_index);
CREATE INDEX idx_books_content ON books(content_id);

CREATE TABLE progress(
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  book_id INTEGER NOT NULL REFERENCES books(id),   -- CASCADE YOK: kitap satırı silinmez
  locator TEXT NOT NULL DEFAULT '',        -- görsel: "12" (0 tabanlı sayfa); metin: {"ch":3,"p":0.42} JSON metni
  percent REAL NOT NULL DEFAULT 0,         -- 0..1
  status TEXT NOT NULL DEFAULT 'unread' CHECK(status IN('unread','reading','finished','dropped')),
  finished_date TEXT,                      -- YYYY-MM-DD (istemci yerel günü) veya NULL
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(user_id, book_id)
);
CREATE TABLE reading_sessions(
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  book_id INTEGER NOT NULL REFERENCES books(id),
  client_id TEXT NOT NULL,                 -- 32 hex, istemci üretir
  started_at INTEGER NOT NULL,
  ended_at INTEGER NOT NULL,               -- son ping zamanı
  active_seconds INTEGER NOT NULL,
  pages_read INTEGER NOT NULL DEFAULT 0,
  start_percent REAL NOT NULL,
  end_percent REAL NOT NULL,
  local_date TEXT NOT NULL,                -- başladığı yerel gün
  UNIQUE(user_id, client_id)
);
CREATE INDEX idx_rs_user_date ON reading_sessions(user_id, local_date);
CREATE TABLE goals(
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  year INTEGER NOT NULL,
  target INTEGER NOT NULL CHECK(target BETWEEN 1 AND 1000),
  PRIMARY KEY(user_id, year)
);
```

Notlar:
- Kullanıcı silinince oturum, ilerleme, okuma oturumu ve hedefleri CASCADE ile gider.
- `books`/`series` satırları hiçbir zaman DELETE edilmez.
- Tüm SQL `rusqlite::params!` ile parametrelidir. `LIKE` araması `ESCAPE '\'` ile kaçışlanır (`%`, `_`, `\` kaçışlanır).
- `DB::open` migrasyonu tek transaction içinde uygular, sonra `user_version`'ı yükseltir.

---

## 5. HTTP uç noktaları

### 5.1 Genel kurallar
- Gövde ve yanıt JSON'dur (aksi belirtilmedikçe).
- Hata gövdesi: `{"error":"<kod>","message":"<Türkçe metin>"}`.
- JSON gövde sınırı 64 KiB (`DefaultBodyLimit`). Aşım 413 `payload_too_large`.
- Durum değiştiren metotlar (POST/PUT/DELETE) `Content-Type: application/json` ister, aksi 415 `unsupported_media`. Gövdesiz POST'lar (logout, scan) bu kuralı uygular fakat gövde istemezler: istemci yine de `{}` ile `application/json` gönderir.
- `Origin` varsa ve `Host` ile eşleşmiyorsa 403 `csrf`.
- Panik → `CatchPanicLayer` → 500 `internal` (ayrıntı sızdırmaz, log'a yazılır).
- Kimlik doğrulama: `krs_session` çerezi → `sessions` araması → `expires_at > now`. Her istekte `last_seen` güncellenir, süre `now+30gün`'e uzar (kayan, en çok dakikada bir yazılır).
- Hata kodları: 400 `bad_request`, 401 `unauthorized`, 403 `forbidden`/`csrf`, 404 `not_found`, 409 `conflict`, 413, 415, 422 `corrupt_book`/`drm_protected`, 429 `too_many_attempts`, 500 `internal`.

### 5.2 Herkese açık
| Metot Yol | İstek → Yanıt | Hatalar |
|---|---|---|
| GET `/health` | → 200 `ok` (text/plain). `SELECT 1` başarısızsa 503 | |
| GET `/` | → `web/index.html` | |
| GET `/static/{ad}` | `app.js`, `tracker.js`, `reader-comic.js`, `reader-text.js`, `stats.js`, `style.css`, `frame.css`, `favicon.svg` (bilinmeyen ad → 404). `Cache-Control: no-cache` | |
| GET `/api/setup-status` | → `{"needs_setup":bool}` | |
| POST `/api/setup` | `{username,password}` → 201 `{id,username,is_admin:true}` + çerez. Yalnız `users` boşken | 409 `conflict` (zaten kurulu), 400 `bad_request` (kural ihlali) |
| POST `/api/login` | `{username,password}` → 200 `{id,username,is_admin}` + çerez | 401 `unauthorized` (genel mesaj, kullanıcı var/yok ayrımı yok), 429 + `Retry-After` |

Doğrulama:
- Kullanıcı adı: 3-32 karakter, `[A-Za-z0-9_.-]`.
- Parola: 8-128 karakter.
- `/api/setup` işlemi tek SQL ile yarışa karşı korunur: `INSERT INTO users(...) SELECT ?,?,1,? WHERE NOT EXISTS(SELECT 1 FROM users)`; etkilenen satır 0 ise 409.

### 5.3 Kimlik gerektirir (herhangi bir kullanıcı)
| Metot Yol | İstek → Yanıt |
|---|---|
| POST `/api/logout` | → 204, oturum satırı silinir, çerez `Max-Age=0` |
| GET `/api/me` | → `{id,username,is_admin}` |
| GET `/api/scan-status` | → `{running,done,total,last_finished_at}` |
| GET `/api/series?q=&sort=name\|added\|recent&limit=60&offset=0` | → `{total,items:[{id,name,book_count,read_count,cover_book_id,last_read_at}]}`. `missing=1` kitaplar sayılmaz, hiç görünür kitabı olmayan seri listelenmez. `sort`: `name` (NOCASE doğal değil, SQL `ORDER BY name COLLATE NOCASE`), `added` (`added_at DESC`), `recent` (kullanıcının son okuduğu, `progress.updated_at DESC`, okumamışlar sona). `q`: seri adı veya kitap başlığında ESCAPE'li LIKE. `limit` 1-200, geçersiz → 400 |
| GET `/api/series/{id}` | → `{id,name,books:[{id,title,format,page_count,size,error,status,percent}]}`, `sort_index` sırası. 404 yoksa/hiç görünür kitap yoksa |
| GET `/api/books/{id}` | → `{id,series_id,series_name,title,format,page_count,error,status,percent,locator}`. `missing=1` ise 404 |
| GET `/api/books/{id}/cover` | → `image/jpeg`, `Cache-Control: private, max-age=86400`. Kapak yoksa 404 (istemci harf yer tutucu çizer). `missing=1` ise de önbellekteki kapak sunulur (geçmiş sayfası için) |
| GET `/api/books/{id}/pages` | görsel biçimler → `{count,names:null}` yerine yalnız `{count}`. Metin biçiminde 400 |
| GET `/api/books/{id}/pages/{n}` | görsel biçimler → sayfa baytları, `Content-Type` uzantıdan (jpeg/png/gif/webp), `X-Content-Type-Options: nosniff`, `Cache-Control: private, max-age=3600`. n sınır dışı 404 |
| GET `/api/books/{id}/toc` | metin biçimleri → `{chapters:[{index,title}]}` |
| GET `/api/books/{id}/chapters/{n}` | metin biçimleri → tam HTML belgesi (`text/html; charset=utf-8`), çerçeve CSP'li (6.3). n sınır dışı 404 |
| GET `/api/books/{id}/asset?p=<değer>` | metin kitabı içi görsel. EPUB'ta `p` = normalize zip yolu (yalnız manifest'te kayıtlı `image/*` ögeler), MOBI'de `p` = 1 tabanlı görsel indeksi. Yanıta `Content-Security-Policy: sandbox; default-src 'none'` ve `nosniff`. Manifest dışı/yok 404 |
| GET `/api/books/{id}/progress` | → `{locator,percent,status}` (kayıt yoksa `{locator:"",percent:0,status:"unread"}`) |
| PUT `/api/books/{id}/progress` | `{locator,percent,local_date}` → 200 `{status,percent}`. Kural 8.2 |
| PUT `/api/books/{id}/status` | `{status,local_date}` → 200 `{status}`. `status` 4 değerden biri; `finished` seçilirse `finished_date=local_date`, diğerlerinde NULL; kayıt yoksa oluşturulur |
| POST `/api/reading/ping` | bkz. 8.3 → 204 |
| GET `/api/history?limit=50&offset=0` | → `{total,items:[{id,book_id,book_title,series_name,book_missing,started_at,ended_at,active_seconds,pages_read,start_percent,end_percent,local_date}]}`, `started_at DESC`, yalnız çağıran kullanıcının satırları |
| GET `/api/stats?today=YYYY-MM-DD` | bkz. 8.4 |
| PUT `/api/goal` | `{year,target}` → 200 `{year,target}`. `year` 2000-2100, `target` 1-1000 |

`local_date` ve `today` formatı katı doğrulanır: `^\d{4}-\d{2}-\d{2}$` + geçerli takvim günü; aksi 400.

Kullanıcı yalnız kendi `progress`/`reading_sessions`/`goals` satırlarını görür, çünkü tüm sorgular `user_id = oturum kullanıcısı` ile filtrelenir. `user_id` istek gövdesinden/yolundan ASLA okunmaz.

### 5.4 Yalnız admin (`is_admin=0` ise 403 `forbidden`)
| Metot Yol | İstek → Yanıt |
|---|---|
| GET `/api/admin/users` | → `[{id,username,is_admin,created_at}]` |
| POST `/api/admin/users` | `{username,password}` → 201 `{id,username,is_admin:false}`. Kullanıcı adı çakışırsa 409 |
| DELETE `/api/admin/users/{id}` | → 204. Kendini silmeye çalışırsa 409 `conflict` (`cannot_delete_self` mesajı). Yoksa 404 |
| PUT `/api/admin/users/{id}/password` | `{password}` → 204; o kullanıcının tüm oturumları silinir |
| POST `/api/admin/scan` | → 202 `{started:true}`; zaten çalışıyorsa 409 |

Yeni oluşturulan kullanıcılar her zaman `is_admin=0` olur. Sistemde tek admin vardır (ilk kayıt).

---

## 6. Güvenlik tasarımı

### 6.1 Yol güvenliği (`safepath.rs`)
```rust
pub fn resolve_book_path(books_root: &Path, rel: &str) -> Result<PathBuf, AppError>;
```
- `rel` NUL içeremez, mutlak olamaz, `..` bileşeni içeremez.
- `root = books_root.canonicalize()`; `joined = root.join(rel)`.
- `symlink_metadata(joined)` bir symlink ise ret.
- `joined.canonicalize()` değerinin `root` ile başlaması şarttır, aksi ret.
- Ret → 404 `not_found` (varlık sızdırmaz).
- Tarayıcı (`scan.rs`) symlink'leri (dosya ve klasör) ATLAR, takip etmez.
- Kullanıcıdan gelen dosya yolu hiçbir uç noktada alınmaz. Sunucu yalnız DB'deki `rel_path`'i kullanır.

### 6.2 Arşiv üyeleri ve sınırlar (`formats/mod.rs` sabitleri)
```
MAX_ENTRY_BYTES   = 64 MiB     // tek üye açık boyutu
MAX_TOTAL_BYTES   = 2 GiB      // arşivdeki tüm üyelerin bildirilen açık toplamı (zip)
MAX_ENTRIES       = 20_000
MAX_XML_BYTES     = 5 MiB      // OPF/container/ncx/nav
MAX_CHAPTER_BYTES = 8 MiB      // bir bölümün HTML'i
MAX_PIXELS        = 64_000_000 // kapak çözümü (image::Limits max_alloc = 256 MiB)
```
- Zip üye adları `ZipFile::enclosed_name()` ile doğrulanır. `None` dönen üye atlanır.
- Üye okuma `Read::take(MAX_ENTRY_BYTES + 1)` ile yapılır. Fazlası → `FormatError::TooLarge`.
- Bildirilen toplam > `MAX_TOTAL_BYTES` veya üye sayısı > `MAX_ENTRIES` → `TooLarge` → kitap `error='corrupt'`.
- Şifreli zip üyesi (`encrypted()`) → `FormatError::Drm`.
- Sayfa adı süzgeci: yalnız `.jpg .jpeg .png .gif .webp` (küçük harfe çevrilerek), ad `__MACOSX/` ile başlamaz ve hiçbir bileşeni `.` ile başlamaz. Adlarda kontrol karakteri varsa atlanır.
- XML: `quick-xml::Reader` kullanılır. DOCTYPE/entity tanımları YOK sayılır (özel entity çözülmez), yalnız beş önceden tanımlı entity ve sayısal karakter referansları çözülür. `Reader::config_mut().check_end_names=false` KAPALIDIR (varsayılan). Girdi `MAX_XML_BYTES` ile sınırlı.

### 6.3 Metin içeriği izolasyonu (kesin yöntem)
Katmanlar (hepsi birlikte uygulanır):
1. `ammonia` ile sunucuda temizleme (`sanitize.rs`). `Builder::default()` temel alınır: `script`, `style`, `iframe`, `object`, `embed`, `form`, `svg`, `link`, `meta`, olay öznitelikleri (`on*`), `style` özniteliği ve `javascript:`/`data:` şemaları kaldırılır.
   - İzinli URL şemaları yalnız `http`, `https`, `mailto`.
   - `link_rel(Some("noopener noreferrer"))`.
   - `attribute_filter`: `img/src` göreliyse normalize edilip `/api/books/{id}/asset?p=<percent-encode>` yapılır, mutlak (http/data) ise öznitelik kaldırılır. `a/href` göreli ise `#` yapılır. `a/target` yok.
2. Bölüm, `<iframe sandbox="allow-same-origin" referrerpolicy="no-referrer">` içinde gösterilir. `allow-scripts`, `allow-forms`, `allow-popups`, `allow-top-navigation` YOKTUR. Betik çalışması imkânsızdır. `allow-same-origin` yalnız ebeveyn sayfanın iframe'in stilini/kaydırmasını CSSOM ile yönetmesi içindir.
3. Çerçeve yanıtının başlığı:
   `Content-Security-Policy: sandbox allow-same-origin; default-src 'none'; img-src 'self'; style-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'`
   Yani betik yok, harici kaynak yok, form yok.
4. Uygulama sayfalarının CSP'si (tüm HTML/JSON olmayan statik yanıtlar): `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; frame-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'self'`. Satır içi script ve `style=""` özniteliği kullanılmaz. JS stili CSSOM (`el.style.x = ...`, `setProperty`) ile verir.
5. Tüm yanıtlar: `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`.
6. Bölüm içindeki tüm tıklamalar ebeveyn tarafında yakalanır (`iframe.contentDocument.addEventListener('click')`): `preventDefault()`, `http(s)` bağlantısı `window.open(href,'_blank','noopener,noreferrer')` ile açılır, diğerleri yok sayılır.
7. Çerez HttpOnly olduğundan ve çerçeve betik çalıştırmadığından içerik oturuma erişemez.

Kapsam dışı bırakılanlar: kitap içi bağlantı atlama (iç `href`) ve EPUB'ın kendi CSS'i (bilinçli; `<link>` ve `<style>` temizlenir).

### 6.4 bsdtar (RAR/CBR)
```rust
// formats/comic.rs
async fn bsdtar_list(path: &Path) -> Result<Vec<String>, FormatError>;
async fn bsdtar_read(path: &Path, member: &str) -> Result<Vec<u8>, FormatError>;
fn glob_escape(name: &str) -> String;   // '\' '*' '?' '[' ']' önüne '\' koyar
```
- Komutlar: `bsdtar -tf <abs_path>` ve `bsdtar -xOf <abs_path> -- <glob_escape(member)>`.
- `tokio::process::Command`, shell yok, argümanlar ayrı `.arg()`, `.env_clear()` + `LC_ALL=C.UTF-8`, `stdin(null)`, `stderr(null)`, `kill_on_drop(true)`.
- Zaman aşımı 30 sn (`tokio::time::timeout`), aşımda süreç öldürülür → `FormatError::Timeout` → 422 `corrupt_book`.
- Çıktı: stdout `take(64 MiB + 1)` ile okunur, fazlası → `TooLarge`.
- Liste çıktısı satırlara bölünür. İçinde kontrol karakteri, `..` bileşeni, `/` ile başlama veya `\` olan satırlar atlanır. Sayfa süzgeci 6.2'deki gibidir.
- `bsdtar` çıkış kodu ≠ 0 → `Corrupt`. `bsdtar` yoksa (`NotFound`) → `FormatError::ToolMissing` → 500 `internal`.
- `bsdtar_read`, istemcinin verdiği ad değil, `comic_pages` listesinden indeksle seçilen adı alır. İstemci hiçbir zaman üye adı gönderemez.
- Şifreli RAR: `bsdtar` hata kodu ≠ 0 döner → `Corrupt`.
- Test: 11.2'deki `rar_*` testleri.

---

## 7. Kütüphane ve tarayıcı

### 7.1 Tarama
```rust
pub struct ScanStatus { running: AtomicBool, done: AtomicU32, total: AtomicU32, last_finished_at: AtomicI64 }
pub fn start_scan(state: AppState) -> bool;   // false: zaten çalışıyor
```
- `start_scan`, `running.compare_exchange(false,true)` ile tek kopya garantiler, `tokio::spawn` ile arka planda çalışır. HTTP işleyicileri kilitlenmez.
- Başlangıçta `main` bir kez çağırır. Başka periyodik tarama yoktur.
- Yürüyüş `spawn_blocking` içinde `std::fs::read_dir` ile yinelemeli yapılır. Symlink atlanır. Tanınmayan uzantı atlanır. Uzantı büyük/küçük harf duyarsızdır.
- Dosya başına: `size`+`mtime` DB'dekiyle aynı ve `missing=0` ise atlanır. Aksi halde:
  1. `content_id` hesapla (7.2).
  2. Bu `rel_path`'te satır varsa güncelle.
  3. Yoksa `content_id` eşleşen `missing=1` satır var mı bak. Varsa onu canlandır: `rel_path`, `series_id`, `title`, `size`, `mtime` güncellenir, `missing=0`.
  4. Yoksa yeni satır ekle.
  5. Biçime göre meta çıkar (7.3), kapak üret (`cover.rs`).
- Dosya başına hata taramayı durdurmaz. Satır `error='corrupt'`/`'drm'` ile kaydedilir, `page_count=0`.
- Tarama sonunda: yürüyüşte görülmeyen tüm `missing=0` kitaplar `missing=1` yapılır. Okuma geçmişi dokunulmaz.
- Veritabanı yazmaları dosya başına kısa kilit alır. Uzun işler (özet, açma, kapak) kilit DIŞINDA yapılır.
- `series`: kitabın klasörü (`rel_path`'in dizin kısmı) seridir. `name` = dizin yolundaki bileşenler `" / "` ile birleşir. Kökteki dosyalar `rel_path=''`, `name='Diğer'` serisine girer. `sort_index`: seri içindeki `file_name` doğal sıralamasında 0..n (her taramada yeniden yazılır).

### 7.2 Kalıcı kimlik
`content_id = hex(sha256( size_le_u64 || ilk 64 KiB || son 64 KiB ))`. Dosya 128 KiB'den küçükse tamamı hashlenir. Aynı içerik farklı klasöre taşınınca veya yeniden adlandırılınca okuma geçmişi aynı `books.id` üzerinde korunur. Aynı `content_id`'ye sahip iki görünür dosya varsa her biri ayrı satırdır (eşleşme yalnız `missing=1` satırlara karşı yapılır).

### 7.3 Meta
- Görsel biçimler: `page_count` = süzülmüş sayfa sayısı (0 ise `error='corrupt'`). Başlık = dosya adı (uzantısız).
- EPUB: başlık = OPF `dc:title` (boş değilse), yoksa dosya adı. `page_count` = spine uzunluğu.
- MOBI: başlık = `Mobi::title()` (boş değilse), yoksa dosya adı. `page_count` = bölüm sayısı.
- DRM tespiti: EPUB'ta `META-INF/encryption.xml` içinde `EncryptedData`'nın `EncryptionMethod Algorithm` değeri `http://www.idpf.org/2008/embedding` veya `http://ns.adobe.com/pdf/enc#RC` DIŞINDA bir şeyse `drm`. MOBI'de `Mobi::encryption()` `Encryption::No` DIŞINDAYSA `drm`.
- Okuma uç noktaları `error` dolu kitaba 422 `drm_protected` veya `corrupt_book` döner (Türkçe mesaj: "Bu kitap DRM korumalı olduğu için açılamıyor." / "Bu dosya bozuk veya okunamıyor.").

### 7.4 Kapaklar (`cover.rs`)
```rust
pub async fn extract_cover(root: &Path, book: &BookRow, covers_dir: &Path) -> bool; // true: yazıldı
```
- Görsel: ilk sayfa. EPUB: OPF'de `properties="cover-image"` ögesi, yoksa `<meta name="cover" content="id">` ögesi. MOBI: `image_records()` içindeki ilk kayıt.
- Çözme `image::ImageReader` + `Limits{max_alloc: Some(256 MiB), max_image_width/height: Some(16384)}`, `spawn_blocking` içinde.
- `thumbnail` ile genişlik 300 px'e küçültülür (en-boy korunur), JPEG kalite 80, `/data/covers/{book_id}.jpg`. Başarı → `has_cover=1`.
- Başarısızlık kitabı bozuk saymaz, yalnız `has_cover=0`.
- `cover_book_id` (seri kartı): serideki `sort_index` en küçük, `has_cover=1`, `missing=0` kitap.

---

## 8. Okuma takibi mantığı

### 8.1 Konum (locator)
- Görsel: ondalık dizi metni `"12"` (0 tabanlı sayfa). `percent = (sayfa+1)/page_count`.
- Metin: `{"ch":3,"p":0.42}` JSON metni (`ch` bölüm indeksi, `p` bölüm içi kaydırma oranı 0..1). `percent = (ch + p) / page_count`, istemci hesaplar.
- Sunucu doğrular: `percent` 0..1 sonlu sayı, `locator` ≤ 200 bayt. Aksi 400.

### 8.2 Durum kuralları (`PUT progress`)
Yeni durum şu sırayla belirlenir:
1. `percent >= 0.99` → `finished`. `finished_date`, önceki durum zaten `finished` değilse `local_date` olur.
2. Aksi halde önceki durum `finished` ise `finished` kalır (tekrar okuma bitmiş sayılmaya devam eder, tarih korunur).
3. Aksi halde (`unread`, `dropped`, `reading`, kayıt yok) → `reading`.
Elle durum değişikliği (`PUT status`) her zaman uygulanır. Sonraki `progress` isteği yukarıdaki kuralı yeniden değerlendirir.

### 8.3 Oturum günlüğü (`POST /api/reading/ping`)
İstek:
```json
{"client_id":"32hex","book_id":1,"active_seconds":127,"pages_read":9,
 "start_percent":0.10,"end_percent":0.22,"local_date":"2026-11-08"}
```
- `client_id` `^[0-9a-f]{32}$`, `active_seconds` 0..86400, `pages_read` 0..100000, yüzdeler 0..1. Aksi 400.
- Kitap yoksa 404.
- `active_seconds == 0 && pages_read == 0` ve satır yoksa → satır oluşturulmaz (204).
- Satır yoksa: `started_at = now - active_seconds` (en az 0'a kırpılmış), `local_date` istekten.
- Satır varsa (UPSERT, `(user_id, client_id)`): `active_seconds = MAX(eski, yeni)` ama üst sınır `(now - started_at) + 5`. `pages_read = MAX(eski, yeni)`. `end_percent` yenisi. `ended_at = now`. `start_percent`/`local_date`/`started_at` değişmez.
- İstemci tarafı (`tracker.js`): ping 15 sn'de bir; `document.visibilityState==='hidden'` olunca ve `pagehide` olayında ayrıca `navigator.sendBeacon('/api/reading/ping', new Blob([JSON.stringify(body)],{type:'application/json'}))` gönderilir. Sekme kapanırsa son beacon verisi kaybolmaz.
- Boşta sayımı: saniyelik zamanlayıcı, yalnız `visibilityState==='visible'` VE son kullanıcı girdisi (pointerdown, keydown, touchstart, wheel, scroll, çevrilen sayfa) ≤ 60 sn önceyse `active_seconds` artar. Beacon/ping anında hiç artış yoksa da ping gönderilir (ended_at ilerlemez çünkü active_seconds aynı kalır; yine `ended_at=now` yazılır, süre DEĞİŞMEZ).
- `client_id`, `crypto.getRandomValues` ile üretilir (`crypto.randomUUID` düz HTTP'de yoktur, KULLANILMAZ).
- `pages_read`: görsel okuyucuda ziyaret edilen farklı sayfa sayısı; metin okuyucuda bölüm başına ziyaret edilen farklı `floor(scrollTop/clientHeight)` ekranları toplamı.

### 8.4 İstatistik (`GET /api/stats?today=YYYY-MM-DD`)
Yanıt:
```json
{"total_seconds":0,"total_pages":0,"finished_count":0,
 "streak_current":0,"streak_longest":0,
 "daily":[{"date":"2026-10-10","seconds":0,"pages":0}],
 "goal":{"year":2026,"target":null,"done":0}}
```
- `daily`: `today`dan geriye 30 gün (eski → yeni), olmayan günler 0. Tarih aritmetiği saf takvim hesabıdır (`days_from_civil` el yazımı, saat dilimi yok).
- Tüm toplamlar yalnız çağıran kullanıcınındır.
- `total_*`: tüm `reading_sessions` toplamı.
- `finished_count`: `status='finished'` olan `progress` satırı sayısı.
- Seri günü: ≥ 60 toplam `active_seconds`'u olan `local_date` günü. `streak_current`: `today` (yoksa dün) ile biten ardışık gün sayısı. `streak_longest`: en uzun ardışık dizi.
- `goal.year` = `today`'in yılı. `target` yoksa `null`. `done` = o yıl `finished_date` ile başlayan `finished` satırı sayısı.

---

## 9. Arayüz (Türkçe, vanilla)

### 9.1 Sayfalar (hash rotaları)
`#/giris`, `#/kurulum` (`needs_setup` ise otomatik), `#/` (kütüphane: arama kutusu, sıralama seçici, kapak ızgarası, "Kütüphaneyi tara" butonu yalnız adminde ve tarama durumu rozeti), `#/seri/{id}`, `#/oku/{bookId}`, `#/gecmis`, `#/istatistik`, `#/yonetim` (yalnız admin: kullanıcı listesi/ekle/sil/parola, tara).
- Üst çubuk: Kütüphane, Geçmiş, İstatistik, Yönetim (admin), kullanıcı adı, Çıkış.
- 401 yanıtı → `#/giris`.
- Yer tutucu kapak: SVG, başlığın ilk harfi.

### 9.2 Görsel okuyucu (`reader-comic.js`)
- Tek sayfa `<img>`. Sonraki 3 sayfa `new Image().src=...` ile ön yüklenir. Önceki 1 sayfa da.
- Klavye: ←/→ ve PageUp/PageDown sayfa, Home/End ilk/son, `f` sığdırma modu döngüsü, `Esc` seriye dön. Odaklanabilir düğmeler (`aria-label` Türkçe).
- Dokunmatik: yatay kaydırma (>50 px) sayfa çevirir; ekranın sol/sağ üçte biri dokunuşu önceki/sonraki.
- Sığdırma: Genişliğe sığdır, Yüksekliğe sığdır, Sayfaya sığdır, Gerçek boyut. Seçim `localStorage` (`krs.fit`).
- Sağ-sola okuma: yoktur (kapsam dışı).
- Konum: sayfa değişince 1 sn gecikmeli (debounce) `PUT progress`; çıkışta (`pagehide`) beacon DEĞİL, `fetch(..., {keepalive:true})` ile gönderilir. Açılışta `progress.locator` sayfasından devam.

### 9.3 Metin okuyucu (`reader-text.js`)
- Sol panel: içindekiler (`toc`), klavyeyle erişilebilir liste. Bölüm değişimi: ileri/geri düğmeleri ve `[` `]` tuşları.
- Çerçeve içinde bölüm. Yazı boyutu A-/A+ (14-28 px, adım 2), tema Açık/Koyu/Sepya. Seçim `localStorage` (`krs.fs`, `krs.theme`). Uygulama, `frame.css` değişkenlerini (`--fs`, `--bg`, `--fg`) `iframe.contentDocument.documentElement.style.setProperty` ile her bölüm yüklenince ve değişimde yazar.
- Kaydırma konumu: `scroll` olayı debounce ile `{ch,p}` hesaplar, `PUT progress` gönderir. Açılışta `ch` yüklenir, `load` sonrası `scrollTop = p*(scrollHeight-clientHeight)`.
- Klavye: Space/PageDown aşağı, Shift+Space/PageUp yukarı, bölüm sonunda sonraki bölüme geçer.

### 9.4 Geçmiş ve istatistik (`stats.js`)
- `#/gecmis`: tablo (kitap, seri, tarih, süre `dk sn`, sayfa, yüzde aralığı); silinmiş kitaplar "(silinmiş) başlık" olarak gösterilir. "Daha fazla" düğmesi sayfalar.
- `#/istatistik`: kartlar (toplam süre, toplam sayfa, tamamlanan kitap, güncel seri, en uzun seri), iki SVG sütun grafiği (günlük dakika, günlük sayfa; 30 gün; eksen etiketleri, `<title>` ile erişilebilir değer), yıllık hedef halka/çubuğu ve hedef giriş formu. Grafik yardımcısı `charts.js` AYRI DOSYA DEĞİL, `stats.js` içindedir. Renkler `style.css` sınıflarıyla (CSS değişkeni), satır içi stil yok.
- Yerel bugünü `new Date()` ile `YYYY-MM-DD` (yerel bileşenlerden) üretir.

### 9.5 Erişilebilirlik / duyarlılık
- `:focus-visible` halkası, ≥ 44 px dokunma hedefi, `lang="tr"`, `meta viewport`, 360 px ile 1920 px arası kırılma (`@media (max-width: 700px)`).
- `prefers-color-scheme` ile uygulama teması koyu/açık.

---

## 10. Docker ve CI

### 10.1 Dockerfile
```
FROM rust:1.89-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY web ./web
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libarchive-tools ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --uid 10001 --no-create-home --shell /usr/sbin/nologin app \
 && mkdir -p /data /books && chown 10001:10001 /data
COPY --from=build /src/target/release/kavita-rs /usr/local/bin/kavita-rs
ENV PORT=3435 DATA_DIR=/data BOOKS_DIR=/books
USER 10001
EXPOSE 3435
VOLUME ["/data", "/books"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD ["kavita-rs","healthcheck"]
ENTRYPOINT ["kavita-rs"]
```
`kavita-rs healthcheck`: `127.0.0.1:$PORT`'a `std::net::TcpStream` ile bağlanır, `GET /health` gönderir, durum satırında `200` yoksa `exit(1)`. curl gerekmez. `web/` dosyaları `include_str!` ile derlemeye gömülür, çalışma imajına kopyalanmaz.

### 10.2 `.github/workflows/ci.yml`
- Tetikleyici: `push` (tüm dallar) ve `pull_request`. Üst düzey `permissions: contents: read`.
- İş `test`: `actions/checkout@v4`, `dtolnay/rust-toolchain@master` (`toolchain: 1.89.0`, `components: rustfmt, clippy`), `Swatinem/rust-cache@v2`, `sudo apt-get update && sudo apt-get install -y libarchive-tools`, `[ -f Cargo.lock ] || cargo generate-lockfile`, `actions/upload-artifact@v4` (`name: cargo-lock`, `path: Cargo.lock`), `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`.
- İş `image` (`needs: test`, `if: hashFiles('Cargo.lock') != ''`; hashFiles checkout sonrası olduğundan bu iş kendi checkout'unu yapar ve `if` yerine ilk adımda `test -f Cargo.lock` kontrolü + `steps` koşulu kullanır): `docker/setup-buildx-action@v3`, `docker/build-push-action@v6` (`push: false`, `load: true`, `tags: kavita-rs:ci`, `platforms: linux/amd64`, `cache-from/to: type=gha`), `bash scripts/smoke.sh kavita-rs:ci`.
- İş `publish` (`needs: image`, yalnız `if: github.ref == 'refs/heads/main' && github.event_name == 'push'`): iş düzeyi `permissions: contents: read, packages: write`. `docker/login-action@v3` (`registry: ghcr.io`, `username: ${{ github.actor }}`, `password: ${{ secrets.GITHUB_TOKEN }}`), `docker/setup-buildx-action@v3`, `docker/build-push-action@v6` (`push: true`, `tags: ghcr.io/mustafabayramguven/kavita-rs:latest`, `platforms: linux/amd64`, `cache-from: type=gha`).
- İlk yayından sonra GHCR paketi bir kez public yapılmalıdır (CEO sorusu 4).

### 10.3 `scripts/smoke.sh <imaj>`
`set -euo pipefail`. Adımlar:
1. `docker volume create smoke-data`, boş geçici dizin `books`.
2. `docker run -d --name smoke -p 3435:3435 -v smoke-data:/data -v "$books":/books:ro "$IMG"` (named volume, imajdaki sahiplik 10001'e miras kalır).
3. 30 sn boyunca `curl -fsS http://127.0.0.1:3435/health` bekle, çıktı `ok` olmalı.
4. `GET /` → 200 ve gövdede `kavita-rs`.
5. `GET /api/me` → 401.
6. `POST /api/setup` (`Content-Type: application/json`, gövde `{"username":"smoke","password":"smoke-pass-123"}`) ile çerez kavanozu (`-c jar`) → 201; ardından `-b jar` ile `GET /api/me` → 200 (düz HTTP'de oturumun çalıştığını, yani `Secure` bayrağı olmadığını kanıtlar).
7. İkinci `POST /api/setup` → 409.
8. `docker exec smoke id -u` ≠ `0` ve `docker exec smoke bsdtar --version` başarılı.
9. `docker inspect --format '{{.State.Health.Status}}'` en çok 60 sn içinde `healthy` olur.
10. `trap` ile `docker rm -f smoke; docker volume rm smoke-data`.

### 10.4 `docker-compose.yml`
```yaml
services:
  kavita-rs:
    image: ghcr.io/mustafabayramguven/kavita-rs:latest
    ports: ["3435:3435"]
    volumes:
      - ./books:/books:ro
      - kavita-data:/data
    restart: unless-stopped
volumes:
  kavita-data:
```

### 10.5 README (Türkçe, kısa)
`docker compose up -d`, `http://localhost:3435` ilk açılışta kurulum ekranı: ilk oluşturulan hesap yöneticidir, kayıt sonra kapanır; `./books` altına her seri için bir klasör; ortam değişkenleri tablosu (bölüm 2); HTTPS arkasında `COOKIE_SECURE=1`; lisans GPL-3.0; Kavita ile bağlantısı yok notu.

---

## 11. Testler (kabul)

### 11.1 Fixture üreticileri (`tests/common/mod.rs`, tümü programatik)
```rust
pub fn png(w:u32,h:u32,rgb:[u8;3]) -> Vec<u8>;                       // image crate ile
pub fn make_cbz(dir:&Path, name:&str, pages:&[(&str,Vec<u8>)]);      // zip crate ile
pub fn make_epub(dir:&Path, name:&str, opts:EpubOpts) -> PathBuf;    // container.xml, OPF, nav, 3 bölüm, bir görsel, kapak
pub fn make_epub_malicious(dir:&Path) -> PathBuf;                    // script, onerror, javascript:, ../ görsel yolu, DOCTYPE entity bombası
pub fn make_epub_drm(dir:&Path) -> PathBuf;                          // encryption.xml, bilinmeyen algoritma
pub fn make_mobi(dir:&Path, name:&str) -> PathBuf;                   // elle PalmDB+PalmDOC+MOBI başlığı, sıkıştırmasız, 2 bölüm (pagebreak), 1 PNG görsel kaydı
pub fn make_tar_as_cbr(dir:&Path, name:&str, files:&[(&str,Vec<u8>)]) -> PathBuf; // tar crate ile; .cbr uzantısı
pub async fn test_app() -> (Router, TempDirs);                       // geçici DATA_DIR/BOOKS_DIR, tower::oneshot ile çağrılır
pub async fn login_as_admin(app:&Router) -> String;                  // çerez değeri
```

### 11.2 Test listesi (hepsi `cargo test` ile CI'de)
Kimlik (`auth_flow.rs`):
- `setup_creates_first_admin_then_closes`: ilk `/api/setup` 201, ikinci 409.
- `unauthenticated_endpoints_return_401`: bölüm 5.3'teki tüm yollar çerezsiz 401 (health/setup-status/setup/login/static hariç).
- `non_admin_gets_403_on_admin_routes`: admin kullanıcı oluşturur, normal kullanıcı olarak girip her admin yolunda 403.
- `cookie_flags`: `Set-Cookie` HttpOnly ve SameSite=Strict içerir, `Secure` İÇERMEZ (varsayılan); `COOKIE_SECURE=1` ile içerir.
- `password_is_argon2`: DB'deki hash `$argon2id$` ile başlar.
- `login_rate_limit`: 5 hatalı giriş sonrası 6. istek 429 ve `Retry-After`; doğru parola da kilit süresince 429.
- `csrf_origin_mismatch_403` ve `post_requires_json_content_type_415`.
- `delete_user_removes_data`: kullanıcı silinince `progress`/`reading_sessions` satırları gider; kendini silme 409.
Kütüphane (`library.rs`):
- `scan_builds_series_and_books`, `search_and_sort`, `scan_non_blocking` (tarama sürerken `/health` anında yanıtlar; tarama durumunda ikinci `POST /api/admin/scan` 409), `deleted_file_hidden_but_history_kept` (dosya silinir, tarama, kitap listede yok, `/api/history` satırı hâlâ var ve `book_missing=true`), `moved_file_revives_same_book_id` (aynı içerik yeniden adlandırılır, `books.id` değişmez, ilerleme korunur), `symlink_ignored_by_scan`, `cover_generated_and_cached` (300 px, `covers/{id}.jpg` var).
Biçimler (`formats.rs`, `reader.rs`):
- `cbz_pages_natural_order` (`p2.png` < `p10.png`), `cbz_skips_zipslip_and_hidden_names`, `cbz_entry_over_limit_rejected` (sınır sabiti test için küçük bir `limit` parametresiyle çağrılır: sayfa okuma fonksiyonu `max_entry` argümanı alır, üretim sabiti geçirilir), `cbz_corrupt_returns_422`.
- `epub_toc_and_chapter`, `epub_asset_only_manifest_images`, `epub_asset_path_traversal_404` (`p=../../etc/passwd` ve `%2e%2e/` biçimleri), `epub_xml_entity_not_expanded` (DOCTYPE entity bombasının sonucu bellek patlatmaz ve genişlemez), `epub_drm_returns_422_drm_protected`.
- `epub_sanitizes_xss`: kötü amaçlı EPUB bölümünde yanıt gövdesinde `<script`, `onerror`, `javascript:` YOK; yanıt CSP başlığı beklenen metni içerir.
- `mobi_chapters_and_images`: 2 bölüm, ikinci bölümdeki `recindex` görseli `/api/books/{id}/asset?p=1` olur ve PNG baytları döner.
- `mobi_drm_flag` (encryption alanı ≠ 0 olan sahte MOBI → `drm`).
- `rar_glob_escape_unit` (`glob_escape("a[1]*?.png")` beklenen çıktı).
- `rar_via_bsdtar_lists_and_reads` (`make_tar_as_cbr` ile `.cbr` uzantılı tar; üye adları `p1.png`, `[x]*.png` içerir; liste ve `[x]*.png` okuması TAM o üyenin baytlarını döner, başka üyeyi döndürmez), `rar_timeout_kills_process` (sahte uzun süreli komut yerine `bsdtar_run` iç fonksiyonu `sleep 5` ile ve 200 ms zaman aşımıyla çağrılır ve `Timeout` döner), `rar_output_cap`.
- `rar_real_fixture`: `tests/fixtures/sample.rar` varsa 2 sayfalı gerçek RAR'ı listeler/okur; yoksa `eprintln!("sample.rar yok, atlandı")` ile geçer (CEO sorusu 1).
Takip (`tracking.rs`):
- `progress_roundtrip_comic_and_text`, `status_rules` (8.2'nin üç kuralı + elle `dropped`), `ping_upsert_monotonic_and_clamped`, `ping_zero_creates_nothing`, `history_only_own_rows`, `stats_streaks_and_daily` (sabit `today` ile bilinen oturumlardan beklenen seri/günlük dizi), `stats_other_user_isolated`, `goal_roundtrip_and_done_count`, `local_date_validation_400`.
Güvenlik (`security.rs`):
- `resolve_book_path_rejects_dotdot_abs_symlink` (birim), `sql_injection_in_search_is_inert` (`q=' OR 1=1 --` sonuç dönmez, tablo bozulmaz), `security_headers_present` (CSP, nosniff).

### 11.3 Smoke
`scripts/smoke.sh` (10.3) CI `image` işinde koşar.

---

## 12. Arayüzler ve imzalar (Rust)

```rust
// config.rs
pub struct Config { pub port:u16, pub data_dir:PathBuf, pub books_dir:PathBuf, pub cookie_secure:bool }
impl Config { pub fn from_env() -> Result<Self,String>; }

// error.rs
pub enum AppError { BadRequest(&'static str), Unauthorized, Forbidden, Csrf, NotFound, Conflict(&'static str),
  PayloadTooLarge, UnsupportedMedia, Corrupt, Drm, TooManyAttempts{retry_after:u64}, Internal }
impl IntoResponse for AppError {}
impl From<rusqlite::Error> for AppError {}   // Internal (log'lar)
impl From<formats::FormatError> for AppError {}  // Drm→422 drm_protected, Corrupt|TooLarge|Timeout→422 corrupt_book, ToolMissing|Io→Internal

// db.rs
#[derive(Clone)] pub struct Db(Arc<Mutex<Connection>>);
impl Db {
  pub fn open(path:&Path) -> rusqlite::Result<Db>;     // pragma + migrate
  pub async fn run<T:Send+'static>(&self, f: impl FnOnce(&mut Connection)->Result<T,AppError>+Send+'static) -> Result<T,AppError>;
}

// app.rs
#[derive(Clone)] pub struct AppState { pub db:Db, pub cfg:Arc<Config>, pub scan:Arc<ScanStatus>, pub limiter:Arc<LoginLimiter> }
pub fn build_app(state:AppState) -> Router;

// auth.rs
pub struct AuthUser { pub id:i64, pub username:String, pub is_admin:bool }  // axum extractor (FromRequestParts)
pub struct AdminUser(pub AuthUser);                                          // admin extractor, 403
pub async fn hash_password(pw:String) -> Result<String,AppError>;           // argon2id, spawn_blocking
pub async fn verify_password(pw:String, hash:String) -> bool;
pub fn new_session_token() -> String;       // 32 bayt rastgele, base64 url-safe no pad
pub fn token_hash(token:&str) -> String;    // sha256 hex
pub struct LoginLimiter { /* Mutex<HashMap<String,Bucket>> */ }
impl LoginLimiter { pub fn check(&self, user:&str, ip:IpAddr) -> Result<(),u64>; pub fn fail(&self, user:&str, ip:IpAddr); pub fn ok(&self, user:&str); }
pub async fn origin_and_content_type_guard(req:Request, next:Next) -> Response;  // middleware

// formats/mod.rs
#[derive(Clone,Copy,PartialEq,Eq)] pub enum Format { Epub, Mobi, Cbz, Zip, Rar, Cbr }
impl Format { pub fn from_path(p:&Path)->Option<Format>; pub fn is_comic(self)->bool; pub fn as_str(self)->&'static str; }
pub enum FormatError { Corrupt, Drm, TooLarge, Timeout, ToolMissing, Io(std::io::Error) }

// formats/comic.rs
pub async fn comic_pages(path:&Path, fmt:Format) -> Result<Vec<String>,FormatError>;
pub async fn comic_read(path:&Path, fmt:Format, member:&str, max_entry:u64) -> Result<Vec<u8>,FormatError>;

// formats/epub.rs
pub struct EpubInfo { pub title:Option<String>, pub spine:Vec<String>, pub toc:Vec<(String,usize)>, pub cover:Option<String>, pub manifest_images:HashMap<String,String> /*yol→mime*/ }
pub fn epub_open(path:&Path) -> Result<EpubInfo,FormatError>;         // DRM kontrolü burada
pub fn epub_chapter_html(path:&Path, info:&EpubInfo, n:usize) -> Result<String,FormatError>; // ham XHTML metni
pub fn epub_asset(path:&Path, info:&EpubInfo, p:&str) -> Result<(Vec<u8>,String),FormatError>;
pub fn normalize_zip_path(base_dir:&str, href:&str) -> Option<String>; // %-çöz, fragment/query at, '..' çöz, kökten çıkarsa None

// formats/mobi.rs
pub struct MobiBook { pub title:String, pub chapters:Vec<String> /*ham html*/, pub images:Vec<Vec<u8>> }
pub fn mobi_open(path:&Path) -> Result<MobiBook,FormatError>;         // DRM → Drm

// formats/sanitize.rs
pub fn sanitize_chapter(raw_html:&str, book_id:i64, chapter_dir:&str, mode:SrcMode) -> String; // SrcMode::{Epub,Mobi}
pub fn frame_document(body:&str, title:&str) -> String;                // <!doctype html> + frame.css bağlantısı
pub const FRAME_CSP: &str;

// formats/natural.rs
pub fn natural_cmp(a:&str, b:&str) -> std::cmp::Ordering;              // rakam öbekleri sayısal, harf duyarsız

// scan.rs
pub fn start_scan(state:AppState) -> bool;
// safepath.rs
pub fn resolve_book_path(root:&Path, rel:&str) -> Result<PathBuf,AppError>;
```
Not: `main.rs` `serve` açılışında `Db::open`, `covers` klasörünü oluşturur, süresi geçmiş oturumları siler (saatlik `tokio::time::interval` ile tekrarlanır), `start_scan` çağırır, `axum::serve(...).with_graceful_shutdown(ctrl_c + SIGTERM)`.

---

## 13. Görev listesi (bağımlılık sırasıyla)

Kural: Aynı dosyaya iki görev dokunuyorsa görev numarası sırasıyla çalışır; sonraki görev öncekinin commit'ini bekler. Her görev sonunda `git commit` (trailer'lı) + `git push`. Yerelde derleme/test YOKTUR, doğrulama CI'dir (`gh run watch`, `gh run view --log-failed`). Başarısız CI'yi düzeltmek görevi yazan işçinin sorumluluğudur. Commit trailer'ı:
```
Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01S2hbav9nVUQp4gv4mryLkM
```

| # | İşçi | Görev | Dosyalar | Bağımlı |
|---|---|---|---|---|
| 1 | İŞÇİ-2 | Depo: `/home/mustafa/kavita-rs` oluştur, `git init -b main`, `.gitignore`, `.dockerignore`, `LICENSE` (GPL-3.0 tam metin, gnu.org'dan `curl`), `BLUEPRINT.md` (CEO kaydetmediyse), `gh repo create mustafabayramguven/kavita-rs --public --source=. --remote=origin`. İlk commit, push | `.gitignore .dockerignore LICENSE BLUEPRINT.md` | - |
| 2 | İŞÇİ-2 | `Cargo.toml` (bölüm 1, `[package] name="kavita-rs", license="GPL-3.0-only", edition, rust-version, resolver`; `[[bin]] name="kavita-rs"`), `rust-toolchain.toml`, sahte `src/main.rs` (`fn main(){}`) ve `src/lib.rs` (boş), `.github/workflows/ci.yml` (10.2), `Dockerfile`, `docker-compose.yml`, `scripts/smoke.sh`, `README.md`. `web/index.html` yer tutucu (Dockerfile COPY'si kırılmasın). Push | `Cargo.toml rust-toolchain.toml src/main.rs src/lib.rs .github/workflows/ci.yml Dockerfile docker-compose.yml scripts/smoke.sh README.md web/index.html` | 1 |
| 3 | İŞÇİ-2 | İlk CI koşusunu izle (`gh run watch`). `test` işi yeşil olmalı (boş proje). `gh run download --name cargo-lock`; `Cargo.lock`'u commit/push et (1.1). `image` ve `publish` işleri bu aşamada derlenemez olabilir, çünkü `main.rs` boş; sorun bunlarla ilgiliyse görev 4'ten sonra yeniden koşulur | `Cargo.lock` | 2 |
| 4 | İŞÇİ-1 | Çekirdek: `config.rs`, `error.rs`, `db.rs`, `migrations/0001_init.sql`, `safepath.rs` (+birim testler), `main.rs` (serve + healthcheck alt komutu), `lib.rs` modül bildirimleri, `app.rs` (AppState, `build_app`, güvenlik başlıkları, `/health`, statik sunum tablosu `web/*` dosyalarıyla; henüz var olmayan JS/CSS dosyaları için `web/` altında boş yer tutucu dosyaları İŞÇİ-2 görev 5'te oluşturur) | `src/{config,error,db,safepath,main,lib,app}.rs src/migrations/0001_init.sql` | 3 |
| 5 | İŞÇİ-2 | `web/` yer tutucu dosyaları: `app.js tracker.js reader-comic.js reader-text.js stats.js style.css frame.css favicon.svg` (boş/bir satır yorum), `index.html` kabuğu (`<script src="/static/...">` sırası: tracker, reader-comic, reader-text, stats, app; `<link>` stil) | `web/*` (JS/CSS içeriği görev 14-20'de) | 3 |
| 6 | İŞÇİ-1 | `auth.rs` (argon2, token, extractors, limiter, origin/content-type guard) + `api/mod.rs`, `api/auth.rs` (setup-status, setup, login, logout, me) + `tests/common/mod.rs` (`test_app`, `login_as_admin`, `png`) + `tests/auth_flow.rs` (cookie_flags, setup, 401 listesi, rate limit, csrf, argon2) | `src/auth.rs src/api/{mod,auth}.rs tests/common/mod.rs tests/auth_flow.rs` | 4 |
| 7 | İŞÇİ-1 | `api/admin.rs` (kullanıcılar, parola, scan tetikleme) + `non_admin_gets_403`, `delete_user_removes_data` testleri (`tests/auth_flow.rs` içine ekle) | `src/api/admin.rs tests/auth_flow.rs` | 6 |
| 8 | İŞÇİ-1 | `formats/mod.rs`, `formats/natural.rs`, `formats/comic.rs` (zip + bsdtar, glob_escape, sınırlar, `bsdtar_run` iç fonksiyonu zaman aşımı/çıktı sınırı parametreli) + `tests/common/mod.rs`'e `make_cbz`, `make_tar_as_cbr` ekle + `tests/formats.rs` (cbz_*, rar_* testleri) | `src/formats/{mod,natural,comic}.rs tests/common/mod.rs tests/formats.rs` | 7 |
| 9 | İŞÇİ-1 | `formats/epub.rs`, `formats/sanitize.rs`, `formats/mobi.rs` + `make_epub`, `make_epub_malicious`, `make_epub_drm`, `make_mobi` fixture'ları + `tests/formats.rs` (epub_*, mobi_*) | `src/formats/{epub,sanitize,mobi}.rs tests/common/mod.rs tests/formats.rs` | 8 |
| 10 | İŞÇİ-1 | `formats/cover.rs` + `scan.rs` (tarama, kimlik, canlandırma, ScanStatus) + `main.rs` içinde açılış taraması ve oturum temizliği | `src/formats/cover.rs src/scan.rs src/main.rs` | 9 |
| 11 | İŞÇİ-1 | `api/library.rs` (series, series/{id}, books/{id}, cover, scan-status) + `tests/library.rs` | `src/api/{library,mod}.rs tests/library.rs` | 10 |
| 12 | İŞÇİ-1 | `api/reader.rs` (pages, pages/{n}, toc, chapters/{n}, asset; 422 eşlemeleri; çerçeve CSP) + `tests/reader.rs` | `src/api/{reader,mod}.rs tests/reader.rs` | 11 |
| 13 | İŞÇİ-1 | `api/tracking.rs` (progress, status, ping, history) + `api/stats.rs` (stats, goal) + `tests/tracking.rs` + `tests/security.rs` | `src/api/{tracking,stats,mod}.rs tests/tracking.rs tests/security.rs` | 12 |
| 14 | İŞÇİ-2 | `web/style.css` ve `web/frame.css` (tema değişkenleri açık/koyu/sepya, ızgara, düğmeler, odak halkası, tablo, mobil kırılma, grafik sınıfları, okuyucu düzeni) | `web/style.css web/frame.css` | 5 |
| 15 | İŞÇİ-1 | `web/app.js`: fetch sarmalayıcı, router, giriş/kurulum, kütüphane (arama/sıralama/kapaklar/tara butonu/durum), seri sayfası (durum seçici), yönetim sayfası | `web/app.js` | 13, 14 |
| 16 | İŞÇİ-1 | `web/tracker.js` (boşta algılama, ping, sendBeacon, getRandomValues kimliği, progress kaydı yardımcıları) | `web/tracker.js` | 15 |
| 17 | İŞÇİ-1 | `web/reader-comic.js` (9.2) | `web/reader-comic.js` | 16 |
| 18 | İŞÇİ-1 | `web/reader-text.js` (9.3) | `web/reader-text.js` | 17 |
| 19 | İŞÇİ-1 | `web/stats.js` (geçmiş, istatistik, hedef, SVG grafikler) | `web/stats.js` | 18 |
| 20 | İŞÇİ-2 | `web/favicon.svg`, `index.html` son hali (başlık, `lang`, erişilebilirlik), README gözden geçirme, `tests/security.rs`'e dokunmadan statik dosya yanıtlarını denetleyen `tests/static_files.rs` (her `/static/*` 200, bilinmeyen ad 404, `index.html` içinde `http://`/`https://` CDN bağlantısı YOK, satır içi `<script>` ve `style=` YOK) | `web/favicon.svg web/index.html README.md tests/static_files.rs` | 19 |
| 21 | İŞÇİ-2 | Push + CI doğrulaması: `fmt`, `clippy -D warnings`, `test`, `image` (smoke) işleri YEŞİL olana kadar `gh run watch` / `gh run view --log-failed`. Hata kodla ilgiliyse hatayı ilgili modülün işçisine (İŞÇİ-1) geri gönder; yapılandırma/workflow/Dockerfile hatasıysa kendisi düzeltir. `Cargo.lock` değişmesi gerekirse 1.1 madde 6 | - | 20 |
| 22 | İŞÇİ-2 | `main` koşusunda `publish` işinin yeşil olduğunu ve `ghcr.io/mustafabayramguven/kavita-rs:latest` etiketinin oluştuğunu doğrula (`gh api /users/mustafabayramguven/packages/container/kavita-rs`). Paket görünürlüğünü CEO'ya bildir (soru 4) | - | 21 |

Not: Görev 8-9'daki fixture/`tests/common/mod.rs` ve `tests/formats.rs` birbirini izleyen görevlerde aynı dosyaya eklenir; bu yüzden sıra zorunludur. Görev 15-19 kendi JS dosyalarına yazar; ortak çağrı arayüzü: `app.js` `window.KRS = { api(method,url,body), toast(msg), navigate(hash), todayLocal(), me }` tanımlar. `tracker.js` `window.KRS.tracker = { start(bookId,startPercent), touch(), addPage(key), setPercent(p), stop() }` tanımlar. `app.js` yüklenmeden diğerleri bunlara dokunmaz (hepsi `DOMContentLoaded`'dan sonra çağrılır). Okuyucular `window.KRS.readers = { comic:{open(book,mount)}, text:{open(book,mount)} }` ile kayıt olur.

---

## CEO'YA SORULAR

1. **Gerçek RAR fixture'ı:** RAR üreten özgür araç yok. 2 sayfalı küçük bir `sample.rar` (içinde `p1.png`, `p2.png`, RAR5) kendi makinenizde üretip `tests/fixtures/sample.rar` olarak commit eder misiniz? Gelene kadar RAR yolu yalnız `.cbr` uzantılı tar (bsdtar aynı kod yolunu kullanır) ile test edilir. Gerçek RAR formatı CI'da doğrulanmaz.
2. **MOBI kapsamı:** `mobi 0.8.0` yalnız başlık, ham metin ve görsel kayıtlarını verir. Bölümler `<mbp:pagebreak>` ile bölünür (içindekiler "Bölüm N"). Kitap içi bağlantılar (`filepos`) çalışmaz. KF8/AZW3 desteği vaat edilmez. Kabul edilir mi? Yetersiz çıkarsa kendi MOBI ayrıştırıcımızı yazmak bu planın dışındadır ve size sorulur.
3. **EPUB'ın kendi CSS'i ve SVG kapakları:** Güvenlik için EPUB CSS'i ve `<svg>` tamamen atılıyor. SVG içine gömülmüş kapak sayfaları boş görünür, bazı EPUB'lar kendi biçimlendirmesini kaybeder. Kabul edilir mi?
4. **GHCR paket görünürlüğü:** İlk yayından sonra paketin public yapılması gerekir. Bu GitHub arayüzünden elle yapılır (veya yetkili `gh api` çağrısı). CEO yapacak mı, yoksa işçi `gh api` ile denesin mi?
5. **Commit trailer çelişkisi:** Brief `Co-Authored-By: Claude Opus 5.5` ister, ama bu oturumun sistem hatırlatıcısı `Claude Sonnet 5.5` yazıyor. Blueprint brief'teki Opus 5.5 trailer'ını esas aldı. Doğru mu?
6. **Git kimliği:** Yerelde `user.name`/`user.email` ayarlı mı? Değilse hangi ad/e-posta ile commit atılsın?
7. **Tek admin:** Admin sonradan başka admin yapamaz ve admin kendi parolasını değiştiremez (yalnız başka kullanıcıların parolasını sıfırlayabilir). Admin parolasını unutursa çıkış yolu DB'yi elle düzenlemektir. İstenen kapsam bu mu, kendi parolasını değiştirme eklensin mi?
8. **Seri günü eşiği:** Bir günün seri sayılması için en az 60 sn aktif okuma şartı kondu. Uygun mu?

---

Atlananlar/riskler: Hiçbir crate API'si derlenerek doğrulanmadı. Sürümlerin varlığı crates.io ile, `mobi` API'si GitHub kaynağıyla doğrulandı. Axum 0.8, argon2 0.5, ammonia 4 ve zip 2 çağrı biçimlerinde CI'da ilk derleme hataları çıkabilir; düzeltme kodda yapılır, sürüm yükseltilmez. En büyük risk: elle üretilen sahte MOBI fixture'ının `mobi` crate'inin ayrıştırıcısı tarafından kabul edilmesi (görev 9'da iterasyon gerektirebilir).
---

## 14. CEO düzeltmeleri (önceki bölümlerle çelişirse bu bölüm kazanır)

"CEO'YA SORULAR" bölümü İPTAL EDİLDİ. Sorular kapandı, yanıtlar aşağıdadır. Yeni soru yoktur.

### 14.0 Tüm işçiler için kural
- Yerelde cargo, rustc, docker, npm, pip, playwright ÇALIŞTIRILMAZ ve sisteme hiçbir şey kurulmaz.
- Yalnız `git`, `gh`, `curl` ve kurulum gerektirmeyen `python3 -I` kullanılır.
- fmt hatası, CI günlüğündeki diff'ten elle düzeltilir.

### 14.1 Görev sırası düzeltmeleri
- Görev 5 (`web/` yer tutucuları), görev 4'ten ÖNCE çalışır. Gerekçe: `include_str!` dosyaları olmadan `app.rs` derlenmez. Yeni sıra: 1, 2, 3, 5, 4, 6, ...
- Görev 3 `Cargo.lock`'u commit'ler. Bölüm 1.1 adım 4'teki "Görev 4" ifadesi "Görev 3" olarak okunur.
- `POST /api/admin/scan` (`api/admin.rs` içindeki işleyici) ve ona ait testler görev 7'den görev 10'a taşınır, çünkü `scan.rs` görev 10'da doğar. Görev 7 yalnız kullanıcı yönetimini, parola sıfırlamayı ve 14.5'teki `PUT /api/me/password`'u içerir.
- `unauthenticated_endpoints_return_401` testi görev 6'da yalnız o an var olan yollarla (`/api/me`, `/api/logout`) başlar. Görev 13'te bölüm 5.3'ün tamamına genişletilir.
- `.dockerignore` `Cargo.lock`'u DIŞLAMAZ. İçeriği: `target/`, `data/`, `.git/`.
- Görev 1: depo içinde yalnız `git config user.name mustafabayramguven` ve `git config user.email mustafabayramguven@users.noreply.github.com` çalışır. Global git yapılandırması DEĞİŞTİRİLMEZ.
- Yeni görev 19b, görev 19'dan sonra ve görev 20'den önce çalışır (14.6).
- Commit trailer'ı bölüm 13'teki gibi kalır (`Claude Opus 5.5`).

### 14.2 Metin izolasyonu düzeltmesi (bölüm 6.3'ü değiştirir)
Gerekçe: Chromium, `allow-scripts` içermeyen sandbox'lı iframe'in düğümlerine ebeveynin eklediği olay dinleyicilerini çalıştırmaz (`Blocked script execution`). Kaydırma, tık ve klavye takibi bozulurdu.

Yeni kural:
- Metin okuyucudaki `<iframe>` elemanında `sandbox` özniteliği YOKTUR. Yalnız `referrerpolicy="no-referrer"` ve `title` taşır (`reader-text.js`).
- Çerçeve yanıtının (`GET /api/books/{id}/chapters/{n}`) CSP başlığında `sandbox` direktifi YOKTUR. `formats/sanitize.rs` içinde sabit tam olarak şudur:
  ```rust
  pub const FRAME_CSP: &str = "default-src 'none'; img-src 'self'; style-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'";
  ```
- Betik engeli = ammonia temizliği + CSP `default-src 'none'` (betik kaynağı hiçbir yerden yüklenemez, satır içi betik çalışmaz).
- `GET /api/books/{id}/asset` yanıtındaki `Content-Security-Policy: sandbox; default-src 'none'` KALIR (görsel doğrudan belge olarak açılırsa SVG betiği çalışmasın).
- Bölüm 6.3'te madde 2 ve madde 3 bu yeni kuralla değiştirilmiş sayılır. Madde 1 (ammonia), 4 (uygulama CSP'si), 5 (başlıklar), 6 (tıklamaların ebeveyn tarafında yakalanması) ve 7 geçerlidir.
- Ebeveyn JS, iframe'in `contentDocument`'ine (aynı kaynak) dinleyici ekler ve stil yazar. Bu artık engellenmez.
- Testler yeni başlığı birebir doğrular. `epub_sanitizes_xss` ve `reader.rs` içindeki çerçeve testleri: yanıt başlığı `FRAME_CSP` ile aynıdır ve `sandbox` alt dizgisini İÇERMEZ. Gövdede `<script`, `onerror`, `javascript:` yoktur.

### 14.3 SVG kapak sayfaları
- EPUB CSS'inin atılması kabul edilmiştir. README'ye yazılır.
- Calibre tarzı `<svg><image xlink:href="X"/></svg>` kapak sayfaları boş kalamaz. `formats/sanitize.rs` içine eklenir:
  ```rust
  pub fn svg_image_to_img(html: &str) -> String;
  ```
- Davranış:
  - `sanitize_chapter` içinde ammonia'dan ÖNCE çalışır.
  - İçinde `<image` bulunan her `<svg ...>...</svg>` bloğu, ilk `<image>` ögesinin `xlink:href` (yoksa `href`) değeriyle `<img src="X">` olarak değiştirilir.
  - Değer çift tırnaklı ya da tek tırnaklıysa tırnak çıkarılır, `"` ve `<` içeren değer `&quot;`/`&lt;` ile kaçışlanır.
  - `<image>` içermeyen `<svg>` blokları dokunulmadan kalır, ammonia kaldırır.
  - Dönüşen `<img src>` ardından ammonia `attribute_filter`'ından geçer (göreli yol `/api/books/{id}/asset?p=...` olur).
  - Manifest dışı yol `asset` uç noktasında 404 verir (mevcut kural).
- Test (`tests/formats.rs`): `epub_svg_cover_becomes_img`. `make_epub` fixture'ına `EpubOpts { svg_cover: bool }` eklenir. Sonuç HTML'inde `<svg` yok, `<img src="/api/books/{id}/asset?p=` var ve o `asset` isteği PNG baytlarını döner. Ayrıca `svg_image_to_img` için birim test (`svg_without_image_untouched`).

### 14.4 RAR fixture'ları (bölüm 11.2'yi değiştirir)
- Gerçek RAR dosyaları libarchive v3.7.7 test dosyalarından alınır (BSD lisanslı):
  - `libarchive/test/test_read_format_rar5_stored.rar.uu`
  - `libarchive/test/test_read_format_rar.rar.uu`
- İŞÇİ-2 (görev 2'ye ek olarak) şunları yapar:
  1. İndirme için `mktemp -d` ile depo DIŞINDA yeni boş bir dizin kullanır. Betiği o dizinin dışında tutar, yolları argüman olarak verir.
  2. `curl https://raw.githubusercontent.com/libarchive/libarchive/v3.7.7/libarchive/test/<ad>` ile iki `.uu` dosyasını indirir.
  3. Çözme: `python3 -I` ile yalnız stdlib. `.uu` dosyasının `begin <mod> <ad>` satırından sonrasındaki her satır `binascii.a2b_uu` ile çözülüp birleştirilir, `end` satırında durulur.
  4. Çıktılar: `tests/fixtures/rar5_stored.rar` ve `tests/fixtures/rar3.rar`. Commit edilir.
  5. `tests/fixtures/README.md`: kaynak URL'leri, etiket (`v3.7.7`) ve libarchive BSD lisans atfı.
- Beklenen üye adları ve içerikler, aynı etikette `libarchive/test/test_read_format_rar5.c` ve `libarchive/test/test_read_format_rar.c` kaynağından (`curl`) `assertEqualString`/`assertEqualMem` çağrılarından okunur ve testlere sabit olarak yazılır. Sabit bayt beklentisi yoksa test üye adı + boyut doğrular.
- `tests/formats.rs` içindeki `rar_real_fixture` testi ATLANMAZ. `rar5_stored.rar` ve `rar3.rar` için `bsdtar_list` + `bsdtar_read` ile gerçek okuma yapar. Dosya yoksa `assert!(path.exists())` ile FAIL olur.
- `comic.rs` bu testler için `pub` yapar:
  ```rust
  pub async fn bsdtar_list(path: &Path) -> Result<Vec<String>, FormatError>;
  pub async fn bsdtar_read(path: &Path, member: &str) -> Result<Vec<u8>, FormatError>;
  ```
  Bu fixture'ların üyeleri resim değildir; testler `comic_pages` değil, ham `bsdtar_list`/`bsdtar_read` çağırır.
- `make_tar_as_cbr` tabanlı testler (`rar_via_bsdtar_lists_and_reads`, `rar_timeout_kills_process`, `rar_output_cap`, `rar_glob_escape_unit`) KALIR.
- `scripts/smoke.sh` ek adımı: `tests/fixtures/rar5_stored.rar` dosyasını `books` geçici dizinine konteyner başlamadan kopyalar, sonra `docker exec smoke bsdtar -tf /books/rar5_stored.rar` çıkış kodu 0 ve çıktı boş olmamalıdır. Smoke adım 4-7 etkilenmez.

### 14.5 Parola değiştirme ve kapsam notları
**Yeni uç nokta (bölüm 5.3'e eklenir, herkes için):** `PUT /api/me/password`
- İstek: `{"current_password":"...","new_password":"..."}`. Yanıt: 204.
- `new_password` 8-128 karakter, aksi 400 `bad_request`.
- Mevcut parola yanlışsa 401 `unauthorized` ve giriş sınırlayıcıya sayılır: `limiter.check(username, ip)` önce çağrılır (kilitliyse 429), yanlışsa `limiter.fail`, doğruysa `limiter.ok`. İşleyici `ConnectInfo<SocketAddr>` alır.
- Başarıda o kullanıcının mevcut oturum DIŞINDAKİ tüm oturumları silinir: `DELETE FROM sessions WHERE user_id=? AND token_hash<>?`.
- Dosya: `src/api/auth.rs` (`change_password` işleyicisi, `route("/api/me/password", put(...))`).
- Test (`tests/auth_flow.rs`): `change_own_password`. İki oturum aç, birincide parola değiştir (204), ikinci oturum 401, birinci 200, eski parolayla giriş 401, yeni parolayla giriş 200, yanlış `current_password` 401, kısa `new_password` 400.
- Görev 7 bu uç nokta ve testi içerir.

**Arayüz (görev 15, `web/app.js`):** Üst çubukta kullanıcı adı bir düğmedir. Tıklanınca `#/hesap` açılır: "Mevcut parola", "Yeni parola" alanları ve "Parolayı değiştir" düğmesi. Başarıda toast "Parola değiştirildi", hatada API'nin Türkçe mesajı gösterilir.

**README'ye eklenir:**
- Yönetici parolasını unutursa kurtarma yolu yoktur; `DATA_DIR/kavita.db` silinerek sıfırdan kurulum yapılır.
- MOBI desteği klasik MOBI içindir. KF8-only/AZW3 dosyaları desteklenmez.
- EPUB'ın kendi CSS'i uygulanmaz.
- RAR/CBR okuma `bsdtar` ile yapılır.

Seri günü eşiği 60 sn olarak kesinleşti (bölüm 8.4).

### 14.6 Dosya adları, RAR performansı, tarayıcı testi

**UTF-8 olmayan dosya adları (`src/scan.rs`)**
- Yürüyüşte dizin girdisinin yolu `to_str()` ile çevrilir. `None` ise girdi atlanır ve `tracing::warn!("UTF-8 olmayan dosya adı atlandı: {:?}", path)` yazılır. Lossy dönüşüm YAPILMAZ. Dosya ve klasör adları için geçerlidir.
- Test (`tests/library.rs`): `non_utf8_name_skipped`. `std::os::unix::ffi::OsStrExt` ile adı `b"bozuk\xff.cbz"` olan geçerli bir CBZ yazılır. Tarama sonrası `/api/series` sonucunda yoktur, geçerli adlı ikinci CBZ vardır.

**RAR performansı (`src/formats/comic.rs`)**
`comic_read` içinde şu yorum aynen bulunur:
`// shortcut: her sayfa isteğinde arşiv baştan listelenir/açılır, büyük solid RAR'da yavaşlarsa liste önbelleği ekle`

**Tarayıcı doğrulaması: görev 19b ve CI `e2e` işi**

Dosyalar:
- `scripts/e2e/make_books.py`: yalnız Python stdlib (`zipfile`, `zlib`, `struct`, `sys`, `os`). Argüman: çıktı dizini. Üretir:
  - `Test Serisi/deneme.cbz`: 3 sayfa, 64x64 farklı düz renkli PNG, adlar `p1.png p2.png p3.png`.
  - `Test Serisi/deneme.epub`: 2 bölüm (`ch1.xhtml`, `ch2.xhtml`, her biri en az 80 paragraf), ikinci bölümde bir PNG görsel, `nav.xhtml`, `content.opf`, `META-INF/container.xml`, `mimetype` (ilk üye, sıkıştırmasız).
- `scripts/e2e/package.json`: `{"private":true,"devDependencies":{"@playwright/test":"1.55.0"}}`. `package-lock.json` yoktur. CI `npm install --no-audit --no-fund` kullanır.
- `scripts/e2e/playwright.config.mjs`: `testDir: '.'`, `testMatch: 'reader.spec.mjs'`, `use: { baseURL: 'http://127.0.0.1:3435', headless: true }`, `workers: 1`, `timeout: 90000`.
- `scripts/e2e/reader.spec.mjs`: tek `test('okuyucu uçtan uca', ...)`. `page.on('console')` ve `page.on('pageerror')` hata toplar. Adımlar:
  1. `#/kurulum` formu ile `e2e` / `e2e-pass-123` kurulumu.
  2. Arayüzde "Kütüphaneyi tara", `/api/scan-status` `running:false` olana kadar yokla. Kütüphanede `Test Serisi` kartı görünür, serinin içinde `deneme` iki kitap (`.cbz`, `.epub`) listelenir.
  3. CBZ aç (`#/oku/{id}`), `ArrowRight` ile ikinci sayfaya geç, `/api/books/{id}/progress` GET locator `"1"` olana kadar bekle, sayfayı yeniden yükle, sayfa göstergesi "2 / 3" olmalı.
  4. EPUB aç, iframe içinde `scrollingElement.scrollTop` değerini `frame.evaluate` ile yaz, `scroll` olayının `PUT progress` tetiklediğini bekle, yeniden yükle, iframe'de `scrollTop` kaydırılan değerin ±5 px'i olmalı.
  5. Konsol mesajlarında `Content Security Policy`, `Blocked script`, `Refused to` geçmez; `pageerror` sayısı 0.
  6. `/api/reading/ping` yanıtlarından en az biri 204 olmalı.
  7. `#/gecmis` sayfasında en az bir satır `deneme` içerir.
  8. `#/istatistik` sayfasında `svg` ve içinde en az bir `rect` vardır.
- Zorunlu test kancaları: `reader-comic.js` göstergeyi `id="page-indicator"` içinde "N / M" yazar. Geçmiş tablosu `id="history-table"`. Seri kartları `data-testid="series-card"`.

CI değişiklikleri (`.github/workflows/ci.yml`, görev 19b):
- `test` işine `cargo test` öncesi: `for f in web/*.js; do node --check "$f"; done`.
- Yeni iş `e2e` (`needs: image`, aynı koşul): checkout, `actions/setup-node@v4` (`node-version: 22`), `docker/setup-buildx-action@v3`, imajı `image` işiyle aynı biçimde `kavita-rs:ci` olarak yükler (`load: true`, `cache-from: type=gha`), `docker volume create e2e-data`, `mkdir /tmp/e2e-books && python3 -I scripts/e2e/make_books.py /tmp/e2e-books`, `docker run -d --name e2e -p 3435:3435 -v e2e-data:/data -v /tmp/e2e-books:/books:ro kavita-rs:ci`, `/health` bekleme döngüsü (30 sn), `cd scripts/e2e && npm install --no-audit --no-fund && npx playwright install --with-deps chromium && npx playwright test`, `if: failure()` ile `docker logs e2e` ve `actions/upload-artifact@v4` (`scripts/e2e/test-results`), `if: always()` ile `docker rm -f e2e`.
- `publish` işi `needs: [image, e2e]` olur.

### Güncellenmiş görev tablosu

| # | İşçi | Değişiklik |
|---|---|---|
| 1 | İŞÇİ-2 | Depo içi `git config user.name/user.email`. `.dockerignore` Cargo.lock'u dışlamaz |
| 2 | İŞÇİ-2 | `tests/fixtures/rar5_stored.rar`, `rar3.rar`, `README.md` eklenir (14.4). `smoke.sh` RAR5 adımıyla yazılır |
| 5 | İŞÇİ-2 | Görev 4'ten ÖNCE |
| 7 | İŞÇİ-1 | `POST /api/admin/scan` ÇIKAR. `PUT /api/me/password` + `change_own_password` EKLE |
| 10 | İŞÇİ-1 | `POST /api/admin/scan` ve scan testleri buraya (`src/api/admin.rs`, `tests/library.rs`), ayrıca `non_utf8_name_skipped` |
| 13 | İŞÇİ-1 | `unauthenticated_endpoints_return_401` 5.3'ün tamamına genişler (`/api/me/password` dahil) |
| 15 | İŞÇİ-1 | `#/hesap` parola formu, test kancaları |
| 19b | İŞÇİ-1 | `scripts/e2e/*`, CI `e2e` işi, `node --check`, `publish needs: [image, e2e]` |
| 22 | İŞÇİ-2 | 14.7'ye göre |

### 14.7 Görev 22 yeni metni
Görev 22 (İŞÇİ-2): `main` koşusunda `publish` işinin yeşil olduğunu doğrular, sonucu CEO'ya raporlar. Görünürlüğü DEĞİŞTİRMEYİ DENEMEZ.
1. `gh api /users/mustafabayramguven/packages/container/kavita-rs --jq .visibility`.
2. Anonim çekme sınaması: `curl -s "https://ghcr.io/token?scope=repository:mustafabayramguven/kavita-rs:pull"` yanıtındaki `token` ile `curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer <token>" -H "Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.v2+json" https://ghcr.io/v2/mustafabayramguven/kavita-rs/manifests/latest`. 200 ise anonim çekilebilir, 401/403/404 ise değildir.
3. Rapor: `visibility` değeri ve anonim çekme sonucu.
