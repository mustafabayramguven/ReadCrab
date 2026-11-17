-- Yazan sekme/cihaz: başka bir cihazın daha yeni konumu eski bir sekme tarafından ezilmesin.
ALTER TABLE progress ADD COLUMN client_id TEXT;
