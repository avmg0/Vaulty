-- Checking for new versions is off until the person turns it on (Vaulty works offline by default).
ALTER TABLE user_settings ADD COLUMN update_check TEXT NOT NULL DEFAULT 'off';

-- When this profile last checked. Bookkeeping only, so it's kept out of the undo history.
CREATE TABLE update_checks (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_at TEXT NOT NULL
);
