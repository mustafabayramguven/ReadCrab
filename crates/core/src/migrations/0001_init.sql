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
