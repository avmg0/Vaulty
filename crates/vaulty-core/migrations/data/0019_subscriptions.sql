-- Places the member said aren't subscriptions, so they're not suggested again.
CREATE TABLE subscription_dismissed (
    key TEXT PRIMARY KEY,
    at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
