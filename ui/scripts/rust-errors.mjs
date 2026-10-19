// Collects the messages Rust sends to the window (Error::Invalid / Error::Auth / `bad(...)`) so the
// window can show them in the person's language.
//
//   node scripts/rust-errors.mjs          update src/i18n/rust-errors.json and the English texts
//   node scripts/rust-errors.mjs --check  fail if either is out of date or a translation is missing
//
// Each message becomes a template: `{x}` placeholders turn into {{p0}}, {{p1}}..., matched at run
// time by a regular expression. Translations live under "rust" in each locale file.
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = new URL("../../", import.meta.url).pathname;
const sources = [join(root, "crates/vaulty-core/src"), join(root, "src-tauri/src")];
const out = new URL("../src/i18n/rust-errors.json", import.meta.url);
const locales = new URL("../src/i18n/locales/", import.meta.url);
const check = process.argv.includes("--check");

const rustFiles = (dir) => readdirSync(dir).flatMap((f) => {
  const p = join(dir, f);
  return statSync(p).isDirectory() ? rustFiles(p) : p.endsWith(".rs") ? [p] : [];
});

/** The string literal starting at `i` (a `"`), unescaped, and where it ends. */
function literal(src, i) {
  let s = "";
  for (let j = i + 1; j < src.length; j++) {
    const c = src[j];
    if (c === "\\") {
      const n = src[j + 1];
      s += n === "n" ? "\n" : n === "t" ? "\t" : n;
      j++;
    } else if (c === '"') return { text: s, end: j };
    else s += c;
  }
  return null;
}

/** Splits a format string into literal parts and placeholders. */
function template(text) {
  const parts = [];
  let lit = "", n = 0;
  for (let i = 0; i < text.length; i++) {
    if (text.startsWith("{{", i)) { lit += "{"; i++; continue; }
    if (text.startsWith("}}", i)) { lit += "}"; i++; continue; }
    if (text[i] === "{") {
      const close = text.indexOf("}", i);
      parts.push(lit, { p: n++ });
      lit = "";
      i = close;
      continue;
    }
    lit += text[i];
  }
  parts.push(lit);
  return parts;
}

const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
// Error::Invalid("..."), Error::Auth(format!("...")), bad("...") helpers, and msg("...") for reasons
// passed along and wrapped later (see `error::msg`).
const CALL = /(?:Error::(?:Invalid|Auth)\(\s*(?:format!\(\s*)?|\bbad\(\s*(?:format!\(\s*)?|\bmsg\(\s*)"/g;

const found = new Map();
for (const dir of sources) {
  for (const file of rustFiles(dir)) {
    // Unit tests at the bottom of a file aren't messages people see.
    const src = readFileSync(file, "utf8").split("#[cfg(test)]")[0];
    for (const m of src.matchAll(CALL)) {
      const lit = literal(src, m.index + m[0].length - 1);
      if (!lit || !lit.text.trim()) continue;
      const isFormat = /format!\(\s*$/.test(m[0].slice(0, -1));
      found.set(lit.text, isFormat || /\{[^{}]*\}/.test(lit.text.replace(/\{\{|\}\}/g, "")) ? template(lit.text) : [lit.text]);
    }
  }
}
// The lockout message comes from the error type itself.
found.set("too many attempts; try again in {0} seconds", template("too many attempts; try again in {0} seconds"));

// A template with no fixed text ("{e}") would match anything; such messages pass through as they are.
for (const [raw, parts] of found) if (!parts.some((p) => typeof p === "string" && p.trim())) found.delete(raw);

const entries = [...found.entries()].map(([raw, parts]) => {
  const params = parts.filter((p) => typeof p === "object").length;
  const en = parts.map((p) => (typeof p === "object" ? `{{p${p.p}}}` : p.replace(/\{\{/g, "{ {"))).join("");
  const re = "^" + parts.map((p) => (typeof p === "object" ? "([\\s\\S]+?)" : escapeRe(p))).join("") + "$";
  const literalLength = parts.filter((p) => typeof p === "string").join("").length;
  return { id: "e" + createHash("sha1").update(raw).digest("hex").slice(0, 8), en, re, params, literalLength };
})
  // Most specific first: fewer placeholders, then more fixed text.
  .sort((a, b) => a.params - b.params || b.literalLength - a.literalLength || a.id.localeCompare(b.id));

const catalogue = JSON.stringify(entries.map(({ id, re }) => ({ id, re })), null, 1) + "\n";
const readJson = (u) => JSON.parse(readFileSync(u, "utf8"));
const enFile = new URL("en.json", locales);
const en = readJson(enFile);
const wantedEn = Object.fromEntries(entries.map((e) => [e.id, e.en]));

let problems = [];
const current = (() => { try { return readFileSync(out, "utf8"); } catch { return ""; } })();
if (current !== catalogue) problems.push("src/i18n/rust-errors.json is out of date");
if (JSON.stringify(en.rust ?? {}) !== JSON.stringify(wantedEn)) problems.push('the "rust" texts in en.json are out of date');

if (check) {
  for (const f of readdirSync(locales).filter((f) => f.endsWith(".json") && f !== "en.json")) {
    const tr = readJson(new URL(f, locales)).rust ?? {};
    const missing = entries.filter((e) => !tr[e.id]);
    missing.forEach((e) => problems.push(`${f}: no translation for "${e.en}" (rust.${e.id})`));
    for (const e of entries) {
      const want = [...e.en.matchAll(/\{\{p\d+\}\}/g)].map((m) => m[0]).sort().join();
      const got = tr[e.id] && [...tr[e.id].matchAll(/\{\{p\d+\}\}/g)].map((m) => m[0]).sort().join();
      if (tr[e.id] && want !== got) problems.push(`${f}: rust.${e.id} must use ${want || "no placeholders"}`);
    }
  }
  problems.forEach((p) => console.log(p));
  console.log(`${entries.length} messages from Rust`);
  process.exit(problems.length ? 1 : 0);
}

writeFileSync(out, catalogue);
en.rust = wantedEn;
writeFileSync(enFile, JSON.stringify(en, null, 2) + "\n");
// Drop translations of messages that no longer exist.
for (const f of readdirSync(locales).filter((f) => f.endsWith(".json") && f !== "en.json")) {
  const u = new URL(f, locales), j = readJson(u);
  if (j.rust) {
    j.rust = Object.fromEntries(Object.entries(j.rust).filter(([k]) => k in wantedEn));
    writeFileSync(u, JSON.stringify(j, null, 2) + "\n");
  }
}
console.log(`${entries.length} messages from Rust written`);
