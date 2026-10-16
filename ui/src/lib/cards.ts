import type { CardNetwork } from "@/api";

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

/** Typical number length for the masked display; Amex is 15. */
export const isAmex = (n: CardNetwork | null) => n === "amex";

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
