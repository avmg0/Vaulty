-- Link an account to a bank / card issuer / broker from the bundled directory
-- (src/institutions.rs), and let the user attach their own logo image.
ALTER TABLE accounts ADD COLUMN institution_id TEXT;
-- Small image as a data URL (PNG / JPEG / WebP, resized in the app). Stored only in the
-- member's own database; never shipped with the app.
ALTER TABLE accounts ADD COLUMN logo TEXT;
