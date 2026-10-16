import type { CardKind, CardNetwork } from "@/api";

/** Mirrors vaulty-core cards::detect_network (keep in sync; the Rust side has the tests). */
export function detectNetwork(number: string): CardNetwork | null {
  const d = number.replace(/\D/g, "");
  if (!d) return null;
  const p = (n: number) => (d.length >= n ? Number(d.slice(0, n)) : NaN);
  const inRange = (n: number, lo: number, hi: number) => { const v = p(n); return v >= lo && v <= hi; };
  if (inRange(6, 508500, 508999) || inRange(6, 606985, 607984) || inRange(6, 608001, 608500) || inRange(6, 652150, 653149) || inRange(2, 81, 82)) return "rupay";
  if (inRange(2, 34, 34) || inRange(2, 37, 37)) return "amex";
  if (inRange(2, 51, 55) || inRange(4, 2221, 2720)) return "mastercard";
  if (d.startsWith("4")) return "visa";
  if (inRange(4, 6011, 6011) || inRange(3, 644, 649) || inRange(2, 65, 65)) return "discover";
  if (inRange(4, 3528, 3589)) return "jcb";
  if (inRange(3, 300, 305) || inRange(2, 36, 36) || inRange(2, 38, 39)) return "diners";
  if (inRange(2, 62, 62)) return "unionpay";
  if (inRange(2, 50, 50) || inRange(2, 56, 58) || inRange(4, 6304, 6304) || inRange(4, 6759, 6759)) return "maestro";
  return null;
}

export function luhnOk(number: string) {
  const digits = number.replace(/\D/g, "").split("").map(Number);
  if (digits.length < 12) return false;
  const sum = digits.reverse().reduce((s, d, i) => s + (i % 2 ? (d * 2 > 9 ? d * 2 - 9 : d * 2) : d), 0);
  return sum % 10 === 0;
}

/** Plain-text network names (no trademarked artwork). */
export const NETWORK_NAME: Record<CardNetwork, string> = {
  visa: "Visa", mastercard: "Mastercard", amex: "American Express", discover: "Discover", rupay: "RuPay",
  diners: "Diners Club", jcb: "JCB", unionpay: "UnionPay", maestro: "Maestro",
};


/** Next occurrence of a day-of-month (clamped to month length), from today. */
export function nextDay(dayOfMonth: number, from = new Date()) {
  const today = new Date(from.getFullYear(), from.getMonth(), from.getDate());
  const at = (y: number, m: number) => new Date(y, m, Math.min(dayOfMonth, new Date(y, m + 1, 0).getDate()));
  let d = at(today.getFullYear(), today.getMonth());
  if (d < today) d = at(today.getFullYear(), today.getMonth() + 1);
  const days = Math.round((d.getTime() - today.getTime()) / 86_400_000);
  return { date: d, days };
}

export function ordinal(n: number) {
  const s = ["th", "st", "nd", "rd"], v = n % 100;
  return n + (s[(v - 20) % 10] || s[v] || s[0]);
}

export const KIND_LABEL: Record<CardKind, string> = { credit: "Credit", debit: "Debit", prepaid: "Prepaid", gift: "Gift card", other: "Other" };

/** "XXXX XXXX XXXX 4242" (Amex: "XXXX XXXXXX X4242"). Never more than the last 4 digits. */
export function maskedNumber(network: CardNetwork | null, last4: string | null) {
  const tail = last4 ?? "XXXX";
  return network === "amex" ? `XXXX XXXXXX X${tail}` : `XXXX XXXX XXXX ${tail}`;
}

/** Relative luminance (0 dark – 1 light) of a #rrggbb colour. */
export function luminance(hex: string) {
  const n = parseInt(hex.slice(1), 16);
  const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => { const c = v / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; });
  return 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2];
}

/** Light text on dark backgrounds and vice versa. */
export const inkFor = (lum: number): "light" | "dark" => (lum > 0.45 ? "dark" : "light");

export function expiryText(m: number | null, y: number | null) {
  return m && y ? `${String(m).padStart(2, "0")}/${String(y).slice(-2)}` : null;
}

/** Months until the end of the expiry month (negative once expired). */
export function monthsUntilExpiry(m: number, y: number, from = new Date()) {
  return (y - from.getFullYear()) * 12 + (m - 1 - from.getMonth());
}
