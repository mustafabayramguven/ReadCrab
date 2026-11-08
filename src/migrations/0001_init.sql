CREATE TABLE users(
  id INTEGER PRIMARY KEY,
  username TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash TEXT NOT NULL,
  is_admin INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE TABLE sessions(
  token_hash TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  last_seen INTEGER NOT NULL
);
CREATE INDEX idx_sessions_user ON sessions(user_id);

CREATE TABLE series(
  id INTEGER PRIMARY KEY,
  rel_path TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  added_at INTEGER NOT NULL
);
CREATE TABLE books(
  id INTEGER PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES series(id),
  rel_path TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  format TEXT NOT NULL CHECK(format IN('epub','mobi','cbz','zip','rar','cbr')),
  size INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  content_id TEXT NOT NULL,
  page_count INTEGER NOT NULL DEFAULT 0,
  has_cover INTEGER NOT NULL DEFAULT 0,
  sort_index INTEGER NOT NULL DEFAULT 0,
  error TEXT,
  missing INTEGER NOT NULL DEFAULT 0,
  added_at INTEGER NOT NULL
);
CREATE INDEX idx_books_series ON books(series_id, missing, sort_index);
CREATE INDEX idx_books_content ON books(content_id);

CREATE TABLE progress(
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  book_id INTEGER NOT NULL REFERENCES books(id),
  locator TEXT NOT NULL DEFAULT '',
  percent REAL NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'unread' CHECK(status IN('unread','reading','finished','dropped')),
  finished_date TEXT,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(user_id, book_id)
);
CREATE TABLE reading_sessions(
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  book_id INTEGER NOT NULL REFERENCES books(id),
  client_id TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  ended_at INTEGER NOT NULL,
  active_seconds INTEGER NOT NULL,
  pages_read INTEGER NOT NULL DEFAULT 0,
  start_percent REAL NOT NULL,
  end_percent REAL NOT NULL,
  local_date TEXT NOT NULL,
  UNIQUE(user_id, client_id)
);
CREATE INDEX idx_rs_user_date ON reading_sessions(user_id, local_date);
CREATE TABLE goals(
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  year INTEGER NOT NULL,
  target INTEGER NOT NULL CHECK(target BETWEEN 1 AND 1000),
  PRIMARY KEY(user_id, year)
);
