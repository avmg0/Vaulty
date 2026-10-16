# Tax region packs

Each file describes one country's personal income tax as data, so Vaulty can estimate tax without
country-specific code. File name: the ISO 3166 country code (`IN.json`).

A pack lists tax `years`; each year has one or more `regimes` (India's old and new regime, for
example). A regime has:

- `slabs`: progressive bands per age group (`age_min`); the highest `age_min` not above the
  person's age applies. `upto: null` means no upper limit. `rate` is a percentage.
- `standard_deduction`: a flat deduction, optionally only for `salary` income.
- `rebate`: tax credit when taxable income is at most `income_limit`, with optional marginal relief
  (tax never exceeds the income above the limit).
- `surcharge`: extra percentage of tax above income thresholds, with marginal relief.
- `cess`: a percentage added to tax plus surcharge.
- `deductions`: what the person can claim, each with an optional `cap` (`cap_senior` from age 60,
  `cap_alt` when the person ticks `alt_label`).

`rounding` rounds taxable income and the final tax to the nearest multiple. Amounts are in whole
units of the pack's currency.

Every pack needs `sources` (official pages first) and `last_checked`. Tax rules change every year:
add a new entry to `years` rather than editing an old one, and add worked examples from an official
source to `crates/vaulty-core/tests/tax.rs`.
