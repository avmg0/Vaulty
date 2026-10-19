//! Keyboard shortcuts: a fixed list of actions with defaults, and each member's own changes.
//!
//! Keys are written as `Mod+Shift+K`: `Mod` is Ctrl on Windows and Linux, Cmd on macOS; then
//! optional `Shift` and `Alt`; then one key (a letter, digit, punctuation key or F1–F12). Only
//! changed shortcuts are stored; an empty string turns a shortcut off.
use crate::error::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

/// Every action that can have a shortcut, with its default (`""` = none until the member sets one).
pub const ACTIONS: &[(&str, &str)] = &[
    ("quick_add", "Mod+J"),
    ("palette", "Mod+K"),
    ("undo", "Mod+Z"),
    ("hide_balances", "Mod+H"),
    ("lock", "Mod+L"),
    ("panic", "Mod+Shift+L"),
    ("sidebar", "Mod+B"),
    ("settings", "Mod+,"),
    ("go_dashboard", "Mod+1"),
    ("go_transactions", "Mod+2"),
    ("go_accounts", "Mod+3"),
    ("go_reports", "Mod+4"),
    ("go_budgets", ""),
    ("go_recurring", ""),
    ("go_cards", ""),
    ("go_import", ""),
];

/// Combinations the system or the web view already use; taking them would break copy, paste,
/// quitting, closing the window and so on.
const RESERVED: &[&str] = &[
    "Mod+A", "Mod+C", "Mod+V", "Mod+X", "Mod+Y", "Mod+Q", "Mod+W", "Mod+R", "Mod+N", "Mod+T", "Mod+P", "Mod+F",
    "Mod+Shift+Z", "Mod+Alt+I", "Mod+Shift+I", "Mod+M", "Mod+Shift+R",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Hotkey {
    pub action: String,
    /// What it is now (`""` = off).
    pub keys: String,
    pub default: String,
    pub custom: bool,
}

fn default_of(action: &str) -> Option<&'static str> {
    ACTIONS.iter().find(|(a, _)| *a == action).map(|(_, k)| *k)
}

/// Puts a combination in the one canonical form, or explains what's wrong with it.
pub fn normalize(keys: &str) -> Result<String> {
    let bad = |m: &str| Error::Invalid(m.to_string());
    let parts: Vec<&str> = keys.split('+').map(str::trim).collect();
    // A "+" key itself would split into empty parts; it isn't offered.
    if parts.iter().any(|p| p.is_empty()) { return Err(bad("that key can't be used in a shortcut")); }
    let (mods, key) = parts.split_at(parts.len() - 1);
    let (mut m, mut shift, mut alt) = (false, false, false);
    for p in mods {
        match p.to_ascii_lowercase().as_str() {
            "mod" | "ctrl" | "cmd" | "meta" | "control" | "command" => m = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            _ => return Err(bad("use Ctrl (Cmd on a Mac), Shift and Alt with one other key")),
        }
    }
    let key = key[0];
    let k = if key.len() == 1 {
        let c = key.chars().next().unwrap_or(' ').to_ascii_uppercase();
        if !(c.is_ascii_alphanumeric() || ",./;'[]\\-=`".contains(c)) { return Err(bad("that key can't be used in a shortcut")); }
        c.to_string()
    } else if let Some(n) = key.strip_prefix(['F', 'f']).and_then(|n| n.parse::<u8>().ok()).filter(|n| (1..=12).contains(n)) {
        format!("F{n}")
    } else {
        return Err(bad("that key can't be used in a shortcut"));
    };
    // Plain letters would fire while typing; function keys may stand alone.
    if !m && !alt && !k.starts_with('F') { return Err(bad("a shortcut needs Ctrl (Cmd on a Mac) or Alt")); }
    let mut out = String::new();
    if m { out.push_str("Mod+"); }
    if shift { out.push_str("Shift+"); }
    if alt { out.push_str("Alt+"); }
    out.push_str(&k);
    if RESERVED.contains(&out.as_str()) { return Err(bad("that shortcut belongs to the system (copy, paste, quit and so on)")); }
    Ok(out)
}

/// All shortcuts for this profile, in the fixed order of `ACTIONS`.
pub fn list(conn: &Connection) -> Result<Vec<Hotkey>> {
    ACTIONS.iter().map(|(action, default)| {
        let custom: Option<String> = conn.query_row("SELECT keys FROM hotkeys WHERE action = ? AND is_custom = 1", [action], |r| r.get(0)).optional()?;
        Ok(Hotkey { action: action.to_string(), keys: custom.clone().unwrap_or_else(|| default.to_string()), default: default.to_string(), custom: custom.is_some() })
    }).collect()
}

/// Changes one shortcut. `None` puts back the default; `Some("")` turns it off. Refuses a
/// combination another action already uses, naming that action.
pub fn set(conn: &Connection, action: &str, keys: Option<&str>) -> Result<Vec<Hotkey>> {
    let default = default_of(action).ok_or_else(|| Error::Invalid(format!("unknown shortcut action `{action}`")))?;
    let wanted = match keys {
        None => default.to_string(),
        Some(k) if k.trim().is_empty() => String::new(),
        Some(k) => normalize(k)?,
    };
    if !wanted.is_empty() {
        if let Some(other) = list(conn)?.into_iter().find(|h| h.action != action && h.keys == wanted) {
            return Err(Error::Invalid(format!("{wanted} is already used for {}", other.action)));
        }
    }
    if wanted == default {
        conn.execute("DELETE FROM hotkeys WHERE action = ?", [action])?;
    } else {
        conn.execute(
            "INSERT INTO hotkeys (id, action, keys, category, is_custom) VALUES (?1, ?1, ?2, 'app', 1)
             ON CONFLICT (action) DO UPDATE SET keys = excluded.keys, is_custom = 1, updated_at = CURRENT_TIMESTAMP",
            params![action, wanted],
        )?;
    }
    list(conn)
}

/// Every shortcut back to its default.
pub fn reset_all(conn: &Connection) -> Result<Vec<Hotkey>> {
    conn.execute("DELETE FROM hotkeys", [])?;
    list(conn)
}
