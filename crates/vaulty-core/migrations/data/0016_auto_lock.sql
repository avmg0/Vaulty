-- Auto-lock moves from the window's storage into the profile, so the app itself can enforce it.
-- Minutes without use before the profile locks; 0 means never.
ALTER TABLE user_settings ADD COLUMN auto_lock_minutes INTEGER NOT NULL DEFAULT 5;
