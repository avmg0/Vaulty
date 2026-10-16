-- Shared items carry exact minor units, like the members' own ledgers (data migration 0004).
ALTER TABLE shared_items ADD COLUMN amount_minor INTEGER NOT NULL DEFAULT 0;
ALTER TABLE shared_items ADD COLUMN amount_scale INTEGER NOT NULL DEFAULT 100;

UPDATE shared_items SET amount_scale = CASE
  WHEN currency IN ('BIF','CLP','DJF','GNF','ISK','JPY','KMF','KRW','PYG','RWF','UGX','UYI','VND','VUV','XAF','XOF','XPF') THEN 1
  WHEN currency IN ('BHD','IQD','JOD','KWD','LYD','OMR','TND') THEN 1000
  ELSE 100 END;
UPDATE shared_items SET amount_minor = CAST(ROUND(amount * amount_scale) AS INTEGER);

ALTER TABLE shared_items DROP COLUMN amount;
