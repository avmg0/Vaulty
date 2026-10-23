import { useState } from "react";
import { Check, Loader2, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type TaxPayment, type TaxPlan } from "@/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate, formatMoney, formatNumber } from "@/lib/utils";
import { today } from "@/components/projects/ProjectDialogs";

const WHOLE: Intl.NumberFormatOptions = { maximumFractionDigits: 0, minimumFractionDigits: 0 };
const SELECT = "border-input dark:bg-input/30 h-9 w-full rounded-md border bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]";
const KINDS: TaxPayment["kind"][] = ["tds", "advance", "self_assessment"];

/** Tax paid for the year (TDS, advance, self-assessment), what's left, and advance-tax dates. */
export function TaxPaidCard({ plan, onChange }: { plan: TaxPlan; onChange: () => Promise<void> }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const money = (n: number) => formatMoney(n, plan.currency, hideBalances, WHOLE);
  const usedName = plan.comparison.estimates.find((e) => e.regime === plan.used)?.name ?? "";
  const advanceDue = plan.advance.some((a) => a.needed > 0);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("tax.paidTitle")}</CardTitle>
        <CardDescription>{t("tax.paidDesc", { context: plan.country })}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        <dl className="grid gap-3 sm:grid-cols-3">
          <Figure label={t("tax.taxFor", { regime: usedName })} value={money(plan.total)} />
          <Figure label={t("tax.paidSoFar")} value={money(plan.paid)} sub={plan.tds > 0 ? t("tax.ofWhichTds", { amount: money(plan.tds), context: plan.country }) : undefined} />
          {plan.balance > 0
            ? <Figure label={t("tax.stillToPay")} value={money(plan.balance)} strong />
            : <Figure label={plan.balance < 0 ? t("tax.refund") : t("tax.allPaid")} value={money(-plan.balance)} strong good />}
        </dl>

        {plan.advance.length > 0 && (
          <section className="space-y-2" aria-labelledby="advance-h">
            <h2 id="advance-h" className="text-sm font-medium">{t("tax.advanceTitle")}</h2>
            {advanceDue ? (
              <ol className="divide-y rounded-lg border">
                {plan.advance.map((a) => (
                  <li key={a.date} className="flex flex-wrap items-center justify-between gap-x-3 gap-y-0.5 px-3 py-2 text-sm">
                    <span className={cn("tabular-nums sm:w-28", a.past && "text-muted-foreground")}>{formatDate(a.date)}</span>
                    <span className="text-muted-foreground order-last w-full text-xs sm:order-none sm:w-auto sm:flex-1">{t("tax.advanceBy", { percent: formatNumber(a.percent), amount: money(a.needed) })}</span>
                    {a.short > 0
                      ? <span className={cn("text-xs font-medium tabular-nums", a.past ? "text-destructive" : "")}>{a.past ? t("tax.shortBy", { amount: money(a.short) }) : t("tax.toPay", { amount: money(a.short) })}</span>
                      : <span className="flex items-center gap-1 text-xs text-emerald-700 dark:text-emerald-400"><Check className="size-3.5" />{t("tax.covered")}</span>}
                  </li>
                ))}
              </ol>
            ) : <p className="text-muted-foreground text-sm">{t("tax.noAdvance")}</p>}
            {plan.advance_note && <p className="text-muted-foreground text-xs">{plan.advance_note}</p>}
          </section>
        )}

        <section className="space-y-2" aria-labelledby="payments-h">
          <h2 id="payments-h" className="text-sm font-medium">{t("tax.payments")}</h2>
          {plan.payments.length === 0 ? <p className="text-muted-foreground text-sm">{t("tax.noPayments", { context: plan.country })}</p> : (
            <ul className="divide-y rounded-lg border">
              {plan.payments.map((p) => <PaymentRow key={p.id} p={p} money={money} onChange={onChange} country={plan.country} />)}
            </ul>
          )}
          <AddPayment plan={plan} onChange={onChange} />
        </section>
      </CardContent>
    </Card>
  );
}

function Figure({ label, value, sub, strong, good }: { label: string; value: string; sub?: string; strong?: boolean; good?: boolean }) {
  return (
    <div className={cn("rounded-lg border p-3", strong && "bg-muted/40")}>
      <dt className="text-muted-foreground text-xs">{label}</dt>
      <dd className={cn("text-lg font-semibold tabular-nums", good && "text-emerald-700 dark:text-emerald-400")}>{value}</dd>
      {sub && <dd className="text-muted-foreground text-xs">{sub}</dd>}
    </div>
  );
}

function PaymentRow({ p, money, onChange, country }: { p: TaxPayment; money: (n: number) => string; onChange: () => Promise<void>; country: string }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const remove = () => { setBusy(true); api.tax.deletePayment(p.id).then(onChange).finally(() => setBusy(false)); };
  return (
    <li className="flex items-center gap-3 px-3 py-2 text-sm">
      <div className="min-w-0 flex-1">
        <div className="truncate">{t(`tax.kind.${p.kind}`, { context: country })}</div>
        <div className="text-muted-foreground truncate text-xs">{formatDate(p.date)}{p.note && ` · ${p.note}`}</div>
      </div>
      <span className="tabular-nums">{money(p.amount)}</span>
      <Button size="icon" variant="ghost" className="size-8" onClick={remove} disabled={busy} aria-label={t("tax.removePayment")}><Trash2 className="size-4" /></Button>
    </li>
  );
}

function AddPayment({ plan, onChange }: { plan: TaxPlan; onChange: () => Promise<void> }) {
  const { t } = useTranslation();
  const [kind, setKind] = useState<TaxPayment["kind"]>("tds");
  const [date, setDate] = useState(() => { const d = today(); return d < plan.year.start ? plan.year.start : d; });
  const [amount, setAmount] = useState("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const add = (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    api.tax.addPayment(plan.country, plan.year.id, { kind, date, amount, note })
      .then(() => { setAmount(""); setNote(""); setError(""); return onChange(); })
      .catch((err) => setError(errorText(err)))
      .finally(() => setBusy(false));
  };
  return (
    <form onSubmit={add} className="grid gap-2 rounded-lg border border-dashed p-3 sm:grid-cols-[9rem_9.5rem_minmax(0,1fr)]">
      <div className="space-y-1"><Label htmlFor="tp-kind" className="text-xs">{t("tax.paymentKind")}</Label>
        <select id="tp-kind" className={SELECT} value={kind} onChange={(e) => setKind(e.target.value as TaxPayment["kind"])}>
          {KINDS.map((k) => <option key={k} value={k}>{t(`tax.kind.${k}`, { context: plan.country })}</option>)}
        </select>
      </div>
      <div className="space-y-1"><Label htmlFor="tp-date" className="text-xs">{t("tax.paymentDate")}</Label>
        <Input id="tp-date" type="date" min={plan.year.start} value={date} onChange={(e) => setDate(e.target.value)} required />
      </div>
      <div className="space-y-1"><Label htmlFor="tp-amount" className="text-xs">{t("tax.paymentAmount")}</Label>
        <Input id="tp-amount" inputMode="decimal" placeholder="0" value={amount} onChange={(e) => setAmount(e.target.value.replace(/[^\d.]/g, ""))} required />
      </div>
      <div className="space-y-1 sm:col-span-2"><Label htmlFor="tp-note" className="text-xs">{t("tax.paymentNote")}</Label>
        <Input id="tp-note" maxLength={200} placeholder={t("tax.paymentNoteHint")} value={note} onChange={(e) => setNote(e.target.value)} />
      </div>
      <div className="flex items-end"><Button type="submit" className="w-full" disabled={busy || !amount}>{busy && <Loader2 className="animate-spin" />}{t("tax.addPayment")}</Button></div>
      {error && <p role="alert" className="text-destructive text-sm sm:col-span-3">{error}</p>}
    </form>
  );
}
