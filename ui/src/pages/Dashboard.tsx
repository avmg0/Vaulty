import { useEffect, useState } from "react";
import { ArrowDownRight, ArrowUpRight, PiggyBank, Wallet } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, type Account, type Transaction } from "@/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { formatMoney, formatNumber } from "@/lib/utils";
import { useTheme } from "@/hooks/theme";
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

  useEffect(() => {
    api.accounts.getAll().then(setAccounts);
    api.transactions.getAll(monthRange()).then(setMonth);
    api.transactions.getAll({ limit: 8 }).then(setRecent);
  }, []);

  const sum = (t: string) => month.filter((x) => x.type === t).reduce((s, x) => s + x.amount, 0);
  const income = sum("income"), expense = sum("expense");
  const netWorth = accounts.reduce((s, a) => s + a.current_balance, 0);
  const money = (n: number) => formatMoney(n, accounts[0]?.currency ?? "USD", hideBalances);

  const stats = [
    { label: t("dashboard.netWorth"), value: money(netWorth), icon: Wallet, hint: t("dashboard.accountsCount", { count: accounts.length }) },
    { label: t("dashboard.income"), value: money(income), icon: ArrowUpRight, hint: t("dashboard.clearedPending") },
    { label: t("dashboard.spent"), value: money(expense), icon: ArrowDownRight, hint: t("dashboard.clearedPending") },
    { label: t("dashboard.saved"), value: money(income - expense), icon: PiggyBank, hint: income ? t("dashboard.savingsRate", { rate: formatNumber(Math.round(((income - expense) / income) * 100)) }) : "—" },
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
              <div className="text-2xl font-semibold tabular-nums">{s.value}</div>
              <p className="text-muted-foreground mt-1 text-xs">{s.hint}</p>
            </CardContent>
          </Card>
        ))}
      </div>
      <Card>
        <CardHeader><CardTitle>{t("dashboard.recent")}</CardTitle><CardDescription>{t("dashboard.recentHint")}</CardDescription></CardHeader>
        <CardContent className="divide-y">
          {recent.length === 0 && <p className="text-muted-foreground py-6 text-center text-sm">{t("dashboard.noTransactions")}</p>}
          {recent.map((t) => <TransactionRow key={t.id} t={t} hidden={hideBalances} />)}
        </CardContent>
      </Card>
    </div>
  );
}
