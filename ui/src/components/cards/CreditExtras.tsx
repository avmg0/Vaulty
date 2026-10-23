import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { CalendarClock, Loader2, Minus, Plus, Sparkles, Trash2, Trophy } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_CHANGED, errorText, type BenefitInput, type BenefitKind, type BestPick, type CardBenefit, type CardCycle, type CardCycles, type Category, type WalletCard } from "@/api";
import { Segmented } from "@/components/Segmented";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn, formatDate, formatMoney } from "@/lib/utils";

type Money = (n: number | null | undefined) => ReactNode;
const days = (iso: string) => Math.round((new Date(`${iso}T00:00:00`).getTime() - new Date(new Date().toDateString()).getTime()) / 86_400_000);

const TONE: Record<CardCycle["status"], string> = {
  paid: "bg-emerald-100 text-emerald-900 dark:bg-emerald-500/20 dark:text-emerald-200",
  nothing_due: "bg-muted text-muted-foreground",
  min_paid: "bg-sky-100 text-sky-900 dark:bg-sky-500/20 dark:text-sky-200",
  due: "bg-muted text-foreground",
  carried: "bg-amber-100 text-amber-900 dark:bg-amber-500/20 dark:text-amber-200",
  overdue: "bg-red-100 text-red-900 dark:bg-red-500/20 dark:text-red-200",
  missed: "bg-red-100 text-red-900 dark:bg-red-500/20 dark:text-red-200",
};
function StatusChip({ status }: { status: CardCycle["status"] }) {
  const { t } = useTranslation();
  return <span className={cn("rounded-full px-2 py-0.5 text-xs font-medium whitespace-nowrap", TONE[status])}>{t(`cards.cycle.status.${status}`)}</span>;
}

function Stat({ label, value, strong }: { label: string; value: ReactNode; strong?: boolean }) {
  return <div className="min-w-0"><div className="text-muted-foreground">{label}</div><div className={cn("truncate tabular-nums", strong ? "text-lg font-semibold" : "font-medium")}>{value}</div></div>;
}

/** The latest statement: what it asks for, what's paid, by when; and the ones before. */
export function StatementCard({ card, money }: { card: WalletCard; money: Money }) {
  const { t } = useTranslation();
  const [cy, setCy] = useState<CardCycles | null | undefined>(undefined);
  const [entering, setEntering] = useState(false);
  const load = () => api.cardExtras.cycles(card.id).then(setCy).catch(() => setCy(null));
  useEffect(() => { load(); const on = () => load(); window.addEventListener(DATA_CHANGED, on); return () => window.removeEventListener(DATA_CHANGED, on); }, [card.id]);
  if (cy === undefined) return null;
  if (cy === null) return null;
  const c = cy.current;
  const dueIn = c ? days(c.due_date) : 0;
  const when = (iso: string, o: Intl.DateTimeFormatOptions = { day: "numeric", month: "short" }) => formatDate(iso, o);
  return (
    <Card>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div className="flex items-center gap-3">
            <div className={cn("grid size-10 place-items-center rounded-lg", c && ["due", "overdue"].includes(c.status) && dueIn <= 3 ? "bg-destructive/10 text-destructive" : "bg-secondary")}><CalendarClock className="size-5" aria-hidden /></div>
            <div>
              <div className="font-medium">{c ? t("cards.cycle.statementOf", { date: when(c.statement_date, { day: "numeric", month: "long" }) }) : t("cards.cycle.first", { date: when(cy.next_statement, { day: "numeric", month: "long" }) })}</div>
              {c && c.left > 0 && <div className="text-muted-foreground text-sm">{t(dueIn < 0 ? "cards.cycle.wasDue" : "cards.cycle.payBy", { date: when(c.due_date, { weekday: "short", day: "numeric", month: "long" }), count: Math.abs(dueIn) })}</div>}
            </div>
          </div>
          <div className="flex items-center gap-2">
            {c && <StatusChip status={c.status} />}
            {c && <Button variant="ghost" size="sm" onClick={() => setEntering(true)}>{c.entered ? t("cards.cycle.editFigures") : t("cards.cycle.enterFigures")}</Button>}
          </div>
        </div>
        {c && c.total > 0 && (
          <div className="grid grid-cols-2 gap-x-6 gap-y-3 text-sm">
            <Stat label={t("cards.cycle.total")} value={money(c.total)} />
            <Stat label={t("cards.cycle.minimum")} value={money(c.min)} />
            <Stat label={t("cards.cycle.paid")} value={money(c.paid)} />
            <Stat label={t("cards.cycle.left")} value={money(c.left)} strong />
          </div>
        )}
        {c && c.status === "carried" && <p className="text-sm text-amber-800 dark:text-amber-300">{t("cards.cycle.carriedHint")}</p>}
        <div className="text-muted-foreground grid gap-1 border-t pt-3 text-sm">
          <p>{t("cards.cycle.unbilled", { amount: "", date: when(cy.next_statement) })}<span className="text-foreground font-medium tabular-nums">{money(cy.unbilled)}</span></p>
          <p className="flex items-center gap-1.5"><Sparkles className="size-3.5 shrink-0" aria-hidden />{t("cards.cycle.buyToday", { count: cy.interest_free_days, date: when(cy.buy_today_due) })}</p>
        </div>
        {cy.history.length > 0 && (
          <details className="group">
            <summary className="text-muted-foreground hover:text-foreground cursor-pointer text-sm">{t("cards.cycle.history", { count: cy.history.length })}</summary>
            <ul className="mt-2 divide-y text-sm">
              {cy.history.map((h) => (
                <li key={h.statement_date} className="flex items-center gap-3 py-1.5">
                  <span className="w-24 shrink-0">{when(h.statement_date, { day: "numeric", month: "short", year: "2-digit" })}</span>
                  <span className="flex-1 tabular-nums">{money(h.total)}</span>
                  <StatusChip status={h.status} />
                </li>
              ))}
            </ul>
          </details>
        )}
      </CardContent>
      {entering && c && <EnterStatement card={card} cycle={c} onClose={() => { setEntering(false); load(); }} />}
    </Card>
  );
}

function EnterStatement({ card, cycle, onClose }: { card: WalletCard; cycle: CardCycle; onClose: () => void }) {
  const { t } = useTranslation();
  const [total, setTotal] = useState(String(cycle.total));
  const [min, setMin] = useState(String(cycle.min));
  const [error, setError] = useState("");
  const save = async (e: FormEvent) => {
    e.preventDefault();
    try { await api.cardExtras.enterStatement(card.id, { statement_date: cycle.statement_date, total, min }); onClose(); } catch (err) { setError(errorText(err)); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader><DialogTitle>{t("cards.cycle.enterTitle", { date: formatDate(cycle.statement_date, { day: "numeric", month: "long" }) })}</DialogTitle><DialogDescription>{t("cards.cycle.enterDesc")}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="grid gap-3">
          <div className="grid gap-1.5"><Label htmlFor="st-total">{t("cards.cycle.total")} ({card.currency})</Label><Input id="st-total" type="number" min="0" step="0.01" value={total} onChange={(e) => setTotal(e.target.value)} required /></div>
          <div className="grid gap-1.5"><Label htmlFor="st-min">{t("cards.cycle.minimum")} ({card.currency})</Label><Input id="st-min" type="number" min="0" step="0.01" value={min} onChange={(e) => setMin(e.target.value)} /></div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            {cycle.entered && <Button type="button" variant="ghost" className="me-auto" onClick={() => api.cardExtras.forgetStatement(card.id, cycle.statement_date).then(onClose)}>{t("cards.cycle.useEstimate")}</Button>}
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button>{t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function Bar({ value, done }: { value: number; done?: boolean }) {
  return <div className="bg-muted h-1.5 overflow-hidden rounded-full"><div className={cn("h-full rounded-full", done ? "bg-emerald-500" : "bg-primary")} style={{ width: `${Math.min(100, Math.max(0, value * 100))}%` }} /></div>;
}

/** Rewards, milestones and perks, with how far along each is. */
export function BenefitsCard({ card, money }: { card: WalletCard; money: Money }) {
  const { t } = useTranslation();
  const [list, setList] = useState<CardBenefit[] | null>(null);
  const [editing, setEditing] = useState<CardBenefit | "new" | null>(null);
  const load = () => api.cardExtras.benefits(card.id).then(setList).catch(() => setList([]));
  useEffect(() => { load(); }, [card.id]);
  const nextFee = (() => {
    if (!card.annual_fee) return null;
    const [m, d] = (card.card_year_start ?? "01-01").split("-").map(Number);
    const now = new Date(); let y = now.getFullYear();
    if (new Date(y, m - 1, d) <= now) y += 1;
    return `${y}-${String(m).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
  })();
  return (
    <Card>
      <CardHeader className="flex-row items-start justify-between gap-2">
        <div className="space-y-1.5"><CardTitle className="flex items-center gap-2"><Trophy className="size-4" aria-hidden />{t("cards.benefits.title")}</CardTitle>
          <CardDescription>{nextFee ? t("cards.benefits.fee", { amount: formatMoney(card.annual_fee!, card.currency), date: formatDate(nextFee) }) : t("cards.benefits.desc")}</CardDescription></div>
        <Button variant="outline" size="sm" onClick={() => setEditing("new")}><Plus />{t("cards.benefits.add")}</Button>
      </CardHeader>
      <CardContent>
        {list && list.length === 0 && <p className="text-muted-foreground text-sm">{t("cards.benefits.empty")}</p>}
        <ul className="divide-y">
          {list?.map((b) => {
            const p = b.progress;
            return (
              <li key={b.id} className="space-y-1.5 py-2.5">
                <div className="flex items-center gap-2">
                  <button className="min-w-0 flex-1 truncate text-left text-sm font-medium hover:underline" onClick={() => setEditing(b)}>{b.title}</button>
                  {b.kind === "perk" && (
                    <div className="flex items-center gap-1">
                      <Button size="icon" variant="ghost" className="size-7" aria-label={t("cards.benefits.unuse", { name: b.title })} disabled={!p.used} onClick={() => api.cardExtras.usePerk(b.id, -1).then(load)}><Minus /></Button>
                      <span className="w-14 text-center text-sm tabular-nums">{p.used}{p.allowance ? ` / ${p.allowance}` : ""}</span>
                      <Button size="icon" variant="ghost" className="size-7" aria-label={t("cards.benefits.use", { name: b.title })} onClick={() => api.cardExtras.usePerk(b.id, 1).then(load)}><Plus /></Button>
                    </div>
                  )}
                </div>
                {b.kind === "reward" && (
                  <>
                    <p className="text-muted-foreground text-xs">{[t("cards.benefits.rateOn", { rate: b.rate, what: b.category_name ?? b.merchant ?? t("cards.benefits.everything") }), b.cap != null && t("cards.benefits.cap", { amount: formatMoney(b.cap, card.currency) })].filter(Boolean).join(" · ")}</p>
                    <p className="text-sm">{t("cards.benefits.earned", { amount: "" })}<span className="font-medium tabular-nums">{money(p.earned)}</span>{p.capped && <span className="text-muted-foreground"> · {t("cards.benefits.capReached")}</span>}</p>
                    {b.cap != null && <Bar value={(p.earned ?? 0) / (b.cap || 1)} done={p.capped} />}
                  </>
                )}
                {b.kind === "milestone" && (
                  <>
                    <p className="text-sm">{p.reached ? t("cards.benefits.reached") : t("cards.benefits.toGo", { left: formatMoney(p.left ?? 0, card.currency), target: formatMoney(p.target ?? 0, card.currency), date: formatDate(p.since) })}</p>
                    <Bar value={(p.spent ?? 0) / (p.target || 1)} done={p.reached} />
                  </>
                )}
                {b.kind === "perk" && p.allowance != null && <Bar value={(p.used ?? 0) / (p.allowance || 1)} />}
                {b.notes && <p className="text-muted-foreground text-xs">{b.notes}</p>}
              </li>
            );
          })}
        </ul>
      </CardContent>
      {editing && <BenefitDialog card={card} benefit={editing === "new" ? null : editing} onClose={() => { setEditing(null); load(); }} />}
    </Card>
  );
}

function BenefitDialog({ card, benefit, onClose }: { card: WalletCard; benefit: CardBenefit | null; onClose: () => void }) {
  const { t } = useTranslation();
  const [cats, setCats] = useState<Category[]>([]);
  const [v, setV] = useState<BenefitInput>(() => benefit ? {
    kind: benefit.kind, title: benefit.title, category_id: benefit.category_id, merchant: benefit.merchant, rate: benefit.rate ?? "",
    cap: benefit.cap != null ? String(benefit.cap) : "", target: benefit.target != null ? String(benefit.target) : "", allowance: benefit.allowance ?? "", notes: benefit.notes ?? "",
  } : { kind: "reward", title: "", rate: "", cap: "", category_id: "", merchant: "" });
  const [error, setError] = useState("");
  const set = (p: Partial<BenefitInput>) => setV((x) => ({ ...x, ...p }));
  useEffect(() => { api.categories.getAll().then((c) => setCats(c.filter((x) => x.type === "expense"))); }, []);
  const save = async (e: FormEvent) => {
    e.preventDefault();
    try { if (benefit) await api.cardExtras.updateBenefit(benefit.id, v); else await api.cardExtras.addBenefit(card.id, v); onClose(); } catch (err) { setError(errorText(err)); }
  };
  const field = (id: string, label: string, input: ReactNode) => <div className="grid gap-1.5"><Label htmlFor={id}>{label}</Label>{input}</div>;
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader><DialogTitle>{benefit ? t("cards.benefits.editTitle") : t("cards.benefits.addTitle")}</DialogTitle><DialogDescription>{t(`cards.benefits.kindHint.${v.kind}`)}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="grid gap-3">
          <Segmented value={v.kind} onChange={(k: BenefitKind) => set({ kind: k })} label={t("cards.benefits.kindLabel")}
            options={(["reward", "milestone", "perk"] as BenefitKind[]).map((k) => ({ value: k, label: t(`cards.benefits.kinds.${k}`) }))} />
          {field("ben-title", t("cards.benefits.name"), <Input id="ben-title" autoFocus maxLength={80} value={v.title} onChange={(e) => set({ title: e.target.value })} placeholder={t(`cards.benefits.placeholder.${v.kind}`)} required />)}
          {v.kind === "reward" && (
            <>
              <div className="grid grid-cols-2 gap-3">
                {field("ben-rate", t("cards.benefits.rate"), <Input id="ben-rate" type="number" min="0.1" max="100" step="0.1" value={String(v.rate ?? "")} onChange={(e) => set({ rate: e.target.value })} required />)}
                {field("ben-cap", t("cards.benefits.capLabel", { currency: card.currency }), <Input id="ben-cap" type="number" min="0" step="1" value={v.cap ?? ""} onChange={(e) => set({ cap: e.target.value })} />)}
              </div>
              {field("ben-cat", t("cards.benefits.on"), (
                <NativeSelect id="ben-cat" value={v.category_id ?? ""} onChange={(e) => set({ category_id: e.target.value || null })}>
                  <option value="">{t("cards.benefits.everythingOrPlace")}</option>
                  {cats.map((c) => <option key={c.id} value={c.id}>{c.parent_id ? "  · " : ""}{c.name}</option>)}
                </NativeSelect>
              ))}
              {!v.category_id && field("ben-place", t("cards.benefits.place"), <Input id="ben-place" maxLength={60} value={v.merchant ?? ""} onChange={(e) => set({ merchant: e.target.value })} placeholder={t("cards.benefits.placeHint")} />)}
            </>
          )}
          {v.kind === "milestone" && field("ben-target", t("cards.benefits.targetLabel", { currency: card.currency }), <Input id="ben-target" type="number" min="1" step="1" value={v.target ?? ""} onChange={(e) => set({ target: e.target.value })} required />)}
          {v.kind === "perk" && field("ben-allow", t("cards.benefits.allowance"), <Input id="ben-allow" type="number" min="1" max="1000" step="1" value={String(v.allowance ?? "")} onChange={(e) => set({ allowance: e.target.value })} />)}
          {field("ben-notes", t("cards.benefits.notes"), <Input id="ben-notes" maxLength={500} value={v.notes ?? ""} onChange={(e) => set({ notes: e.target.value })} />)}
          {v.kind !== "reward" && <p className="text-muted-foreground text-xs">{t("cards.benefits.yearHint")}</p>}
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            {benefit && <Button type="button" variant="ghost" className="text-destructive me-auto" onClick={() => api.cardExtras.deleteBenefit(benefit.id).then(onClose)}><Trash2 />{t("common.delete")}</Button>}
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button>{t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Which credit card to use for a purchase today: most back, then most days to pay without interest. */
export function BestCardFinder() {
  const { t } = useTranslation();
  const [amount, setAmount] = useState("");
  const [cat, setCat] = useState("");
  const [place, setPlace] = useState("");
  const [cats, setCats] = useState<Category[]>([]);
  const [picks, setPicks] = useState<BestPick[] | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => { api.categories.getAll().then((c) => setCats(c.filter((x) => x.type === "expense"))); }, []);
  useEffect(() => {
    const id = setTimeout(() => { setBusy(true); api.cardExtras.best(amount, cat, place).then(setPicks).catch(() => setPicks([])).finally(() => setBusy(false)); }, 250);
    return () => clearTimeout(id);
  }, [amount, cat, place]);
  if (picks && picks.length === 0) return null;
  return (
    <Card>
      <CardHeader><CardTitle className="flex items-center gap-2"><Sparkles className="size-4" aria-hidden />{t("cards.best.title")}</CardTitle><CardDescription>{t("cards.best.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-3">
        <div className="grid gap-2 sm:grid-cols-[8rem_1fr_1fr]">
          <Input aria-label={t("cards.best.amount")} placeholder={t("cards.best.amount")} type="number" min="0" step="1" value={amount} onChange={(e) => setAmount(e.target.value)} />
          <NativeSelect aria-label={t("cards.best.category")} value={cat} onChange={(e) => setCat(e.target.value)}>
            <option value="">{t("cards.best.anyCategory")}</option>
            {cats.map((c) => <option key={c.id} value={c.id}>{c.parent_id ? "  · " : ""}{c.name}</option>)}
          </NativeSelect>
          <Input aria-label={t("cards.best.place")} placeholder={t("cards.best.place")} maxLength={60} value={place} onChange={(e) => setPlace(e.target.value)} />
        </div>
        <ol className="space-y-1.5" aria-busy={busy}>
          {picks?.slice(0, 4).map((p, i) => (
            <li key={p.card_id} className={cn("flex flex-wrap items-center gap-x-3 gap-y-0.5 rounded-lg border px-3 py-2", i === 0 && p.fits && "border-primary/60 bg-primary/5", !p.fits && "opacity-60")}>
              <span className="font-medium">{p.name}</span>
              <span className="text-muted-foreground text-sm">
                {[p.earns != null ? t("cards.best.earns", { amount: formatMoney(p.earns, p.currency) }) : p.rate ? t("cards.best.rate", { rate: p.rate }) : t("cards.best.noReward"),
                  p.reward].filter(Boolean).join(" · ")}
              </span>
              <span className="text-muted-foreground ms-auto text-xs">
                {!p.fits ? t("cards.best.noLimit") : p.pay_by ? t("cards.best.payBy", { date: formatDate(p.pay_by, { day: "numeric", month: "short" }), count: p.interest_free_days ?? 0 }) : t("cards.best.noDates")}
              </span>
            </li>
          ))}
        </ol>
        {busy && <Loader2 className="text-muted-foreground size-4 animate-spin" aria-hidden />}
      </CardContent>
    </Card>
  );
}
