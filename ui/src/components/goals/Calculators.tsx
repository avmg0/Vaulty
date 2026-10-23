import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, errorText } from "@/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useLocale } from "@/hooks/locale";
import { useTheme } from "@/hooks/theme";
import { cn, formatMoney, formatNumber } from "@/lib/utils";

const WHOLE: Intl.NumberFormatOptions = { maximumFractionDigits: 0, minimumFractionDigits: 0 };
type Kind = "money" | "percent" | "years" | "months" | "age" | "choice";
type Field = { key: string; kind: Kind; def: string; options?: string[] };
type Out = { key: string; money?: boolean; path?: string[] };
type Calc = { id: string; fields: Field[]; main: Out; rows: Out[]; chart?: { list: string; a: string; b: string; stack?: boolean }; only?: string };

/** Each calculator: its inputs (with starting values), the headline figure and the rest. */
const CALCS: Calc[] = [
  { id: "sip", fields: [{ key: "monthly", kind: "money", def: "10000" }, { key: "rate", kind: "percent", def: "12" }, { key: "years", kind: "years", def: "10" }, { key: "step_up", kind: "percent", def: "0" }],
    main: { key: "value", money: true }, rows: [{ key: "invested", money: true }, { key: "gains", money: true }], chart: { list: "series", a: "invested", b: "value" } },
  { id: "lumpsum", fields: [{ key: "amount", kind: "money", def: "100000" }, { key: "rate", kind: "percent", def: "12" }, { key: "years", kind: "years", def: "10" }],
    main: { key: "value", money: true }, rows: [{ key: "invested", money: true }, { key: "gains", money: true }], chart: { list: "series", a: "invested", b: "value" } },
  { id: "emi", fields: [{ key: "principal", kind: "money", def: "2500000" }, { key: "rate", kind: "percent", def: "8.5" }, { key: "years", kind: "years", def: "20" }, { key: "extra", kind: "money", def: "0" }],
    main: { key: "emi", money: true }, rows: [{ key: "interest", money: true }, { key: "total", money: true }, { key: "months" },
      { key: "months_saved", path: ["with_extra", "months_saved"] }, { key: "interest_saved", money: true, path: ["with_extra", "interest_saved"] }],
    chart: { list: "schedule", a: "principal", b: "interest", stack: true } },
  { id: "fd", fields: [{ key: "principal", kind: "money", def: "100000" }, { key: "rate", kind: "percent", def: "7" }, { key: "months", kind: "months", def: "12" },
      { key: "compounding", kind: "choice", def: "quarterly", options: ["quarterly", "monthly", "half_yearly", "yearly", "simple"] }],
    main: { key: "value", money: true }, rows: [{ key: "invested", money: true }, { key: "gains", money: true }] },
  { id: "rd", fields: [{ key: "monthly", kind: "money", def: "5000" }, { key: "rate", kind: "percent", def: "7" }, { key: "months", kind: "months", def: "24" }],
    main: { key: "value", money: true }, rows: [{ key: "invested", money: true }, { key: "gains", money: true }] },
  { id: "ppf", only: "INR", fields: [{ key: "yearly", kind: "money", def: "150000" }, { key: "rate", kind: "percent", def: "7.1" }, { key: "years", kind: "years", def: "15" }],
    main: { key: "value", money: true }, rows: [{ key: "invested", money: true }, { key: "gains", money: true }], chart: { list: "series", a: "invested", b: "value" } },
  { id: "goal", fields: [{ key: "target", kind: "money", def: "1000000" }, { key: "years", kind: "years", def: "5" }, { key: "saved", kind: "money", def: "0" }, { key: "rate", kind: "percent", def: "10" }, { key: "inflation", kind: "percent", def: "6" }],
    main: { key: "monthly", money: true }, rows: [{ key: "future_target", money: true }, { key: "invested", money: true }] },
  { id: "retirement", fields: [{ key: "age", kind: "age", def: "30" }, { key: "retire_age", kind: "age", def: "60" }, { key: "life_age", kind: "age", def: "85" }, { key: "monthly_expenses", kind: "money", def: "50000" },
      { key: "saved", kind: "money", def: "0" }, { key: "inflation", kind: "percent", def: "6" }, { key: "rate_before", kind: "percent", def: "11" }, { key: "rate_after", kind: "percent", def: "7" }],
    main: { key: "corpus", money: true }, rows: [{ key: "monthly", money: true }, { key: "monthly_expenses_then", money: true }] },
  { id: "inflation", fields: [{ key: "amount", kind: "money", def: "100000" }, { key: "inflation", kind: "percent", def: "6" }, { key: "years", kind: "years", def: "10" }],
    main: { key: "future", money: true }, rows: [{ key: "today_worth", money: true }] },
];

const pick = (r: Record<string, unknown>, o: Out) => (o.path ?? [o.key]).reduce<unknown>((x, k) => (x as Record<string, unknown> | undefined)?.[k], r) as number | undefined;

export function Calculators() {
  const { t } = useTranslation();
  const { currency } = useLocale();
  const list = useMemo(() => CALCS.filter((c) => !c.only || c.only === currency), [currency]);
  const [id, setId] = useState(list[0].id);
  const calc = list.find((c) => c.id === id) ?? list[0];
  return (
    <div className="grid grid-cols-1 items-start gap-6 md:grid-cols-[13rem_minmax(0,1fr)]">
      <nav aria-label={t("calc.list")} className="md:sticky md:top-0">
        <NativeSelect aria-label={t("calc.list")} className="md:hidden" value={id} onChange={(e) => setId(e.target.value)}>
          {list.map((c) => <option key={c.id} value={c.id}>{t(`calc.${c.id}.name`)}</option>)}
        </NativeSelect>
        <ul className="hidden space-y-0.5 md:block">
          {list.map((c) => (
            <li key={c.id}>
              <button type="button" aria-current={c.id === id ? "page" : undefined} onClick={() => setId(c.id)}
                className={cn("w-full rounded-md px-3 py-2 text-start text-sm", c.id === id ? "bg-secondary font-medium" : "text-muted-foreground hover:bg-accent hover:text-foreground")}>
                {t(`calc.${c.id}.name`)}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <CalcCard key={calc.id} calc={calc} currency={currency} />
    </div>
  );
}

function CalcCard({ calc, currency }: { calc: Calc; currency: string }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(calc.fields.map((f) => [f.key, f.def])));
  const [result, setResult] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    const h = setTimeout(() => {
      api.calc.run(calc.id, values).then((r) => { setResult(r); setError(""); }).catch((e) => setError(errorText(e)));
    }, 200);
    return () => clearTimeout(h);
  }, [calc.id, values]);
  const show = (o: Out, n: number) => o.money ? formatMoney(n, currency, hideBalances, WHOLE) : formatNumber(n);
  const main = result ? pick(result, calc.main) : undefined;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t(`calc.${calc.id}.name`)}</CardTitle>
        <CardDescription>{t(`calc.${calc.id}.desc`)}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {calc.fields.map((f) => (
            <div key={f.key} className="grid gap-1.5">
              <Label htmlFor={`c-${f.key}`}>{t(`calc.${calc.id}.${f.key}`)}</Label>
              {f.kind === "choice" ? (
                <NativeSelect id={`c-${f.key}`} value={values[f.key]} onChange={(e) => setValues({ ...values, [f.key]: e.target.value })}>
                  {f.options!.map((o) => <option key={o} value={o}>{t(`calc.compounding.${o}`)}</option>)}
                </NativeSelect>
              ) : (
                <div className="relative">
                  <Input id={`c-${f.key}`} inputMode="decimal" value={values[f.key]} className={cn(f.kind !== "money" && "pe-14")}
                    onChange={(e) => setValues({ ...values, [f.key]: e.target.value.replace(/[^\d.]/g, "") })} />
                  {f.kind !== "money" && <span className="text-muted-foreground pointer-events-none absolute inset-y-0 end-3 flex items-center text-xs">{t(`calc.unit.${f.kind}`)}</span>}
                </div>
              )}
            </div>
          ))}
        </div>

        {error ? <p role="alert" className="text-destructive text-sm">{error}</p> : result && main != null && (
          <div className="space-y-5" aria-live="polite">
            <div>
              <div className="text-muted-foreground text-sm">{t(`calc.${calc.id}.${calc.main.key}`)}</div>
              <div className="text-4xl font-semibold tracking-tight tabular-nums">{show(calc.main, main)}</div>
            </div>
            <dl className="grid gap-3 sm:grid-cols-3">
              {calc.rows.map((o) => {
                const n = pick(result, o);
                if (n == null) return null;
                return (
                  <div key={o.key} className="rounded-lg border p-3">
                    <dt className="text-muted-foreground text-xs">{t(`calc.${calc.id}.${o.key}`)}</dt>
                    <dd className="text-lg font-semibold tabular-nums">{show(o, n)}</dd>
                  </div>
                );
              })}
            </dl>
            {calc.chart && Array.isArray(result[calc.chart.list]) && <YearChart rows={result[calc.chart.list] as Record<string, number>[]} spec={calc.chart} calcId={calc.id} currency={currency} />}
            <p className="text-muted-foreground text-xs">{t("calc.estimate")}</p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

/** Bars by year: for growth, what was put in and what it grew to; for a loan, principal and
 *  interest paid. Thinned to at most 30 bars. */
function YearChart({ rows, spec, calcId, currency }: { rows: Record<string, number>[]; spec: NonNullable<Calc["chart"]>; calcId: string; currency: string }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const step = Math.ceil(rows.length / 30);
  const shown = rows.filter((_, i) => (rows.length - 1 - i) % step === 0);
  const top = Math.max(...shown.map((r) => spec.stack ? r[spec.a] + r[spec.b] : Math.max(r[spec.a], r[spec.b])), 1);
  const H = 140;
  const label = (k: string) => t(`calc.${calcId}.${spec.stack ? `${k}_paid` : k === spec.b ? "value" : k}`);
  return (
    <figure className="space-y-2">
      <div className="flex h-[140px] items-end gap-[3px]" role="img" aria-label={t("calc.chartLabel")}>
        {shown.map((r) => {
          const a = r[spec.a] / top * H, b = r[spec.b] / top * H;
          const tip = `${t("calc.year", { n: r.year })}: ${label(spec.a)} ${formatMoney(r[spec.a], currency, hideBalances, WHOLE)}, ${label(spec.b)} ${formatMoney(r[spec.b], currency, hideBalances, WHOLE)}`;
          return (
            <div key={r.year} className="flex min-w-0 flex-1 flex-col justify-end" title={tip}>
              {spec.stack ? (<>
                <div className="bg-primary/35 rounded-t-sm" style={{ height: b }} />
                <div className="bg-primary" style={{ height: a }} />
              </>) : (
                <div className="bg-primary relative rounded-t-sm" style={{ height: b }}>
                  <div className="bg-muted-foreground/40 absolute inset-x-0 bottom-0" style={{ height: Math.min(a, b) }} />
                </div>
              )}
            </div>
          );
        })}
      </div>
      <figcaption className="text-muted-foreground flex flex-wrap justify-between gap-2 text-xs">
        <span>{t("calc.year", { n: shown[0]?.year })} – {t("calc.year", { n: shown.at(-1)?.year })}</span>
        <span className="flex gap-3">
          {spec.stack ? (<>
            <span className="flex items-center gap-1"><i className="bg-primary inline-block size-2 rounded-sm" />{label(spec.a)}</span>
            <span className="flex items-center gap-1"><i className="bg-primary/35 inline-block size-2 rounded-sm" />{label(spec.b)}</span>
          </>) : (<>
            <span className="flex items-center gap-1"><i className="bg-muted-foreground/40 inline-block size-2 rounded-sm" />{label(spec.a)}</span>
            <span className="flex items-center gap-1"><i className="bg-primary inline-block size-2 rounded-sm" />{t(`calc.${calcId}.gains`)}</span>
          </>)}
        </span>
      </figcaption>
    </figure>
  );
}
