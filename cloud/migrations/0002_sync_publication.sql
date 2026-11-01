-- Allocate journal positions only after obtaining the per-user publication lock.
-- The lock lasts until commit, including writers outside the HTTP service.
ALTER TABLE sync_changes ALTER COLUMN cursor DROP DEFAULT;
CREATE FUNCTION publish_sync_cursor() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  PERFORM 1 FROM users WHERE id = NEW.user_id FOR UPDATE;
  NEW.cursor := nextval('sync_changes_cursor_seq');
  RETURN NEW;
END;
$$;
CREATE TRIGGER sync_publication BEFORE INSERT ON sync_changes
FOR EACH ROW EXECUTE FUNCTION publish_sync_cursor();
