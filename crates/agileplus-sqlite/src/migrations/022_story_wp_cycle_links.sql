-- UP
CREATE TABLE IF NOT EXISTS story_work_packages (
    story_id INTEGER NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
    wp_id    INTEGER NOT NULL REFERENCES work_packages(id) ON DELETE CASCADE,
    PRIMARY KEY (story_id, wp_id)
);

CREATE INDEX IF NOT EXISTS idx_story_work_packages_wp ON story_work_packages(wp_id);

CREATE TABLE IF NOT EXISTS cycle_stories (
    cycle_id INTEGER NOT NULL REFERENCES cycles(id) ON DELETE CASCADE,
    story_id INTEGER NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
    added_at TEXT    NOT NULL,
    PRIMARY KEY (cycle_id, story_id)
);

CREATE INDEX IF NOT EXISTS idx_cycle_stories_story ON cycle_stories(story_id);

-- DOWN
DROP INDEX IF EXISTS idx_cycle_stories_story;
DROP TABLE IF EXISTS cycle_stories;
DROP INDEX IF EXISTS idx_story_work_packages_wp;
DROP TABLE IF EXISTS story_work_packages;
