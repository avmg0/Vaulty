import { useCallback, useEffect, useState } from "react";
import { ChevronDown, FileText, HeartPulse, Home, Plane, Plus, Shield, ShieldCheck, Car, Umbrella, Trash2, Pencil, Wallet, ClipboardList } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, POLICY_KINDS, type InsuranceTotal, type Policy, type PolicyClaim, type PolicyKind } from "@/api";
import { ClaimDialog, PayDialog, PolicyDialog } from "@/components/insurance/Dialogs";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate, formatMoney, formatNumber } from "@/lib/utils";

const WHOLE: Intl.NumberFormatOptions = { maximumFractionDigits: 0, minimumFractionDigits: 0 };
const ICON: Record<PolicyKind, LucideIcon> = { term_life: Umbrella, life: Umbrella, health: HeartPulse, motor: Car, home: Home, travel: Plane, accident: ShieldCheck, other: Shield };
const DUE_TONE = { soon: "bg-amber-100 text-amber-900 dark:bg-amber-950 dark:text-amber-200", late: "bg-red-100 text-red-900 dark:bg-red-950 dark:text-red-200", lapsing: "bg-red-600 text-white", none: "" };

export function InsurancePage() {
  const { t } = useTranslation();
  const [policies, setPolicies] = useState<Policy[] | null>(null);
  const [totals, setTotals] = useState<InsuranceTotal[]>([]);
  const [editing, setEditing] = useState<Policy | "new" | null>(null);
  const [paying, setPaying] = useState<Policy | null>(null);
  const [claiming, setClaiming] = useState<{ policy: Policy; claim: PolicyClaim | null } | null>(null);
  const [error, setError] = useState("");
  const reload = useCallback(() => {
    Promise.all([api.insurance.list(), api.insurance.summary()]).then(([p, s]) => { setPolicies(p); setTotals(s); setError(""); }).catch((e) => setError(errorText(e)));
  }, []);
  useEffect(() => { reload(); window.addEventListener(DATA_RESTORED, reload); return () => window.removeEventListener(DATA_RESTORED, reload); }, [reload]);
  const done = (changed: boolean) => { setEditing(null); setPaying(null); setClaiming(null); if (changed) reload(); };

  if (!policies) return error ? <p role="alert" className="text-destructive text-sm">{error}</p> : null;
  const due = policies.filter((p) => p.due !== "none").sort((a, b) => (a.days_to_due ?? 0) - (b.days_to_due ?? 0));
  const kinds = POLICY_KINDS.filter((k) => policies.some((p) => p.kind === k));

  return (
    <div className="mx-auto max-w-6xl space-y-6">
      {policies.length === 0 ? (
        <Card><CardContent className="space-y-4 py-10 text-center">
          <div className="bg-secondary mx-auto grid size-12 place-items-center rounded-xl"><Shield className="size-5" /></div>
          <div className="space-y-1"><p className="font-semibold">{t("insurance.emptyTitle")}</p><p className="text-muted-foreground mx-auto max-w-md text-sm">{t("insurance.emptyBody")}</p></div>
          <Button onClick={() => setEditing("new")}><Plus />{t("insurance.add")}</Button>
        </CardContent></Card>
      ) : (<>
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="grid flex-1 gap-3 sm:grid-cols-3">{totals.map((s) => <Totals key={s.currency} s={s} />)}</div>
          <Button onClick={() => setEditing("new")}><Plus />{t("insurance.add")}</Button>
        </div>
        {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
        {due.length > 0 && (
          <section aria-labelledby="due-h" className="rounded-xl border p-4">
            <h2 id="due-h" className="mb-2 text-sm font-medium">{t("insurance.dueTitle")}</h2>
            <ul className="divide-y">
              {due.map((p) => (
                <li key={p.id} className="flex flex-wrap items-center gap-3 py-2 text-sm">
                  <span className="min-w-0 flex-1 truncate font-medium">{p.name}</span>
                  <DueChip p={p} />
                  <span className="tabular-nums">{p.premium > 0 && formatMoney(p.premium, p.currency, false, WHOLE)}</span>
                  <Button size="sm" variant="outline" onClick={() => setPaying(p)}><Wallet />{t("insurance.recordPayment")}</Button>
                </li>
              ))}
            </ul>
          </section>
        )}
        {kinds.map((k) => (
          <section key={k} aria-labelledby={`k-${k}`} className="space-y-2">
            <h2 id={`k-${k}`} className="text-muted-foreground text-sm font-medium">{t(`insurance.kinds.${k}`)}</h2>
            <div className="grid grid-cols-1 gap-3">
              {policies.filter((p) => p.kind === k).map((p) => (
                <PolicyRow key={p.id} p={p} onEdit={() => setEditing(p)} onPay={() => setPaying(p)} onClaim={(c) => setClaiming({ policy: p, claim: c })} onChange={reload} />
              ))}
            </div>
          </section>
        ))}
      </>)}
      {editing && <PolicyDialog policy={editing === "new" ? null : editing} onClose={done} />}
      {paying && <PayDialog policy={paying} onClose={done} />}
      {claiming && <ClaimDialog policy={claiming.policy} claim={claiming.claim} onClose={done} />}
    </div>
  );
}

function Totals({ s }: { s: InsuranceTotal }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const m = (n: number) => formatMoney(n, s.currency, hideBalances, WHOLE);
  return (<>
    <div className="rounded-xl border p-4">
      <div className="text-muted-foreground text-xs">{t("insurance.lifeCover")}</div>
      <div className="text-2xl font-semibold tracking-tight tabular-nums">{m(s.life_cover)}</div>
      <p className="text-muted-foreground mt-1 text-xs">
        {s.life_multiple != null ? t("insurance.multiple", { n: formatNumber(s.life_multiple, { maximumFractionDigits: 1 }), income: m(s.income_year) }) : t("insurance.noIncome")}
      </p>
      <p className="text-muted-foreground text-xs">{t("insurance.ruleOfThumb")}</p>
    </div>
    <div className="rounded-xl border p-4">
      <div className="text-muted-foreground text-xs">{t("insurance.healthCover")}</div>
      <div className="text-2xl font-semibold tracking-tight tabular-nums">{m(s.health_cover)}</div>
    </div>
    <div className="rounded-xl border p-4">
      <div className="text-muted-foreground text-xs">{t("insurance.premiumsYear")}</div>
      <div className="text-2xl font-semibold tracking-tight tabular-nums">{m(s.premiums_yearly)}</div>
      <p className="text-muted-foreground mt-1 text-xs">{t("insurance.perMonthAvg", { amount: m(s.premiums_yearly / 12) })}</p>
    </div>
  </>);
}

function DueChip({ p }: { p: Policy }) {
  const { t } = useTranslation();
  if (p.due === "none" || p.days_to_due == null) return null;
  const d = p.days_to_due;
  return <span className={cn("rounded-full px-2 py-0.5 text-xs font-medium", DUE_TONE[p.due])}>
    {d === 0 ? t("insurance.dueToday") : d > 0 ? t("insurance.dueIn", { count: d }) : p.due === "lapsing" ? t("insurance.lapsing", { count: -d }) : t("insurance.lateBy", { count: -d })}
  </span>;
}

function PolicyRow({ p, onEdit, onPay, onClaim, onChange }: { p: Policy; onEdit: () => void; onPay: () => void; onClaim: (c: PolicyClaim | null) => void; onChange: () => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [open, setOpen] = useState(false);
  const m = (n: number) => formatMoney(n, p.currency, hideBalances, WHOLE);
  const Icon = ICON[p.kind];
  const inactive = p.status !== "active";
  return (
    <Card className={cn("gap-0 py-0", inactive && "opacity-70")}>
      <button type="button" aria-expanded={open} onClick={() => setOpen(!open)} className="flex w-full flex-wrap items-center gap-x-4 gap-y-2 p-4 text-start">
        <div className="bg-secondary grid size-10 shrink-0 place-items-center rounded-lg"><Icon className="size-5" aria-hidden /></div>
        <div className="min-w-0 flex-1 basis-48">
          <div className="truncate font-medium">{p.name}{inactive && <span className="text-muted-foreground font-normal"> · {t(`insurance.statuses.${p.status}`)}</span>}</div>
          <div className="text-muted-foreground truncate text-xs">{[p.insurer, p.policy_no, p.covers].filter(Boolean).join(" · ")}</div>
        </div>
        {p.cover != null && <div className="text-end"><div className="text-muted-foreground text-xs">{t("insurance.cover")}</div><div className="font-semibold tabular-nums">{m(p.cover)}</div></div>}
        <div className="text-end">
          <div className="text-muted-foreground text-xs">{t(`insurance.frequencies.${p.frequency}`)}</div>
          <div className="tabular-nums">{m(p.premium)}</div>
        </div>
        <div className="flex min-w-28 flex-col items-end gap-1 text-xs">
          {p.next_due && <span className="text-muted-foreground">{t("insurance.nextOn", { date: formatDate(p.next_due) })}</span>}
          <DueChip p={p} />
        </div>
        <ChevronDown className={cn("text-muted-foreground size-4 transition-transform", open && "rotate-180")} aria-hidden />
      </button>
      {open && (
        <CardContent className="space-y-4 border-t pt-4 pb-4">
          <dl className="grid gap-3 text-sm sm:grid-cols-3">
            {([["holder", p.holder], ["nominee", p.nominee], ["contact", p.contact], ["startDate", p.start_date && formatDate(p.start_date)], ["endDate", p.end_date && formatDate(p.end_date)],
              ["paidThisYear", m(p.paid_this_year)]] as [string, string | null][]).filter(([, v]) => v).map(([k, v]) => (
              <div key={k}><dt className="text-muted-foreground text-xs">{t(`insurance.${k}`)}</dt><dd className="break-words">{v}</dd></div>
            ))}
          </dl>
          {!p.nominee && (p.kind === "term_life" || p.kind === "life") && <p className="text-xs text-amber-700 dark:text-amber-400">{t("insurance.noNominee")}</p>}
          {p.document_title && <p className="text-muted-foreground flex items-center gap-1.5 text-xs"><FileText className="size-3.5" />{t("insurance.inDocuments", { title: p.document_title })}</p>}
          {p.notes && <p className="text-muted-foreground text-sm">{p.notes}</p>}

          <div className="grid gap-4 md:grid-cols-2">
            <section aria-label={t("insurance.payments")}>
              <h3 className="mb-1.5 text-xs font-medium">{t("insurance.payments")}</h3>
              {p.payments.length === 0 ? <p className="text-muted-foreground text-xs">{t("insurance.noPayments")}</p> : (
                <ul className="divide-y rounded-lg border text-sm">
                  {p.payments.slice(0, 6).map((x) => (
                    <li key={x.id} className="flex items-center gap-3 px-3 py-1.5">
                      <span className="flex-1">{formatDate(x.date)}</span><span className="tabular-nums">{m(x.amount)}</span>
                      <Button size="icon" variant="ghost" className="size-7" aria-label={t("insurance.removePayment")} onClick={() => api.insurance.deletePayment(x.id).then(onChange)}><Trash2 className="size-3.5" /></Button>
                    </li>
                  ))}
                </ul>
              )}
            </section>
            <section aria-label={t("insurance.claims")}>
              <h3 className="mb-1.5 text-xs font-medium">{t("insurance.claims")}</h3>
              {p.claims.length === 0 ? <p className="text-muted-foreground text-xs">{t("insurance.noClaims")}</p> : (
                <ul className="divide-y rounded-lg border text-sm">
                  {p.claims.map((c) => (
                    <li key={c.id}>
                      <button type="button" onClick={() => onClaim(c)} className="hover:bg-accent flex w-full items-center gap-3 px-3 py-1.5 text-start">
                        <span className="min-w-0 flex-1 truncate">{c.what}<span className="text-muted-foreground block text-xs">{formatDate(c.date)} · {t(`insurance.claimStatuses.${c.status}`)}</span></span>
                        <span className="text-end tabular-nums">{m(c.claimed)}{c.settled != null && <span className="text-muted-foreground block text-xs">{t("insurance.paidOut", { amount: m(c.settled) })}</span>}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </section>
          </div>
          <div className="flex flex-wrap gap-2 border-t pt-3">
            {p.status === "active" && p.premium > 0 && <Button size="sm" variant="outline" onClick={onPay}><Wallet />{t("insurance.recordPayment")}</Button>}
            <Button size="sm" variant="outline" onClick={() => onClaim(null)}><ClipboardList />{t("insurance.addClaim")}</Button>
            <Button size="sm" variant="ghost" onClick={onEdit}><Pencil />{t("common.edit")}</Button>
            <Button size="sm" variant="ghost" className="text-destructive ms-auto" aria-label={t("insurance.deleteNamed", { name: p.name })} onClick={() => api.insurance.delete(p.id).then(onChange)}><Trash2 /></Button>
          </div>
        </CardContent>
      )}
    </Card>
  );
}
