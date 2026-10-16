//! Directory of banks, card issuers, brokers and wallets, so an account can be linked to its
//! institution and shown with a recognisable badge.
//!
//! The lists live in `data/banks/<COUNTRY>.json` (ISO 3166 code; `XX` = international) and are
//! community-maintained like region packs.
//!
//! Logos: an entry may name a logo file (`"logo": "in-hdfc.svg"`) placed in
//! `ui/public/institutions/`. Logos belong to their owners (see TRADEMARKS.md). Entries without a
//! file get a monogram badge, and users can attach their own image to any account.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const FILES: &[&str] = &[
    include_str!("../data/banks/IN.json"),
    include_str!("../data/banks/US.json"),
    include_str!("../data/banks/GB.json"),
    include_str!("../data/banks/CA.json"),
    include_str!("../data/banks/AU.json"),
    include_str!("../data/banks/SG.json"),
    include_str!("../data/banks/AE.json"),
    include_str!("../data/banks/XX.json"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Institution {
    pub id: String,
    pub name: String,
    /// Up to a few characters, used for the monogram badge.
    pub short: String,
    /// bank | card | broker | wallet
    pub kind: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Optional logo file name in `ui/public/institutions/` (SVG, PNG or WebP).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    /// Filled in from the file it came from.
    #[serde(default)]
    pub country: String,
}

#[derive(Deserialize)]
struct File {
    country: String,
    institutions: Vec<Institution>,
}

pub fn all() -> &'static [Institution] {
    static ALL: OnceLock<Vec<Institution>> = OnceLock::new();
    ALL.get_or_init(|| {
        FILES
            .iter()
            .flat_map(|raw| {
                let f: File = serde_json::from_str(raw).expect("bundled institution list is valid JSON");
                let country = f.country;
                f.institutions.into_iter().map(move |mut i| {
                    i.country = country.clone();
                    i
                })
            })
            .collect()
    })
}

pub fn get(id: &str) -> Option<&'static Institution> {
    all().iter().find(|i| i.id == id)
}

/// Country most likely meant by an account currency, to rank local institutions first.
pub fn country_for_currency(currency: &str) -> Option<&'static str> {
    Some(match currency.to_ascii_uppercase().as_str() {
        "INR" => "IN",
        "USD" => "US",
        "GBP" => "GB",
        "CAD" => "CA",
        "AUD" => "AU",
        "SGD" => "SG",
        "AED" => "AE",
        _ => return None,
    })
}

/// Case-insensitive search on name, short name and aliases. Results from `country` (and
/// international ones) come first; then prefix matches before substring matches.
pub fn search(query: &str, country: Option<&str>, limit: usize) -> Vec<&'static Institution> {
    let q = query.trim().to_lowercase();
    let mut hits: Vec<(u8, &Institution)> = all()
        .iter()
        .filter_map(|i| {
            let fields = std::iter::once(i.name.to_lowercase())
                .chain(std::iter::once(i.short.to_lowercase()))
                .chain(i.aliases.iter().map(|a| a.to_lowercase()));
            let mut best: Option<u8> = None;
            for f in fields {
                let score = if q.is_empty() || f == q {
                    0
                } else if f.starts_with(&q) || f.split_whitespace().any(|w| w.starts_with(&q)) {
                    1
                } else if f.contains(&q) {
                    2
                } else {
                    continue;
                };
                best = Some(best.map_or(score, |b| b.min(score)));
            }
            let local = match country {
                Some(c) if i.country == c => 0,
                Some(_) if i.country == "XX" => 1,
                Some(_) => 3,
                None => 0,
            };
            best.map(|s| (local * 4 + s, i))
        })
        .collect();
    hits.sort_by(|(a, x), (b, y)| a.cmp(b).then_with(|| x.name.cmp(&y.name)));
    hits.into_iter().take(limit).map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn lists_are_valid_and_ids_unique() {
        let mut ids = HashSet::new();
        for i in all() {
            assert!(ids.insert(i.id.clone()), "duplicate id {}", i.id);
            assert!(!i.name.is_empty() && !i.short.is_empty());
            assert!(matches!(i.kind.as_str(), "bank" | "card" | "broker" | "wallet"), "{}: kind {}", i.id, i.kind);
            assert!(i.id.starts_with(&format!("{}-", i.country.to_lowercase())), "{} not prefixed by country", i.id);
        }
        assert!(all().len() > 100);
    }

    /// Every `logo` an entry names must exist, be a plain file name, and be a supported image type.
    #[test]
    fn referenced_logo_files_exist() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/public/institutions");
        for i in all() {
            let Some(logo) = &i.logo else { continue };
            assert!(!logo.contains('/') && !logo.contains(".."), "{}: logo must be a file name", i.id);
            assert!([".svg", ".png", ".webp"].iter().any(|e| logo.ends_with(e)), "{}: unsupported logo type", i.id);
            assert!(dir.join(logo).is_file(), "{}: missing ui/public/institutions/{logo}", i.id);
        }
    }

    #[test]
    fn search_ranks_local_and_prefix_matches_first() {
        let r = search("hdfc", Some("IN"), 5);
        assert_eq!(r[0].id, "in-hdfc");
        assert!(r.iter().any(|i| i.id == "in-hdfcsec"));
        let amex = search("amex", Some("US"), 3);
        assert_eq!(amex[0].id, "us-amex");
        assert_eq!(search("state bank", None, 1)[0].short, "SBI");
        assert!(search("zzzz", None, 5).is_empty());
        assert_eq!(search("", Some("SG"), 50)[0].country, "SG");
    }
}
