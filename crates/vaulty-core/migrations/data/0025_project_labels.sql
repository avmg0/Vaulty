-- A project's own names for the standard columns ("Part" instead of "Item", "Supplier" instead
-- of "Bought from"): {"item": "Part", "vendor": "Supplier"}. Unnamed ones keep the app's words.
ALTER TABLE projects ADD COLUMN labels TEXT NOT NULL DEFAULT '{}';
