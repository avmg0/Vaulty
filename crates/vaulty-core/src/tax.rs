//! Personal income tax estimates from region packs (`data/tax/<COUNTRY>.json`).
//!
//! The rules are data; this module only knows the generic shapes: progressive bands by age,
//! a standard deduction or tax-free allowance (optionally tapered away at higher incomes),
//! capped deductions, a rebate, surcharge and cess, each with optional marginal relief, and
//! levies charged alongside income tax (such as National Insurance). All arithmetic is in
//! integer minor units.

use crate::{error::{Error, Result}, money};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const PACKS: &[&str] = &[include_str!("../data/tax/IN.json"), include_str!("../data/tax/GB.json")];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pack {
    pub country: String,
    pub currency: String,
    pub last_checked: String,
    #[serde(default)] pub notes: Option<String>,
    pub sources: Vec<String>,
    pub years: Vec<Year>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Year {
    pub id: String,
    pub label: String,
    pub start: String,
    pub end: String,
    #[serde(default)] pub rounding: Rounding,
    pub regimes: Vec<Regime>,
    /// When tax is paid ahead during the year, if the country has that.
    #[serde(default)] pub advance_tax: Option<AdvanceTax>,
}

/// Tax paid in instalments during the year: by each date (MM-DD, in the year's order), at least
/// `percent` of the year's tax (after tax deducted at source), once that is `threshold` or more.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvanceTax {
    pub threshold: i64,
    pub instalments: Vec<Instalment>,
    #[serde(default)] pub note: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instalment { pub date: String, pub percent: f64 }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Rounding { #[serde(default)] pub taxable: i64, #[serde(default)] pub tax: i64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regime {
    pub id: String,
    pub name: String,
    #[serde(default)] pub default: bool,
    #[serde(default)] pub description: Option<String>,
    pub slabs: Vec<Slabs>,
    #[serde(default)] pub standard_deduction: Option<StandardDeduction>,
    #[serde(default)] pub rebate: Option<Rebate>,
    #[serde(default)] pub surcharge: Option<Surcharge>,
    #[serde(default)] pub cess: Option<Cess>,
    #[serde(default)] pub deductions: Vec<Deduction>,
    /// Charged alongside income tax on their own base (e.g. National Insurance on earnings).
    #[serde(default)] pub levies: Vec<Levy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slabs { pub age_min: u32, pub bands: Vec<Band> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Band { pub upto: Option<i64>, pub rate: f64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardDeduction {
    pub amount: i64,
    #[serde(default)] pub applies_to: Option<String>,
    /// What the form calls it ("Personal Allowance"); "Standard deduction" when not given.
    #[serde(default)] pub label: Option<String>,
    /// Reduced by 1 for every `per` of income (after deductions) above `above`.
    #[serde(default)] pub taper: Option<Taper>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Taper { pub above: i64, pub per: i64 }
/// A charge alongside income tax, on salary or on all income, in bands like the slabs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Levy { pub id: String, pub name: String, #[serde(default)] pub base: Option<String>, pub bands: Vec<Band>, #[serde(default)] pub note: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rebate { pub income_limit: i64, pub max: i64, #[serde(default)] pub marginal_relief: bool }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Surcharge { #[serde(default)] pub marginal_relief: bool, pub bands: Vec<SurchargeBand> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurchargeBand { pub above: i64, pub rate: f64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cess { pub name: String, pub rate: f64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deduction {
    pub id: String,
    pub label: String,
    #[serde(default)] pub hint: Option<String>,
    #[serde(default)] pub cap: Option<i64>,
    #[serde(default)] pub cap_senior: Option<i64>,
    #[serde(default)] pub cap_alt: Option<i64>,
    #[serde(default)] pub alt_label: Option<String>,
}

/// All bundled packs, parsed and checked.
pub fn packs() -> Result<Vec<Pack>> {
    PACKS.iter().map(|s| {
        let p: Pack = serde_json::from_str(s).map_err(|e| Error::Invalid(format!("tax pack: {e}")))?;
        validate(&p)?;
        Ok(p)
    }).collect()
}

pub fn pack(country: &str) -> Result<Pack> {
    packs()?.into_iter().find(|p| p.country.eq_ignore_ascii_case(country))
        .ok_or_else(|| Error::Invalid(format!("no tax rules for {country} yet")))
}

/// Countries with a tax pack, e.g. `["IN"]`.
pub fn countries() -> Result<Vec<String>> { Ok(packs()?.into_iter().map(|p| p.country).collect()) }

fn validate(p: &Pack) -> Result<()> {
    let bad = |m: String| Err(Error::Invalid(format!("tax pack {}: {m}", p.country)));
    if p.sources.is_empty() { return bad("needs at least one source".into()); }
    for y in &p.years {
        if y.regimes.is_empty() { return bad(format!("{} has no regimes", y.id)); }
        if y.regimes.iter().filter(|r| r.default).count() > 1 { return bad(format!("{} has several default regimes", y.id)); }
        for r in &y.regimes {
            if !r.slabs.iter().any(|s| s.age_min == 0) { return bad(format!("{}/{} needs slabs for age 0", y.id, r.id)); }
            for s in &r.slabs {
                let mut prev = 0;
                for (i, b) in s.bands.iter().enumerate() {
                    if !(0.0..=100.0).contains(&b.rate) { return bad(format!("{}/{}: rate {} out of range", y.id, r.id, b.rate)); }
                    match b.upto {
                        Some(u) if u > prev => prev = u,
                        None if i == s.bands.len() - 1 => {}
                        _ => return bad(format!("{}/{}: bands must rise and only the last may be open", y.id, r.id)),
                    }
                }
                if s.bands.last().is_some_and(|b| b.upto.is_some()) { return bad(format!("{}/{}: last band must be open (upto: null)", y.id, r.id)); }
            }
            for l in &r.levies {
                let mut prev = 0;
                for (i, b) in l.bands.iter().enumerate() {
                    let ok = (0.0..=100.0).contains(&b.rate) && match b.upto { Some(u) => u > prev, None => i == l.bands.len() - 1 };
                    if !ok { return bad(format!("{}/{}: levy {} bands must rise, rates 0–100, only the last open", y.id, r.id, l.id)); }
                    if let Some(u) = b.upto { prev = u; }
                }
                if l.bands.last().is_some_and(|b| b.upto.is_some()) { return bad(format!("{}/{}: levy {} last band must be open", y.id, r.id, l.id)); }
            }
            if r.standard_deduction.as_ref().and_then(|s| s.taper.as_ref()).is_some_and(|t| t.per <= 0) { return bad(format!("{}/{}: taper needs per > 0", y.id, r.id)); }
        }
    }
    Ok(())
}

/// Basis points from a percentage (5 → 500, 2.5 → 250).
fn bp(rate: f64) -> i64 { (rate * 100.0).round() as i64 }
/// `amount × bp / 10 000`, rounded half up.
fn pct(amount: i64, bps: i64) -> i64 { ((amount as i128 * bps as i128 + 5_000) / 10_000) as i64 }
fn round_to(v: i64, step: i64) -> i64 { if step <= 1 { v } else { (v + step / 2) / step * step } }

pub struct Input {
    pub age: u32,
    pub salary: i64,
    pub other: i64,
    pub claims: BTreeMap<String, i64>,
    /// Deduction ids whose `alt_label` the person ticked.
    pub alt: Vec<String>,
}

/// Parses the UI's request. Amounts are decimals in the pack's currency.
pub fn parse_input(v: &Value, currency: &str) -> Result<Input> {
    let amt = |x: Option<&Value>| -> Result<i64> {
        let m = match x { None | Some(Value::Null) => 0, Some(Value::String(s)) if s.trim().is_empty() => 0, Some(x) => money::parse_minor(x, currency)? };
        if m < 0 { return Err(Error::Invalid("amounts can't be negative".into())); }
        Ok(m)
    };
    let age = v.get("age").and_then(Value::as_u64).unwrap_or(30).min(130) as u32;
    let mut claims = BTreeMap::new();
    if let Some(obj) = v.get("deductions").and_then(Value::as_object) {
        for (k, x) in obj { claims.insert(k.clone(), amt(Some(x))?); }
    }
    let alt = v.get("alt").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
    Ok(Input { age, salary: amt(v.get("salary"))?, other: amt(v.get("other_income"))?, claims, alt })
}

#[derive(Debug, Serialize)]
pub struct BandLine { pub from: i64, pub upto: Option<i64>, pub rate: f64, pub taxed: i64, pub tax: i64 }
#[derive(Debug, Serialize)]
pub struct LevyLine { pub id: String, pub name: String, pub amount: i64 }
#[derive(Debug, Serialize)]
pub struct DeductionLine { pub id: String, pub label: String, pub claimed: i64, pub allowed: i64, pub cap: Option<i64> }

/// One regime's estimate. Money fields are minor units.
#[derive(Debug, Serialize)]
pub struct Estimate {
    pub regime: String,
    pub name: String,
    pub gross: i64,
    pub standard_deduction: i64,
    pub deductions: Vec<DeductionLine>,
    pub taxable: i64,
    pub bands: Vec<BandLine>,
    pub slab_tax: i64,
    pub rebate: i64,
    pub surcharge: i64,
    pub surcharge_rate: f64,
    pub cess: i64,
    pub standard_deduction_label: Option<String>,
    pub levies: Vec<LevyLine>,
    pub total: i64,
    /// Basis points of gross income.
    pub effective_bp: i64,
}

/// Tax on `amount` through progressive bands.
fn banded(bands: &[Band], amount: i64, scale: i64) -> (i64, Vec<BandLine>) {
    let (mut from, mut total, mut lines) = (0i64, 0i64, vec![]);
    for b in bands {
        let upto = b.upto.map(|u| u * scale);
        let top = upto.map_or(amount, |u| amount.min(u));
        let taxed = (top - from).max(0);
        let tax = pct(taxed, bp(b.rate));
        lines.push(BandLine { from, upto, rate: b.rate, taxed, tax });
        total += tax;
        match upto { Some(u) => from = u, None => break }
    }
    (total, lines)
}

fn slab_tax(r: &Regime, age: u32, taxable: i64, scale: i64) -> (i64, Vec<BandLine>) {
    let slabs = r.slabs.iter().filter(|s| s.age_min <= age).max_by_key(|s| s.age_min).expect("validated: age 0 slabs exist");
    banded(&slabs.bands, taxable, scale)
}

/// Income tax after rebate, before surcharge and cess.
fn tax_after_rebate(r: &Regime, age: u32, taxable: i64, scale: i64) -> (i64, i64, Vec<BandLine>) {
    let (tax, lines) = slab_tax(r, age, taxable, scale);
    let rebate = match &r.rebate {
        Some(rb) if taxable <= rb.income_limit * scale => tax.min(rb.max * scale),
        // Marginal relief: tax can't exceed the income above the limit.
        Some(rb) if rb.marginal_relief => (tax - (taxable - rb.income_limit * scale)).max(0),
        _ => 0,
    };
    (tax, rebate, lines)
}

fn surcharge_for(r: &Regime, age: u32, taxable: i64, tax: i64, scale: i64) -> (i64, f64) {
    let Some(s) = &r.surcharge else { return (0, 0.0) };
    let Some((i, band)) = s.bands.iter().enumerate().rfind(|(_, b)| taxable > b.above * scale) else { return (0, 0.0) };
    let mut sur = pct(tax, bp(band.rate));
    if s.marginal_relief {
        // Tax plus surcharge may exceed that at the threshold by no more than the income above it.
        let at = band.above * scale;
        let (t, rb, _) = tax_after_rebate(r, age, at, scale);
        let t = t - rb;
        let prev_rate = if i == 0 { 0.0 } else { s.bands[i - 1].rate };
        let ceiling = t + pct(t, bp(prev_rate)) + (taxable - at);
        sur = sur.min((ceiling - tax).max(0));
    }
    (sur, band.rate)
}

pub fn estimate(year: &Year, r: &Regime, input: &Input, currency: &str) -> Estimate {
    let scale = money::scale(currency);
    let gross = input.salary + input.other;
    let senior = input.age >= 60;
    let deductions: Vec<DeductionLine> = r.deductions.iter().map(|d| {
        let cap = if input.alt.contains(&d.id) { d.cap_alt.or(d.cap) } else if senior { d.cap_senior.or(d.cap) } else { d.cap };
        let cap = cap.map(|c| c * scale);
        let claimed = input.claims.get(&d.id).copied().unwrap_or(0);
        DeductionLine { id: d.id.clone(), label: d.label.clone(), claimed, allowed: cap.map_or(claimed, |c| claimed.min(c)), cap }
    }).collect();
    let deducted: i64 = deductions.iter().map(|d| d.allowed).sum();
    let std = r.standard_deduction.as_ref().map_or(0, |sd| {
        let base = if sd.applies_to.as_deref() == Some("salary") { input.salary } else { gross };
        // A taper takes 1 off the allowance for every `per` of income (after deductions) above `above`.
        let amount = match &sd.taper {
            Some(t) => (sd.amount * scale - ((gross - deducted - t.above * scale).max(0) / t.per)).max(0),
            None => sd.amount * scale,
        };
        base.min(amount)
    });
    let taxable = round_to((gross - std - deducted).max(0), year.rounding.taxable * scale);

    let (slab, rebate, bands) = tax_after_rebate(r, input.age, taxable, scale);
    let tax = slab - rebate;
    let (surcharge, surcharge_rate) = surcharge_for(r, input.age, taxable, tax, scale);
    let cess = r.cess.as_ref().map_or(0, |c| pct(tax + surcharge, bp(c.rate)));
    let levies: Vec<LevyLine> = r.levies.iter().map(|l| {
        let base = if l.base.as_deref() == Some("salary") { input.salary } else { gross };
        LevyLine { id: l.id.clone(), name: l.name.clone(), amount: banded(&l.bands, base, scale).0 }
    }).collect();
    let total = round_to(tax + surcharge + cess, year.rounding.tax * scale) + levies.iter().map(|l| l.amount).sum::<i64>();
    let effective_bp = if gross > 0 { (total as i128 * 10_000 / gross as i128) as i64 } else { 0 };
    Estimate {
        regime: r.id.clone(), name: r.name.clone(), gross, standard_deduction: std, deductions, taxable, bands,
        slab_tax: slab, rebate, surcharge, surcharge_rate, cess, total, effective_bp,
        standard_deduction_label: r.standard_deduction.as_ref().and_then(|s| s.label.clone()), levies,
    }
}

pub fn year_of<'a>(p: &'a Pack, year: Option<&str>) -> Result<&'a Year> {
    match year {
        Some(id) => p.years.iter().find(|y| y.id == id).ok_or_else(|| Error::Invalid(format!("tax year {id}"))),
        None => p.years.iter().max_by(|a, b| a.start.cmp(&b.start)).ok_or_else(|| Error::Invalid("tax year".into())),
    }
}

/// Estimates every regime of a year and recommends the cheapest (the default regime wins ties).
pub fn compare(country: &str, year: Option<&str>, request: &Value) -> Result<Value> {
    let p = pack(country)?;
    let y = year_of(&p, year)?;
    let input = parse_input(request, &p.currency)?;
    let estimates: Vec<Estimate> = y.regimes.iter().map(|r| estimate(y, r, &input, &p.currency)).collect();
    let best = estimates.iter().zip(&y.regimes)
        .min_by_key(|(e, r)| (e.total, !r.default))
        .map(|(e, _)| e.regime.clone());
    let saving = match estimates.iter().map(|e| e.total).collect::<Vec<_>>().as_slice() {
        [] | [_] => 0,
        all => all.iter().max().unwrap() - all.iter().min().unwrap(),
    };
    Ok(json!({
        "country": p.country, "currency": p.currency, "year": { "id": y.id, "label": y.label, "start": y.start, "end": y.end },
        "estimates": estimates.iter().map(|e| to_json(e, &p.currency)).collect::<Vec<_>>(),
        "recommended": best, "saving": money::to_decimal(saving, &p.currency),
        "sources": p.sources, "last_checked": p.last_checked, "notes": p.notes,
    }))
}

/// Pack metadata for the form: years, regimes and their deductions.
pub fn describe(country: &str) -> Result<Value> {
    let p = pack(country)?;
    Ok(serde_json::to_value(&p)?)
}

fn to_json(e: &Estimate, cur: &str) -> Value {
    let d = |m: i64| money::to_decimal(m, cur);
    json!({
        "regime": e.regime, "name": e.name, "gross": d(e.gross), "standard_deduction": d(e.standard_deduction),
        "deductions": e.deductions.iter().map(|l| json!({ "id": l.id, "label": l.label, "claimed": d(l.claimed), "allowed": d(l.allowed), "cap": l.cap.map(d) })).collect::<Vec<_>>(),
        "taxable": d(e.taxable),
        "bands": e.bands.iter().map(|b| json!({ "from": d(b.from), "upto": b.upto.map(d), "rate": b.rate, "taxed": d(b.taxed), "tax": d(b.tax) })).collect::<Vec<_>>(),
        "slab_tax": d(e.slab_tax), "rebate": d(e.rebate), "surcharge": d(e.surcharge), "surcharge_rate": e.surcharge_rate,
        "cess": d(e.cess), "standard_deduction_label": e.standard_deduction_label,
        "levies": e.levies.iter().map(|l| json!({ "id": l.id, "name": l.name, "amount": d(l.amount) })).collect::<Vec<_>>(),
        "total": d(e.total), "total_minor": e.total, "effective_rate": e.effective_bp as f64 / 100.0,
    })
}
