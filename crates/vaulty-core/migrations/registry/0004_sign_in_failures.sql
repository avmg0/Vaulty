-- Wrong passwords and codes, kept until the next successful sign-in so the member can be told
-- about them (the throttle counter alone resets on success).
CREATE TABLE sign_in_failures (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    at      INTEGER NOT NULL
);
CREATE INDEX sign_in_failures_user ON sign_in_failures (user_id, at);

-- What to tell the member after their next sign-in (JSON), set when a sign-in succeeds.
ALTER TABLE users ADD COLUMN failure_notice TEXT;
