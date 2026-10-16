# Translating Vaulty

Every piece of interface text lives in `locales/<language>.json`. English (`en.json`) is the
source; any key missing from another language falls back to English.

To add a language:

1. Copy `locales/en.json` to `locales/<code>.json`, using the ISO 639-1 code (`ta`, `bn`, `es`…).
2. Translate the values. Keep the keys and anything in `{{double braces}}` unchanged, and keep
   tags such as `<b>…</b>` around the matching words.
3. Plural keys end in `_one`, `_other` (and `_few`, `_many`… where the language needs them; see
   https://www.i18next.com/translation-function/plurals).
4. Import the file in `index.ts` and add the language to `LANGUAGES` with its own name and
   writing direction (`rtl` for Arabic, Hebrew, Urdu, Persian).
5. Run `npm run i18n:check` to list missing or extra keys.

Numbers, dates and currency amounts are formatted by the operating system's locale data, so they
need no translation.
