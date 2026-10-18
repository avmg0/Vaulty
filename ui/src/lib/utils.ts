import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** BCP 47 locale for numbers and dates (e.g. "en-IN" groups as 12,34,567). Set by LocaleProvider. */
let numberLocale: string | undefined;
export function setNumberLocale(locale: string | undefined) { numberLocale = locale; }
export function getNumberLocale() { return numberLocale; }

/**
 * The profile's display choices. "auto" follows the language and region. Set by LocaleProvider
 * from the profile's settings, like the number locale.
 */
export type FormatPrefs = { dateFormat: "auto" | "dmy" | "mdy" | "ymd"; negative: "minus" | "parentheses"; decimals: "auto" | "0" | "2" };
export const DEFAULT_FORMATS: FormatPrefs = { dateFormat: "auto", negative: "minus", decimals: "auto" };
let formats: FormatPrefs = DEFAULT_FORMATS;
export function setFormatPrefs(p: FormatPrefs) { formats = p; }

export function formatMoney(amount: number, currency = "USD", hidden = false, opts?: Intl.NumberFormatOptions) {
  if (hidden) return "••••";
  // A fixed number of decimals applies unless the caller asked for its own (e.g. whole-number charts).
  const digits = formats.decimals !== "auto" && opts?.maximumFractionDigits === undefined && opts?.minimumFractionDigits === undefined
    ? { minimumFractionDigits: Number(formats.decimals), maximumFractionDigits: Number(formats.decimals) } : {};
  const paren = formats.negative === "parentheses" && amount < 0;
  try {
    const text = new Intl.NumberFormat(numberLocale, { style: "currency", currency, ...digits, ...opts }).format(paren ? -amount : amount);
    return paren ? `(${text})` : text;
  } catch {
    return `${amount.toFixed(2)} ${currency}`;
  }
}

export function formatNumber(n: number, opts?: Intl.NumberFormatOptions) {
  return new Intl.NumberFormat(numberLocale, opts).format(n);
}

/**
 * Formats a Date or a `YYYY-MM-DD` string (read as a local date, not UTC midnight). Full dates
 * follow the profile's date order when it isn't "auto"; partial ones (just a month) never change.
 */
export function formatDate(d: Date | string, opts: Intl.DateTimeFormatOptions = { day: "numeric", month: "short", year: "numeric" }) {
  return formatDateAs(d, formats.dateFormat, opts);
}

/** `formatDate` with an explicit date order (used to show examples of each choice). */
export function formatDateAs(d: Date | string, order: FormatPrefs["dateFormat"], opts: Intl.DateTimeFormatOptions = { day: "numeric", month: "short", year: "numeric" }) {
  const date = typeof d === "string" && /^\d{4}-\d{2}-\d{2}$/.test(d) ? new Date(`${d}T00:00:00`) : new Date(d);
  if (order === "auto" || !opts.day || !opts.month || !opts.year || opts.dateStyle) return new Intl.DateTimeFormat(numberLocale, opts).format(date);
  if (order === "ymd") return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
  const parts = new Intl.DateTimeFormat(numberLocale, opts).formatToParts(date);
  const part = (t: Intl.DateTimeFormatPartTypes) => parts.find((p) => p.type === t)?.value ?? "";
  const weekday = opts.weekday ? `${part("weekday")}, ` : "";
  const [day, month, year] = [part("day"), part("month"), part("year")];
  const numeric = opts.month === "numeric" || opts.month === "2-digit";
  if (numeric) return weekday + (order === "dmy" ? `${day}/${month}/${year}` : `${month}/${day}/${year}`);
  return weekday + (order === "dmy" ? `${day} ${month} ${year}` : `${month} ${day}, ${year}`);
}

/**
 * Short amounts for chart axes and labels. Indian rupee amounts use thousand, lakh and crore
 * (₹92.5K, ₹4.1L, ₹1.2Cr), since Intl's en-IN "T" reads like trillion. Others use the region's
 * own compact form ($92.5K, €1.2M).
 */
export function formatCompactMoney(amount: number, currency = "USD", hidden = false) {
  if (hidden) return "••••";
  const abs = Math.abs(amount), sign = amount < 0 ? "-" : "";
  if (currency === "INR") {
    const sym = formatMoney(0, "INR").replace(/[\d.,\s]/g, "") || "₹";
    const n = (v: number) => new Intl.NumberFormat(numberLocale, { maximumFractionDigits: 1 }).format(v);
    if (abs >= 1e7) return `${sign}${sym}${n(abs / 1e7)}Cr`;
    if (abs >= 1e5) return `${sign}${sym}${n(abs / 1e5)}L`;
    if (abs >= 1e3) return `${sign}${sym}${n(abs / 1e3)}K`;
    return `${sign}${sym}${n(abs)}`;
  }
  return formatMoney(amount, currency, false, { notation: "compact", maximumFractionDigits: 1 });
}
