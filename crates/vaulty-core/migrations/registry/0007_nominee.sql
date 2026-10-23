-- The helpers' secret and its line, sealed under the member's data key. Lets the member (and only
-- them, with their password) issue a nominee card later without changing their helpers.
ALTER TABLE helper_sets ADD COLUMN line_sealed TEXT;

-- Who may see a read-only summary of the member's money if something happens to them: a printed
-- nominee card plus one helper, after a waiting period the member can cancel by signing in.
CREATE TABLE nominees (
  user_id        TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  wait_days      INTEGER NOT NULL DEFAULT 7,
  letter_sealed  TEXT,                    -- the member's letter to them, sealed under the data key
  created_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE nominee_requests (
  user_id     TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  started_at  INTEGER NOT NULL,
  allowed     INTEGER NOT NULL DEFAULT 0  -- the member signed in and let it go ahead now
);
