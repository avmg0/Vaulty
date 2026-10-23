-- Settings for this device rather than a member: the tray icon, the quick-add shortcut and
-- whether Vaulty keeps running when its window is closed. Nothing private.
CREATE TABLE app_settings (
  k  TEXT PRIMARY KEY,
  v  TEXT NOT NULL
);
