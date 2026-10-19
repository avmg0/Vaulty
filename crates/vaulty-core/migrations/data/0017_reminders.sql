-- Reminders shown as system notifications while the profile is unlocked.
ALTER TABLE user_settings ADD COLUMN notifications TEXT NOT NULL DEFAULT 'on';
-- Off by default: notifications can appear on a locked screen.
ALTER TABLE user_settings ADD COLUMN notify_amounts INTEGER NOT NULL DEFAULT 0;
-- Opt-in: keep only due dates (no names or amounts) unencrypted to remind while locked.
ALTER TABLE user_settings ADD COLUMN notify_locked INTEGER NOT NULL DEFAULT 0;

-- Which reminders have been shown, so each is shown once. Bookkeeping, kept out of the history.
CREATE TABLE reminders_sent (
    key TEXT PRIMARY KEY,
    at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
