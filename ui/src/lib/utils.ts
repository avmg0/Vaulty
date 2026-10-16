import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** BCP 47 locale for numbers and dates (e.g. "en-IN" groups as 12,34,567). Set by LocaleProvider. */
let numberLocale: string | undefined;
export function setNumberLocale(locale: string | undefined) { numberLocale = locale; }
export function getNumberLocale() { return numberLocale; }

export function formatMoney(amount: number, currency = "USD", hidden = false, opts?: Intl.NumberFormatOptions) {
  if (hidden) return "••••";
  try {
    return new Intl.NumberFormat(numberLocale, { style: "currency", currency, ...opts }).format(amount);
  } catch {
    return `${amount.toFixed(2)} ${currency}`;
  }
}

export function formatNumber(n: number, opts?: Intl.NumberFormatOptions) {
  return new Intl.NumberFormat(numberLocale, opts).format(n);
}

/** Formats a Date or a `YYYY-MM-DD` string (read as a local date, not UTC midnight). */
export function formatDate(d: Date | string, opts: Intl.DateTimeFormatOptions = { day: "numeric", month: "short", year: "numeric" }) {
  const date = typeof d === "string" && /^\d{4}-\d{2}-\d{2}$/.test(d) ? new Date(`${d}T00:00:00`) : new Date(d);
  return new Intl.DateTimeFormat(numberLocale, opts).format(date);
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
