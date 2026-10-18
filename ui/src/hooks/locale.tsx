import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import i18n, { LANGUAGES, isLanguage, type LanguageCode } from "@/i18n";
import { COUNTRY_CURRENCY } from "@/i18n/regions";
import { DEFAULT_FORMATS, setFormatPrefs, setNumberLocale, type FormatPrefs } from "@/lib/utils";
import { MONTH_IDS, type FiscalStart, type WeekStart } from "@/lib/formats";

/** Per-profile display choices (dates, negatives, decimals, week and financial-year start). */
export type DisplayPrefs = FormatPrefs & { weekStart: WeekStart; fiscalStart: FiscalStart };
export const DEFAULT_DISPLAY: DisplayPrefs = { ...DEFAULT_FORMATS, weekStart: "auto", fiscalStart: "auto" };

/** Display prefs from saved settings; anything unexpected (e.g. an old value) means "auto". */
export function displayFrom(s: { date_format?: string; negative_format?: string; decimals?: string; start_of_week?: string; fiscal_year_start?: string }): DisplayPrefs {
  const pick = <T extends string>(v: string | undefined, allowed: readonly T[], fallback: T): T => (allowed as readonly string[]).includes(v ?? "") ? (v as T) : fallback;
  return {
    dateFormat: pick(s.date_format, ["auto", "dmy", "mdy", "ymd"] as const, "auto"),
    negative: pick(s.negative_format, ["minus", "parentheses"] as const, "minus"),
    decimals: pick(s.decimals, ["auto", "0", "2"] as const, "auto"),
    weekStart: pick(s.start_of_week, ["auto", "sunday", "monday", "saturday"] as const, "auto"),
    fiscalStart: pick(s.fiscal_year_start, ["auto", ...MONTH_IDS] as const, "auto"),
  };
}

const KEY = "vaulty.locale";
const CURRENCY_KEY = "vaulty.currency";

/** Best guess from the device, e.g. "hi-IN"; falls back to "en-US". */
export function deviceLocale(): string {
  const nav = typeof navigator !== "undefined" ? navigator.languages?.[0] ?? navigator.language : undefined;
  try { return Intl.getCanonicalLocales(nav ?? "en-US")[0]; } catch { return "en-US"; }
}

export function splitLocale(locale: string): { language: string; country: string } {
  const loc = new Intl.Locale(locale);
  const region = loc.region && /^[A-Z]{2}$/.test(loc.region) ? loc.region : "";
  return { language: loc.language, country: region || (loc.maximize().region ?? "US") };
}

export const currencyFor = (country: string) => COUNTRY_CURRENCY[country] ?? "USD";

type Ctx = {
  /** BCP 47 tag driving number/date formats, e.g. "hi-IN". */
  locale: string;
  language: LanguageCode;
  country: string;
  dir: "ltr" | "rtl";
  /** Main currency: the default for new accounts and cards, and for totals. */
  currency: string;
  setLocale: (locale: string) => void;
  setCurrency: (currency: string) => void;
  display: DisplayPrefs;
  setDisplay: (p: DisplayPrefs) => void;
};
const LocaleCtx = createContext<Ctx | null>(null);

function stored(): string {
  try { return localStorage.getItem(KEY) ?? deviceLocale(); } catch { return deviceLocale(); }
}

/**
 * Holds the interface language and region. The device remembers the last choice so the sign-in
 * screens appear in it; once signed in, the profile's own setting takes over (see `useProfileLocale`).
 */
export function LocaleProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState(stored);
  const { language: lang, country } = splitLocale(locale);
  const [currency, setCurrencyState] = useState(() => { try { return localStorage.getItem(CURRENCY_KEY) ?? currencyFor(country); } catch { return currencyFor(country); } });
  const language: LanguageCode = isLanguage(lang) ? lang : "en";
  const [display, setDisplay] = useState<DisplayPrefs>(DEFAULT_DISPLAY);
  const dir = LANGUAGES.find((l) => l.code === language)?.dir ?? "ltr";

  const setLocale = useCallback((l: string) => {
    setLocaleState(l);
    try { localStorage.setItem(KEY, l); } catch { /* private mode */ }
  }, []);
  const setCurrency = useCallback((c: string) => {
    setCurrencyState(c);
    try { localStorage.setItem(CURRENCY_KEY, c); } catch { /* private mode */ }
  }, []);

  // Applied during render so the first paint already uses the right language and number format.
  setNumberLocale(locale);
  setFormatPrefs(display);
  if (i18n.language !== language) void i18n.changeLanguage(language);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dir = dir;
  }, [locale, dir]);

  const value = useMemo(() => ({ locale, language, country, dir, currency, setLocale, setCurrency, display, setDisplay }),
    [locale, language, country, dir, currency, setLocale, setCurrency, display]);
  return <LocaleCtx.Provider value={value}>{children}</LocaleCtx.Provider>;
}

export function useLocale() {
  const c = useContext(LocaleCtx);
  if (!c) throw new Error("useLocale outside LocaleProvider");
  return c;
}

/** Display names for languages, regions and currencies in the current interface language. */
export function useDisplayNames() {
  const { locale } = useLocale();
  return useMemo(() => {
    const make = (type: Intl.DisplayNamesType) => { try { return new Intl.DisplayNames([locale, "en"], { type }); } catch { return null; } };
    const region = make("region"), currency = make("currency"), language = make("language");
    return {
      region: (c: string) => region?.of(c) ?? c,
      currency: (c: string) => currency?.of(c) ?? c,
      language: (c: string) => language?.of(c) ?? c,
    };
  }, [locale]);
}

const FALLBACK_CURRENCIES = ["AED", "AUD", "BDT", "BRL", "CAD", "CHF", "CNY", "EUR", "GBP", "HKD", "IDR", "INR", "JPY", "KRW", "LKR", "MXN", "MYR", "NPR", "NZD", "PHP", "PKR", "SAR", "SEK", "SGD", "THB", "TRY", "USD", "ZAR"];

/** Every currency the platform knows how to format (about 160 on current systems). */
export function allCurrencies(): string[] {
  try { return Intl.supportedValuesOf("currency"); } catch { return FALLBACK_CURRENCIES; }
}
