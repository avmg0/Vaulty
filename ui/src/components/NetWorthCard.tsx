import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type NetWorth, type NetWorthPoint } from "@/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Segmented } from "@/components/Segmented";
import { useTheme } from "@/hooks/theme";
import { cn, formatCompactMoney, formatDate, formatMoney } from "@/lib/utils";

const WHOLE: Intl.NumberFormatOptions = { maximumFractionDigits: 0, minimumFractionDigits: 0 };
type Part = "cash" | "investments" | "owed_to_you" | "debts" | "you_owe";
const UP: Part[] = ["cash", "investments", "owed_to_you"];
const DOWN: Part[] = ["debts", "you_owe"];
const COLOR: Record<Part, string> = {
  cash: "fill-sky-600 dark:fill-sky-400", investments: "fill-emerald-600 dark:fill-emerald-400", owed_to_you: "fill-violet-500 dark:fill-violet-400",
  debts: "fill-rose-500 dark:fill-rose-400", you_owe: "fill-amber-500 dark:fill-amber-400",
};
const DOT: Record<Part, string> = { cash: "bg-sky-600 dark:bg-sky-400", investments: "bg-emerald-600 dark:bg-emerald-400", owed_to_you: "bg-violet-500 dark:bg-violet-400", debts: "bg-rose-500 dark:bg-rose-400", you_owe: "bg-amber-500 dark:bg-amber-400" };

/** Net worth month by month: what you have stacked above the line, what you owe below it, and
 *  the net as a line through them. */
export function NetWorthCard() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [months, setMonths] = useState<"12" | "36" | "120">("12");
  const [data, setData] = useState<NetWorth[] | null>(null);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  useEffect(() => { const bump = () => setVersion((v) => v + 1); window.addEventListener(DATA_RESTORED, bump); return () => window.removeEventListener(DATA_RESTORED, bump); }, []);
  useEffect(() => { api.reports.netWorth(Number(months)).then((d) => { setData(d); setError(""); }).catch((e) => setError(errorText(e))); }, [months, version]);

  return (
    <Card>
      <CardHeader className="flex-row flex-wrap items-start justify-between gap-2">
        <div className="space-y-1.5"><CardTitle>{t("networth.title")}</CardTitle><CardDescription>{t("networth.desc")}</CardDescription></div>
        <Segmented className="print-hide" label={t("networth.period")} value={months} onChange={setMonths}
          options={[{ value: "12", label: t("networth.year") }, { value: "36", label: t("networth.years", { count: 3 }) }, { value: "120", label: t("networth.years", { count: 10 }) }]} />
      </CardHeader>
      <CardContent className="space-y-6">
        {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
        {data?.length === 0 && <p className="text-muted-foreground text-sm">{t("networth.empty")}</p>}
        {data?.map((n) => {
          const m = (x: number) => formatMoney(x, n.currency, hideBalances, WHOLE);
          const now = n.points.at(-1)!;
          const change = (x: number | null) => x == null ? null : <span className={cn("tabular-nums", x > 0 ? "text-emerald-700 dark:text-emerald-400" : x < 0 ? "text-rose-700 dark:text-rose-400" : "")}>{x > 0 ? "+" : x < 0 ? "−" : ""}{m(Math.abs(x))}</span>;
          return (
            <section key={n.currency} className="space-y-4" aria-label={t("networth.inCurrency", { currency: n.currency })}>
              <div className="flex flex-wrap items-end gap-x-8 gap-y-2">
                <div>
                  <div className="text-muted-foreground text-xs">{t("networth.now")}{data.length > 1 && ` · ${n.currency}`}</div>
                  <div className="text-3xl font-semibold tracking-tight tabular-nums">{m(n.net)}</div>
                </div>
                {n.change_month != null && <div className="text-sm"><div className="text-muted-foreground text-xs">{t("networth.thisMonth")}</div>{change(n.change_month)}</div>}
                <div className="text-sm"><div className="text-muted-foreground text-xs">{t("networth.overPeriod", { date: formatDate(n.points[0].date, { month: "short", year: "numeric" }) })}</div>{change(n.change_period)}</div>
              </div>
              <Chart points={n.points} currency={n.currency} />
              <ul className="grid gap-2 text-sm sm:grid-cols-5">
                {[...UP, ...DOWN].map((k) => (
                  <li key={k} className="flex items-start gap-2">
                    <span className={cn("mt-1.5 size-2.5 shrink-0 rounded-sm", DOT[k])} aria-hidden />
                    <span><span className="text-muted-foreground block text-xs">{t(`networth.parts.${k}`)}</span><span className="tabular-nums">{DOWN.includes(k) && now[k] > 0 ? "−" : ""}{m(now[k])}</span></span>
                  </li>
                ))}
              </ul>
            </section>
          );
        })}
        {data && data.length > 0 && <p className="text-muted-foreground text-xs">{t("networth.note")}</p>}
      </CardContent>
    </Card>
  );
}

function Chart({ points, currency }: { points: NetWorthPoint[]; currency: string }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  // Drawn at the width it's shown at, so labels stay readable on a narrow window.
  const box = useRef<HTMLElement>(null);
  const [W, setW] = useState(720);
  useEffect(() => {
    const el = box.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(([e]) => setW(Math.max(240, Math.round(e.contentRect.width))));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const H = 200, pad = 4;
  const top = Math.max(1, ...points.map((p) => UP.reduce((s, k) => s + p[k], 0)), ...points.map((p) => p.net));
  const bottom = Math.max(0, ...points.map((p) => DOWN.reduce((s, k) => s + p[k], 0)), ...points.map((p) => -p.net));
  const span = top + bottom;
  const y = (v: number) => pad + (top - v) / span * (H - 2 * pad);
  const slot = W / points.length;
  const bw = Math.max(2, Math.min(36, slot * 0.62));
  const x = (i: number) => slot * i + slot / 2;
  const line = points.map((p, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(p.net).toFixed(1)}`).join(" ");
  const every = Math.ceil(points.length / Math.max(3, Math.floor(W / 80)));
  const label = (d: string, i: number) => i === points.length - 1 ? t("networth.today") : formatDate(d, points.length > 14 ? { month: "short", year: "2-digit" } : { month: "short" });
  return (
    <figure ref={box}>
      <svg width={W} height={H + 18} viewBox={`0 0 ${W} ${H + 18}`} className="block max-w-full" role="img" aria-label={t("networth.chartLabel")}>
        <line x1={0} x2={W} y1={y(0)} y2={y(0)} className="stroke-border" strokeWidth={1} />
        {points.map((p, i) => {
          let up = 0, down = 0;
          const tip = `${label(p.date, i)}: ${t("networth.net")} ${formatMoney(p.net, currency, hideBalances, WHOLE)}`;
          return (
            <g key={p.date}>
              <title>{tip}</title>
              {UP.map((k) => { const h = p[k] / span * (H - 2 * pad); const r = <rect key={k} x={x(i) - bw / 2} y={y(up + p[k])} width={bw} height={Math.max(0, h)} className={COLOR[k]} opacity={0.85} />; up += p[k]; return r; })}
              {DOWN.map((k) => { const h = p[k] / span * (H - 2 * pad); const r = <rect key={k} x={x(i) - bw / 2} y={y(-down)} width={bw} height={Math.max(0, h)} className={COLOR[k]} opacity={0.85} />; down += p[k]; return r; })}
              {(i % every === 0 || i === points.length - 1) && <text x={x(i)} y={H + 14} textAnchor="middle" className="fill-muted-foreground text-[11px]">{label(p.date, i)}</text>}
            </g>
          );
        })}
        <path d={line} fill="none" className="stroke-foreground" strokeWidth={2} strokeLinejoin="round" />
        {points.map((p, i) => <circle key={p.date} cx={x(i)} cy={y(p.net)} r={points.length > 40 ? 1.5 : 3} className="fill-background stroke-foreground" strokeWidth={1.5} />)}
      </svg>
      <figcaption className="sr-only">{points.map((p, i) => `${label(p.date, i)} ${formatCompactMoney(p.net, currency, hideBalances)}`).join(", ")}</figcaption>
    </figure>
  );
}
