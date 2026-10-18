import { useEffect, useMemo, useState } from "react";
import { ArrowDownRight, ArrowUpRight, PiggyBank, Wallet } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, type Account, type Transaction } from "@/api";
import { AnimatedMoney } from "@/components/AnimatedMoney";
import { ProgressBar } from "@/components/ProgressBar";
import { Skeleton, SkeletonRows } from "@/components/Skeleton";
import { AnimatedItem, AnimatedList } from "@/components/AnimatedList";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { formatCompactMoney, formatMoney, formatNumber } from "@/lib/utils";
import { BarChart } from "@/components/charts/BarChart";
import { Donut, type Slice } from "@/components/charts/Donut";
import { Sparkline } from "@/components/charts/Sparkline";
import { CHART_COLORS, INCOME_COLOR, SPENDING_COLOR } from "@/components/charts/palette";
import { useTheme } from "@/hooks/theme";
import { useLocale } from "@/hooks/locale";
import { TransactionRow } from "./Transactions";

const iso = (x: Date) => `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}-${String(x.getDate()).padStart(2, "0")}`;
const MONTHS = 6;

/** The last six months, oldest first, including this one. */
function lastMonths() {
  const d = new Date();
  return Array.from({ length: MONTHS }, (_, i) => new Date(d.getFullYear(), d.getMonth() - (MONTHS - 1 - i), 1));
}

export function Dashboard() {
  const { hideBalances } = useTheme();
  const { t } = useTranslation();
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [half, setHalf] = useState<Transaction[]>([]);
  const [recent, setRecent] = useState<Transaction[]>([]);
  // Amounts appear once the data is in, so they never roll up from zero on first view.
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    const load = () => {
      Promise.all([
        api.accounts.getAll().then(setAccounts),
        api.transactions.getAll({ startDate: iso(lastMonths()[0]) }).then(setHalf),
        api.transactions.getAll({ limit: 8 }).then(setRecent),
      ]).finally(() => setLoaded(true));
    };
    load();
    window.addEventListener(DATA_RESTORED, load);
    return () => window.removeEventListener(DATA_RESTORED, load);
  }, []);

  const months = lastMonths();
  const keyOf = (date: string) => date.slice(0, 7);
  const thisMonth = iso(months[MONTHS - 1]).slice(0, 7);
  const month = half.filter((x) => keyOf(x.date) === thisMonth);
  const sum = (t: string) => month.filter((x) => x.type === t).reduce((s, x) => s + x.amount, 0);
  const income = sum("income"), expense = sum("expense");
  const netWorth = accounts.reduce((s, a) => s + a.current_balance, 0);
  const { currency: mainCurrency } = useLocale();
  const cur = accounts[0]?.currency ?? mainCurrency;
  const money = (n: number) => <AnimatedMoney value={n} currency={cur} hidden={hideBalances} showChange />;
  const rate = income > 0 ? (income - expense) / income : null;
  const { locale } = useLocale();
  const compact = (n: number) => formatCompactMoney(n, cur, hideBalances);
  const whole = (n: number) => formatMoney(n, cur, hideBalances, { maximumFractionDigits: 0 });

  // Income and spending per month, and net worth at the end of each month (worked back from today).
  const flow = useMemo(() => months.map((m) => {
    const k = iso(m).slice(0, 7);
    const inMonth = half.filter((x) => keyOf(x.date) === k);
    const tot = (ty: string) => inMonth.filter((x) => x.type === ty).reduce((a, x) => a + x.amount, 0);
    return { label: new Intl.DateTimeFormat(locale, { month: "short" }).format(m), income: tot("income"), spending: tot("expense") };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [half, locale]);
  const trend = useMemo(() => {
    const ends = new Array<number>(MONTHS);
    ends[MONTHS - 1] = netWorth;
    for (let i = MONTHS - 1; i > 0; i--) ends[i - 1] = ends[i] - (flow[i].income - flow[i].spending);
    return ends;
  }, [flow, netWorth]);

  // This month's spending by category: the five largest, the rest grouped.
  const slices = useMemo(() => {
    const by = new Map<string, { value: number; color?: string | null }>();
    for (const x of month) if (x.type === "expense") {
      const name = x.masked ? t("hidden.category") : x.category_name || t("dashboard.uncategorised");
      const e = by.get(name) ?? { value: 0, color: x.category_color };
      e.value += x.amount; by.set(name, e);
    }
    const sorted = [...by.entries()].sort((a, b) => b[1].value - a[1].value).slice(0, 5);
    // Categories without a colour of their own get one nobody else is using.
    const used = new Set(sorted.map(([, v]) => v.color?.toLowerCase()).filter(Boolean));
    const spare = CHART_COLORS.filter((c) => !used.has(c.toLowerCase()));
    let next = 0;
    const top: Slice[] = sorted.map(([label, v]) => ({ key: label, label, value: v.value, color: v.color || spare[next++ % Math.max(1, spare.length)] || CHART_COLORS[0] }));
    const rest = [...by.values()].reduce((a, v) => a + v.value, 0) - top.reduce((a, x) => a + x.value, 0);
    if (rest > 0) top.push({ key: "__other", label: t("dashboard.other"), value: rest, color: "#94a3b8" });
    return top;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [half, t]);

  const stats = [
    { label: t("dashboard.netWorth"), value: money(netWorth), icon: Wallet, hint: t("dashboard.accountsCount", { count: accounts.length }), trend: true },
    { label: t("dashboard.income"), value: money(income), icon: ArrowUpRight, hint: t("dashboard.clearedPending") },
    { label: t("dashboard.spent"), value: money(expense), icon: ArrowDownRight, hint: t("dashboard.clearedPending") },
    { label: t("dashboard.saved"), value: money(income - expense), icon: PiggyBank, hint: rate != null ? t("dashboard.savingsRate", { rate: formatNumber(Math.round(rate * 100)) }) : "—", bar: rate != null && rate > 0 ? rate : null },
  ];

  return (
    <div className="space-y-6">
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {stats.map((s) => (
          <Card key={s.label} className="gap-2">
            <CardHeader className="flex-row items-center justify-between">
              <CardDescription>{s.label}</CardDescription>
              <s.icon className="text-muted-foreground size-4" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-semibold tabular-nums">{loaded ? s.value : <Skeleton className="h-8 w-32" />}</div>
              <p className="text-muted-foreground mt-1 text-xs">{s.hint}</p>
              {s.bar != null && <ProgressBar value={s.bar} className="mt-2" barClassName="bg-success" label={s.hint} />}
              {s.trend && loaded && <Sparkline values={trend} className="mt-2 h-8 w-full" label={t("dashboard.trend")} />}
            </CardContent>
          </Card>
        ))}
      </div>
      <div className="grid gap-4 xl:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <Card>
          <CardHeader><CardTitle>{t("dashboard.flowTitle")}</CardTitle><CardDescription>{t("dashboard.flowDesc")}</CardDescription></CardHeader>
          <CardContent>
            {loaded ? (
              <BarChart label={t("dashboard.flowTitle")} format={compact}
                series={[{ key: "income", label: t("dashboard.seriesIncome"), color: INCOME_COLOR }, { key: "spending", label: t("dashboard.seriesSpending"), color: SPENDING_COLOR }]}
                groups={flow.map((f) => ({ label: f.label, values: { income: f.income, spending: f.spending } }))} />
            ) : <Skeleton className="h-52 w-full" />}
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle>{t("dashboard.categoriesTitle")}</CardTitle><CardDescription>{t("dashboard.categoriesDesc")}</CardDescription></CardHeader>
          <CardContent>
            {!loaded ? <Skeleton className="h-44 w-full" />
              : slices.length === 0 ? <p className="text-muted-foreground py-10 text-center text-sm">{t("dashboard.noSpending")}</p>
              : <Donut slices={slices} format={whole} label={t("dashboard.categoriesTitle")}
                  center={<div><div className="text-muted-foreground text-[11px]">{t("dashboard.seriesSpending")}</div><div className="text-sm font-semibold tabular-nums">{compact(expense)}</div></div>} />}
          </CardContent>
        </Card>
      </div>
      <Card>
        <CardHeader><CardTitle>{t("dashboard.recent")}</CardTitle><CardDescription>{t("dashboard.recentHint")}</CardDescription></CardHeader>
        <CardContent className="divide-y">
          {!loaded && <SkeletonRows rows={3} />}
          {loaded && recent.length === 0 && <p className="text-muted-foreground py-6 text-center text-sm">{t("dashboard.noTransactions")}</p>}
          <AnimatedList>
            {recent.map((t) => <AnimatedItem key={t.id}><TransactionRow t={t} hidden={hideBalances} /></AnimatedItem>)}
          </AnimatedList>
        </CardContent>
      </Card>
    </div>
  );
}
