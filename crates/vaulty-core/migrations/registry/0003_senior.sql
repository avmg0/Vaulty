-- Senior mode: larger text and buttons and a shorter menu. Kept here (not in the member's
-- encrypted data) so the sign-in screen can use it and an admin can set it up for someone.
ALTER TABLE users ADD COLUMN senior INTEGER NOT NULL DEFAULT 0;
