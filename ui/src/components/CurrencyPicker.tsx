import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { allCurrencies, currencyFor, useDisplayNames, useLocale } from "@/hooks/locale";
import { cn } from "@/lib/utils";

/** Symbol as the current locale writes it, e.g. "₹" for INR in en-IN; empty when it's just the code. */
function symbolOf(code: string, locale: string) {
  try {
    const sym = new Intl.NumberFormat(locale, { style: "currency", currency: code, currencyDisplay: "narrowSymbol" })
      .formatToParts(0).find((p) => p.type === "currency")?.value;
    return sym && sym !== code ? sym : "";
  } catch { return ""; }
}

type Props = { id?: string; value: string; onChange: (code: string) => void; disabled?: boolean; className?: string };

/**
 * Every ISO 4217 currency, named in the interface language. The user's main currency and their
 * country's currency are listed first.
 */
export function CurrencyPicker({ id, value, onChange, disabled, className }: Props) {
  const { locale, country, currency: main } = useLocale();
  const names = useDisplayNames();
  const { t } = useTranslation();
  const { top, rest } = useMemo(() => {
    const label = (c: string) => { const s = symbolOf(c, locale); return `${names.currency(c)} (${c}${s ? `, ${s}` : ""})`; };
    const pinned = [...new Set([value, main, currencyFor(country), "USD", "EUR"].filter(Boolean))];
    const all = [...new Set([...allCurrencies(), ...pinned])];
    const coll = new Intl.Collator(locale);
    return {
      top: pinned.map((c) => ({ c, l: label(c) })),
      rest: all.filter((c) => !pinned.includes(c)).map((c) => ({ c, l: label(c) })).sort((a, b) => coll.compare(a.l, b.l)),
    };
  }, [locale, country, main, value, names]);

  return (
    <select id={id} value={value} disabled={disabled} onChange={(e) => onChange(e.target.value)}
      className={cn("border-input bg-transparent dark:bg-input/30 h-9 w-full min-w-0 rounded-md border px-3 text-sm shadow-xs outline-none",
        "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] disabled:opacity-50", className)}>
      <optgroup label={t("currencyPicker.suggested")}>{top.map(({ c, l }) => <option key={c} value={c}>{l}</option>)}</optgroup>
      <optgroup label={t("currencyPicker.all")}>{rest.map(({ c, l }) => <option key={c} value={c}>{l}</option>)}</optgroup>
    </select>
  );
}
