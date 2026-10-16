# Institution logos

Optional logo files for the bank / card / broker directory in
`crates/vaulty-core/data/banks/<COUNTRY>.json`.

To add a logo:
1. Put the image here, named after the institution id, e.g. `in-hdfc.svg`.
   SVG is preferred; PNG or WebP (square, at least 128×128) also work.
2. Add `"logo": "in-hdfc.svg"` to that institution's entry in the JSON file.
3. Run `cargo test -p vaulty-core` (it checks that every referenced file exists).

Use the institution's official artwork, and follow its published brand guidelines where they
exist. All logos remain the property of their owners; see `TRADEMARKS.md` at the repository root.
Institutions without a logo file are shown with a monogram badge, and users can attach their
own image to any account in the app.
