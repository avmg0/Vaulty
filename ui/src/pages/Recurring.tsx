import { useEffect, useMemo, useState, type FormEvent } from "react";
import { ArrowDownRight, ArrowLeftRight, ArrowUpRight, Check, Loader2, MoreHorizontal, Pause, Pencil, Play, Plus, Repeat, SkipForward, Trash2, Zap } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { api, DATA_RESTORED, errorText, type Account, type Category, type Recurring, type RecurringInput } from "@/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Segmented } from "@/components/Segmented";
import { SkeletonRows } from "@/components/Skeleton";
import { AnimatedItem, AnimatedList } from "@/components/AnimatedList";
import { useTheme } from "@/hooks/theme";
import { ordinal } from "@/lib/cards";
import { cn, formatDate, formatMoney } from "@/lib/utils";

const localToday = () => { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; };

/** "Every month on the 5th", "Every 2 weeks on Monday", "Every year on 5 Oct". */
export function scheduleText(t: TFunction, r: Pick<Recurring, "frequency" | "every" | "start_date">) {
  const start = new Date(`${r.start_date}T00:00:00`);
  const on = r.frequency === "monthly" ? ordinal(start.getDate())
    : r.frequency === "weekly" ? formatDate(start, { weekday: "long" })
    : r.frequency === "yearly" ? formatDate(start, { day: "numeric", month: "short" }) : "";
  return t(`recurring.every.${r.frequency}`, { count: r.every, on });
}

/** "in 3 days", "today", "5 days overdue". */
export function dueText(t: TFunction, days: number | null) {
  if (days == null) return t("recurring.noMore");
  if (days === 0) return t("recurring.today");
  if (days === 1) return t("recurring.tomorrow");
  return days > 0 ? t("recurring.inDays", { count: days }) : t("recurring.overdueBy", { count: -days });
}

const ORDER = ["overdue", "due", "soon", "upcoming", "paused", "ended"] as const;

export function RecurringPage() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [items, setItems] = useState<Recurring[] | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [editing, setEditing] = useState<Recurring | "new" | null>(null);
  const [paying, setPaying] = useState<Recurring | null>(null);
  const [error, setError] = useState("");

  const load = () => api.recurring.list().then(setItems).catch((e) => setError(errorText(e)));
  useEffect(() => {
    load();
    api.accounts.getAll().then(setAccounts);
    api.categories.getAll().then(setCategories);
    window.addEventListener(DATA_RESTORED, load);
    return () => window.removeEventListener(DATA_RESTORED, load);
  }, []);

  const groups = useMemo(() => {
    const by: Record<string, Recurring[]> = {};
    for (const r of items ?? []) {
      const g = r.status === "due" || r.status === "soon" ? "soon" : r.status === "ended" ? "paused" : r.status;
      (by[g] ??= []).push(r);
    }
    return ORDER.filter((g) => by[g]?.length).map((g) => ({ g, list: by[g] }));
  }, [items]);

  // The next 30 days, money out and in (transfers aren't either).
  const month = useMemo(() => {
    let out = 0, inn = 0;
    for (const r of items ?? []) {
      if (r.status === "paused" || r.days_until == null || r.days_until > 30) continue;
      const times = r.frequency === "daily" ? Math.floor(30 / r.every) || 1 : r.frequency === "weekly" ? Math.floor(30 / (7 * r.every)) || 1 : 1;
      if (r.type === "expense") out += r.amount * times; else if (r.type === "income") inn += r.amount * times;
    }
    return { out, inn };
  }, [items]);
  const cur = items?.[0]?.currency ?? accounts[0]?.currency ?? "INR";

  const act = async (fn: () => Promise<unknown>) => { setError(""); try { await fn(); load(); } catch (e) { setError(errorText(e)); } };

  return (
    <div className="max-w-4xl space-y-4">
      <div className="flex flex-wrap items-end gap-4">
        <div className="flex gap-8">
          <div><div className="text-muted-foreground text-sm">{t("recurring.next30Out")}</div><div className="text-2xl font-semibold tabular-nums">{formatMoney(month.out, cur, hideBalances)}</div></div>
          <div><div className="text-muted-foreground text-sm">{t("recurring.next30In")}</div><div className="text-success text-2xl font-semibold tabular-nums">{formatMoney(month.inn, cur, hideBalances)}</div></div>
        </div>
        <Button className="ml-auto" onClick={() => setEditing("new")} disabled={!accounts.length} title={accounts.length ? "" : t("tx.needAccount")}><Plus />{t("recurring.add")}</Button>
      </div>
      {error && <p className="text-destructive text-sm">{error}</p>}
      {!items && <Card><CardContent><SkeletonRows rows={4} /></CardContent></Card>}
      {items?.length === 0 && (
        <Card><CardContent className="flex flex-col items-center gap-3 py-10 text-center">
          <div className="bg-secondary grid size-12 place-items-center rounded-xl"><Repeat className="size-5" /></div>
          <div className="font-semibold">{t("recurring.emptyTitle")}</div>
          <p className="text-muted-foreground max-w-sm text-sm">{t("recurring.emptyBody")}</p>
        </CardContent></Card>
      )}
      {groups.map(({ g, list }) => (
        <Card key={g} className="py-2">
          <CardHeader className="pt-3"><CardTitle className={cn("text-sm", g === "overdue" && "text-destructive")}>{t(`recurring.group.${g}`)}</CardTitle></CardHeader>
          <CardContent className="divide-y">
            <AnimatedList>
              {list.map((r) => (
                <AnimatedItem key={r.id}>
                  <Row r={r} hidden={hideBalances} onPay={() => setPaying(r)} onEdit={() => setEditing(r)}
                    onSkip={() => act(() => api.recurring.skip(r.id))} onToggle={() => act(() => api.recurring.setActive(r.id, r.is_active !== 1))}
                    onDelete={() => act(() => api.recurring.delete(r.id))} />
                </AnimatedItem>
              ))}
            </AnimatedList>
          </CardContent>
        </Card>
      ))}
      {editing && <RecurringDialog initial={editing === "new" ? null : editing} accounts={accounts} categories={categories}
        onClose={() => setEditing(null)} onSaved={() => { setEditing(null); load(); }} />}
      {paying && <PayDialog r={paying} onClose={() => setPaying(null)} onPaid={() => { setPaying(null); load(); }} />}
    </div>
  );
}

function Row({ r, hidden, onPay, onEdit, onSkip, onToggle, onDelete }: {
  r: Recurring; hidden: boolean; onPay: () => void; onEdit: () => void; onSkip: () => void; onToggle: () => void; onDelete: () => void;
}) {
  const { t } = useTranslation();
  const [menu, setMenu] = useState(false);
  const Icon = r.type === "income" ? ArrowUpRight : r.type === "transfer" ? ArrowLeftRight : ArrowDownRight;
  const urgent = r.status === "overdue" || r.status === "due" || r.status === "soon";
  return (
    <div className="group flex items-center gap-3 py-3">
      {/* Paused rows fade only the icon: faded text would be too hard to read. "Paused" is said in words. */}
      <div className={cn("grid size-9 shrink-0 place-items-center rounded-lg text-base", r.type === "income" ? "bg-success/15 text-success" : "bg-secondary", r.status === "paused" && "opacity-40 grayscale")}>
        {r.category_icon ?? <Icon className="size-4" />}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <button type="button" className="truncate text-sm font-medium hover:underline" onClick={onEdit}>{r.name}</button>
          {r.auto_pay === 1 && <Badge variant="secondary" className="gap-0.5 px-1.5 text-[10px]" title={t("recurring.autoHint")}><Zap className="size-2.5" />{t("recurring.auto")}</Badge>}
        </div>
        <div className="text-muted-foreground truncate text-xs">
          {[scheduleText(t, r), r.type === "transfer" ? `${r.account_name} → ${r.to_account_name}` : r.account_name].filter(Boolean).join(" · ")}
        </div>
      </div>
      <div className="text-end">
        <div className={cn("text-sm font-medium tabular-nums", r.type === "income" && "text-success")}>
          {r.type === "income" ? "+" : r.type === "expense" ? "−" : ""}{formatMoney(r.amount, r.currency, hidden)}
        </div>
        <div className={cn("text-xs", r.status === "overdue" ? "text-destructive" : "text-muted-foreground")}>
          {r.status === "paused" ? t("recurring.paused") : r.next_due_date ? `${formatDate(r.next_due_date, { day: "numeric", month: "short" })} · ${dueText(t, r.days_until)}` : t("recurring.noMore")}
        </div>
      </div>
      {r.next_due_date && r.is_active === 1
        ? <Button size="sm" variant={urgent ? "default" : "outline"} onClick={onPay}><Check />{t("recurring.markPaid")}</Button>
        : <span className="w-[6.6rem]" />}
      <div className="relative">
        <Button variant="ghost" size="icon" className="size-8" onClick={() => setMenu((m) => !m)} aria-label={t("recurring.more")}><MoreHorizontal /></Button>
        {menu && (
          <div className="bg-popover absolute right-0 z-20 mt-1 w-48 rounded-md border p-1 shadow-md" onMouseLeave={() => setMenu(false)}>
            {r.next_due_date && r.is_active === 1 && <Item icon={SkipForward} onClick={() => { setMenu(false); onSkip(); }}>{t("recurring.skip")}</Item>}
            <Item icon={r.is_active ? Pause : Play} onClick={() => { setMenu(false); onToggle(); }}>{r.is_active ? t("recurring.pause") : t("recurring.resume")}</Item>
            <Item icon={Pencil} onClick={() => { setMenu(false); onEdit(); }}>{t("tx.edit")}</Item>
            <Item icon={Trash2} destructive onClick={() => { setMenu(false); onDelete(); }}>{t("budgets.delete")}</Item>
          </div>
        )}
      </div>
    </div>
  );
}

function Item({ icon: Icon, children, onClick, destructive }: { icon: typeof Pause; children: React.ReactNode; onClick: () => void; destructive?: boolean }) {
  return <button onClick={onClick} className={cn("hover:bg-accent flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm", destructive && "text-destructive")}><Icon className="size-4" />{children}</button>;
}

/** Confirms a payment, with this occurrence's date and amount adjustable. */
function PayDialog({ r, onClose, onPaid }: { r: Recurring; onClose: () => void; onPaid: () => void }) {
  const { t } = useTranslation();
  const [date, setDate] = useState(r.next_due_date ?? localToday());
  const [amount, setAmount] = useState(String(r.amount));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const submit = async (e: FormEvent) => {
    e.preventDefault(); setBusy(true); setError("");
    try { await api.recurring.pay(r.id, date, amount !== String(r.amount) ? amount : undefined); onPaid(); }
    catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader><DialogTitle>{t("recurring.payTitle", { name: r.name })}</DialogTitle>
          <DialogDescription>{t("recurring.payDesc", { date: formatDate(r.next_due_date ?? date) })}</DialogDescription></DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-2"><Label htmlFor="pay-amount">{t("tx.amount")}</Label><Input id="pay-amount" type="number" step="0.01" min="0" value={amount} onChange={(e) => setAmount(e.target.value)} required /></div>
            <div className="grid gap-2"><Label htmlFor="pay-date">{t("recurring.paidOn")}</Label><Input id="pay-date" type="date" value={date} onChange={(e) => setDate(e.target.value)} required /></div>
          </div>
          {amount !== String(r.amount) && <p className="text-muted-foreground text-xs">{t("recurring.thisTimeOnly")}</p>}
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button type="button" variant="ghost" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <Check />}{t("recurring.markPaid")}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function RecurringDialog({ initial, accounts, categories, onClose, onSaved }: {
  initial: Recurring | null; accounts: Account[]; categories: Category[]; onClose: () => void; onSaved: () => void;
}) {
  const { t } = useTranslation();
  const [f, setF] = useState<RecurringInput>(() => initial ? {
    name: initial.name, type: initial.type, amount: String(initial.amount), account_id: initial.account_id ?? accounts[0]?.id ?? "",
    to_account_id: initial.to_account_id ?? undefined, category_id: initial.category_id ?? undefined, notes: initial.notes ?? undefined,
    frequency: initial.frequency, every: initial.every, start_date: initial.start_date, end_date: initial.end_date ?? undefined,
    reminder_days: initial.reminder_days, auto_pay: initial.auto_pay === 1,
  } : { name: "", type: "expense", amount: "", account_id: accounts[0]?.id ?? "", frequency: "monthly", every: 1, start_date: localToday(), reminder_days: 3, auto_pay: false });
  const set = (p: Partial<RecurringInput>) => setF((x) => ({ ...x, ...p }));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const cats = categories.filter((c) => c.type === f.type);
  const submit = async (e: FormEvent) => {
    e.preventDefault(); setBusy(true); setError("");
    const body = { ...f, to_account_id: f.type === "transfer" ? f.to_account_id : undefined, category_id: f.type === "transfer" ? undefined : f.category_id || undefined, end_date: f.end_date || undefined };
    try { if (initial) await api.recurring.update(initial.id, body); else await api.recurring.create(body); onSaved(); }
    catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[92vh] overflow-y-auto">
        <DialogHeader><DialogTitle>{initial ? t("recurring.editTitle") : t("recurring.newTitle")}</DialogTitle>
          <DialogDescription>{t("recurring.dialogDesc")}</DialogDescription></DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <Segmented value={f.type} onChange={(type) => set({ type, category_id: undefined })} className="flex w-full" itemClassName="py-1.5" label={t("tx.type")}
            options={[{ value: "expense" as const, label: t("tx.expense") }, { value: "income" as const, label: t("tx.income") },
              ...(accounts.length > 1 ? [{ value: "transfer" as const, label: t("tx.transfer") }] : [])]} />
          <div className="grid grid-cols-[1fr_9rem] gap-3">
            <div className="grid gap-2"><Label htmlFor="r-name">{t("accounts.name")}</Label><Input id="r-name" autoFocus value={f.name} onChange={(e) => set({ name: e.target.value })} placeholder={t("recurring.namePh")} required maxLength={80} /></div>
            <div className="grid gap-2"><Label htmlFor="r-amount">{t("tx.amount")}</Label><Input id="r-amount" type="number" step="0.01" min="0" value={f.amount} onChange={(e) => set({ amount: e.target.value })} required /></div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-2"><Label htmlFor="r-acc">{f.type === "transfer" ? t("tx.from") : t("tx.account")}</Label>
              <NativeSelect id="r-acc" value={f.account_id} onChange={(e) => set({ account_id: e.target.value })}>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></div>
            {f.type === "transfer" ? (
              <div className="grid gap-2"><Label htmlFor="r-to">{t("tx.to")}</Label>
                <NativeSelect id="r-to" value={f.to_account_id ?? ""} onChange={(e) => set({ to_account_id: e.target.value })} required>
                  <option value="" disabled>{t("tx.chooseAccount")}</option>
                  {accounts.filter((a) => a.id !== f.account_id).map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></div>
            ) : (
              <div className="grid gap-2"><Label htmlFor="r-cat">{t("tx.category")}</Label>
                <NativeSelect id="r-cat" value={f.category_id ?? ""} onChange={(e) => set({ category_id: e.target.value })}>
                  <option value="">{t("dashboard.uncategorised")}</option>{cats.map((c) => <option key={c.id} value={c.id}>{c.icon} {c.name}</option>)}</NativeSelect></div>
            )}
          </div>
          <div className="grid gap-2">
            <Label>{t("recurring.repeats")}</Label>
            <div className="flex items-center gap-2">
              <span className="text-muted-foreground text-sm">{t("recurring.everyLabel")}</span>
              <Input aria-label={t("recurring.everyLabel")} type="number" min={1} max={366} className="w-20" value={f.every} onChange={(e) => set({ every: Math.max(1, Number(e.target.value) || 1) })} />
              <NativeSelect aria-label={t("recurring.repeats")} className="flex-1" value={f.frequency} onChange={(e) => set({ frequency: e.target.value as RecurringInput["frequency"] })}>
                {(["daily", "weekly", "monthly", "yearly"] as const).map((x) => <option key={x} value={x}>{t(`recurring.unit.${x}`, { count: f.every })}</option>)}
              </NativeSelect>
            </div>
            <p className="text-muted-foreground text-xs">{scheduleText(t, f)}</p>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-2"><Label htmlFor="r-start">{t("recurring.firstDate")}</Label><Input id="r-start" type="date" value={f.start_date} onChange={(e) => set({ start_date: e.target.value })} required /></div>
            <div className="grid gap-2"><Label htmlFor="r-end">{t("recurring.endDate")}</Label><Input id="r-end" type="date" value={f.end_date ?? ""} onChange={(e) => set({ end_date: e.target.value })} /></div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-2"><Label htmlFor="r-remind">{t("recurring.remind")}</Label>
              <NativeSelect id="r-remind" value={f.reminder_days} onChange={(e) => set({ reminder_days: Number(e.target.value) })}>
                {[0, 1, 2, 3, 5, 7, 14].map((d) => <option key={d} value={d}>{d === 0 ? t("recurring.onTheDay") : t("recurring.daysBefore", { count: d })}</option>)}</NativeSelect></div>
          </div>
          <label className="flex items-start justify-between gap-4 rounded-lg border p-3">
            <span className="grid gap-0.5"><span className="flex items-center gap-1.5 text-sm font-medium"><Zap className="size-3.5" />{t("recurring.autoPay")}</span>
              <span className="text-muted-foreground text-xs">{t("recurring.autoPayDesc")}</span></span>
            <Switch checked={!!f.auto_pay} onCheckedChange={(auto_pay) => set({ auto_pay })} />
          </label>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button type="button" variant="ghost" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}{initial ? t("cards.saveChanges") : t("recurring.add")}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
