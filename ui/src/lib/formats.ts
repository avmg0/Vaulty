/** Resolving the "auto" choices for week start and financial year from the profile's region. */

export type WeekStart = "auto" | "sunday" | "monday" | "saturday";
export const MONTH_IDS = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"] as const;
export type FiscalStart = "auto" | (typeof MONTH_IDS)[number];

/** Countries whose financial (tax) year doesn't start in January. */
const FISCAL_BY_COUNTRY: Record<string, (typeof MONTH_IDS)[number]> = {
  IN: "april", GB: "april", NZ: "april", JP: "april", HK: "april", ZA: "march",
  AU: "july", PK: "july", BD: "july", EG: "july", KE: "july", NP: "july",
};

export function resolveFiscalStart(pref: FiscalStart, country: string): (typeof MONTH_IDS)[number] {
  return pref !== "auto" ? pref : FISCAL_BY_COUNTRY[country] ?? "january";
}

/** First day of the week for a locale, from the browser's week data when it has it. */
export function resolveWeekStart(pref: WeekStart, locale: string): Exclude<WeekStart, "auto"> {
  if (pref !== "auto") return pref;
  try {
    const l = new Intl.Locale(locale) as Intl.Locale & { getWeekInfo?: () => { firstDay: number }; weekInfo?: { firstDay: number } };
    const first = (l.getWeekInfo?.() ?? l.weekInfo)?.firstDay;
    if (first) return first === 7 ? "sunday" : first === 6 ? "saturday" : "monday";
  } catch { /* fall through */ }
  const region = locale.split("-").pop() ?? "";
  return ["US", "CA", "IN", "JP", "BR", "MX", "IL", "PH", "KR", "TW", "HK", "ZA"].includes(region) ? "sunday"
    : ["AE", "EG", "SA", "IR", "AF", "QA", "KW", "OM", "BH", "JO", "SY", "IQ", "LY", "DZ"].includes(region) ? "saturday" : "monday";
}
