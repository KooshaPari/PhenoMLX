-- UP: add labels column to features table (stored as JSON array text)
ALTER TABLE features ADD COLUMN labels TEXT NOT NULL DEFAULT '[]';

-- DOWN
-- ALTER TABLE features DROP COLUMN labels;  -- SQLite doesn't support DROP COLUMN on older versions
