import i18n from "@/i18n";
import catalogue from "./rust-errors.json";

/**
 * Shows a message from the app (Rust) in the person's language. Messages are matched against the
 * catalogue made by scripts/rust-errors.mjs; parts filled in at run time (a name, an amount, a
 * nested reason) are carried over, and a nested reason is translated too. Anything unknown is
 * shown as it came.
 */
const entries = catalogue.map(({ id, re }) => ({ id, re: new RegExp(re) }));

/** Low-level failures get a calm, general message instead of the technical one. */
const GENERAL: [RegExp, string][] = [
  [/^database error: /, "errors.data"],
  [/^io error: /, "errors.file"],
  [/^json error: /, "errors.format"],
  [/^csv error: /, "errors.csv"],
];

export function translateError(message: string, depth = 0): string {
  for (const [re, key] of GENERAL) {
    if (re.test(message)) {
      console.warn("Vaulty:", message); // the technical detail, for reporting a problem
      return i18n.t(key);
    }
  }
  if (depth > 3) return message;
  for (const { id, re } of entries) {
    const m = re.exec(message);
    if (!m) continue;
    const params = Object.fromEntries(m.slice(1).map((v, i) => [`p${i}`, translateError(v, depth + 1)]));
    return i18n.t(`rust.${id}`, { ...params, defaultValue: message });
  }
  return message;
}
