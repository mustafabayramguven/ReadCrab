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
