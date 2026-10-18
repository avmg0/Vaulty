-- Display formats per profile. "auto" follows the profile's language and region.
ALTER TABLE user_settings ADD COLUMN decimals TEXT NOT NULL DEFAULT 'auto';

-- Nobody could change these before (there was no setting for them), so the old built-in
-- defaults become "auto" rather than forcing US-style dates and a January year on everyone.
UPDATE user_settings SET date_format = 'auto' WHERE date_format = 'MM/dd/yyyy' OR date_format IS NULL;
UPDATE user_settings SET start_of_week = 'auto' WHERE start_of_week = 'sunday' OR start_of_week IS NULL;
UPDATE user_settings SET fiscal_year_start = 'auto' WHERE fiscal_year_start = 'january' OR fiscal_year_start IS NULL;
UPDATE user_settings SET negative_format = 'minus' WHERE negative_format IS NULL;
