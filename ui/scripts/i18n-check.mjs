// Lists keys missing from, or extra in, each translation compared with en.json.
import { readdirSync, readFileSync } from "node:fs";
const dir = new URL("../src/i18n/locales/", import.meta.url);
const flat = (o, p = "") => Object.entries(o).flatMap(([k, v]) => (v && typeof v === "object" ? flat(v, `${p}${k}.`) : [`${p}${k}`]));
const base = (k) => k.replace(/_(zero|one|two|few|many|other)$/, "");
const en = new Set(flat(JSON.parse(readFileSync(new URL("en.json", dir)))).map(base));
let bad = false;
for (const f of readdirSync(dir).filter((f) => f.endsWith(".json") && f !== "en.json")) {
  const keys = new Set(flat(JSON.parse(readFileSync(new URL(f, dir)))).map(base));
  const missing = [...en].filter((k) => !keys.has(k)), extra = [...keys].filter((k) => !en.has(k));
  console.log(`${f}: ${en.size - missing.length}/${en.size} translated`);
  missing.forEach((k) => console.log(`  missing ${k}`));
  extra.forEach((k) => { console.log(`  extra   ${k}`); bad = true; });
}
process.exit(bad ? 1 : 0);
