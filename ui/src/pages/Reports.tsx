import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type Forecast, type ReportOverview, type SafeToSpend } from "@/api";
import { BarChart } from "@/components/charts/BarChart";
import { Donut } from "@/components/charts/Donut";
import { LineChart } from "@/components/charts/LineChart";
import { CHART_COLORS, INCOME_COLOR, LINE_COLOR, SPENDING_COLOR } from "@/components/charts/palette";
import { Segmented } from "@/components/Segmented";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { NativeSelect } from "@/components/ui/input";
import { useLocale } from "@/hooks/locale";
import { useTheme } from "@/hooks/theme";
import { MONTH_IDS, resolveFiscalStart } from "@/lib/formats";
import { cn, formatCompactMoney, formatDate, formatMoney } from "@/lib/utils";

type Period = "month" | "3m" | "6m" | "12m" | "year" | "fy";
const pad = (n: number) => String(n).padStart(2, "0");
const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

/** First and last day of a period ending this month. */
function bounds(p: Period, fiscalMonth: number): [string, string] {
  const now = new Date();
  const end = new Date(now.getFullYear(), now.getMonth() + 1, 0);
  const back = (m: number) => new Date(now.getFullYear(), now.getMonth() - m + 1, 1);
  const start = p === "month" ? back(1) : p === "3m" ? back(3) : p === "6m" ? back(6) : p === "12m" ? back(12)
    : p === "year" ? new Date(now.getFullYear(), 0, 1)
    : new Date(now.getMonth() >= fiscalMonth ? now.getFullYear() : now.getFullYear() - 1, fiscalMonth, 1);
  return [iso(start), iso(end)];
}

export function Reports() {
  const { t } = useTranslation();
  const { locale, display } = useLocale();
  const { hideBalances } = useTheme();
  const country = locale.split("-")[1] ?? "IN";
  const fiscalMonth = MONTH_IDS.indexOf(resolveFiscalStart(display.fiscalStart, country));
  const [period, setPeriod] = useState<Period>("6m");
  const [overview, setOverview] = useState<ReportOverview | null>(null);
  const [forecast, setForecast] = useState<Forecast | null>(null);
  const [safe, setSafe] = useState<SafeToSpend | null>(null);
  const [typical, setTypical] = useState(true);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);

  useEffect(() => { const bump = () => setVersion((v) => v + 1); window.addEventListener(DATA_RESTORED, bump); return () => window.removeEventListener(DATA_RESTORED, bump); }, []);
  useEffect(() => {
    const [from, to] = bounds(period, fiscalMonth);
    api.reports.overview(from, to).then(setOverview).catch((e) => setError(errorText(e)));
  }, [period, fiscalMonth, version]);
  useEffect(() => {
    api.reports.forecast(60).then(setForecast).catch((e) => setError(errorText(e)));
    api.reports.safeToSpend().then(setSafe).catch(() => {});
  }, [version]);

  const cur = overview?.currency ?? forecast?.currency ?? "INR";
  const money = (n: number) => formatMoney(n, cur, hideBalances);
  const compact = (n: number) => formatCompactMoney(n, cur, hideBalances);
  const monthLabel = (m: string) => new Date(Number(m.slice(0, 4)), Number(m.slice(5)) - 1, 1).toLocaleDateString(locale, { month: "short" });
  const nameOf = (n: string) => (n === "\u0000uncategorised" ? t("dashboard.uncategorised") : n === "\u0000hidden" ? t("reports.hidden") : n);

  const points = useMemo(() => (forecast?.points ?? []).map((p, i) => ({
    label: formatDate(p.date, { weekday: "short", day: "numeric", month: "short" }),
    value: typical ? p.expected : p.scheduled,
    tick: i % 14 === 0 ? formatDate(p.date, { day: "numeric", month: "short" }) : undefined,
  })), [forecast, typical]);
  const lowestIndex = useMemo(() => {
    if (!points.length) return undefined;
    let m = 0; points.forEach((p, i) => { if (p.value < points[m].value) m = i; });
    return m;
  }, [points]);

  return (
    <div className="max-w-6xl space-y-4">
      {error && <p className="text-destructive text-sm">{error}</p>}

      <div className="grid gap-4 lg:grid-cols-[22rem_minmax(0,1fr)]">
        <Card>
          <CardHeader><CardTitle>{t("reports.safe.title")}</CardTitle><CardDescription>{t("reports.safe.desc")}</CardDescription></CardHeader>
          <CardContent className="space-y-3">
            {safe && <>
              <div>
                <div className={cn("text-3xl font-semibold tabular-nums", safe.short && "text-destructive")}>{money(safe.short ? safe.balance - safe.committed : safe.amount)}</div>
                <p className="text-muted-foreground text-sm">
                  {safe.short ? t("reports.safe.short", { amount: money(safe.committed - safe.balance) })
                    : t(safe.income_name ? "reports.safe.until" : "reports.safe.untilMonthEnd", { date: formatDate(safe.until, { day: "numeric", month: "long" }), name: safe.income_name })}
                </p>
              </div>
              {!safe.short && <p className="text-sm">{t("reports.safe.perDay", { amount: money(safe.per_day), count: safe.days })}</p>}
              <dl className="text-muted-foreground grid grid-cols-[1fr_auto] gap-x-4 gap-y-1 border-t pt-3 text-xs">
                <dt>{t("reports.safe.balance")}</dt><dd className="text-foreground text-end tabular-nums">{money(safe.balance)}</dd>
                <dt>{t("reports.safe.committed")}</dt><dd className="text-foreground text-end tabular-nums">−{money(safe.committed)}</dd>
              </dl>
            </>}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex-row flex-wrap items-start justify-between gap-2">
            <div className="space-y-1.5"><CardTitle>{t("reports.forecast.title")}</CardTitle>
              <CardDescription>{forecast && t("reports.forecast.desc", { amount: money(forecast.typical_day) })}</CardDescription></div>
            <Segmented value={typical ? "typical" : "scheduled"} onChange={(v) => setTypical(v === "typical")} label={t("reports.forecast.title")}
              options={[{ value: "typical", label: t("reports.forecast.typical") }, { value: "scheduled", label: t("reports.forecast.scheduledOnly") }]} />
          </CardHeader>
          <CardContent>
            {points.length > 1 && <LineChart points={points} format={compact} color={LINE_COLOR} label={t("reports.forecast.title")} height={210}
              mark={lowestIndex} markLabel={lowestIndex != null ? t("reports.forecast.lowest", { amount: compact(points[lowestIndex].value) }) : undefined} />}
            {forecast && forecast.other_currencies > 0 && <p className="text-muted-foreground mt-2 text-xs">{t("reports.otherAccounts", { count: forecast.other_currencies })}</p>}
            {forecast && forecast.changes.length > 0 && (
              <ul className="mt-3 grid gap-x-6 gap-y-1 text-sm sm:grid-cols-2">
                {forecast.changes.slice(0, 8).map((c, i) => (
                  <li key={i} className="flex items-baseline justify-between gap-2">
                    <span className="truncate"><span className="text-muted-foreground me-2 text-xs tabular-nums">{formatDate(c.date, { day: "numeric", month: "short" })}</span>{c.title}</span>
                    <span className={cn("shrink-0 tabular-nums", c.amount > 0 && "text-success")}>{c.amount > 0 ? "+" : "−"}{money(Math.abs(c.amount))}</span>
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3 pt-2">
        <h2 className="text-lg font-semibold">{t("reports.period.heading")}</h2>
        <NativeSelect aria-label={t("reports.period.label")} className="w-auto" value={period} onChange={(e) => setPeriod(e.target.value as Period)}>
          {(["month", "3m", "6m", "12m", "year", "fy"] as Period[]).map((p) => <option key={p} value={p}>{t(`reports.period.${p}`)}</option>)}
        </NativeSelect>
      </div>

      {overview && <>
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <Stat label={t("reports.income")} value={money(overview.income)} />
          <Stat label={t("reports.spending")} value={money(overview.spending)} />
          <Stat label={t("reports.saved")} value={money(overview.income - overview.spending)} tone={overview.income - overview.spending < 0 ? "text-destructive" : undefined} />
          <Stat label={t("reports.savingsRate")} value={overview.savings_rate == null ? "—" : `${Math.round(overview.savings_rate * 100)}%`} />
        </div>
        {overview.other_currencies > 0 && <p className="text-muted-foreground text-xs">{t("reports.otherCurrencies", { count: overview.other_currencies, currency: cur })}</p>}

        {overview.months.length > 1 && (
          <Card>
            <CardHeader><CardTitle>{t("reports.byMonth")}</CardTitle></CardHeader>
            <CardContent>
              <BarChart label={t("reports.byMonth")} format={compact}
                series={[{ key: "income", label: t("reports.income"), color: INCOME_COLOR }, { key: "spending", label: t("reports.spending"), color: SPENDING_COLOR }]}
                groups={overview.months.slice(-12).map((m) => ({ label: monthLabel(m.month), values: { income: m.income, spending: m.spending } }))} />
            </CardContent>
          </Card>
        )}

        <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <Card>
            <CardHeader><CardTitle>{t("reports.byCategory")}</CardTitle></CardHeader>
            <CardContent>
              {overview.categories.length === 0 ? <p className="text-muted-foreground text-sm">{t("reports.nothing")}</p> : (
                <Donut label={t("reports.byCategory")} format={compact} center={<span className="text-sm font-semibold">{compact(overview.spending)}</span>}
                  slices={overview.categories.slice(0, 8).map((c, i) => ({ key: c.id ?? `u${i}`, label: `${c.icon ?? ""} ${nameOf(c.name)}`.trim(), value: c.amount, color: c.color ?? CHART_COLORS[i % CHART_COLORS.length] }))} />
              )}
            </CardContent>
          </Card>
          <Card>
            <CardHeader><CardTitle>{t("reports.places")}</CardTitle><CardDescription>{t("reports.placesDesc")}</CardDescription></CardHeader>
            <CardContent>
              {overview.places.length === 0 ? <p className="text-muted-foreground text-sm">{t("reports.nothing")}</p> : (
                <ol className="space-y-2">
                  {overview.places.map((p, i) => (
                    <li key={i} className="space-y-1">
                      <div className="flex items-baseline justify-between gap-2 text-sm">
                        <span className="truncate">{nameOf(p.name)} <span className="text-muted-foreground text-xs">· {t("reports.times", { count: p.count })}</span></span>
                        <span className="shrink-0 tabular-nums">{money(p.amount)}</span>
                      </div>
                      <div className="bg-muted h-1.5 overflow-hidden rounded-full"><div className="h-full rounded-full" style={{ width: `${(p.amount / overview.places[0].amount) * 100}%`, backgroundColor: SPENDING_COLOR }} /></div>
                    </li>
                  ))}
                </ol>
              )}
            </CardContent>
          </Card>
        </div>

        {overview.income_sources.length > 0 && (
          <Card>
            <CardHeader><CardTitle>{t("reports.incomeSources")}</CardTitle></CardHeader>
            <CardContent>
              <ul className="grid gap-x-8 gap-y-1 text-sm sm:grid-cols-2">
                {overview.income_sources.map((s, i) => (
                  <li key={i} className="flex justify-between gap-2"><span className="truncate">{s.icon} {nameOf(s.name)}</span><span className="tabular-nums">{money(s.amount)}</span></li>
                ))}
              </ul>
            </CardContent>
          </Card>
        )}
      </>}
    </div>
  );
}

function Stat({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return (
    <Card className="gap-1 py-4">
      <CardContent>
        <div className="text-muted-foreground text-xs">{label}</div>
        <div className={cn("text-xl font-semibold tabular-nums", tone)}>{value}</div>
      </CardContent>
    </Card>
  );
}
