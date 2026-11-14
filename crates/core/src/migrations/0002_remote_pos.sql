ALTER TABLE progress ADD COLUMN remote_locator TEXT NOT NULL DEFAULT '';
ALTER TABLE progress ADD COLUMN remote_percent REAL NOT NULL DEFAULT 0;
ALTER TABLE progress ADD COLUMN remote_status TEXT NOT NULL DEFAULT '';
ALTER TABLE progress ADD COLUMN remote_updated_at INTEGER NOT NULL DEFAULT 0;   -- 0: öneri yok
ALTER TABLE progress ADD COLUMN declined_ts INTEGER NOT NULL DEFAULT 0;         -- "Burada kal" denen sunucu damgası
