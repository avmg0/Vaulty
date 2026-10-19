//! Refuses passwords that are easy to guess: the most common ones (a bundled list, so this works
//! offline), a common word with a few digits or symbols added, the person's own name, or one
//! character repeated. Length rules are checked separately.
use std::{collections::HashSet, sync::LazyLock};

static COMMON: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    include_str!("../data/common-passwords.txt").lines().filter(|l| !l.is_empty() && !l.starts_with('#')).collect()
});

/// Why `password` is too easy to guess, or `None` if it's fine. `name` is the profile's name.
pub fn weakness(password: &str, name: &str) -> Option<&'static str> {
    let p = password.trim().to_lowercase();
    if p.chars().collect::<HashSet<_>>().len() <= 1 {
        return Some("that password is one character repeated; choose something harder to guess");
    }
    if COMMON.contains(p.as_str()) {
        return Some("that's one of the most common passwords; choose something less obvious");
    }
    // A common word with up to four digits or symbols added before or after ("dragon2024", "@123password").
    let core = p.trim_end_matches(|c: char| !c.is_alphabetic()).trim_start_matches(|c: char| !c.is_alphabetic());
    let added = p.chars().count() - core.chars().count();
    if added > 0 && added <= 4 && core.chars().count() >= 4 && COMMON.contains(core) {
        return Some("that's a common password with a few characters added; choose something less obvious");
    }
    // The person's name (any part of it, 3+ letters) with little else.
    let letters: String = p.chars().filter(|c| c.is_alphabetic()).collect();
    for part in name.to_lowercase().split_whitespace().filter(|w| w.chars().count() >= 3) {
        if letters == part || (p.contains(part) && p.chars().count() - part.chars().count() <= 4) {
            return Some("that password is mostly your name; choose something others wouldn't guess");
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::weakness;

    #[test]
    fn easy_passwords_are_refused() {
        for p in ["password", "Password", "12345678", "qwertyuiop", "iloveyou", "password@123", "Dragon2024", "!!sunshine", "aaaaaaaaaa", "india@123"] {
            assert!(weakness(p, "Jane Doe").is_some(), "{p} should be refused");
        }
        for p in ["janedoe1", "Jane@2024", "doe!doe"] {
            assert!(weakness(p, "Jane Doe").is_some(), "{p} is mostly the name");
        }
    }

    #[test]
    fn reasonable_passwords_pass() {
        for p in ["correct horse battery staple", "parent-password", "vaulty-demo", "मेरा घर नीला है", "tiger lily 2 kettle", "Jane's red bicycle 1998"] {
            assert_eq!(weakness(p, "Jane Doe"), None, "{p} should be allowed");
        }
    }
}
