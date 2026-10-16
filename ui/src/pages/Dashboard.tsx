import { useEffect, useState } from "react";
import { ArrowDownRight, ArrowUpRight, PiggyBank, Wallet } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, type Account, type Transaction } from "@/api";
import { AnimatedMoney } from "@/components/AnimatedMoney";
import { ProgressBar } from "@/components/ProgressBar";
import { Skeleton, SkeletonRows } from "@/components/Skeleton";
import { AnimatedItem, AnimatedList } from "@/components/AnimatedList";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { formatNumber } from "@/lib/utils";
import { useTheme } from "@/hooks/theme";
import { useLocale } from "@/hooks/locale";
import { TransactionRow } from "./Transactions";

function monthRange() {
  const d = new Date();
  const iso = (x: Date) => `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}-${String(x.getDate()).padStart(2, "0")}`;
  return { startDate: iso(new Date(d.getFullYear(), d.getMonth(), 1)), endDate: iso(new Date(d.getFullYear(), d.getMonth() + 1, 0)) };
}

export function Dashboard() {
  const { hideBalances } = useTheme();
  const { t } = useTranslation();
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [month, setMonth] = useState<Transaction[]>([]);
  const [recent, setRecent] = useState<Transaction[]>([]);
  // Amounts appear once the data is in, so they never roll up from zero on first view.
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    const load = () => {
      Promise.all([
        api.accounts.getAll().then(setAccounts),
        api.transactions.getAll(monthRange()).then(setMonth),
        api.transactions.getAll({ limit: 8 }).then(setRecent),
      ]).finally(() => setLoaded(true));
    };
    load();
    window.addEventListener(DATA_RESTORED, load);
    return () => window.removeEventListener(DATA_RESTORED, load);
  }, []);

  const sum = (t: string) => month.filter((x) => x.type === t).reduce((s, x) => s + x.amount, 0);
  const income = sum("income"), expense = sum("expense");
  const netWorth = accounts.reduce((s, a) => s + a.current_balance, 0);
  const { currency: mainCurrency } = useLocale();
  const cur = accounts[0]?.currency ?? mainCurrency;
  const money = (n: number) => <AnimatedMoney value={n} currency={cur} hidden={hideBalances} showChange />;
  const rate = income > 0 ? (income - expense) / income : null;

  const stats = [
    { label: t("dashboard.netWorth"), value: money(netWorth), icon: Wallet, hint: t("dashboard.accountsCount", { count: accounts.length }) },
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
            </CardContent>
          </Card>
        ))}
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
