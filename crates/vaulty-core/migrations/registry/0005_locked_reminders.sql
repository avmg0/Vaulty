-- Dates a member asked to be reminded about while their profile is locked: only the date, no
-- names or amounts (the member's data stays encrypted). Replaced each time they lock.
CREATE TABLE locked_reminders (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date    TEXT NOT NULL,
    PRIMARY KEY (user_id, date)
);
