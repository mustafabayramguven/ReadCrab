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

---

## 15. Material You tasarım sistemi (önceki bölümlerle çelişirse bu bölüm kazanır)

Bu bölüm 9. bölümün görsel ve etkileşim katmanını, 5.3'teki `GET /api/series` sözleşmesini ve 13/14'teki görev dağılımını değiştirir. Aşağıda "değişir" denmeyen her şey geçerlidir. Kısıtlar sabittir:
- Node derleme yoktur.
- CDN yoktur.
- `@material/web` kullanılmaz.
- Uygulama CSP'si `style-src 'self'` kalır, `style=""` özniteliği yoktur.
- `FRAME_CSP` (14.2) değişmez.
- JS yalnız CSSOM ile stil verir (`el.style.setProperty`, `el.style.x = ...`).
- Kavita logosu ve Google logosu yoktur.
- Kullanılmayan bileşen için CSS yazılmaz.

Doğrulanan sabit kaynaklar (jsDelivr üzerinden HTTP 200, 2026-11-08):

| Amaç | URL | Not |
|---|---|---|
| Latin yazı tipi | `https://cdn.jsdelivr.net/npm/@fontsource-variable/roboto-flex@5.3.0/files/roboto-flex-latin-wght-normal.woff2` | 34320 bayt |
| Latin-ext yazı tipi | `https://cdn.jsdelivr.net/npm/@fontsource-variable/roboto-flex@5.3.0/files/roboto-flex-latin-ext-wght-normal.woff2` | 23480 bayt |
| Yazı tipi lisansı | `https://cdn.jsdelivr.net/npm/@fontsource-variable/roboto-flex@5.3.0/LICENSE` | OFL |
| İkon SVG'leri | `https://cdn.jsdelivr.net/npm/@material-symbols/svg-400@0.48.3/outlined/<ad>.svg` ve `<ad>-fill.svg` | Apache-2.0, her SVG `viewBox="0 -960 960 960"` |
| e2e erişilebilirlik | npm `@axe-core/playwright` sürüm `4.11.0` | Doğrulandı |

Kullanılacak ikon adlarının hepsi `0.48.3` paketinde 200 döndü. Adlar 15.5'te listelenir.

### 15.1 Dosya değişiklikleri (bölüm 3 ağacına eklemeler)

```
web/theme.js              (YENİ) palet üretici + tema uygulayıcı
web/theme.test.mjs        (YENİ) Node birim testi, CI'de koşar, sunulmaz
web/icons.svg             (YENİ) SVG sprite
web/fonts/roboto-flex-latin.woff2
web/fonts/roboto-flex-latin-ext.woff2
web/fonts/LICENSE         OFL metni (fontsource LICENSE)
web/ICONS-LICENSE.txt     Material Symbols Apache-2.0 atfı + kaynak URL + sürüm
scripts/build_icons.py    (YENİ) icons.svg üretici (python3 -I, yalnız stdlib)
```

Statik sunum (task 4, `app.rs`):
- Yol `/static/{*ad}` olur. Tablo şu adları tanır, diğerleri 404:
  - `app.js`, `tracker.js`, `reader-comic.js`, `reader-text.js`, `stats.js`, `theme.js`
  - `style.css`, `frame.css`
  - `favicon.svg`, `icons.svg`
  - `fonts/roboto-flex-latin.woff2`, `fonts/roboto-flex-latin-ext.woff2`
- Gömme `include_str!` ile yapılır. Yazı tipi dosyaları `include_bytes!` ile gömülür.
- İçerik türleri:
  - `application/javascript; charset=utf-8`
  - `text/css; charset=utf-8`
  - `image/svg+xml`
  - `font/woff2`
- `Cache-Control`: JS ve CSS için `no-cache`. `icons.svg`, `favicon.svg` ve yazı tipleri için `public, max-age=86400`.
- Uygulama CSP'si (6.3 madde 4) değişmez. `font-src 'self'` zaten `default-src 'self'` ile gelir. Ayrı bir direktif eklenmez.
- `web/theme.test.mjs` ve `web/fonts/LICENSE` sunulmaz.

`web/index.html` kabuğu (task 20a son halini verir):
- `<head>` içinde sıra: `<meta charset>`, viewport, `<meta name="color-scheme" content="light dark">`, `<meta name="theme-color">`, `<link rel="icon" href="/static/favicon.svg">`, `<link rel="stylesheet" href="/static/style.css">`, ardından senkron (`defer`/`async` YOK) `<script src="/static/theme.js"></script>`. Bu sıra ilk boyamadan önce paleti yazar, FOUC olmaz.
- Gövde sonunda sıra: `tracker.js`, `reader-comic.js`, `reader-text.js`, `stats.js`, `app.js`.
- `<noscript>` içinde Türkçe bir satır bulunur: "Bu uygulama için JavaScript gerekir."

### 15.2 Token sistemi (`web/style.css` başı)

Adlandırma M3'tür. Değerler `:root` altında tanımlanır.

**Renk rolleri** (hepsi `--md-sys-color-<rol>`). `theme.js` bunların hepsini `documentElement.style.setProperty` ile yazar:
- `primary`, `on-primary`, `primary-container`, `on-primary-container`
- `secondary`, `on-secondary`, `secondary-container`, `on-secondary-container`
- `tertiary`, `on-tertiary`, `tertiary-container`, `on-tertiary-container`
- `error`, `on-error`, `error-container`, `on-error-container`
- `surface`, `surface-dim`, `surface-bright`
- `surface-container-lowest`, `surface-container-low`, `surface-container`, `surface-container-high`, `surface-container-highest`
- `on-surface`, `on-surface-variant`, `outline`, `outline-variant`
- `inverse-surface`, `inverse-on-surface`, `inverse-primary`
- `scrim` (her zaman `#000000`)

**JS'siz yedek değerler** (M3 taban çizgisi, mor tohum `#6750A4`). `:root` açık değerleri taşır:
- `@media (prefers-color-scheme: dark)` içinde `:root:not([data-theme="light"])` koyu değerleri taşır.
- `:root[data-theme="dark"]` aynı koyu değerleri taşır.
- `theme.js` çalışınca satır içi (inline) değerler bunları geçersiz kılar.

| Rol | Açık | Koyu |
|---|---|---|
| primary | #6750A4 | #D0BCFF |
| on-primary | #FFFFFF | #381E72 |
| primary-container | #EADDFF | #4F378B |
| on-primary-container | #21005D | #EADDFF |
| secondary | #625B71 | #CCC2DC |
| on-secondary | #FFFFFF | #332D41 |
| secondary-container | #E8DEF8 | #4A4458 |
| on-secondary-container | #1D192B | #E8DEF8 |
| tertiary | #7D5260 | #EFB8C8 |
| on-tertiary | #FFFFFF | #492532 |
| tertiary-container | #FFD8E4 | #633B48 |
| on-tertiary-container | #31111D | #FFD8E4 |
| error | #B3261E | #F2B8B5 |
| on-error | #FFFFFF | #601410 |
| error-container | #F9DEDC | #8C1D18 |
| on-error-container | #410E0B | #F9DEDC |
| surface | #FEF7FF | #141218 |
| surface-dim | #DED8E1 | #141218 |
| surface-bright | #FEF7FF | #3B383E |
| surface-container-lowest | #FFFFFF | #0F0D13 |
| surface-container-low | #F7F2FA | #1D1B20 |
| surface-container | #F3EDF7 | #211F26 |
| surface-container-high | #ECE6F0 | #2B2930 |
| surface-container-highest | #E6E0E9 | #36343B |
| on-surface | #1D1B20 | #E6E0E9 |
| on-surface-variant | #49454F | #CAC4D0 |
| outline | #79747E | #938F99 |
| outline-variant | #CAC4D0 | #49454F |
| inverse-surface | #322F35 | #E6E0E9 |
| inverse-on-surface | #F5EFF7 | #322F35 |
| inverse-primary | #D0BCFF | #6750A4 |
| scrim | #000000 | #000000 |

**Tipografi.** Aile `--md-ref-typeface-brand: "Roboto Flex", system-ui, -apple-system, "Segoe UI", Roboto, sans-serif`. Her stil için `--md-sys-typescale-<rol>-<boyut>-{size,line,weight,tracking}` değişkenleri tanımlanır. Boyut ve satır yüksekliği px, ağırlık sayı, tracking px:

| Rol | Boyut / satır / ağırlık / tracking |
|---|---|
| display-large | 57/64/400/-0.25 |
| display-medium | 45/52/400/0 |
| display-small | 36/44/400/0 |
| headline-large | 32/40/400/0 |
| headline-medium | 28/36/400/0 |
| headline-small | 24/32/400/0 |
| title-large | 22/28/400/0 |
| title-medium | 16/24/500/0.15 |
| title-small | 14/20/500/0.1 |
| body-large | 16/24/400/0.5 |
| body-medium | 14/20/400/0.25 |
| body-small | 12/16/400/0.4 |
| label-large | 14/20/500/0.1 |
| label-medium | 12/16/500/0.5 |
| label-small | 11/16/500/0.5 |

Her stil için yardımcı sınıf bulunur (`.type-body-large` vb.). Yalnız kullanılanlar yazılır.

`@font-face` (aile `"Roboto Flex"`, `font-weight: 100 1000`, `font-display: swap`):
- latin: `/static/fonts/roboto-flex-latin.woff2`, `unicode-range: U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD`.
- latin-ext: `/static/fonts/roboto-flex-latin-ext.woff2`, `unicode-range: U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF`.
- Türkçe `ğ Ğ ş Ş İ ı ç ö ü` bu iki alt kümeyle karşılanır.

**Şekil** (`--md-sys-shape-corner-*`):

| Ad | Değer |
|---|---|
| none | 0 |
| extra-small | 4px |
| small | 8px |
| medium | 12px |
| large | 16px |
| extra-large | 28px |
| full | 9999px |

**Yükselti** (`--md-sys-elevation-level0..3`):
- level0: `none`
- level1: `0 1px 2px rgb(0 0 0 / .30), 0 1px 3px 1px rgb(0 0 0 / .15)`
- level2: `0 1px 2px rgb(0 0 0 / .30), 0 2px 6px 2px rgb(0 0 0 / .15)`
- level3: `0 1px 3px rgb(0 0 0 / .30), 0 4px 8px 3px rgb(0 0 0 / .15)`

**Hareket:**
- Easing:
  - `--md-sys-motion-easing-emphasized: cubic-bezier(.2,0,0,1)`
  - `-emphasized-decelerate: cubic-bezier(.05,.7,.1,1)`
  - `-emphasized-accelerate: cubic-bezier(.3,0,.8,.15)`
  - `-standard: cubic-bezier(.2,0,0,1)`
- Süre (ms): `short1` 50, `short2` 100, `short3` 150, `short4` 200, `medium1` 250, `medium2` 300, `medium3` 350, `medium4` 400, `long1` 450, `long2` 500.
- `@media (prefers-reduced-motion: reduce)` ve `html[data-motion="reduced"]` içinde:
  - Tüm süre değişkenleri `0ms` olur.
  - İskelet ve belirsiz (indeterminate) progress animasyonları `animation: none` olur.
  - Dalga (ripple) üretilmez.

**Durum katmanı (state layer):** Ortak seçici listesi `.btn, .icon-btn, .fab, .chip, .list-item, .nav-item, .card, .seg-btn`. Bu elemanlar `position: relative; overflow: hidden` olur ve `::before { content:""; position:absolute; inset:0; background: currentColor; opacity:0; border-radius: inherit; pointer-events:none; transition: opacity var(--md-sys-motion-duration-short2) }` kuralını alır.
- `:hover::before` opaklığı `.08`
- `:focus-visible::before` opaklığı `.10`
- `:active::before` opaklığı `.10`

Görünür odak halkası tüm etkileşimli elemanlarda `:focus-visible { outline: 3px solid var(--md-sys-color-secondary); outline-offset: 2px }` olur. Yalnız `:focus` ile halka çizilmez.

### 15.3 `web/theme.js`: palet üretimi (kesin formül)

Dosya klasik betiktir (modül değil). Başlık yorumu: `// shortcut: HCT yerine OKLCH, renkler resmi M3 üreticisinden birebir değil`. Sonda `window.KRS_THEME = {...}` atanır ve `if (typeof module !== 'undefined') module.exports = window_or_local_obj` satırı vardır. Böylece Node testi `createRequire(import.meta.url)('./theme.js')` ile yükleyebilir. `window` yoksa (Node) `globalThis` kontrolü hata vermez.

**Dönüşümler** (Ottosson OKLab, sabitler aynen):
- sRGB aktarımı: `lin = c <= 0.04045 ? c/12.92 : ((c+0.055)/1.055)**2.4`; ters yönde `c <= 0.0031308 ? 12.92*c : 1.055*c**(1/2.4) - 0.055`.
- Lineer sRGB → OKLab:
  - `l = 0.4122214708 r + 0.5363325363 g + 0.0514459929 b`
  - `m = 0.2119034982 r + 0.6806995451 g + 0.1073969566 b`
  - `s = 0.0883024619 r + 0.2817188376 g + 0.6299787005 b`
  - `l_,m_,s_ = küp kök`
  - `L = 0.2104542553 l_ + 0.7936177850 m_ - 0.0040720468 s_`
  - `a = 1.9779984951 l_ - 2.4285922050 m_ + 0.4505937099 s_`
  - `b = 0.0259040371 l_ + 0.7827717662 m_ - 0.8086757660 s_`
- OKLab → lineer sRGB:
  - `l_ = L + 0.3963377774 a + 0.2158037573 b`
  - `m_ = L - 0.1055613458 a - 0.0638541728 b`
  - `s_ = L - 0.0894841775 a - 1.2914855480 b`
  - Üçü küp alınır (`l,m,s`).
  - `r = 4.0767416621 l - 3.3077115913 m + 0.2309699292 s`
  - `g = -1.2684380046 l + 2.6097574011 m - 0.3413193965 s`
  - `b = -0.0041960863 l - 0.7034186147 m + 1.7076147010 s`
- OKLCH: `C = hypot(a,b)`, `h = atan2(b,a)` derece cinsinden 0..360.

**İşlevler:**
```js
function toneToY(tone)        // tone 0..100 → Y: t>8 ? ((t+16)/116)**3 : t/903.3   (CIE L*)
function luminance(hex)       // WCAG göreli parlaklık (0.2126 R + 0.7152 G + 0.0722 B, lineer)
function contrast(hexA, hexB) // (Ymax+0.05)/(Ymin+0.05)
function hexToOklch(hex)      // → {L,C,h}
function maxChroma(L, h, C)   // sığma: C'den başlayıp 24 adım ikili aramayla gamut içi en büyük c ≤ C
function colorAtTone(h, C, tone)  // → '#rrggbb'
function generatePalette(seedHex, dark)  // → { 'primary': '#..', ... }
function readerColors(name, seedHex)     // name: 'light'|'dark'|'sepia' → {bg,fg,link}
function apply(seedHex, mode)    // mode: 'auto'|'light'|'dark'
function init()                  // localStorage'ı okur, apply eder, auto için matchMedia dinler
```

**`colorAtTone(h, C, tone)`** (luminans eşleme, kontrast garantisi için):
1. `Yt = toneToY(tone)`. `Yt` ≤ 0.0001 ise `#000000`, ≥ 0.9999 ise `#ffffff` döner.
2. `L = cbrt(Yt)`.
3. En çok 12 kez tekrarla: `c = maxChroma(L, h, C)`, lineer sRGB hesapla ve [0,1]'e kırp, `Y = 0.2126 r + 0.7152 g + 0.0722 b`. `|Y − Yt| < 1e-4` ise dur. Aksi halde `L = clamp(L * cbrt(Yt / max(Y,1e-6)), 0, 1)`.
4. Gamma uygula, `round(c*255)` ile onaltılık yaz.

Bu yöntem her tonun gerçek parlaklığını M3 tonuna eşitler, bu yüzden kontrast oranları M3 ton farklarıyla öngörülür.

**Palet parametreleri** (kaynak renk `hexToOklch` → `{L0,C0,h0}`):
- `Cp = clamp(C0, 0.05, 0.17)`
- primary paleti: `(h0, Cp)`
- secondary paleti: `(h0, max(0.02, Cp*0.33))`
- tertiary paleti: `((h0 + 60) % 360, max(0.04, Cp*0.55))`
- neutral: `(h0, 0.006)`
- neutral-variant: `(h0, 0.012)`
- error paleti: `(29.2, 0.19)` (sabit, gamut sınırlar)

**Rol → palet/ton** (`tone(p,t) = colorAtTone(h_p, C_p, t)`):

| Rol | Açık | Koyu |
|---|---|---|
| primary | P40 | P80 |
| on-primary | P100 | P20 |
| primary-container | P90 | P30 |
| on-primary-container | P10 | P90 |
| secondary / on-secondary / secondary-container / on-secondary-container | S40 / S100 / S90 / S10 | S80 / S20 / S30 / S90 |
| tertiary ailesi | T40 / T100 / T90 / T10 | T80 / T20 / T30 / T90 |
| error ailesi | E40 / E100 / E90 / E10 | E80 / E20 / E30 / E90 |
| surface | N98 | N6 |
| surface-dim | N87 | N6 |
| surface-bright | N98 | N24 |
| surface-container-lowest | N100 | N4 |
| surface-container-low | N96 | N10 |
| surface-container | N94 | N12 |
| surface-container-high | N92 | N17 |
| surface-container-highest | N90 | N22 |
| on-surface | N10 | N90 |
| on-surface-variant | NV30 | NV80 |
| outline | NV50 | NV60 |
| outline-variant | NV80 | NV30 |
| inverse-surface | N20 | N90 |
| inverse-on-surface | N95 | N20 |
| inverse-primary | P80 | P40 |
| scrim | #000000 | #000000 |

(P/S/T/E/N/NV sırasıyla primary, secondary, tertiary, error, neutral, neutral-variant.)

**Hazır kaynak renkler** (`SEEDS`, bu sırayla):

| Ad | Hex |
|---|---|
| Mor | #6750A4 |
| Mavi | #0B57D0 |
| Yeşil | #2E7D32 |
| Turuncu | #C25E00 |
| Pembe | #B3265F |
| Turkuaz | #00796B |

Varsayılan Mor, mod `auto`.

**`apply(seedHex, mode)`:**
1. `dark = mode==='dark' || (mode==='auto' && matchMedia('(prefers-color-scheme: dark)').matches)`.
2. `generatePalette(seedHex, dark)` sonucunu `documentElement.style.setProperty('--md-sys-color-' + rol, hex)` ile yazar.
3. `documentElement.dataset.theme = dark ? 'dark' : 'light'`.
4. `documentElement.style.colorScheme = dark ? 'dark' : 'light'`.
5. `<meta name="theme-color">` içeriğini `surface` rengine ayarlar.

**Saklama ve başlatma:** `localStorage['krs.seed']` (`#rrggbb`, geçersizse Mor) ve `localStorage['krs.mode']` (`auto|light|dark`, geçersizse `auto`). Sunucuya hiçbir şey gönderilmez. `init()` mod `auto` iken `matchMedia(...).addEventListener('change', ...)` ile yeniden uygular. Hareket anahtarı `localStorage['krs.motion']` (`reduced` ise `documentElement.dataset.motion='reduced'`) `init()` içinde okunur. `window.KRS_THEME` ayrıca `setSeed(hex)`, `setMode(m)`, `setMotion(bool)` işlevlerini dışa verir. Bunlar localStorage'a yazar ve `apply` eder.

**`readerColors(name, seedHex)`** (metin okuyucu için, uygulama modundan bağımsız):
- `'light'`: `generatePalette(seed,false)` içinden `bg=surface`, `fg=on-surface`, `link=primary`.
- `'dark'`: `generatePalette(seed,true)` içinden aynı üç rol.
- `'sepia'`: tohumdan bağımsız sabit `bg=#F4ECD8`, `fg=#433422`, `link=#7A4A12`.

### 15.4 `web/theme.test.mjs` (CI'de `node web/theme.test.mjs`)

Yalnız `node:assert/strict` ve `node:module` kullanılır. Bağımlılık yoktur. Başarısızlıkta `process.exit(1)`.

Testler:
1. `roundtrip`: 20 sabit örnek için `hexToOklch → colorAtTone` değil, saf dönüşüm `oklchToHex(hexToOklch(x)) === x` (±1 kanal toleransı).
2. `tone_luminance`: 6 tohum × `[10,40,80,90]` tonu için `|luminance(colorAtTone(h,Cp,t)) − toneToY(t)| < 0.01`.
3. `palette_shape`: her tohum ve mod için 15.3 tablosundaki tüm roller var ve hepsi `/^#[0-9a-f]{6}$/`.
4. `contrast_pairs`: 6 tohum × açık/koyu için şu çiftlerin hepsinde `contrast ≥ 4.5`:
   - `on-primary/primary`, `on-primary-container/primary-container`
   - `on-secondary/secondary`, `on-secondary-container/secondary-container`
   - `on-tertiary/tertiary`, `on-tertiary-container/tertiary-container`
   - `on-error/error`, `on-error-container/error-container`
   - `on-surface/surface`, `on-surface/surface-dim`, `on-surface/surface-bright`
   - `on-surface/` her biri için `surface-container-lowest`, `-low`, `surface-container`, `-high`, `-highest`
   - `on-surface-variant/surface`, `on-surface-variant/surface-container-highest`
   - `inverse-on-surface/inverse-surface`, `inverse-primary/inverse-surface`
5. `nontext_pairs`: aynı 12 kombinasyonda `outline/surface ≥ 3` ve `primary/surface ≥ 3`.
6. `reader_colors`: `light`, `dark`, `sepia` × 6 tohum için `fg/bg ≥ 7` ve `link/bg ≥ 4.5`.

CI (`.github/workflows/ci.yml`, `test` işi): 14.6 E'deki `node --check` döngüsünün hemen altına `node web/theme.test.mjs` adımı eklenir. Node `ubuntu-latest` üzerinde hazırdır. Kurulum yoktur.

### 15.5 İkonlar

`scripts/build_icons.py` (yalnız stdlib: `urllib.request`, `re`, `os`, `sys`, `tempfile`; `python3 -I` ile çalışır):
- Her ad için `https://cdn.jsdelivr.net/npm/@material-symbols/svg-400@0.48.3/outlined/<ad>.svg` indirir.
- Çıktı: `web/icons.svg`, kökü `<svg xmlns="http://www.w3.org/2000/svg">`, her ikon `<symbol id="<ad>" viewBox="0 -960 960 960"><path d="..."/></symbol>`.
- `d` değeri `re.search(r'<path d="([^"]+)"')` ile alınır, tek `<path>` olmayan SVG'de betik hata verip durur.
- İndirilen ham dosyalar `tempfile.mkdtemp()` ile yeni boş dizine yazılır, depoya girmez.
- Betik depoya commit edilir. Betiğin docstring'inde sürüm ve lisans (`Apache-2.0`) yazar.

İkon listesi (hepsi doğrulandı):
- Gezinti: `menu_book`, `history`, `bar_chart`, `person`, `admin_panel_settings`, ve aktif durum için `menu_book-fill`, `history-fill`, `bar_chart-fill`, `person-fill`, `admin_panel_settings-fill`.
- Genel: `search`, `close`, `arrow_back`, `arrow_forward`, `chevron_left`, `chevron_right`, `add`, `delete`, `check`, `check_circle`, `logout`, `lock`, `error`, `sync`, `sort`, `more_vert`, `keyboard_arrow_up`.
- Okuyucu: `format_list_bulleted`, `text_fields`, `tune`, `fit_screen`, `fullscreen`, `auto_stories`.
- Tema: `light_mode`, `dark_mode`, `palette`.
- İstatistik: `schedule`, `flag`, `local_fire_department`.

Kullanım: `<svg class="icon" aria-hidden="true"><use href="/static/icons.svg#menu_book"/></svg>`. `.icon { width: 24px; height: 24px; fill: currentColor; flex: none }`. İkon yazı tipi yoktur.

`web/ICONS-LICENSE.txt`: "Material Symbols, Google, Apache License 2.0, https://github.com/google/material-design-icons, paket `@material-symbols/svg-400@0.48.3`" ve Apache-2.0 sürüm bağlantısı.

### 15.6 Düzen ve bileşenler (kesin sınıf adları)

Sınıf adları görevler arasında sözleşmedir. İşçiler başka ad uydurmaz.

**Kabuk:**
- `body > .app-shell` (grid). `>= 840px`: sol `.nav-rail` (genişlik 80px, `surface`, öğeler `.nav-item` dikey ikon+etiket, aktif öğe 56x32 `secondary-container` hap göstergesi ve `-fill` ikon, etiket `label-medium`).
- `< 840px`: altta `.nav-bar` (yükseklik 80px, `surface-container`, aynı `.nav-item`, 3-5 öğe).
- Öğeler: Kütüphane (`menu_book`, `#/`), Geçmiş (`history`, `#/gecmis`), İstatistik (`bar_chart`, `#/istatistik`), Hesap (`person`, `#/hesap`), Yönetim (`admin_panel_settings`, `#/yonetim`, yalnız admin). Aktif öğede `aria-current="page"`.
- Üst: `.top-app-bar` (yükseklik 64px, başlık `title-large`, sağda `.icon-btn`'lar). Kaydırınca `surface-container` arka planına geçer (`scroll` olayı + sınıf `.scrolled`).
- İçerik `main.page` (en çok 1200px, 16px kenar boşluğu, ≥840px'te 24px).
- Okuyucuda (`#/oku/...`) `body.reading` sınıfı rail/bar/top bar'ı gizler.

**Kütüphane sayfası (`#/`):**
- `.search-bar`: yükseklik 56px, `surface-container-high`, tam yuvarlak, solda `search` ikonu, `<input type="search" aria-label="Kütüphanede ara">`, doluyken sağda `close` `.icon-btn`. 250 ms debounce ile `q` gönderir.
- Altında iki filter chip grubu (`.chip-group[role="radiogroup"]`, her `.chip[role="radio"][aria-checked]`, seçili iken `secondary-container` ve `check` ikonu):
  - Sıralama: `Ad` (`name`), `Eklenme` (`added`), `Son okunan` (`recent`).
  - Durum: `Tümü`, `Okunuyor`, `Bitti`, `Okunmadı`.
- Kapak ızgarası `.grid`: `grid-template-columns: repeat(auto-fill, minmax(150px, 1fr))`, `>=840px`'te `minmax(180px, 1fr)`, boşluk 16px (`>=840px` 24px).
- `.card` (`surface-container-low`, radius `medium`, `data-testid="series-card"`, bir `<a>`): üstte `.cover` (`aspect-ratio: 2/3`, `<img loading="lazy" alt="">`, kapak 404 ise harf yer tutucu `div.cover-fallback` `primary-container` zemin, `on-primary-container` metin, başlığın ilk harfi). Cover üstünde alt kenarda 4px `.progress-linear` (okuma ilerlemesi), sağ üstte `.badge` (durum: `Bitti` ya da `Okunuyor`, `tertiary-container`). Altında başlık (`title-small`, en çok 2 satır) ve `body-small` "N cilt".
- `.fab` (yalnız admin): 56x56, radius `large`, `primary-container`, `sync` ikonu, `aria-label="Kütüphaneyi tara"`. Konum sağ alt, `<840px`'te nav bar'ın 16px üstü. Tarama çalışırken ikon yerine `.progress-circular` gösterir (`done/total` belirlenimli) ve `aria-busy="true"`, tıklama devre dışı. Bitişte `KRS.toast("Tarama tamamlandı")`.

**`GET /api/series` sözleşme değişikliği (bölüm 5.3'ü değiştirir, task 11):**
- Yeni sorgu parametresi `status=all|reading|finished|unread` (varsayılan `all`). Geçersizse 400 `bad_request`.
- Yanıt öğesine iki alan eklenir: `reading_count` (serideki `status='reading'` görünür kitap sayısı) ve `percent` (görünür kitapların ortalaması; her kitap için `finished` ise 1, değilse `COALESCE(progress.percent,0)`). Bunlar çağıran kullanıcının ilerlemesinden hesaplanır.
- Filtre:
  - `reading` = `reading_count>0 OR (read_count>0 AND read_count<book_count)`
  - `finished` = `read_count=book_count`
  - `unread` = `read_count=0 AND reading_count=0`
- Rozet: `read_count=book_count` ise `Bitti`, `reading` koşulu doğruysa `Okunuyor`, aksi halde yok.
- Test (`tests/library.rs`): `series_status_filter` (üç durumda doğru serileri döner, geçersiz değer 400, `percent` değerleri beklenen ortalama).

**Seri sayfası (`#/seri/{id}`):**
- Üst `.top-app-bar` geri oku (`arrow_back`) ve seri adı.
- Kitaplar `.list` içinde `.list-item` olarak: soldaki 56x84 küçük kapak, `title-medium` başlık, `body-small` "Biçim · N sayfa/bölüm", sağda durum chip'i. Satırın altında `.progress-linear`.
- Satıra tıklama okuyucuyu açar. Sağdaki durum `.icon-btn` (`more_vert`) küçük menü yerine bir `dialog` açar: dört durum `radio` ile ("Okunmadı", "Okunuyor", "Bitti", "Bırakıldı") ve "Kaydet" `.btn.filled`; `PUT /api/books/{id}/status` çağrılır.
- `error` dolu kitapta satır soluk, metin "DRM korumalı" ya da "Bozuk dosya", tıklama devre dışı.

**Düğmeler:** `.btn` (yükseklik 40px, radius `full`, yatay padding 24px, `label-large`). Varyantlar `.btn.filled` (`primary`/`on-primary`), `.btn.tonal` (`secondary-container`/`on-secondary-container`), `.btn.outlined` (1px `outline`, metin `primary`), `.btn.text` (metin `primary`, padding 12px). `.icon-btn` (48x48, radius `full`, ikon 24px).

**Metin alanı:** `.field` içinde `<input placeholder=" ">` ve `<label>`. Outlined: 56px yükseklik, 1px `outline`, radius `extra-small`. Odakta 2px `primary`, etiket `:focus ~ label, :not(:placeholder-shown) ~ label` ile `top` ve `font-size` değişir (label-small, `surface` zeminli kesinti). Hata: `aria-invalid="true"` ile `error` rengi; altında `.field-help` (`body-small`) metni.

**Diyalog:** `<dialog class="md-dialog">`, `surface-container-high`, radius `extra-large`, padding 24px, genişlik `min(560px, calc(100vw - 48px))`, `::backdrop` `color-mix(in srgb, var(--md-sys-color-scrim) 32%, transparent)`. Başlık `headline-small`, gövde `body-medium`, alt satırda `.btn.text` düğmeleri sağa dayalı. Açılış `opacity`+`translateY(16px)` geçişi `medium2` ve `emphasized-decelerate`.

**Snackbar:** tek `div.snackbar[role="status"][aria-live="polite"]`, `inverse-surface`/`inverse-on-surface`, radius `extra-small`, yükseklik 48px, alt orta (`<840px`'te nav bar'ın 16px üstü). `KRS.toast(msg, opts)` imzası `KRS.toast(msg:string, opts?:{action?:string,onAction?:()=>void})`. Eylem düğmesi `.btn.text` rengi `inverse-primary`. Süre 4000 ms, yeni çağrı eskisini değiştirir.

**Progress:**
- `.progress-linear` 4px, iz `surface-container-highest`, dolgu `primary`, `role="progressbar"` `aria-valuenow`. Dolgu genişliği `style.width` CSSOM ile.
- `.progress-circular`: 48px SVG, `r=18`, iz `secondary-container`, ark `primary`, `stroke-dasharray`/`stroke-dashoffset` SVG öznitelikleriyle. Belirsiz mod CSS `rotate` animasyonu.

**Segmented düğme:** `.segmented[role="radiogroup"]` içinde `.seg-btn[role="radio"][aria-checked]`. Yükseklik 40px, 1px `outline`, seçili `secondary-container` ve `check` ikonu.

**Slider:** `<input type="range" class="slider">` özel CSS ile (iz 4px, `primary` dolgusu için `linear-gradient` `--val` değişkeniyle, JS `style.setProperty('--val', '%')`, tutamaç 20px daire `primary`). `aria-valuetext` "NN px".

**Switch:** `<input type="checkbox" role="switch" class="switch">`, 52x32 iz, açıkken `primary` zemin ve `on-primary` tutamaç, kapalıyken `surface-container-highest` zemin, 2px `outline` kenar. Yalnız Hesap sayfasında "Hareketi azalt" için kullanılır.

**İskelet ve boş durumlar:**
- `.skeleton`: `surface-container-high`→`surface-container-highest` gradyanı, 1.4s `shimmer` animasyonu. Liste için 6 kart/satır iskeleti gösterilir.
- `.empty`: ortalı sütun, 48px ikon, `title-large` başlık, `body-medium` metin, isteğe bağlı `.btn.tonal`. Mesajlar:
  - Kütüphane boş: "Kütüphane boş" / "/books klasörüne seri klasörleri ekleyin ve ardından tarayın." Admin için "Kütüphaneyi tara" düğmesi.
  - Arama sonucu yok: "Sonuç bulunamadı" / "Başka bir arama deneyin."
  - Geçmiş boş: "Henüz okuma kaydı yok".
  - Hata durumu (`.empty.error`, `error` ikonu): "Yüklenemedi" / API mesajı + "Yeniden dene" `.btn.tonal`.

**Dalga:** `app.js` içinde tek belge düzeyi `pointerdown` işleyicisi; hedef, 15.2'deki state layer seçici listesine uyan en yakın elemanın içinde `span.ripple` oluşturur. Boyut ve konum CSSOM ile (`style.width/height/left/top`), `background: currentColor`, `opacity .10`, `transform: scale(0)→scale(1)` animasyonu `long1` + `emphasized`, `animationend`'de silinir. `prefers-reduced-motion` ya da `data-motion=reduced` iken üretilmez.

**Onay yardımcısı:** `KRS.confirm(title:string, text:string, okLabel:string): Promise<boolean>` bir `.md-dialog` açıp kapanınca çözülür. Kullanıcı silme ve benzeri yerlerde kullanılır.

**Hesap sayfası (`#/hesap`):** Kartlar `surface-container-low` içinde üç bölüm:
1. Görünüm:
   - Tema modu `.segmented` ("Otomatik", "Açık", "Koyu"; `KRS_THEME.setMode`).
   - Kaynak renk: altı `.swatch` (`button[role="radio"]`, 48px daire, `style.background` CSSOM ile tohum rengi, seçiliyken `check` ikonu), yanında `<input type="color" aria-label="Özel renk">`. Seçim anında `KRS_THEME.setSeed`.
   - "Hareketi azalt" `switch`.
2. Parola değiştir (14.5 formu; `.field` x2, `.btn.filled`).
3. Çıkış `.btn.outlined`.

**Yönetim sayfası:** Kullanıcılar `.list` içinde. Her satırda avatar daire (`primary-container`, ilk harf), ad, "Yönetici" ya da "Kullanıcı" rozeti, sağda "Parolayı sıfırla" (`lock` `.icon-btn`) ve "Sil" (`delete` `.icon-btn`, `KRS.confirm` ile). "Kullanıcı ekle" `.btn.filled` bir dialog açar. Admin kendi satırında silme düğmesi yok.

### 15.7 Okuyucu (sürükleyici)

Ortak:
- `.reader` kökü `position: fixed; inset: 0` (`body.reading` iken). Arka plan: görsel okuyucuda `#000000` zemin yerine `surface-container-lowest`'in koyu üretimi `generatePalette(seed,true)['surface-container-lowest']` (görsel sayfalar için karanlık zemin); metin okuyucuda seçili okuma temasının `bg` değeri.
- `.reader-bar.top` (geri oku `arrow_back`, kitap başlığı `title-medium` tek satır, sağda ayar `tune` `.icon-btn`) ve `.reader-bar.bottom` (ilerleme `.progress-linear` + metin "3 / 12" ya da "%42", solda `format_list_bulleted` yalnız metin okuyucuda, sağda `chevron_left`/`chevron_right` `.icon-btn`). Çubuk zeminleri `color-mix(in srgb, var(--md-sys-color-surface) 92%, transparent)`.
- Çubuklar son kullanıcı girdisinden (`pointermove`, `pointerdown`, `keydown`, `touchstart`, `scroll`) 3000 ms sonra gizlenir: `.reader-bar.hidden` (`opacity: 0; transform: translateY(∓100%)`, geçişin sonunda `visibility: hidden`). `pointermove`, dokunuş ve herhangi bir `keydown` çubukları geri getirir. Odak çubuğun içindeyse ya da alt sayfa açıksa gizlenmez.
- Alt sayfa: `<dialog class="sheet">`, `showModal()`, alttan kayar (`translateY(100%)→0`, `medium3`, `emphasized-decelerate`), `surface-container-low`, üst köşeler `extra-large`, üstte 32x4 tutamaç çubuğu, yükseklik en çok `70vh`, `>=840px`'te genişlik 480px ortalı. İçerik kaydırılabilir. `Esc` ve arka plana tıklama kapatır.
- Sayfa/bölüm geçişi: yeni içerik `opacity 0→1` `short4`, `standard`. `reduced motion` iken yok.

Görsel okuyucu (`reader-comic.js`):
- Alt sayfa "Ayarlar": sığdırma `.segmented` (Genişliğe sığdır, Yüksekliğe sığdır, Sayfaya sığdır, Gerçek boyut). `f` tuşu da döner.
- Dokunma bölgeleri: sol üçte bir önceki, sağ üçte bir sonraki, orta üçte bir çubukları aç/kapat.
- `#page-indicator` metni "N / M" alt çubukta (14.6 E kancası).

Metin okuyucu (`reader-text.js`):
- Alt sayfa "İçindekiler": `.list` içinde bölümler, geçerli olan `secondary-container`.
- Alt sayfa "Ayarlar": tema `.segmented` (Açık, Koyu, Sepya), yazı boyutu `.slider` (14-28 px, adım 2) ve yanında `A-`/`A+` `.icon-btn`.
- Tema değerleri `KRS_THEME.readerColors(name, seed)` ile alınır ve çerçeveye `iframe.contentDocument.documentElement.style.setProperty('--bg'|'--fg'|'--link'|'--fs', ...)` ile yazılır. Yazı tipi: `--font: Georgia, "Noto Serif", "DejaVu Serif", serif` (sistem yığını, `font-src` izni gerekmez).
- `web/frame.css`: `html { background: var(--bg); color: var(--fg); font-size: var(--fs, 18px); font-family: var(--font) }`, `body { max-width: 38rem; margin: 0 auto; padding: 24px 20px 96px; line-height: 1.7 }`, `img { max-width: 100%; height: auto }`, `a { color: var(--link) }`, `h1,h2,h3 { line-height: 1.25 }`. Harici kaynak yok.
- Çubuk gizleme ebeveyn belge girdi olaylarına ek olarak çerçeve belgesindeki `scroll`, `pointermove` ve `click` olaylarını da dinler (14.2 sonrası aynı kaynak olduğundan engellenmez).

### 15.8 İstatistik, geçmiş (güncellenen sayfa tasarımı)

- `#/istatistik`:
  - Üstte `.stat-card` ızgarası (5 kart: toplam süre, toplam sayfa, tamamlanan kitap, güncel seri, en uzun seri), kartlar `surface-container-low`, sayı `headline-medium`, etiket `label-medium`, ikon `schedule`/`auto_stories`/`check_circle`/`local_fire_department`/`local_fire_department`.
  - İki SVG sütun grafiği (30 gün: dakika ve sayfa) `.chart-card` içinde. Çubuklar `fill: var(--md-sys-color-primary)` (CSS sınıfı `.bar`), bugün çubuğu `tertiary`, eksen metinleri `on-surface-variant`, kılavuz çizgileri `outline-variant`. Her çubukta `<title>` ("12 Kas: 34 dk"). Grafik `role="img"` ve `aria-label` özeti ("Son 30 günde günlük okuma süresi").
  - Yıllık hedef kartı: `.progress-circular` (72px, `done/target`), ortada "3/12", altında `flag` ikonu, `.field` (hedef sayısı) ve `.btn.filled` "Hedefi kaydet". Hedef yoksa metin "Bu yıl için hedef belirleyin".
- `#/gecmis`: `<ul id="history-table" class="list">` (14.6 E kancası burada `<ul>`'dur, `<table>` değildir). Her `li.list-item`: ilk satır kitap başlığı (`title-medium`), ikinci satır `body-small` "Seri · tarih · süre · sayfa/yüzde", silinmiş kitapta başlığın başında "(silinmiş)" rozeti. Altta "Daha fazla" `.btn.tonal`. Boş durum `.empty`.

### 15.9 Marka

`web/favicon.svg` (kendi çizimimiz, M3 stili): `viewBox="0 0 48 48"`, `<rect width="48" height="48" rx="14" fill="#6750A4"/>` ve beyaz `menu_book` glifi. Glif, 15.5'teki aynı `menu_book` SVG'sinin `d` değeriyle `<g transform="translate(8 8) scale(0.0333) translate(0 960)"><path fill="#fff" d="..."/></g>` biçiminde yerleştirilir. Başka logo yoktur. Uygulama adı arayüzde "kavita-rs" harfleriyle yazı olarak görünür (üst çubuk başlığı ve giriş ekranı).

### 15.10 Görsel doğrulama ve erişilebilirlik (e2e genişlemesi)

`scripts/e2e/package.json` `devDependencies` şu olur: `"@playwright/test": "1.55.0"` ve `"@axe-core/playwright": "4.11.0"`.

`scripts/e2e/reader.spec.mjs` 14.6 E akışına ek olarak ikinci bir test `test('görünüm ve erişilebilirlik', ...)` içerir:
- Önceki akıştaki kurulum, tarama ve oturum verisi yeniden kullanılır (aynı `describe`, `test.describe.configure({ mode: 'serial' })`).
- 4 kombinasyon için döngü: `viewport` 360x800 ve 1280x800, `colorScheme` `light` ve `dark`. Her biri için yeni `browser.newContext({ viewport, colorScheme })`, `context.request.post('/api/login', ...)` ile giriş yapılmış, `page.emulateMedia({ colorScheme })`.
- Altı ekran her kombinasyonda gezilir:

| Ekran adı | Hash rotası | Not |
|---|---|---|
| `kutuphane` | `#/` | |
| `seri` | `#/seri/{id}` | |
| `okuyucu-cbz` | `#/oku/{cbzId}` | |
| `okuyucu-epub` | `#/oku/{epubId}` | |
| `istatistik` | `#/istatistik` | |
| `hesap` | `#/hesap` | |

- Okuyucu ekranlarında `page.mouse.move(...)` ile çubuklar görünür tutulur ve çekim çubuk görünürken yapılır.
- Her ekranda `page.screenshot({ path: 'screenshots/<ekran>-<360|1280>-<light|dark>.png', fullPage: false })`. Toplam 24 PNG. Çekimden önce `await page.waitForLoadState('networkidle')` ve ekrana özgü bir beklenen eleman (ör. `[data-testid="series-card"]`, `#page-indicator`, `.chart-card svg`).
- Her ekranda `new AxeBuilder({ page }).analyze()`; `results.violations.filter(v => v.impact === 'serious' || v.impact === 'critical')` boş olmalıdır, aksi halde test düşer (ihlal `id` ve `help` metni hata mesajına yazılır). Metin okuyucuda çerçeve içeriği de dahildir.
- Bu test 14.6 E'deki konsol/CSP denetimini de çalıştırır.

CI `e2e` işi (`.github/workflows/ci.yml`) ek adım: `actions/upload-artifact@v4` ile `name: ui-screenshots`, `path: scripts/e2e/screenshots`, `if: always()`, `retention-days: 14`. `scripts/e2e/screenshots/` `.gitignore`'a eklenir. Playwright rapor yükleme adımı (`if: failure()`) kalır.

### 15.11 Görev listesi değişiklikleri (13 ve 14 üzerine)

Sıra: 1, 2, 3, 5, 5b, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 14b, 15, 16, 17, 18, 19, 19b, 20a, 20b, 21, 22. Aynı dosyaya dokunan görevler bu sırada çalışır.

| # | İşçi | Görev | Dosyalar | Bağımlı |
|---|---|---|---|---|
| 5 | İŞÇİ-2 | Değişmez (14.1). Yer tutucu `web/*` ve `index.html` kabuğu | | 3 |
| 5b (YENİ) | İŞÇİ-2 | Varlıkları ekle ve commit/push et. (a) Yeni boş bir dizinde (`mktemp -d`, depo dışı) `curl -fsSL` ile iki woff2 dosyasını ve `LICENSE`'ı 15 başındaki URL'lerden indir, `web/fonts/` altına kopyala (`roboto-flex-latin.woff2`, `roboto-flex-latin-ext.woff2`, `LICENSE`); dosyaların ilk 4 baytı `wOF2` olmalı, değilse görev hata verir. (b) `scripts/build_icons.py`'yi yaz (15.5) ve `python3 -I scripts/build_icons.py web/icons.svg` ile `web/icons.svg` üret; `web/ICONS-LICENSE.txt` yaz. (c) `web/theme.js` için tek satırlık yer tutucu (`// yer tutucu`). (d) `web/index.html` kabuğunu 15.1'deki script/stil sırasıyla güncelle (`theme.js` senkron, head içinde). | `web/fonts/*` `web/icons.svg` `web/ICONS-LICENSE.txt` `web/theme.js` `scripts/build_icons.py` `web/index.html` | 5 |
| 4 | İŞÇİ-1 | Statik tablo 15.1'e göre (yeni adlar, `/static/{*ad}`, içerik türleri, önbellek başlıkları, `include_bytes!`) | `src/app.rs` | 5b |
| 11 | İŞÇİ-1 | `GET /api/series` sözleşme değişikliği (15.6): `status` parametresi, `reading_count`, `percent`; test `series_status_filter` | `src/api/library.rs tests/library.rs` | 10 |
| 14 | İŞÇİ-1 (İŞÇİ-2'den geçti) | `web/style.css`: 15.2 token sistemi, `@font-face`, 15.6/15.7/15.8'de adı geçen bileşenlerin CSS'i ve düzen (rail, bar, grid, card, btn, icon-btn, fab, field, chip, list, dialog, snackbar, progress, segmented, slider, switch, skeleton, empty, ripple, sheet, reader, stat-card, chart-card, swatch), `prefers-reduced-motion` kuralları. `web/frame.css` (15.7). Yalnız kullanılan bileşen için kural yazılır | `web/style.css web/frame.css` | 5b |
| 14b (YENİ) | İŞÇİ-1 | `web/theme.js` (15.3) ve `web/theme.test.mjs` (15.4); CI `test` işine `node web/theme.test.mjs` adımı (`node --check` döngüsünden sonra) | `web/theme.js web/theme.test.mjs .github/workflows/ci.yml` | 14 |
| 15 | İŞÇİ-1 | `web/app.js`: kabuk (rail/bar/top bar), router, dalga, `KRS.toast`/`KRS.confirm`, kütüphane (arama, chip'ler, ızgara, FAB, iskelet, boş/hata durumları), seri sayfası (liste, durum diyaloğu), yönetim sayfası, Hesap sayfası (tema/renk/hareket/parola/çıkış). `KRS.todayLocal`, `KRS.api`, `KRS.navigate` aynen 13'teki sözleşme. Kanca adları 14.6 E ve 15.6/15.8 | `web/app.js` | 14b, 13 |
| 16 | İŞÇİ-1 | `web/tracker.js` değişmez | `web/tracker.js` | 15 |
| 17 | İŞÇİ-1 | `web/reader-comic.js`: 15.7 ortak ve görsel okuyucu davranışı (otomatik gizlenen çubuklar, alt sayfa ayarları, dokunma bölgeleri, geçiş) | `web/reader-comic.js` | 16 |
| 18 | İŞÇİ-1 | `web/reader-text.js`: 15.7 ortak ve metin okuyucu davranışı (alt sayfa içindekiler/ayarlar, `readerColors`, slider) | `web/reader-text.js` | 17 |
| 19 | İŞÇİ-1 | `web/stats.js`: 15.8 (stat kartları, token renkli SVG grafikler, circular progress hedef, `#history-table` `<ul>`) | `web/stats.js` | 18 |
| 19b | İŞÇİ-1 | 14.6 E e2e işi artı 15.10 (ekran görüntüleri, axe, `ui-screenshots` artifact'ı, `package.json` bağımlılıkları, `.gitignore`) | `scripts/e2e/* .github/workflows/ci.yml .gitignore` | 19 |
| 20a (20'nin bölünmüş yarısı) | İŞÇİ-1 | `web/index.html` son hali (15.1 sırası, başlık, `lang`, `noscript`, `color-scheme`, erişilebilirlik) ve `web/favicon.svg` (15.9) | `web/index.html web/favicon.svg` | 19b |
| 20b | İŞÇİ-2 | README gözden geçirme (Hesap sayfasından tema/renk seçimi, ekran görüntüsü artifact'ı, lisans notları: GPL-3.0, Roboto Flex OFL, Material Symbols Apache-2.0) ve `tests/static_files.rs` yeni adları kapsar: her ad 200 ve doğru `Content-Type` (`font/woff2` için ilk 4 bayt `wOF2`, `icons.svg` içinde `<symbol id="menu_book"` var), bilinmeyen ad 404, `/static/theme.test.mjs` 404, `/static/fonts/LICENSE` 404, `index.html` içinde `theme.js` betiğinin diğer betiklerden önce ve `defer`/`async` olmadan geçtiği, CDN bağlantısı (`http://`, `https://`) olmadığı, satır içi `<script>` ve `style=` olmadığı | `README.md tests/static_files.rs` | 20a |
| 21 | İŞÇİ-2 | Değişmez. Ek olarak `test` işindeki `node web/theme.test.mjs` ve `e2e` işindeki axe ile ekran görüntüsü adımları yeşil olmalı. Hata JS/CSS ile ilgiliyse İŞÇİ-1'e geri gönderilir | | 20b |
| 22 | İŞÇİ-2 | Değişmez (14.7) | | 21 |

Not: Görev 14'te CSS'in `web/theme.js`'in yazacağı satır içi değişken adlarıyla çakışmaması için tüm renkler yalnız `var(--md-sys-color-*)` olarak kullanılır. CSS içinde onaltılık renk yalnız 15.2'deki JS'siz yedek tabloda bulunur.

Atlananlar/riskler: Hiçbir CSS, JS veya renk formülü çalıştırılmadı. Palet üreticisi yalnız `theme.test.mjs` ile CI'da sınanır. Tablodaki taban çizgisi hex değerleri ve OKLab sabitleri bellekten yazıldı ve doğrulanmadı. Test uyuşmazlık bulursa düzeltme kodda yapılır. `<use href="/static/icons.svg#...">` dış sprite'ın `default-src 'self'` CSP'si altında çalıştığı yalnız e2e ekran görüntüsü ve axe ile görülebilir. Çalışmazsa 15.5 bozulur ve sprite'ın `index.html` içine gömülmesi gerekir. `@axe-core/playwright@4.11.0` ile `@playwright/test@1.55.0` birlikte kullanım uyumu yalnız peer bağımlılık (`playwright-core >= 1.0.0`) üzerinden varsayıldı.

### 15.12 CEO notları
- `web/index.html` gövdesi: `<div id="app" class="app-shell"></div>` tek bağlama noktasıdır; `app.js` kabuğu buraya kurar, `.snackbar` da onun içinde oluşturulur. `<noscript>` satırı bunun yanındadır.
- İşçi 2'nin görev 1-5 sapmaları kabul edildi: `version = "0.1.0"`, `smoke.sh` chmod adımları, `image` işinde `test -f Cargo.lock`, `cache-to: type=gha`.
- Çalışır durumda olana kadar `image`/`e2e` kırmızısı beklenen durumdur; `test` işi her görev sonunda yeşil olmalıdır.
- Force push yasaktır. Düzeltmeler yeni commit'lerle yapılır.
- `<meta name="theme-color" content="#FEF7FF">` kullanılır (ilk değer; `theme.js` çalışınca `surface` ile değişir). `<div id="app" class="app-shell"></div>` görev 15 başında `web/index.html`'e eklenir; `theme-color` ve `<noscript>` görev 20a'da.

---

## 16. CEO düzeltme turu 1 (önceki bölümlerle çelişirse bu bölüm kazanır)

Kaynak: şeytanın avukatı raporu + CEO'nun ekran görüntüsü incelemesi. Her madde tamamlanır; yapılamayan madde `BLOKE:` ile raporlanır. Testler silinmez, zayıflatılmaz.

### 16.1 Arka uç: güvenlik
1. **Giriş sınırlayıcı yarışı (`src/auth.rs`, `src/api/auth.rs` login ve change_password).** `check` yerine atomik `reserve(user, ip) -> Result<(), u64>`: tek kilit altında sınırı denetler ve denemeyi HEMEN sayar. Başarıda `ok(user)` kullanıcı kovasını sıfırlar; IP kovasından o rezervasyonu düşer. Başarısızlıkta başka artış olmaz. Test: `login_rate_limit_burst`: aynı adla 50 eşzamanlı yanlış giriş; en çok 5 tanesi 401, kalanı 429.
2. **Setup DoS (`src/api/auth.rs` setup).** Argon2'den önce `SELECT EXISTS(SELECT 1 FROM users)`; doluysa hemen 409. Yarış koruması için mevcut koşullu INSERT kalır. Test: kurulu sistemde setup 409 (mevcut test yeterli değilse genişlet).
3. **Sınırlayıcı belleği.** Login'de `username` uzunluğu 32'yi veya `password` 128'i aşarsa kova açmadan 401. `fail/reserve` içinde harita 10.000 anahtarı aşarsa süresi dolanlar budanır; hâlâ aşıyorsa en eskiler atılır. Birim test.
4. **Kullanıcı adı zamanlaması.** Kullanıcı yoksa sabit bir sahte argon2id hash'e (`std::sync::LazyLock`, ilk kullanımda üretilir) karşı `verify_password` çalışır, sonuç yok sayılır.
5. **Ping (`src/api/tracking.rs`).** Mevcut satırın `book_id`'si istektekinden farklıysa 409 `conflict`, satır değişmez. Test: `ping_client_id_book_mismatch_409`.

### 16.2 Arka uç: veri ve biçimler
6. **Tarama hataları (`src/scan.rs`).** Yürüyüş `read_dir` hatalarını sayar ve `tracing::error!` yazar. Kök dizin okunamıyorsa VEYA herhangi bir `read_dir` hatası olduysa o taramada `missing=1` işaretleme adımı ATLANIR. Dosya başına `compute_content_id`/meta hataları `tracing::warn!` ile yazılır. Test: `scan_unreadable_root_keeps_books`: önce tara, sonra kökü `chmod 000` yap (root değilse) ve tara, kitaplar `missing=0` kalır.
7. **Türkçe arama ve sıralama (`src/api/library.rs`).** `fn fold_tr(s:&str)->String`: `İ I ı` → `i`, ardından `to_lowercase`. `q` filtresi ve `sort=name` Rust'ta bu katlanmış metin üzerinden yapılır (`natural_cmp` ile); SQL LIKE/NOCASE bunlar için kullanılmaz. `sort=name` doğal sıralamalıdır (`Cilt 2` < `Cilt 10`), blueprint 5.3'teki "doğal değil" ifadesi iptal. Test: `search_and_sort_turkish`: `şeker`↔`ŞEKER`, `ırmak`↔`IRMAK`, `İstanbul`↔`istanbul`.
8. **Nav TOC (`src/formats/epub.rs`).** Yalnız `<nav epub:type="toc">` içindeki bağlantılar alınır; böyle bir nav yoksa ilk `<nav>` kullanılır. `landmarks` ve `page-list` alınmaz. Test: fixture'a landmarks ekleyip TOC'ta olmadığını doğrula.
9. **EPUB asset çift çözme (`epub_asset`, `normalize_zip_path`).** Yüzde çözme tek katmanda olur: `Query` çözülmüş değeri manifest yoluyla doğrudan karşılaştırılır; `epub_asset` ikinci kez çözmez. `p.contains("..")` kaldırılır; manifest anahtar eşleşmesi tek yetkilidir. Test: `foto %20 #1.png` benzeri adlı manifest görseli 200; `../` ve `%2e%2e/` hâlâ 404.
10. **MOBI önbelleği (`src/api/reader.rs`, `AppState`).** Tek girdilik `Mutex<Option<(PathBuf, i64 mtime, Arc<MobiBook>)>>`; `get_toc/get_chapter/get_asset` paylaşır. `images` kopyalanmaz. Yorum: `// shortcut: tek kitaplık önbellek, iki kullanıcı dönüşümlü farklı MOBI okursa yeniden çözülür, gerekirse LRU`. `split_mobi_chapters`: `<mbp:pagebreak` sonrası `>` bulunmazsa döngü biter (O(n²) yok).
11. **RAR yükü (`src/formats/comic.rs`, `src/api/reader.rs`).** Tek girdilik sayfa listesi önbelleği (yol+mtime → `Arc<Vec<String>>`) ve `bsdtar` için `tokio::sync::Semaphore` (2 izin). `reader-comic.js`: `rar`/`cbr` için ön yükleme yalnız sonraki 1 sayfa. Önceki shortcut yorumu buna göre güncellenir.
12. **`PUT status` (`put_status`).** Önceki durum zaten `finished` ise `finished_date` korunur. `getSeries`/kitap yanıtında `status='finished'` için gösterilen `percent` 1'dir.
13. **`/data` hatası (`main.rs`).** `create_dir_all`/`Db::open` başarısızsa panik yok: `eprintln!` ile Türkçe, yolu ve `uid 10001` iznini anlatan mesaj, `exit(2)`.
14. **Testlerde `let _ =` (`tests/tracking.rs`).** Ping sonuçları `assert_eq!(.., 204)` ile doğrulanır.
15. **README:** ters vekil için `proxy_set_header Host $host;` gerekliliği (aksi halde 403 `csrf`).

### 16.3 Arayüz: işlev
16. **Yönlendirici nesli (`web/app.js`, okuyucular, `stats.js`).** `handleRoute` her geçişte `KRS.routeGen++`. Her asenkron render ve `reader.open` adımı await sonrası nesli denetler; eskiyse DOM eklemez, `tracker.start` çağırmaz, dinleyici eklemez, varsa kendi temizliğini yapar ve sonuç atılır. `body.reading` yalnız geçerli rota okuyucuysa kalır. `pollScan`, uçuştaki yanıt sonrası yeni zamanlayıcı kurmaz.
17. **Kütüphane sayfalama (`renderLibrary`/`loadSeries`).** Sonsuz kaydırma: `IntersectionObserver` + yedek "Daha fazla" `.btn.tonal`; `limit=60`, `offset` artar, `total` dolunca durur. İstek sırası: `seq` ile eski yanıtlar atılır; filtre/arama değişince liste sıfırlanır. Test: e2e'de değil, kod denetimiyle; `make_books.py` eklemiyoruz.
18. **Metin okuyucu sınırları (`reader-text.js`).** Son bölümde "sonraki" ve ilk bölümde "önceki" hiçbir şey yapmaz, düğmeler `disabled`; konum değişmez. Ok tuşları: ↑/↓ çerçeveyi kaydırır (ebeveyn tuş dinleyicisi), ←/→ `[`/`]` ile aynı.
19. **Bölüm hataları (`reader-text.js`).** `iframe.onload` içinde `contentDocument.contentType !== 'text/html'` ise: içeriği gösterme, ilerleme YAZMA, `.empty.error` ve "Yeniden dene" göster. JSON gövdesi 401 ise `#/giris`.
20. **Çizgi roman hataları (`reader-comic.js`).** `pageImg.onerror`: sayfa alanında `.empty.error` ve "Yeniden dene". `startX` `pointerup`/`pointercancel`'da sıfırlanır; yalnız sahnede başlayan sürükleme sayılır. Sahneye `touch-action: pan-y pinch-zoom`.
21. **Kısayollar.** `ctrlKey/metaKey/altKey` varsa okuyucu kısayolu çalışmaz, `preventDefault` yok.
22. **Takip (`tracker.js`).**
    - `PUT progress` `local_date` = o anın yerel günü (`KRS.todayLocal()`); ping'de oturum başlangıç günü kalır.
    - `pendingProgress` yalnız başarıdan sonra temizlenir, hatada korunur ve sonraki denemede gider.
    - `visibilitychange:hidden` da `flushProgress(keepalive)` çağırır.
    - 401 → `KRS.navigate('#/giris')`.
    - `activeSeconds` 0'dan başlar.
    - Test: `reader.spec.mjs`.
23. **Geçmiş "Daha fazla".** İstek sürerken düğme `disabled`; sonraki sayfa hatası listeyi silmez, snackbar gösterir.

### 16.4 Arayüz: görsel (ekran görüntüsü bulguları)
24. **Metin okuyucu teması.** `localStorage['krs.readerTheme']` yoksa okuma teması uygulamanın etkin modunu izler (koyu → `dark`, açık → `light`); kullanıcı seçince sabitlenir. Koyu uygulamada beyaz sayfa görünmez. Tema değişince `<iframe>` yüklenmeden arka plan flaşı olmaz: iframe elemanının arka planı önceden `bg` yapılır.
25. **Okuyucu çubukları (15.7'yi değiştirir).** `.reader-bar` opaktır: `background: var(--md-sys-color-surface-container)`. Metin alttan görünmez. Metin okuyucuda çerçeve belgesinin üst/alt boşluğu çubuk yüksekliği kadar: `padding-top: 80px; padding-bottom: 96px` (frame.css); `body` kuralı buna göre.
26. **Çizgi roman okuyucu.** Uygulama modu ne olursa olsun `.reader.comic` kök elemanına koyu palet rolleri CSSOM ile yazılır (`KRS_THEME.generatePalette(seed,true)`), çubuklar koyu görünür.
27. **Grafikler (`stats.js`).**
    - SVG boyutu `viewBox` ölçeklemesi yerine gerçek kap genişliğiyle çizilir (`ResizeObserver`, 150 ms debounce), metin her genişlikte 12 px.
    - Y ekseni ticks: benzersiz ve "güzel" değerler (1-2-5×10ⁿ); `max` 0 ise yalnız 0 ve 1 (etiketsiz boş durum metni "Henüz veri yok"). Dakika ölçeğinde `max < 1` ise saniye birimi kullanılır ve başlık buna göre.
    - Sıfır değerli gün için çubuk çizilmez (`rect` yok); tooltip için tam yükseklikte saydam `rect.hit` çizilebilir ama `fill: transparent`.
    - X etiketleri 360 px'te en çok 3, 1280 px'te 5; çakışmaz.
    - Süre metni: `<60 sn` ise `12 sn`, `<1 sa` ise `5 dk 12 sn`, üstü `2 sa 5 dk`.
28. **Hesap.** `.field` ve form düğmeleri `max-width: 480px`. Swatch 44 px, boşluk 4 px (360 px'te altısı tek satıra sığar); özel renk girişi aynı boyutta.
29. **Chip grupları.** ≥840 px'te gruplar arasında 24 px boşluk ve 1 px `outline-variant` dikey ayırıcı; <840 px'te iki grup ayrı satırda, her biri tek satırda yatay kaydırılabilir (`overflow-x:auto`, kaydırma çubuğu gizli), kırılmaz.

### 16.5 Ekran görüntüsü kapsamı (15.10'a ek)
Her kombinasyon için ayrıca alınır (axe da çalışır): `giris` (oturumsuz), `gecmis` (`#/gecmis`), `yonetim` (`#/yonetim`), `okuyucu-epub-ayarlar` (alt sayfa açık), `okuyucu-comic-ayarlar` (alt sayfa açık), `seri-durum-dialog` (durum diyaloğu açık). Birer kez: `kutuphane-bos` (ilk testte tarama öncesi, 1280 açık), `kurulum` (1280 açık), `kutuphane-yesil` (1280 koyu, `KRS_THEME.setSeed('#2E7D32')` sonrası).

e2e sınamaları güçlenir: 27'deki sıfır çubuk yokluğu ve en az bir veri `rect`'i (`rect:not(.hit)`, sıfır yüksekliksiz); geçmiş satırında süre metni; koyu modda `okuyucu-epub` çerçeve arka planının açık olmadığı (`getComputedStyle` parlaklık < 0.3); 18'deki son bölümde "sonraki" `disabled`; okuyucudan hemen çıkıldığında `body.reading` kalmadığı.

---

## 17. CEO düzeltme turu 2 (önceki bölümlerle çelişirse bu bölüm kazanır)

1. **Bölüm `<title>` metni gövdeye sızıyor (`src/formats/sanitize.rs`).** Ekran görüntüsünde `<title>` metni bölümün başında düz metin görünüyor. `sanitize_chapter`, ammonia'ya `clean_content_tags` içinde `title` dahil ederek (mevcut `script`, `style` korunur) `<title>` içeriğini tamamen atar. MOBI için de geçerlidir. Test (`tests/formats.rs`): `chapter_title_not_in_body`: `<head><title>Sızan Başlık</title></head><body><p>metin</p></body>` temizlenince `Sızan Başlık` yok, `metin` var.
2. **Metin okuyucu ayarları (`reader-text.js`, `style.css`).** Yazı boyutu satırı tek satırdır: `A-` düğmesi, `.slider` (`flex:1`), `A+` düğmesi, `align-items:center`, her genişlikte. `A-`/`A+` metni `label-large` ve `on-surface` renginde; 48x48 `.icon-btn`.
3. **Çizgi roman sığdırma (`reader-comic.js`).** Kısa etiketler: `Genişlik`, `Yükseklik`, `Sayfa`, `Gerçek`; `aria-label` tam adlarla ("Genişliğe sığdır" vb.). `.segmented` asla kırılmaz (`flex-wrap:nowrap`, her `.seg-btn` `flex:1 1 0`, `min-width:0`, metin tek satır, 360 px'te seçili `check` ikonu gizlenir). Alt sayfa `#page-indicator` testini etkilemez.
4. **Sıfır saniyeli oturumlar (`src/api/tracking.rs`, 8.3'ü değiştirir).** Satır yoksa ve `active_seconds == 0` ise `pages_read` ne olursa olsun satır OLUŞTURULMAZ (204). Mevcut satır güncellemesi aynı kalır. `ping_zero_creates_nothing` testi genişletilir: `active_seconds=0, pages_read=1` satır oluşturmaz.
5. **Geçmiş metni (`stats.js`).** Tarih `Intl.DateTimeFormat('tr-TR',{day:'numeric',month:'short',year:'numeric'})` ile yazılır (`local_date` bileşenlerinden, saat dilimi kaymasız; ör. `8 Kas 2026`). Yüzde: başlangıç ve bitiş aynı yuvarlanıyorsa tek değer (`%67`), değilse `%33 → %67`; yüzde parçası `white-space:nowrap`.
6. **`kutuphane-bos` ekran görüntüsü (`scripts/e2e/reader.spec.mjs`).** Başlangıç taraması kitapları zaten bulduğu için görüntü boş durumu göstermiyor. Boş durumu ağ düzeyinde üret: `page.route('**/api/series*', r => r.fulfill({json:{total:0,items:[]}}))`, ekran görüntüsünü al ve `.empty` görünürlüğünü doğrula, sonra `unroute`. Aynı yöntemle arama sonuçsuz durumu `kutuphane-sonuc-yok` olarak alınır.

Uygulama adı bu turda DEĞİŞTİRİLMEZ.

---

## 18. CEO düzeltme turu 3: son (önceki bölümlerle çelişirse bu bölüm kazanır)

1. **401 döngüsü (`web/tracker.js`, `web/app.js`).**
   - `flushProgress` ve ping yanıtı 401 ise: `pendingProgress = null`, oturum bekleyen verisi temizlenir, izleyici durdurulur, sonra YALNIZ `location.hash !== '#/giris'` ise `KRS.navigate('#/giris')`. `KRS.navigate` hedef hash zaten aynıysa ek `handleRoute` tetiklemez.
   - Diğer kalıcı hatalar (400, 403, 404, 409, 413, 415, 422): `pendingProgress` silinir, yeniden denenmez.
   - Yalnız ağ hatası, 429 ve 5xx'te korunur ve sonraki normal zamanlayıcıda yeniden denenir; ayrı yeniden deneme döngüsü kurulmaz.
   - `tracker.stop()`: tek bir `keepalive` denemesi yapar, sonra `pendingProgress` ve kitap kimliği temizlenir; bekleyen veri başka kitaba ya da yeniden girişten sonraya taşınmaz. `tracker.start()` temiz başlar.
   - e2e: okuyucu açıkken `context.clearCookies()`, sayfa ilerlet; `#/giris` görünür, 3 sn boyunca `/api/setup-status` en çok 2 kez çağrılır, giriş formu kaybolmaz.
2. **Çok baytlı parola (`src/api/auth.rs`).** Login'deki `req.password.len() > 128` kaldırılır; yalnız `chars().count() > 128` kalır. Kullanıcı adı denetimi de karakter sayar. Test: `login_long_multibyte_password`: `"ş".repeat(100)` ile setup 201, aynı parolayla login 200.
3. **Önbellek anahtarı (`src/api/reader.rs`, `src/formats/comic.rs`).** MOBI ve RAR/CBR önbellek anahtarı her istekte `std::fs::metadata` ile alınan gerçek `(yol, mtime, boyut)` üçlüsüdür; DB'deki `mtime` kullanılmaz. Test: `comic_pages_cache_detects_file_change`: `.cbr` tar fixture listele, dosyayı farklı sayfa sayısıyla değiştir (mtime farkını `File::set_modified` ile garanti et), yeni liste döner.
4. **Küçük sertleştirmeler.**
   - `src/scan.rs` `walk_dir_rec`: `entry.file_type()` hatası `read_dir` hatası gibi sayılır ve `tracing::error!` yazılır.
   - `tests/auth_flow.rs login_rate_limit_burst`: `1 <= count_401 && count_401 <= 5` ve kalanı 429.
   - `tests/library.rs scan_unreadable_root_keeps_books`: root altında atlanıyorsa `eprintln!("root: test atlandı")` yazar.
   - `DUMMY_HASH`: `main.rs` serve başlangıcında `std::sync::LazyLock::force` ile bir kez üretilir (istek yolunda yürütücü bloklanmaz).
   - `web/stats.js`: hedef kaydetme sonrası yeniden çizimin temizleme işlevi rota temizliğine kaydedilir, öncekini çağırır.

Bilerek düzeltilmeyenler: hesap kilitleme tasarımı, ilk pingde süreyi kendi lehine şişirme, şifreli RAR'ın bozuk görünmesi, offset sayfalamada tarama sırasında tekrar eden kart, `krs.theme` göçü (kullanıcı yok), yayın imajının yeniden derlenmesi.

---

## 19. Ad değişikliği: ReadCrab (önceki bölümlerle çelişirse bu bölüm kazanır)

Depo `mustafabayramguven/ReadCrab` olarak yeniden adlandırıldı. Görünen ad `ReadCrab`.

Değişir:
- Arayüz: `web/index.html` başlığı, giriş/kurulum ekranı, üst çubuk, e2e beklentileri, `tests/` ve `scripts/smoke.sh` içindeki metin denetimi (`ReadCrab`).
- Rust: paket ve ikili adı `readcrab`, kütüphane yolu `readcrab::` (`kavita_rs::` kullanan tüm testler ve `main.rs`). `Cargo.lock` içindeki paket adı aynı commit'te elle güncellenir (`--locked` kırılmasın).
- `Dockerfile`: ikili yolu, `ENTRYPOINT`, `HEALTHCHECK`.
- CI: imaj `ghcr.io/mustafabayramguven/readcrab:latest`, yerel etiket `readcrab:ci`.
- `docker-compose.yml`: servis `readcrab`, imaj, volume `readcrab-data`.
- Veritabanı dosyası `DATA_DIR/readcrab.db` (gerçek kullanıcı verisi yok, göç gerekmez). README buna göre.
- README başlığı, kurulum komutları, depo bağlantıları.
- e2e metin fixture'ı (`make_books.py`) ve `web/favicon.svg` yorumları varsa.

Değişmez: `window.KRS`, `krs_session`, `krs.*` localStorage anahtarları, geçmiş commit'ler, `BLUEPRINT.md`'nin önceki bölümleri. README'de "Kavita ile bağlantısı yoktur" notu kalır; `LICENSE` aynı kalır.

Eski `ghcr.io/mustafabayramguven/kavita-rs` paketine dokunulmaz.

---

## 20. İterasyon 2 (önceki bölümlerle çelişirse bu bölüm kazanır)

Genel kurallar: Gerçek kullanıcı verisi vardır. Şema yalnız `0002` ile değişir, `user_version` 1'den 2'ye çıkar, hiçbir tablo düşürülmez veya yeniden yaratılmaz. Locator biçimleri (`"12"`, `{"ch":3,"p":0.42}`) ve mevcut `krs.*` anahtarları aynen çalışır. Yeni sürümlü paket yok: `Cargo.toml` değişmez (`image` jpeg/png/gif/webp özellikleri yeter). Kod dili Türkçe arayüz, mevcut `el()/icon()/KRS.api` kalıpları kullanılır. Yerelde derleme yok. Tüm doğrulama CI'de.

Doğrulama sonuçları (curl, salt okunur):
- jsDelivr'da `@fontsource-variable` sürümleri var: literata 5.3.0, merriweather 5.3.0, source-serif-4 5.3.1, eb-garamond 5.3.0, inter 5.3.0, lexend 5.3.0 (hepsi OFL-1.1). `@fontsource-variable/atkinson-hyperlegible` YOK (404). Değişim gerekmedi: aynı yazı tipi `@fontsource/atkinson-hyperlegible@5.3.0` statik 400 ve 700 ile alınır (OFL-1.1). Başka yazı tipi değişimi yok.
- Dosya adları `https://cdn.jsdelivr.net/npm/<paket>@<sürüm>/files/<ad>`: değişkenler `<id>-latin-wght-normal.woff2`, `<id>-latin-ext-wght-normal.woff2`, `…-italic.woff2` (Lexend'in italiği yoktur, yalnız normal 2 dosya); Atkinson `atkinson-hyperlegible-{latin,latin-ext}-{400,700}-{normal,italic}.woff2`.
- Gerçek bayt boyutu toplamı (30 dosya, her dosya curl ile indirilip sayıldı): 1.488.592 bayt (~1,42 MiB). Dosya başına: literata latin 52496/italik 53728, ext 42656/43256; merriweather 97548/100572, ext 74960/78260; source-serif-4 50936/51576, ext 42212/44420; eb-garamond 44336/47940, ext 113928/88576; inter 48256/51832, ext 85068/91900; lexend 39680, ext 34476; atkinson latin 17208/18292/17524/18484, ext 9384/9796/9372/9920. `Aa` önizleme metni yalnız ASCII olduğundan listeyi açmak yalnız latin dosyalarını (~0,5 MB) indirir.
- Türkçe glifler: ç ö ü Latin alt kümesinde, ş ğ İ `latin-ext` alt kümesinde (unicode-range `U+0100-02BA…`), ı hem `U+0131` ile `latin`de. Her ikisi birlikte bağlanır. Dosya içi glif varlığı yerelde ayrıştırılamaz (woff2 brotli); kanıtı CI e2e'de `document.fonts.check` yapar (görev 24).
- CSP: `manifest-src` tanımsız, `default-src 'self'`e düşer (CSP3 fallback), `/static/manifest.webmanifest` aynı kökenli olduğundan çalışır; `APP_CSP` değişmez. Kanıt e2e'de "Refused to" konsol denetimi.

### 20.1 Dosya ağacı değişiklikleri

Yeni:
- `src/migrations/0002_iter2.sql`
- `src/api/bookmarks.rs`, `src/api/cover.rs`
- `src/formats/textsearch.rs` (HTML'den düz metin, arama)
- `tests/migration.rs`, `tests/bookmarks.rs`, `tests/cover_custom.rs`, `tests/iter2.rs` (continue, heatmap, history date, detay alanları, arama, pace)
- `web/calc.js` (saf işlevler, UMD: `window.KRS_CALC` + `module.exports`), `web/calc.test.mjs`
- `web/reader-ui.js` (ortak okuyucu bileşenleri: `window.KRS.readerUi`)
- `web/reader-fonts.css`, `web/manifest.webmanifest`, `web/icon-192.png`, `web/icon-512.png`
- `web/fonts/<30 woff2>`, `web/fonts/LICENSE-reader.txt` (dosya adı kuralı 20.8)
- `scripts/make_pwa_icons.py`, `scripts/fonts/README.txt` (hangi URL'lerden ve hangi dosya adıyla indirildiği, yeni boş dizin kuralı)

Değişen: `src/db.rs`, `src/app.rs`, `src/auth.rs`, `src/scan.rs`, `src/api/{mod,library,reader,stats,tracking}.rs`, `src/formats/{cover,mod,sanitize}.rs`, `tests/{common/mod,static_files,reader,formats,security}.rs` (yalnız CSP/statik beklentileri ve yeni testler), `web/{app,tracker,reader-text,reader-comic,stats,theme}.js`, `web/{style.css,frame.css,index.html,icons.svg,theme.test.mjs}`, `scripts/build_icons.py`, `scripts/e2e/{reader.spec.mjs,make_books.py}`, `.github/workflows/ci.yml`, `README.md`.

### 20.2 Şema (`0002_iter2.sql`, yalnız ekleme)

```sql
ALTER TABLE books ADD COLUMN custom_cover INTEGER NOT NULL DEFAULT 0;
ALTER TABLE books ADD COLUMN cover_version INTEGER NOT NULL DEFAULT 0;
CREATE TABLE bookmarks(
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  book_id INTEGER NOT NULL REFERENCES books(id),
  locator TEXT NOT NULL,
  label TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_bookmarks_user_book ON bookmarks(user_id, book_id, created_at);
CREATE INDEX idx_progress_continue ON progress(user_id, status, updated_at);
```
`src/db.rs`: `const MIGRATION_0002`; `if user_version < 1 {…}` bloğunun ardından ayrı transaction ile `if user_version < 2 { execute_batch(MIGRATION_0002); pragma user_version=2 }`. `user_version` okuması bir kez yapılır, sonra ikinci blok için yeniden okunur. Boş DB iki bloğu da çalıştırır, v1 DB yalnız ikincisini.

Test `tests/migration.rs::v1_database_keeps_data`: geçici dosyada `rusqlite::Connection` ile `include_str!("../src/migrations/0001_init.sql")` çalıştır, `user_version=1` yaz; users(1 admin), series, 2 books (biri `has_cover=1`, `sort_index` dolu), progress (locator `{"ch":2,"p":0.5}` ve `"7"`, status reading/finished, `finished_date`), 2 reading_sessions, goals satırı ekle; bağlantıyı kapat; `Db::open` ile aç. Doğrula: `user_version==2`; her tablodaki satır sayıları ve tüm eski sütun değerleri (locator, percent, status, finished_date, active_seconds, local_date) birebir aynı; `books.custom_cover=0`, `cover_version=0`; `bookmarks` tablosu boş ve mevcut; ikinci `Db::open` hatasız ve veri aynı (idempotent).

### 20.3 Arka uç uç noktaları

Ortak: hepsi `AuthUser`; `user_id` oturumdan. Yeni hata kodu yok.

1. **GET `/api/continue`** (`library.rs`) → `{items:[{book_id,title,series_id,series_name,format,percent,cover_v}]}`. SQL: `progress p JOIN books b JOIN series s WHERE p.user_id=?1 AND p.status='reading' AND b.missing=0 AND b.error IS NULL ORDER BY p.updated_at DESC, b.id DESC LIMIT 20`. `percent` = `p.percent`. `cover_v` = `b.cover_version`; `has_cover=0` ise sunucu yine döner, istemci yedek harfi çizer.
2. **GET `/api/books/{id}`** `BookDetailRes`'e eklenir: `cover_version:i64`, `next_book_id:Option<i64>`, `next_book_title:Option<String>`, `prev_book_id:Option<i64>`, `pace_seconds:Option<i64>`. `next`: aynı seride `missing=0 AND error IS NULL AND (sort_index > ?s OR (sort_index = ?s AND id > ?id)) ORDER BY sort_index ASC, id ASC LIMIT 1`; `prev` simetrik DESC. `pace_seconds`: `SELECT SUM(active_seconds), SUM(end_percent-start_percent) FROM reading_sessions WHERE user_id=? AND book_id=? AND end_percent>start_percent AND active_seconds>=30`; `sa>=300 AND sd>=0.02` ise `round(sa/sd)` (tüm kitabı okuma süresi, sn), yoksa `null`. `// shortcut: ileri sarma ve geri okuma ortalamayı bozar, gerekirse son N oturum`.
3. **GET `/api/series`** her öğeye `cover_v` eklenir (aynı alt sorguyla `cover_version`). Kapak seçimi: `ORDER BY b3.custom_cover DESC, b3.sort_index ASC, b3.id ASC` (`has_cover=1 AND missing=0` korunur). **GET `/api/series/{id}`** her kitaba `cover_v` ekler.
4. **GET `/api/books/{id}/cover`** davranışı aynı kalır; sorgu dizisi `?v=` yok sayılır. `Cache-Control` aynı (`private, max-age=86400`); bayatlama `?v=` ile aşılır.
5. **PUT `/api/books/{id}/cover`** (`src/api/cover.rs`, `AdminUser`). Gövde `axum::body::Bytes`; rota `.layer(DefaultBodyLimit::max(10 * 1024 * 1024))` (iç katman dıştaki 64 KiB'ı ezer; 413 mevcut `security_headers` ile `payload_too_large`). `Content-Type` `image/jpeg|image/png|image/webp` değilse `auth.rs` guard'ı 415 verir (20.4). Kitap yok/`missing=1` → 404. `formats::cover::encode_custom_cover(raw:&[u8]) -> Result<Vec<u8>, ()>` (spawn_blocking): `ImageReader::with_guessed_format`, biçim Jpeg/Png/WebP değilse Err; `into_dimensions` ile `w*h > MAX_PIXELS` veya 0 → Err; `Limits{max_alloc:256 MiB, max_image_width:16384, max_image_height:16384}`; çöz; alfa varsa beyaz zemine birleştir; `w>600` ise `thumbnail_exact(600, h*600/w)` (büyütme yok); JPEG kalite 85. Err → 400 `bad_request` ("Görsel okunamadı."). Başarıda `covers/{id}.jpg.tmp` yaz, `rename` ile `{id}.jpg`; sonra `UPDATE books SET has_cover=1, custom_cover=1, cover_version=cover_version+1 WHERE id=?` → 200 `{cover_version}`.
6. **DELETE `/api/books/{id}/cover`** (`AdminUser`): `UPDATE custom_cover=0, cover_version=cover_version+1`; `extract_cover(books_dir,&BookRow,covers_dir)` yeniden çalışır; false ise `has_cover=0` ve dosya silinir. → 200 `{cover_version,has_cover}`. Kitap yok → 404.
7. **Tarama özel kapağı ezmez** (`scan.rs::process_single_book`): `book_id` bulunduktan sonra `SELECT custom_cover`; 1 ise `extract_cover` ve `has_cover` güncellemesi ATLANIR (dosya ve bayrak dokunulmaz). Kitap `missing` olup geri dönse de `custom_cover` satırda kalır.
8. **GET `/api/stats/heatmap?year=YYYY`** (`stats.rs`): `year` zorunlu, 2000-2100 tamsayı, aksi 400. → `{year, years:[…], days:[{date,seconds,pages}]}`. `days`: `SELECT local_date, SUM(active_seconds), SUM(pages_read) FROM reading_sessions WHERE user_id=?1 AND local_date >= '{year}-01-01' AND local_date <= '{year}-12-31' GROUP BY local_date ORDER BY local_date` (yalnız verisi olan günler, seyrek). `years`: kullanıcının `DISTINCT CAST(substr(local_date,1,4) AS INTEGER)` değerleri artan; boşsa `[]`. Saat dilimi yok: yalnız `local_date` metni.
9. **GET `/api/history`** yeni isteğe bağlı `date=YYYY-MM-DD` (`tracking.rs`): `parse_local_date` ile doğrulanır (400); `total` ve liste `AND rs.local_date=?` ile süzülür.
10. **Yer imleri** (`bookmarks.rs`): GET `/api/books/{id}/bookmarks` → `{items:[{id,locator,label,created_at}]}` (`created_at DESC, id DESC`, yalnız çağıranın); kitap yok/missing → 404. POST aynı yol `{locator,label}` → 201 `{id,locator,label,created_at}`; `locator` boş değil ve ≤200 bayt, `label` kırpılmış 1-120 karakter, aksi 400; kullanıcı+kitap başına 100'ü aşarsa 409 `conflict`. DELETE `/api/bookmarks/{id}` → 204; yok veya başkasınınki 404 (aynı sorgu, `AND user_id=?`). Router `api::router()`'a `.merge(bookmarks::router())`; yeni `pub mod bookmarks; pub mod cover;`.
11. **GET `/api/books/{id}/search?q=`** (`reader.rs` + `textsearch.rs`). Komik biçim 400. `q` kırpılır; `fold_tr` sonrası karakter sayısı 2-64 değilse 400. `static SEARCH_SLOTS: tokio::sync::Semaphore = Semaphore::const_new(2)`; `try_acquire` başarısızsa 429 `TooManyAttempts{retry_after:2}`. spawn_blocking içinde: EPUB için `epub_open`, her spine ögesi için `epub_chapter_html`; MOBI için önbellekli `book.chapters`. `textsearch::strip_tags(html:&str)->String`: etiketleri ayıklar; `script/style/title` içeriğini atar; blok etiketleri (`p div br li h1-h6 tr td th blockquote section`) bir boşlukla değiştirir, diğerleri hiçbir şeyle; `&amp; &lt; &gt; &quot; &apos; &nbsp; &#N; &#xH;` çözer; ardışık boşlukları tek boşluğa indirir. Karşılaştırma: metin ve sorgu karakter karakter `library::fold_char(c)->char` (`İ I ı`→`i`, sonra `c.to_lowercase().next()`; 1:1 hizalı) ile katlanır; `fold_tr` `fold_char` üzerine kurulur. Sonuç `{results:[{ch,p,before,hit,after}], truncated:bool}`: `p = eşleşme_karakter_indeksi / bölüm_karakter_sayısı` (0..1, 6 hane), `before/after` ±40 karakter (orijinal metinden, kırpılmış), `hit` orijinal eşleşme. Bölüm sırası artan, en çok 100 sonuç (aşılırsa `truncated:true`). DoS sınırları: toplam taranan metin 64 MiB, toplam süre 10 sn (`Instant`), bölüm başına tek bölüm HTML'i `MAX_CHAPTER_BYTES` sınırlıdır; aşımda `truncated:true` ile mevcut sonuçlar döner. Çakışmasız eşleşmeler (bir sonraki arama eşleşme sonundan başlar). `// shortcut: p metin oranıdır, görsel yüksekliğiyle birebir değildir`.
12. **Kimlik gerektirmeyen yeni statikler** 20.8'de.

### 20.4 CSRF guard (`src/auth.rs::origin_and_content_type_guard`)
`Origin` denetimi aynen kalır. İçerik türü denetimi: `is_cover_put = method==PUT && path.starts_with("/api/books/") && path.ends_with("/cover")`; bu rotada kabul edilen türler `image/jpeg|image/png|image/webp` (eşleşme parametresiz küçük harf) ve ayrıca `application/json` DEĞİL; diğer her tür 415. Diğer tüm rotalarda yalnız `application/json`. Birim/entegrasyon testleri `tests/security.rs`: `text/plain` ile cover PUT 415; `image/png` ile başka rota (`/api/books/1/status`) 415; `image/png` + yabancı `Origin` ile cover PUT 403 `csrf`; `image/gif` 415.

### 20.5 Güvenlik/CSP
`FRAME_CSP` birebir yeni değer: `"default-src 'none'; img-src 'self'; style-src 'self'; font-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'"`. Güncellenecek testler: `tests/formats.rs` satır 274, `tests/reader.rs` (`contains("/static/frame.css")` yanına `contains("/static/reader-fonts.css")`). `style-src` ve betik yasağı değişmez. `frame_document` `<head>`e `frame.css`ten ÖNCE `<link rel="stylesheet" href="/static/reader-fonts.css">` ekler. `APP_CSP` değişmez.

### 20.6 `web/calc.js` (saf, `calc.test.mjs` ile CI'de `node web/calc.test.mjs`)
İşlevler (UMD, `typeof window` korumalı): 
- `heatThresholds(values)`: yalnız `>0` değerler artan sıralanır; `n==0` → `[0,0,0]`; `t_k = sorted[ceil(p_k*n)-1]` için `p = 0.25, 0.5, 0.75`.
- `heatLevel(v, th)`: `v<=0`→0; `v<=th[0]`→1; `v<=th[1]`→2; `v<=th[2]`→3; aksi 4.
- `yearGrid(year)` → `{cols, cells:[{date,col,row}], months:[{label,col}]}`: Pazartesi=0 satır; `col=floor((dayIndex + (weekdayOfJan1Mon0))/7)`; yalnız yılın günleri; `cols` 53, 1 Ocak Pazar ise ve artık yılda 54 olabilir (CEO notu: 53 belirgin, 54 nadir kenar). Gün aritmetiği `Date.UTC` ile, yerel saat yok.
- `monthTotals(days)`, `weekdayTotals(days)` (12 ve 7 uzunluklu saniye dizileri, Pzt=0), `yearSummary(days)` → `{bestMonth, bestDay, daysRead, avgPerDay}` (`bestDay` tek tarih, en çok saniye; `daysRead` = `seconds>0` gün sayısı; `avgPerDay` = toplam/daysRead, 0 ise 0).
- `normalizePrefs(raw, legacy)`: 20.7'deki şemaya zorlar (aralık, adım, bilinmeyen anahtar atılır, geçersiz → varsayılan), `legacy={fs,readerTheme}` yalnız `raw` yoksa göçürülür.
- `pageCount(contentRight, pw)`: `max(1, ceil((contentRight-1)/pw))`; `pageFromRatio(p,n)`: `n<=1?0:round(p*(n-1))`; `ratioFromPage(i,n)`: `n<=1?0:i/(n-1)`.
- `formatEta(pace, overall)`: `pace` null veya `overall>=0.99` → `''`; `rem=pace*(1-overall)`; `<60` → `Bu kitapta ~1 dk'dan az kaldı`; `<600` → dakikaya yuvarla; üstü 5 dk'ya yuvarla; `>=60 dk` → `~2 sa 10 dk`; biçim `Bu kitapta ~{süre} kaldı`.
Testler: eşikler (boş, tek değer, 10 değer), seviye sınırları, `yearGrid(2026)`: 365 hücre, ilk hücre 1 Oca 2026 Perşembe `row=3 col=0`, son hücre 31 Ara; 2023 (1 Oca Pazar → `row=6`), artık yıl 2028 gün sayısı 366, `cols<=54`; `normalizePrefs` (eski `fs=20`, `readerTheme='sepia'` göçü, bozuk JSON, aralık dışı kırpma, `fs` tek sayı çifte yuvarlanır); `formatEta` örnekleri (`pace=7200, overall=0.5`→`~1 sa 0 dk` değil: tam çıktı `Bu kitapta ~1 sa kaldı`; bu nedenle `sa` sıfır dk'da `~1 sa` yazılır); `pageFromRatio/ratioFromPage` ters-uyumu; `yearSummary`.

### 20.7 Okuyucu ayarları ve metin okuyucu

**localStorage** (tek JSON, `krs.readerPrefs`): 
```
{"v":1,"theme":null|"light"|"dark"|"sepia"|"black","font":"literata","fs":18,"lh":1.7,"ws":0,"ls":0,"mg":0,"align":"left","mode":"scroll"}
```
Varsayılan: `theme:null` (uygulama modunu izler; koyuysa `dark`, saf siyah açıksa `black`), `font:"literata"`, `fs:18`, `lh:1.7`, `ws:0`, `ls:0`, `mg:0`, `align:"left"`, `mode:"scroll"`. Aralıklar/adımlar: `fs` 14-28 adım 2; `lh` 1.2-2.2 adım 0.1; `ws` 0-0.5 adım 0.05 (em); `ls` 0-0.1 adım 0.01 (em); `mg` 0-20 adım 1 (%). Yazma: `KRS.readerPrefs.get()/set(patch)/reset()` (`reader-ui.js`, `calc.normalizePrefs` kullanır). İlk okumada `krs.readerPrefs` yoksa eski `krs.fs` ve `krs.readerTheme` okunup yazılır; eski anahtarlar SİLİNMEZ. Yazı tipi kimlikleri ve CSS aileleri:
- `literata` → `"Literata Variable"`, `merriweather` → `"Merriweather Variable"`, `source-serif` → `"Source Serif 4 Variable"`, `eb-garamond` → `"EB Garamond Variable"`, `inter` → `"Inter Variable"`, `atkinson` → `"Atkinson Hyperlegible"`, `lexend` → `"Lexend Variable"`. Görünen adlar: Literata, Merriweather, Source Serif 4, EB Garamond, Inter, Atkinson Hyperlegible, Lexend. Yığın: serif'ler `, Georgia, "Noto Serif", serif`; Inter/Atkinson/Lexend `, system-ui, sans-serif`.

**`web/reader-fonts.css`**: 7 aile için `@font-face` (latin ve latin-ext ayrı `unicode-range`, `font-display: swap`; değişkenler `font-weight: 200 900` (Literata) / `300 900` (Merriweather, kalan değişkenlerde fontsource `wght.css` değerleri) ve `src: url(/static/fonts/<dosya>) format("woff2-variations")`; Atkinson `font-weight: 400` ve `700` ayrı bildirimler, `format("woff2")`; italikler `font-style: italic`). `unicode-range` değerleri fontsource'un `wght.css`/`latin-ext.css` dosyalarındakilerin aynısı (Latin: `U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD`; Ext: `U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF`). Dosya adları 20.8. `index.html`'e `<link rel="stylesheet" href="/static/reader-fonts.css">` (style.css'ten önce). Yalnız kullanılan font yüklenir (tarayıcı `unicode-range` ve kullanım).

**`web/frame.css`** (yeniden): değişkenler `--fs --lh --ws --ls --font --align --hyph --mx --pw --ph --m --bh`; kenar: 
```
html{background:var(--bg);color:var(--fg);font-size:var(--fs,18px);font-family:var(--font,Georgia,"Noto Serif",serif)}
body{max-width:38rem;margin:0 auto;padding:80px calc(20px + var(--mx,0px)) 96px;line-height:var(--lh,1.7);word-spacing:var(--ws,0em);letter-spacing:var(--ls,0em);text-align:var(--align,left);hyphens:var(--hyph,manual);overflow-wrap:break-word}
img{max-width:100%;height:auto}  a{color:var(--link)}  h1,h2,h3{line-height:1.25}
html.paged{height:100%;overflow-x:auto;overflow-y:hidden;scrollbar-width:none}
html.paged::-webkit-scrollbar{display:none}
html.paged body{box-sizing:content-box;max-width:none;margin:0;width:calc(var(--pw) - 2*var(--m));height:calc(var(--ph) - 176px);padding:80px var(--m) 96px;column-width:calc(var(--pw) - 2*var(--m));column-gap:calc(2*var(--m));column-fill:auto}
html.paged img{max-height:var(--bh);width:auto;max-width:100%;object-fit:contain;break-inside:avoid}
html.paged :is(figure,table,pre,blockquote){break-inside:avoid}
html.paged :is(h1,h2,h3,h4){break-after:avoid}
html.paged p{orphans:2;widows:2}
```
`--bh` = `ph - 176px` (px, JS yazar). Sütun düzeni: tek sütun (genişte çift sütun yok). `--m` (px, JS): `max(16 + pw*mg/100, (pw - 38*fs)/2)` yani satır ölçüsü 38 em'i aşmaz. Sayfa adımı her zaman `pw` (= `iframe.clientWidth`). Kaydırma modunda `--mx = round(innerWidth*mg/100)px`.

**`reader-text.js`** (`prefs` modeli, ayarlar, sayfa modu):
- Durum: `prefs`, `curChapter`, `curRatio`, `pageIdx`, `pageTotal`, `paged` (= `prefs.mode==='page'`).
- `applyFrameStyles()` her bölüm yüklenince ve her ayar değişiminde: tema renkleri `KRS_THEME.readerColors(themeKey)`; `--fs`, `--lh`, `--ws`, `--ls`, `--font`, `--align`, `--hyph` (`align==='justify'`→`auto`, aksi `manual`), `--mx` iframe `documentElement.style.setProperty` ile; `documentElement.classList.toggle('paged', paged)`; `relayout()`.
- `relayout()`: yazı/mod/boyut değişiminde (debounce 100 ms, `ResizeObserver` `stage`, iframe `doc.fonts` `loadingdone`, iframe `img` `load` yakalama dinleyicisi): `pw=iframe.clientWidth`, `ph=iframe.clientHeight`; `--pw --ph --m --bh` yaz; `paged` ise `contentRight = range.selectNodeContents(doc.body).getBoundingClientRect().right + scrollLeft`, `pageTotal = calc.pageCount(contentRight, pw)` (boş son sayfa yok), `pageIdx = pageFromRatio(curRatio, pageTotal)`, `scrollTo({left: pageIdx*pw, behavior:'auto'})`; kaydırma modunda `scrollTop = curRatio*maxScroll`. Konum her zaman `curRatio` (bölüm içi oran) ile korunur; mod geçişinde `curRatio` korunur. `// shortcut: oran korunur, yeniden akışta en çok ±1 sayfa kayar; gerekirse caret anchor`. 
- `goPage(i)`: `i>=pageTotal` → son bölüm değilse `loadChapter(ch+1, 0)`, son bölümse `showNextVolume()`; `i<0` → `ch>0` ise `loadChapter(ch-1, 1)` (yükleme sonrası son sayfa); aksi `scrollTo({left:i*pw, behavior: reduced ? 'auto' : 'smooth'})` (reduced: `document.documentElement.dataset.motion==='reduced'` veya `prefers-reduced-motion`); `curRatio=ratioFromPage(i,pageTotal)`; `tracker.addPage(ch+':'+i)`; `tracker.scheduleProgress(book.id, JSON.stringify({ch,p}), overall)`; gösterge güncelle.
- Tuşlar (sayfa modu): ←/PageUp/Shift+Space önceki sayfa, →/PageDown/Space sonraki sayfa; `[`/`]` bölüm; ↑/↓ yok sayılır (preventDefault yok); Home/End yok. Kaydırma modu mevcut davranış korunur ([ ] ve ←/→ bölüm, Space/PageDown ekran). Ctrl/Meta/Alt varsa hiçbiri çalışmaz.
- Dokunma/tık (sayfa modu, iframe belgesi `click`): bağlantı tıklaması önce işlenir; aksi `x/pw < 1/3` önceki sayfa, `> 2/3` sonraki, orta `showBars()` (çubukları aç/kapat). Yatay kaydırma: `scroll` olayında 120 ms sonra `Math.round(scrollLeft/pw)` sayfasına `scrollTo` ile yapış (kullanıcı kaydırması sayfa sayılır, `goPage` gibi progress yazar). `touch-action: pan-x`.
- Gösterge: alt çubukta `#text-indicator` (`type-label-medium`, `white-space:nowrap; overflow:hidden; text-overflow:ellipsis`): sayfa modunda `Bölüm {ch+1} · sayfa {pageIdx+1}/{pageTotal} · %{yüzde}`; kaydırma modunda `Bölüm {ch+1} · %{yüzde}`. Altında `#eta-label` (`type-label-small`, `calc.formatEta(book.pace_seconds, overall)`, boşsa gizli).
- `tracker.addPage` sayfa modunda `ch + ':' + pageIdx` (gerçek çevrilen sayfa); kaydırma modunda mevcut davranış.
- Ayarlar alt sayfası (`dialog.sheet.reader-settings`, `max-height: min(50vh, 420px)`, `overflow-y:auto`, `::backdrop{background:transparent}`, dışa tıklayınca kapanır; 360 px'e sığar). Sıra ve kontroller (her başlık `type-title-small`, her kaydırıcı `input.slider` + sağda değer metni, `aria-label` Türkçe): `Görünüm` (segmented: Açık, Koyu, Sepya, Siyah; 360 px'te check ikonu gizli), `Yazı tipi` (`ul.font-list[role=radiogroup]`, 7 `button.font-option[role=radio]`, her biri `style` YOK: `font-family` `style.fontFamily` CSSOM ile, metni ASCII font adı; seçili `aria-checked=true`), `Yazı boyutu` (A- `.icon-btn` + `.slider` `flex:1` + A+, tek satır, 17.2 kuralı), `Satır aralığı`, `Kelime arası`, `Harf arası`, `Kenar boşluğu`, `Hizalama` (segmented: Sol, İki yana), `Okuma modu` (segmented: Kaydırma, Sayfa), `Varsayılana dön` (`.btn.tonal`, `reset()` ve tüm kontrolleri günceller). Her değişiklik anında `set(patch)` + `applyFrameStyles()`.
- Tema `black`: `readerColors('black')` → `{bg:'#000000', fg: koyu palet on-surface, link: koyu palet primary}`.
- Kenar çubukları: top bar `[geri][başlık][yer imi][tam ekran][ayarlar]`; bottom bar `[içindekiler][önceki bölüm][ilerleme+gösterge+eta][sonraki bölüm]`.

### 20.8 Statik varlıklar (`src/app.rs`)
- Yeni sabitler: `RPFONTS: &[(&str, &[u8])]` (30 giriş, adı `fonts/…` olmadan değil, tam `"fonts/<dosya>"`, `include_bytes!`), `READER_FONTS_CSS`, `CALC_JS`, `READER_UI_JS`, `MANIFEST`, `ICON_192`, `ICON_512`. `static_handler` eşleşmeden önce `if let Some(b)=RPFONTS.iter().find(|(n,_)|*n==ad)`; `font/woff2`, `public, max-age=86400`. Yeni adlar: `calc.js`, `reader-ui.js`, `reader-fonts.css` (`no-cache`), `manifest.webmanifest` (`application/manifest+json`, `no-cache`), `icon-192.png`, `icon-512.png` (`image/png`, `public, max-age=86400`). `fonts/LICENSE-reader.txt` ve `fonts/LICENSE` SERVİS EDİLMEZ (404, mevcut test).
- Dosya adları `web/fonts/`: `literata-latin-wght-normal.woff2`, `literata-latin-wght-italic.woff2`, `literata-latin-ext-wght-normal.woff2`, `literata-latin-ext-wght-italic.woff2`; aynı kalıp `merriweather`, `source-serif-4`, `eb-garamond`, `inter`; `lexend-latin-wght-normal.woff2`, `lexend-latin-ext-wght-normal.woff2`; `atkinson-hyperlegible-{latin,latin-ext}-{400,700}-{normal,italic}.woff2`. Toplam 20+2+8=30 dosya, kaynak URL fontsource adlarının birebir aynısı (yukarıdaki doğrulama).
- `web/fonts/LICENSE-reader.txt`: her yazı tipi için telif satırı (fontsource paket sayfasındaki `package.json` `author`/lisans alanları: hepsi OFL-1.1) ve tam SIL OFL 1.1 metni bir kez (`web/fonts/LICENSE`'taki OFL metninden kopya). `README.md`'ye "Yazı tipleri ve lisansları" kısmı.
- `index.html`: `<link rel="manifest" href="/static/manifest.webmanifest">`, `<link rel="apple-touch-icon" href="/static/icon-192.png">`, `<script src="/static/calc.js">` ve `reader-ui.js` `reader-comic.js`ten ÖNCE, `reader-fonts.css` linki; `theme.js` hâlâ ilk betik.
- `manifest.webmanifest`: `{"name":"ReadCrab","short_name":"ReadCrab","lang":"tr","start_url":"/","scope":"/","display":"standalone","background_color":"#FEF7FF","theme_color":"#6750A4","icons":[{"src":"/static/icon-192.png","sizes":"192x192","type":"image/png","purpose":"any"},{"src":"/static/icon-512.png","sizes":"512x512","type":"image/png","purpose":"any"}]}`. Service worker yok.
- `scripts/make_pwa_icons.py <çıktı_dizini>` (yalnız `zlib`, `struct`, `sys`; `python3 -I` ile çalışır): düz `#6750A4` zemin üzerine ortalanmış beyaz yuvarlatılmış "açık kitap" (iki dikdörtgen) çizen 8 bit RGB PNG yazıcı; 192 ve 512; dosyalar `web/`e kopyalanır ve depoya eklenir.
- `tests/static_files.rs`: `expected` tablosuna yeni adlar ve içerik türleri; tüm 30 yazı tipi için `font/woff2` ve `wOF2` sihirli bayt; `icon-192.png`/`icon-512.png` için `\x89PNG`; `manifest.webmanifest` JSON ayrışır ve `icons.len()==2`; `index.html` testi `manifest`, `reader-fonts.css`, `calc.js`, `reader-ui.js` sırasını denetler (`calc.js` ve `reader-ui.js` `reader-comic.js`ten önce, `theme.js` ilk); `unknown…` testine `fonts/LICENSE-reader.txt` eklenir.
- `.github/workflows/ci.yml` test işine `node web/calc.test.mjs` adımı eklenir.

### 20.9 Tema: saf siyah (`theme.js`, `app.js` Hesap)
- `theme.js`: `getBlack()` (`localStorage['krs.black']==='1'`), `setBlack(bool)` (yaz, `apply` yeniden). `generatePalette(seed, dark, black)`: `black && dark` iken `surface`, `surface-dim` `#000000`; `surface-container-lowest` `#000000`; `-low` `tn(N,6)`, `-container` `tn(N,9)`, `-high` `tn(N,13)`, `-highest` `tn(N,17)`, `surface-bright` `tn(N,20)`; geri kalan roller koyu palet ile aynı. `apply` `dark` ve `getBlack()` ile üçüncü argümanı verir; `root.dataset.black = '1'` yalnız dark+black iken, aksi silinir; `theme-color` meta `pal.surface` (saf siyah açıkken `#000000`). Dışa açılır: `getBlack`, `setBlack`, `readerColors` `'black'` kabul eder (20.7).
- `theme.test.mjs` genişlemesi, 6 tohum için `generatePalette(seed,true,true)`: `surface==='#000000'` ve `surface-dim==='#000000'`; `on-surface` ile `surface` ve her `surface-container*` kontrastı ≥ 7; `on-surface-variant` ile `surface` ve `surface-container`, `-high` kontrastı ≥ 4.5; `on-primary/primary`, `on-primary-container/primary-container`, `on-secondary-container/secondary-container`, `on-tertiary-container/tertiary-container`, `on-error-container/error-container` ≥ 4.5; `-container-high` ile `surface` aynı değil ve parlaklık farkı > 0 (kartlar seçilebilir); `readerColors('black').bg==='#000000'`; `black=false` çıktısı mevcut koyu palet ile birebir aynı.
- Hesap > Görünüm: motion anahtarının altına aynı `input.switch[role=switch]` bileşeniyle `#black-switch`, etiket "Saf siyah" (yardım metni "Koyu modda AMOLED siyahı"); açıkken mod açık temaysa etkisiz, `aria-describedby` ile belirtilir. Çizgi roman sahnesi: `reader-comic.js` palet yazarken `generatePalette(seed,true,getBlack())`, sahne arka planı `getBlack()` ise `#000000`. Okuyucu sheet'inde de `krs.black` etkin.

### 20.10 Ön yüz: Kütüphane, kapak, geçmiş
- `makeCoverNode(bookId, titleText, v)`: `src = '/api/books/'+id+'/cover'+(v!==undefined && v!==null ? '?v='+v : '')`; tüm çağrılara `cover_v` aktarılır (seri kartı, seri sayfası, geçmiş satırı: geçmişte `?v` yok).
- Raf (`renderLibrary`): `searchBar`ın hemen altında `section.shelf[aria-label="Okumaya devam et"]` > `h2.type-title-medium` "Okumaya devam et" + `div.shelf-track[role=list]` (flex, `overflow-x:auto`, `scroll-snap-type:x proximity`, `gap:12px`, `padding-bottom:8px`, kaydırma çubuğu gizli, `scrollbar-width:none`) > `a.shelf-card[role=listitem][href="#/oku/{id}"]` (`flex:0 0 120px`, `scroll-snap-align:start`; 360 px'te kart 112 px): `.shelf-cover` (en-boy 2/3, `makeCoverNode`), `.shelf-title` (2 satır, `-webkit-line-clamp:2`), `createLinearProgress(percent, title+' ilerlemesi')`. `GET /api/continue` renderLibrary başında bir kez çağrılır (hata sessiz, raf yok); `state.q===''` VE `state.status==='all'` VE `items.length>0` ise görünür, aksi `hidden` (DOM'da kalır, `hidden` niteliği). Odak halkası `:focus-visible`; kaydırılabilir bölge anahtarla erişim axe için kartlar odaklanabilir bağlantıdır. `routeGen` denetimi mevcut kalıp.
- Seri sayfası: `openBookStatusDialog` içine yalnız `KRS.me.is_admin` iken "Kapağı değiştir" `.btn.tonal` satırı; tıklayınca `openCoverDialog(book, onSaved)` (yeni `dialog.md-dialog`): `input[type=file][accept="image/jpeg,image/png,image/webp"]` (etiketli), `URL.createObjectURL` önizleme `img` (kapanışta `revokeObjectURL`), `Yükle` (`fetch(PUT, {headers:{'Content-Type': file.type}, body:file, credentials:'same-origin'})`, 10 MiB üstü dosya istemcide reddedilir "Dosya 10 MB'tan büyük."; 400 mesajı toast), `Özel kapağı kaldır` (DELETE; `KRS.api('DELETE', …)`), başarıda `onSaved` (seriyi yeniden çiz, `toast`). Dosya yüklemesi `KRS.api`'yi kullanmaz (JSON başlığı yok); 401 → `#/giris`.
- Geçmiş (`stats.js::renderHistory`): sırası `section.heatmap-card` (heatmap), `section.rhythm` (Yılın ritmi), oturum listesi (`#history-table`). `renderHistory(mount, gen, opts)` `date` süzgecini `state.date` olarak tutar; yüklemeler `&date=` ekler; süzgeç etkinken listenin üstünde `.chip` "{8 Kas 2026} ×" (`aria-label="Gün süzgecini kaldır"`).
- Heatmap bileşeni (`stats.js::buildHeatmap(mount, gen, state)`): yıl seçici `select#heatmap-year` (`years` ∪ {içinde bulunulan yıl}, azalan; başlangıç içinde bulunulan yıl); yıl değişince `GET /api/stats/heatmap?year=` ve geçersiz 400/hata `.empty.error`+Yeniden dene. Çizim: `div.heatmap-scroll[role=region][aria-label][tabindex=0]` (`overflow-x:auto`; çizimden sonra `scrollLeft = scrollWidth`, yani en yeni taraf) > SVG (`hücre 12 px, boşluk 3 px`, solda gün etiketleri Pzt/Çar/Cum 28 px, üstte ay etiketleri `calc.yearGrid().months`), `rect.hm-cell.hm-{0..4}` her biri `<title>` "8 Kas 2026: 34 dk, 12 sayfa" (veri yoksa "8 Kas 2026: okuma yok"; tarih `formatHistoryDate`, süre `formatDuration`). Seviyeler `calc.heatThresholds(seconds)`/`heatLevel` (yıl başına). Sınıflar `style.css`: `.hm-0{fill:var(--md-sys-color-surface-container-high)}`, `.hm-1..4{fill:color-mix(in srgb, var(--md-sys-color-primary) 25%/45%/70%/100%, var(--md-sys-color-surface-container-high))}`, seçili `.hm-cell.selected{stroke:var(--md-sys-color-on-surface);stroke-width:2}`, `:focus-visible` halka. Klavye: SVG içinde roving `tabindex` (yalnız odaktaki hücre `0`), ←/→ ∓7 gün, ↑/↓ ∓1 gün (hücre satır sınırı geçilir), Home/End yıl başı/sonu, Enter/Space seçer; hücre `role=button`, `aria-label` başlık metniyle; tık/Enter `state.date=d` (seçili günse süzgeci kaldırır) ve listeyi yeniler. Altında açıklama `Az [5 küçük kare] Çok`. Koyu/açık/saf siyah: yalnız token sınıfları.
- "Yılın ritmi": `h2` + iki `chart-card` ( `buildBarChart`'a 8. parametre `opts={labelFn, maxLabels}`; ay grafiği 12 çubuk dk, etiketler `Oca…Ara`; gün grafiği 7 çubuk `Pzt…Paz`, 360 px'te ay etiketleri 6'ya düşer) + `dl.summary-grid` dört öğe: "En çok okuduğun ay" (ay adı + süre), "En çok okuduğun gün" (tarih + süre), "Okuduğun gün sayısı", "Günlük ortalama" (okuduğun günlerde; yardım metni). Veri yoksa "Bu yıl için veri yok".

### 20.11 Çizgi roman okuyucu (`reader-comic.js`)
- localStorage: `krs.dir.<bookId>` = `'ltr'|'rtl'` (varsayılan `ltr`), `krs.cmode.<bookId>` = `'page'|'vertical'` (varsayılan `page`). Mevcut `krs.fit` aynen.
- Yön `rtl` (yalnız `page` modunda): ←=sonraki sayfa, →=önceki; PageDown/Space/PageUp mantığı aynı kalır (sonraki/önceki); dokunma bölgeleri (sol/sağ üçte bir) ve yatay kaydırma yönü tersine döner; `Home/End` aynı. Locator değişmez.
- Dikey (webtoon): `div.webtoon` (`overflow-y:auto`, tam alan) içinde `count` adet `div.webtoon-page[data-idx]` (`min-height:70vh`); `IntersectionObserver` (`rootMargin: '200% 0px'`) görünür yer tutucuya `img.webtoon-img` (`loading="lazy"`, `alt="Sayfa N"`, `src=/api/books/{id}/pages/{n}`, `width:100%;height:auto`) ekler, yükleyince `min-height` kaldırılır; RAR/CBR için aynı anda en çok 2 görsel bekler (mevcut 11. madde sınırı). Görünür sayfa: `rAF` ile throttled `scroll`; ilk `offsetTop+offsetHeight > scrollTop + 0.3*clientHeight` olan `idx`; `PUT progress` locator `String(idx)`, `percent=(idx+1)/count`, `tracker.addPage(idx)`; geri yükleme: açılışta `idx` yer tutucusuna `scrollIntoView({block:'start'})` ve komşu ±2 görsel hemen yüklenir; `overflow-anchor:auto`. Klavye: ↑/↓ 60 px, PageUp/Down/Space ekran; ←/→ yok; `f` sığdırma gizli. Sonuna gelince (`idx==count-1` ve alt kenar görünür) `showNextVolume()`.
- Ayarlar alt sayfasına iki segmented: `Okuma yönü` (Soldan sağa, Sağdan sola; dikey modda `disabled`) ve `Görünüm` (Sayfa, Dikey; `sheet` kısa kalsın diye mevcut sığdırma satırı dikeyde gizli) ve "Yer imleri" düğmesi (liste sheet'i açar, 20.12). Çubuk: `[geri][başlık][yer imi][tam ekran][ayarlar]`.
- Mevcut `#page-indicator` ve testler aynı: dikeyde de `n / count`.

### 20.12 Ortak okuyucu bileşenleri (`web/reader-ui.js`, `KRS.readerUi`)
- `fullscreenButton()`: `document.fullscreenEnabled` yoksa `null`; aksi `.icon-btn` (`fullscreen` / `fullscreen_exit`, `aria-label` "Tam ekran"/"Tam ekrandan çık"), `document.documentElement.requestFullscreen()`/`document.exitFullscreen()`, `fullscreenchange` ile ikon; hatalar sessiz; cleanup'ta tam ekrandaysa çıkar.
- `keepAwake()` → `release()`: `window.isSecureContext && navigator.wakeLock` ise `request('screen')`, `visibilitychange` visible olunca yeniden alır, hata sessiz, kullanıcı arayüzü yok; cleanup `release`.
- `bookmarkController(book, {getLocator, getLabel, matches})`: `GET /api/books/{id}/bookmarks` yükler; `toggleBtn` (`bookmark`/`bookmark-fill` ikonu, `aria-label` "Yer imi ekle"/"Yer imini kaldır", `aria-pressed`); `matches(bm)`: görsel locator eşitliği, metin `ch` eşit ve `|Δp|<0.02`; ekle `POST` (label görselde `Sayfa N`, metinde `Bölüm N · %yüzde`), kaldır `DELETE`; `buildList(onGo)` `ul.list` ("Yer imi yok" boşken, her satır `go` + silme `.icon-btn`). Konum değişince `refresh()`.
- `nextVolumeCard(book)`: `book.next_book_id` yoksa `null`; aksi `div.next-volume[role=status]` > `a.btn.filled[href="#/oku/{id}"]` metni `Sonraki cilt: {next_book_title}`; okuyucu `.reader` altına, alt çubuğun üstüne konumlanır (`position:absolute; bottom:80px; left:50%; transform:translateX(-50%)`, 360 px'te `max-width:calc(100% - 32px)`); görünüm yalnız kitabın sonunda (görsel son sayfa; metin son bölüm `ratio>=0.98` veya sayfa modunda son sayfa); geri gidilince gizlenir.
- `etaLabel(book)` → `{el, update(overall)}` (`calc.formatEta`).
- İçindekiler alt sayfası (metin): üç sekme `İçindekiler | Yer imleri | Ara` (`role=tablist` segmented, `aria-selected`, panel `hidden`). `Ara` paneli `input[type=search]` ("Kitapta ara", 350 ms debounce, ≥2 karakter) → `GET /api/books/{id}/search?q=`; liste satırı `before` + `<mark>hit</mark>` + `after`, "Bölüm N" alt metin; `truncated` ise "İlk 100 sonuç gösteriliyor"; tıklayınca sheet kapanır, `loadChapter(ch, p, true)`; 429/hata toast. Görsel okuyucuda arama sekmesi yok.
- `style.css` yeni sınıflar: `.shelf*`, `.heatmap-*`, `.hm-*`, `.rhythm`, `.summary-grid`, `.font-list`, `.font-option`, `.reader-settings`, `.next-volume`, `.webtoon`, `.webtoon-page`, `.webtoon-img`, `.tab-panel`. Satır içi stil yok (yalnız CSSOM değişkenleri); 15.6 düzeni, `--md-sys-*` tokenları ve ≥44 px hedef kuralı korunur. Eksik ikonlar `scripts/build_icons.py` `NAMES`e eklenip `web/icons.svg`e işlenir: `bookmark`, `bookmark-fill` (fill varyantı kalıbı mevcut `*-fill` ile), `fullscreen_exit`, `image`.

### 20.13 Testler (hepsi `cargo test` veya `node`)
- `tests/migration.rs` (20.2).
- `tests/iter2.rs`: `continue_lists_reading_only` (reading+missing=0, 20 üst sınır, `updated_at DESC`, başka kullanıcının kaydı yok, finished/unread hariç); `book_detail_next_prev_and_pace` (3 ciltli seri sort_index sırası, `error` kitabı atlanır, son cilt `next=null`; pace: yetersiz oturum `null`, 3×120 sn ve toplam %6 ilerleme → `pace_seconds==6000`); `heatmap_isolated_per_user` (iki kullanıcı, yalnız kendi günleri; `years`; yıl 1999/2101/`abc`/eksik → 400; Aralık 31 ve Ocak 1 sınırı yıl dışına taşmaz); `history_date_filter` (doğru `total`, geçersiz tarih 400, başka kullanıcının günü boş); `search_epub_turkish_fold` (`ışık`↔`IŞIK`↔`İŞİK` gibi, ±40 parça, 100 sınırı `truncated`, 1 karakterlik `q` 400, görsel kitap 400, HTML etiketi içinde kalan `<p>şeker</p>` bulunur ama `<title>` metni bulunmaz, MOBI bölümü); `search_concurrency_429` birim: iki izin tutulurken üçüncü 429.
- `tests/bookmarks.rs`: ekle/listele, başka kullanıcının silinmesi 404, başkasının kitabı listesinde görünmez (kendi listesi boş), 101. kayıt 409, geçersiz locator 400, missing kitap 404, yetkisiz 401, ardından `DELETE FROM users` ile CASCADE.
- `tests/cover_custom.rs`: admin dışı PUT/DELETE 403; PNG 800×1200 yükle → 200, `GET cover` 600 genişlik JPEG, `cover_version` 1, `cover_book_id` özel kapaklı kitabı seçer (seride 2 kitap, ikinci kitaba yüklenince); bozuk bayt 400; 11 MiB 413; `image/gif` 415; sniff PNG değil metin `image/png` başlığıyla 400; 16385×1 PNG (piksel sınırı/boyut) 400; yeniden `scan_once` sonrası dosya baytları aynı ve `custom_cover=1`; DELETE → `custom_cover=0`, `cover_version` 2, kapak 300 genişlik çıkarılmış hâline döner; kapaksız kitapta DELETE `has_cover=0`.
- `tests/static_files.rs`, `tests/security.rs`, `tests/formats.rs`, `tests/reader.rs` (20.4, 20.5, 20.8).
- `web/calc.test.mjs`, `web/theme.test.mjs` (20.6, 20.9).

### 20.14 e2e (`scripts/e2e/reader.spec.mjs`, `make_books.py`)
Fikstür eklemeleri (`make_books.py`): `Üçüncü Uzun Seri/Cilt 1.epub` ve `Cilt 2.epub` (her biri 2 bölüm, bölüm başına ~60 Türkçe paragraf ve bir PNG, "kelime" ve "şeker" geçen cümle) ve `Üçüncü Dikey/dikey.cbz` (8 tall PNG 600×1800). "Test Serisi" kütüphanede ilk kart kalır (Ü sonrası). Mevcut testler korunur; hiçbir axe kuralı kapatılmaz.
Yeni/genişleyen senaryolar:
1. Sayfa modu: ayarlar sheet'inde `Sayfa` seç; `#text-indicator` `^Bölüm 1 · sayfa 1/\d+ · %\d+$`; → ile `sayfa 2/`; ← geri; son sayfada → sonraki bölüme (indikatör `Bölüm 2 · sayfa 1/`); ilk sayfada ← önceki bölümün SON sayfası; yanlardan tık (iframe sol/sağ üçte bir); konum koruma: yazı boyutu 18→24 yapılınca `curRatio` farkı ≤ `1/(n-1)+0.02`; mod `Kaydırma`ya geçince iframe `scrollTop/maxScroll` ≈ önceki oran (±0.1); pencere boyutu 1280→360 (setViewportSize) ardından oran korunur; son sayfada boş sayfa yok (son sütunda metin görünür: `Range.getBoundingClientRect().right > (n-1)*pw`); görseller `getBoundingClientRect().height <= bh`; `prefers-reduced-motion` altında `scroll-behavior` smooth kullanılmaz.
2. Ayarlar çerçeveye uygulanır: yazı tipini `Lexend` yap → iframe `html` hesaplı `font-family` içerir `Lexend Variable`, `--lh` değeri kaydırıcı sonrası; `Siyah` tema → iframe `html` arka planı `rgb(0, 0, 0)`; `Hizalama İki yana` → `text-align: justify`; ayar kalıcı: yeniden yüklemede aynı; `localStorage['krs.readerPrefs']` JSON ayrışır; `krs.fs=22` ve `krs.readerTheme=sepia` ile (`krs.readerPrefs` yok) açılışta göç eder.
3. Font yüklenmesi + CSP: 7 yazı tipi için sırayla seç; her seçimden sonra `await frame.evaluate(() => document.fonts.ready)` ve `document.fonts.check('16px "<aile>"', 'şğıİ')` true; ağda `/static/fonts/*.woff2` 200; konsol/sayfa hatası ve `Refused to` yok (`assertNoCspOrPageErrors`).
4. Raf: kütüphane `#/` okunmuş kitaptan sonra `.shelf` görünür ve kartı tıklayınca `#/oku/`; arama yazılınca `.shelf[hidden]`; durum süzgeci `Okunuyor` iken gizli; temizlenince döner; 360 px'te `.shelf-track` `scrollWidth > clientWidth` ve `scroll-snap-type` değeri `x proximity`; klavyeyle Tab ile kart odaklanır.
5. Heatmap: `#/gecmis` `.heatmap-scroll` görünür, bugünün hücresi `rect.hm-cell` seviye ≥1 ve `<title>` `/\d+ dk|\d+ sn/`; tıkla → `#history-table li` yalnız o gün (liste ve `chip` görünür), `×` ile süzgeç kalkar; klavye: hücreye odak, `ArrowLeft`, `Enter`; 360 px'te `scrollLeft + clientWidth ≈ scrollWidth` (±2); Yılın ritmi iki grafik ve dört özet; farklı yıl seçince boş durum hata vermez.
6. Saf siyah: Hesap'ta `#black-switch` aç, koyu şemada `getComputedStyle(document.documentElement).getPropertyValue('--md-sys-color-surface').trim()==='#000000'` ve `meta[name=theme-color]` `#000000`; kapatınca değişir; `krs.black` kalıcı.
7. Özel kapak (seri sayfası, admin): "Kapağı değiştir" → dosya seç (testte üretilmiş 800×1200 PNG buffer) → Yükle; `img` `src` `?v=1` içerir ve `naturalWidth==600`; kütüphane kartı aynı sürümü gösterir; "Özel kapağı kaldır" sonrası `naturalWidth==300`.
8. Yer imi ve arama: metin okuyucuda yer imi düğmesi `aria-pressed=true`, içindekiler sheet'inde `Yer imleri` sekmesi satırı, silince boş; `Ara` sekmesine "şeker" yaz → ≥1 sonuç `mark` içerir, tıklayınca sheet kapanır ve sayfa/bölüm değişir; görsel okuyucuda yer imi ekle/kaldır.
9. Sonraki cilt: `Cilt 1` son bölüm sonunda `a:has-text("Sonraki cilt: Cilt 2")` görünür ve tıklayınca `#/oku/`.
10. Webtoon: `dikey.cbz` ayarlardan `Dikey`; `.webtoon-page` sayısı 8; kaydırınca `#page-indicator` ve `PUT progress` locator artar; `krs.cmode.<id>==='vertical'`; `Sağdan sola` (sayfa modunda): ← sonraki sayfaya götürür.
11. Tam ekran düğmesi: desteklenirse `aria-label` "Tam ekran" görünür (headless Chromium'da `fullscreenEnabled` true; tıklama sonrası `document.fullscreenElement` set edilmeye çalışılır, hata beklenmez, yalnız konsol hatası yok). Manifest: `GET /static/manifest.webmanifest` 200, `link[rel=manifest]` var, "Refused to" yok.
Ekran görüntüleri (360 ve 1280, açık/koyu; mevcut adlandırma `ad-{genişlik}-{şema}.png`): `kutuphane-raf` (raf görünür halde), `okuyucu-sayfa-modu` (EPUB, sayfa modu), `okuyucu-ayarlar-yeni` (yeni ayarlar sheet'i, axe dahil), `gecmis-heatmap` (heatmap+ritim), `kutuphane-saf-siyah` (koyu şemada `krs.black=1`), `okuyucu-webtoon`. Her biri `runAxeCheck`'ten geçer.

### 20.15 Görevler (bağımlılık sırasıyla)

1. **İŞÇİ-1**: `0002_iter2.sql`, `db.rs` ve `tests/migration.rs` (20.2).
2. **İŞÇİ-1**: `books` detay alanları (next/prev/pace/cover_version), `/api/continue`, `cover_v` alanları ve özel kapak öncelikli kapak seçimi (`library.rs`), `GET /api/stats/heatmap`, `history?date=` ve `tests/iter2.rs` ilgili testleri (20.3-1…4, 8, 9).
3. **İŞÇİ-1**: `bookmarks.rs`, `api/mod.rs` bağlama, `tests/bookmarks.rs`.
4. **İŞÇİ-1**: `textsearch.rs`, `library::fold_char`, `/search` ucu, birim testleri ve `tests/iter2.rs` arama testleri.
5. **İŞÇİ-1**: `cover.rs` (`encode_custom_cover`, PUT/DELETE), `scan.rs` özel kapak atlama, `auth.rs` guard (20.4), `tests/cover_custom.rs` ve `tests/security.rs` eklemeleri.
6. **İŞÇİ-2**: Yazı tipi dosyalarının indirilmesi (yeni boş dizine `curl`, `scripts/fonts/README.txt` listesiyle; her dosyanın `wOF2` baytı ve 20.8'deki toplam boyut ile doğrulanması; `web/fonts/`e kopya), `LICENSE-reader.txt`, `web/reader-fonts.css`.
7. **İŞÇİ-2**: `scripts/make_pwa_icons.py`, `web/icon-192.png`, `web/icon-512.png`, `manifest.webmanifest`, `build_icons.py` `NAMES` ve `icons.svg` (4 yeni ikon, her biri tek `<path>`).
8. **İŞÇİ-1**: `app.rs` statik tablo (`RPFONTS`, yeni adlar), `sanitize.rs` (`FRAME_CSP`, `frame_document` link), `index.html` düzeni, `tests/static_files.rs`, `tests/formats.rs`, `tests/reader.rs` güncellemeleri.
9. **İŞÇİ-1**: `web/calc.js` + `calc.test.mjs`, CI'ye `node web/calc.test.mjs` (20.6).
10. **İŞÇİ-1**: `theme.js` saf siyah, `theme.test.mjs` genişlemesi, Hesap anahtarı, tema `black` okuyucu rengi (20.9).
11. **İŞÇİ-1**: `reader-ui.js` (20.12) ve `style.css` yeni sınıfları.
12. **İŞÇİ-1**: `frame.css`, `reader-text.js` (prefs, ayarlar sheet'i, sayfa modu, yer imi, arama sekmesi, kalan süre, sonraki cilt, tam ekran, wake lock) (20.7, 20.12).
13. **İŞÇİ-1**: `reader-comic.js` (yön, webtoon, yer imi, kalan süre, sonraki cilt, tam ekran, wake lock) (20.11).
14. **İŞÇİ-1**: `app.js` kütüphane rafı, `makeCoverNode` `?v=`, kapak diyaloğu (20.10).
15. **İŞÇİ-1**: `stats.js` heatmap, yılın ritmi, tarih süzgeci, `buildBarChart` `opts` (20.10).
16. **İŞÇİ-2**: `make_books.py` yeni fikstürler (20.14), `tracker.js`/`app.js` yalnız gerekli `krs.readerPrefs` ve `routeGen` uyumlandırmaları yoksa dokunma.
17. **İŞÇİ-1**: `reader.spec.mjs` senaryoları ve ekran görüntüleri (20.14).
18. **İŞÇİ-2**: `README.md` (yazı tipi atıfları, PWA "ana ekrana ekle", yeni özellikler, `0002` migrasyonu notu, `manifest` çevrimdışı çalışmaz uyarısı) ve `scripts/smoke.sh` gerekirse `/static/manifest.webmanifest` denetimi.

Önerilen mantıksal commit grupları (her biri sonunda CI `test` işi yeşil; `e2e` yalnız 6., 7. grupta güncellenir):
- A: görev 1 (migrasyon ve test).
- B: görev 2, 3, 4 (okuma ve veri uçları, testleri).
- C: görev 5 (özel kapak, guard).
- D: görev 6, 7, 8 (varlıklar, CSP, statik tablo ve testleri; sırayla aynı commit).
- E: görev 9, 10, 11 (calc, tema, reader-ui; yalnız `node --check` ve birim testleri).
- F: görev 12, 13 (okuyucular).
- G: görev 14, 15, 16 (kütüphane rafı, kapak, heatmap, fikstürler).
- H: görev 17, 18 (e2e, ekran görüntüleri, README).

### 20.16 CEO kararları
- Heatmap 53 sütun, gereken yıllarda 54 sütun. "En çok okuduğun gün" tek tarih; "Günlük ortalama" okunan günlere bölünür. Kalan süre yalnız o kitabın oturumlarından; yetersiz veride gizli. `next_book_title` kabul. Tireleme yalnız "İki yana"da. Wake Lock düğmesiz, otomatik. Atkinson statik 400/700 kabul.
- Bu turda commit etiketleri `Claude Opus 5.5` diye sabit DEĞİLDİR: her işçi kendi oturumunun sistem hatırlatıcısındaki attribution satırlarını kullanır. Önceki bölümlerdeki sabit etiket kuralı iptal.
- Mevcut kullanıcı verisini etkileyen hiçbir şey (`user_version`, locator, `krs.*`) bölüm 20 dışında değişmez.
