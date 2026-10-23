import { useCallback, useEffect, useState, type FormEvent } from "react";
import { Calculator, CheckCircle2, Loader2, Pencil, Plus, RotateCcw, Target, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type Account, type Goal, type GoalInput, type GoalStatus, type GoalTotal, type Holding } from "@/api";
import { Segmented } from "@/components/Segmented";
import { Calculators } from "@/components/goals/Calculators";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useLocale } from "@/hooks/locale";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate, formatMoney, formatNumber } from "@/lib/utils";

const WHOLE: Intl.NumberFormatOptions = { maximumFractionDigits: 0, minimumFractionDigits: 0 };
const STATUS_TONE: Record<GoalStatus, string> = {
  done: "bg-emerald-600 text-white", reached: "bg-emerald-600 text-white", on_track: "bg-emerald-100 text-emerald-900 dark:bg-emerald-950 dark:text-emerald-200",
  behind: "bg-amber-100 text-amber-900 dark:bg-amber-950 dark:text-amber-200", overdue: "bg-red-100 text-red-900 dark:bg-red-950 dark:text-red-200", no_date: "bg-muted text-muted-foreground",
};
/** Starting points for a first goal. */
const IDEAS: { key: string; icon: string; years: number; inflation: number; rate: number }[] = [
  { key: "emergency", icon: "🛟", years: 1, inflation: 0, rate: 4 },
  { key: "house", icon: "🏠", years: 5, inflation: 6, rate: 8 },
  { key: "education", icon: "🎓", years: 12, inflation: 8, rate: 11 },
  { key: "car", icon: "🚗", years: 3, inflation: 5, rate: 7 },
  { key: "trip", icon: "✈️", years: 1, inflation: 0, rate: 5 },
  { key: "wedding", icon: "💍", years: 4, inflation: 6, rate: 8 },
];

export function GoalsPage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<"goals" | "calculators">("goals");
  return (
    <div className="mx-auto max-w-6xl space-y-6">
      <Segmented role="tablist" label={t("goals.tabs")} value={tab} onChange={setTab}
        options={[{ value: "goals", label: <><Target className="size-4" />{t("goals.tabGoals")}</> }, { value: "calculators", label: <><Calculator className="size-4" />{t("goals.tabCalculators")}</> }]} />
      {tab === "goals" ? <GoalsList /> : <Calculators />}
    </div>
  );
}

function GoalsList() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [goals, setGoals] = useState<Goal[] | null>(null);
  const [totals, setTotals] = useState<GoalTotal[]>([]);
  const [editing, setEditing] = useState<Goal | Partial<GoalInput> | null>(null);
  const [error, setError] = useState("");
  const reload = useCallback(() => {
    Promise.all([api.goals.list(), api.goals.summary()]).then(([g, s]) => { setGoals(g); setTotals(s); setError(""); }).catch((e) => setError(errorText(e)));
  }, []);
  useEffect(() => { reload(); window.addEventListener(DATA_RESTORED, reload); return () => window.removeEventListener(DATA_RESTORED, reload); }, [reload]);

  if (!goals) return error ? <p role="alert" className="text-destructive text-sm">{error}</p> : null;
  const open = goals.filter((g) => !g.done_on);
  const done = goals.filter((g) => g.done_on);
  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div className="space-y-1">
          {totals.map((s) => (
            <p key={s.currency} className="text-sm">
              <span className="text-2xl font-semibold tracking-tight tabular-nums">{formatMoney(s.saved, s.currency, hideBalances, WHOLE)}</span>
              <span className="text-muted-foreground"> {t("goals.ofTotal", { amount: formatMoney(s.target, s.currency, hideBalances, WHOLE), count: s.goals })}</span>
              {s.monthly > 0 && <span className="text-muted-foreground"> · {t("goals.plannedMonthly", { amount: formatMoney(s.monthly, s.currency, hideBalances, WHOLE) })}</span>}
            </p>
          ))}
        </div>
        {goals.length > 0 && <Button onClick={() => setEditing({})}><Plus />{t("goals.add")}</Button>}
      </div>
      {error && <p role="alert" className="text-destructive text-sm">{error}</p>}

      {goals.length === 0 ? (
        <Card><CardContent className="space-y-5 py-10 text-center">
          <div className="bg-secondary mx-auto grid size-12 place-items-center rounded-xl"><Target className="size-5" /></div>
          <div className="space-y-1"><p className="font-semibold">{t("goals.emptyTitle")}</p><p className="text-muted-foreground mx-auto max-w-md text-sm">{t("goals.emptyBody")}</p></div>
          <div className="flex flex-wrap justify-center gap-2">
            {IDEAS.map((i) => (
              <button key={i.key} type="button" className="hover:bg-accent rounded-full border px-3 py-1.5 text-sm"
                onClick={() => setEditing({ name: t(`goals.ideas.${i.key}`), icon: i.icon, inflation: i.inflation, return_rate: i.rate, target_date: yearsFromNow(i.years) })}>
                <span aria-hidden className="me-1.5">{i.icon}</span>{t(`goals.ideas.${i.key}`)}
              </button>
            ))}
          </div>
          <Button variant="outline" onClick={() => setEditing({})}><Plus />{t("goals.addOwn")}</Button>
        </CardContent></Card>
      ) : (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {open.map((g) => <GoalCard key={g.id} g={g} onEdit={() => setEditing(g)} onChange={reload} />)}
        </div>
      )}
      {done.length > 0 && (
        <section className="space-y-3" aria-labelledby="done-h">
          <h2 id="done-h" className="text-muted-foreground text-sm font-medium">{t("goals.doneTitle", { count: done.length })}</h2>
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">{done.map((g) => <GoalCard key={g.id} g={g} onEdit={() => setEditing(g)} onChange={reload} />)}</div>
        </section>
      )}
      {editing && <GoalDialog start={editing} onClose={(changed) => { setEditing(null); if (changed) reload(); }} />}
    </div>
  );
}

function yearsFromNow(n: number) { const d = new Date(); d.setFullYear(d.getFullYear() + n); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; }

function GoalCard({ g, onEdit, onChange }: { g: Goal; onEdit: () => void; onChange: () => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const money = (n: number) => formatMoney(n, g.currency, hideBalances, WHOLE);
  const [busy, setBusy] = useState(false);
  const act = (p: Promise<unknown>) => { setBusy(true); p.then(onChange).finally(() => setBusy(false)); };
  // How far the plan gets by the date, drawn behind what's saved.
  const reach = g.projected != null && g.target_then > 0 ? Math.min(1, g.projected / g.target_then) : g.progress;
  const finished = g.status === "done";
  return (
    <Card className={cn("gap-0 py-5", finished && "opacity-75")}>
      <CardContent className="space-y-4">
        <div className="flex items-start gap-3">
          <div className="bg-secondary grid size-10 shrink-0 place-items-center rounded-lg text-lg" aria-hidden>{g.icon || "🎯"}</div>
          <div className="min-w-0 flex-1">
            <h2 className="truncate font-semibold">{g.name}</h2>
            <p className="text-muted-foreground text-xs">
              {g.target_date ? t("goals.byDate", { date: formatDate(g.target_date, { month: "short", year: "numeric" }) }) : t("goals.noDate")}
              {g.months_left != null && g.months_left > 0 && !finished && ` · ${t("goals.monthsLeft", { count: g.months_left })}`}
            </p>
          </div>
          <span className={cn("shrink-0 rounded-full px-2 py-0.5 text-xs font-medium", STATUS_TONE[g.status])}>{t(`goals.status.${g.status}`)}</span>
        </div>

        <div className="space-y-1.5">
          <div className="flex items-baseline justify-between gap-2">
            <span className="text-xl font-semibold tabular-nums">{money(g.saved)}</span>
            <span className="text-muted-foreground text-sm tabular-nums">{t("goals.ofTarget", { amount: money(g.target_then) })}</span>
          </div>
          <div className="bg-muted relative h-2.5 overflow-hidden rounded-full" role="progressbar" aria-valuemin={0} aria-valuemax={100}
            aria-valuenow={Math.round(g.progress * 100)} aria-label={t("goals.progressLabel", { name: g.name })}>
            {!finished && reach > g.progress && <div title={t("goals.projectedLegend")} className="bg-primary/15 absolute inset-y-0 start-0 rounded-full" style={{ width: `${reach * 100}%` }} />}
            <div className={cn("absolute inset-y-0 start-0 rounded-full", g.status === "done" || g.status === "reached" ? "bg-emerald-600" : "bg-primary")} style={{ width: `${g.progress * 100}%` }} />
          </div>
          <div className="text-muted-foreground flex justify-between text-xs">
            <span>{formatNumber(g.progress * 100, { maximumFractionDigits: 0 })}%</span>
            {g.inflation > 0 && g.target_then !== g.target && <span>{t("goals.inTodayMoney", { amount: money(g.target), rate: formatNumber(g.inflation) })}</span>}
          </div>
        </div>

        {!finished && g.status !== "reached" && (
          <dl className="grid grid-cols-2 gap-3 text-sm">
            {g.needed_monthly != null && <div><dt className="text-muted-foreground text-xs">{t("goals.needed")}</dt><dd className="font-medium tabular-nums">{t("goals.perMonth", { amount: money(g.needed_monthly) })}</dd></div>}
            <div><dt className="text-muted-foreground text-xs">{t("goals.planned")}</dt><dd className="tabular-nums">{g.monthly > 0 ? t("goals.perMonth", { amount: money(g.monthly) }) : "—"}</dd></div>
          </dl>
        )}
        {!finished && g.status !== "reached" && g.projected != null && g.monthly > 0 && <p className="text-muted-foreground -mt-2 text-xs">{t("goals.projected", { amount: money(g.projected), rate: formatNumber(g.return_rate) })}</p>}

        {g.links.length > 0 && (
          <ul className="text-muted-foreground space-y-0.5 text-xs">
            {g.links.map((l) => (
              <li key={(l.account_id ?? l.holding_id)!} className={cn("flex justify-between gap-3", l.over && "text-amber-700 dark:text-amber-400")}>
                <span className="truncate">{l.share < 100 ? t("goals.shareOf", { share: l.share, name: l.name }) : l.name}{l.over && ` · ${t("goals.overClaimed")}`}</span>
                <span className="tabular-nums">{money(l.value)}</span>
              </li>
            ))}
            {g.saved_by_hand > 0 && <li className="flex justify-between gap-3"><span>{t("goals.savedElsewhere")}</span><span className="tabular-nums">{money(g.saved_by_hand)}</span></li>}
          </ul>
        )}

        <div className="flex flex-wrap gap-2 border-t pt-3">
          <Button size="sm" variant="ghost" onClick={onEdit}><Pencil />{t("common.edit")}</Button>
          {finished
            ? <Button size="sm" variant="ghost" disabled={busy} onClick={() => act(api.goals.setDone(g.id, false))}><RotateCcw />{t("goals.reopen")}</Button>
            : <Button size="sm" variant="ghost" disabled={busy} onClick={() => act(api.goals.setDone(g.id, true))}><CheckCircle2 />{t("goals.markDone")}</Button>}
          <Button size="sm" variant="ghost" className="text-destructive ms-auto" disabled={busy} onClick={() => act(api.goals.delete(g.id))} aria-label={t("goals.deleteNamed", { name: g.name })}><Trash2 /></Button>
        </div>
      </CardContent>
    </Card>
  );
}

type LinkRow = { key: string; account_id?: string; holding_id?: string; name: string; on: boolean; share: string };

function GoalDialog({ start, onClose }: { start: Goal | Partial<GoalInput>; onClose: (changed: boolean) => void }) {
  const { t } = useTranslation();
  const { currency: mine } = useLocale();
  const existing = "id" in start ? start as Goal : null;
  const s = start as Partial<GoalInput> & Partial<Goal>;
  const [name, setName] = useState(s.name ?? "");
  const [icon, setIcon] = useState(s.icon ?? "");
  const [currency] = useState(s.currency ?? mine);
  const [target, setTarget] = useState(existing ? String(existing.target) : "");
  const [date, setDate] = useState(s.target_date ?? "");
  const [saved, setSaved] = useState(existing ? (existing.saved_by_hand ? String(existing.saved_by_hand) : "") : "");
  const [monthly, setMonthly] = useState(existing?.monthly ? String(existing.monthly) : "");
  const [rate, setRate] = useState(String(s.return_rate ?? 0));
  const [todays, setTodays] = useState(Number(s.inflation ?? 0) > 0);
  const [inflation, setInflation] = useState(String(Number(s.inflation ?? 0) || 6));
  const [priority, setPriority] = useState(s.priority ?? 2);
  const [notes, setNotes] = useState(s.notes ?? "");
  const [links, setLinks] = useState<LinkRow[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const amount = (v: string) => v.replace(/[^\d.]/g, "");

  useEffect(() => {
    Promise.all([api.accounts.getAll(), api.investments.list()]).then(([a, h]: [Account[], Holding[]]) => {
      const had = new Map((existing?.links ?? []).map((l) => [(l.account_id ?? l.holding_id)!, l.share]));
      setLinks([
        ...a.filter((x) => x.ledger_kind === "asset" && x.currency === currency).map((x) => ({ key: x.id, account_id: x.id, name: x.name, on: had.has(x.id), share: String(had.get(x.id) ?? 100) })),
        ...h.filter((x) => !x.closed && x.currency === currency).map((x) => ({ key: x.id, holding_id: x.id, name: x.name, on: had.has(x.id), share: String(had.get(x.id) ?? 100) })),
      ]);
    }).catch(() => setLinks([]));
  }, [currency, existing]);
  const setLink = (key: string, patch: Partial<LinkRow>) => setLinks((l) => l && l.map((x) => x.key === key ? { ...x, ...patch } : x));

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    const goal: GoalInput = {
      name, icon: icon || null, currency, target, target_date: date || null, saved, monthly, return_rate: rate || 0, inflation: todays ? inflation || 0 : 0, priority, notes: notes || null,
      ...(links ? { links: links.filter((l) => l.on).map((l) => ({ ...(l.account_id ? { account_id: l.account_id } : { holding_id: l.holding_id }), share: Math.min(100, Math.max(1, Number(l.share) || 100)) })) } : {}),
    };
    try { if (existing) await api.goals.update(existing.id, goal); else await api.goals.create(goal); onClose(true); }
    catch (err) { setError(errorText(err)); setBusy(false); }
  };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(false); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{existing ? t("goals.editTitle") : t("goals.newTitle")}</DialogTitle>
          <DialogDescription>{t("goals.dialogDesc")}</DialogDescription>
        </DialogHeader>
        <form onSubmit={save} className="grid gap-4">
          <div className="grid grid-cols-[4rem_minmax(0,1fr)] gap-3">
            <div className="grid gap-1.5"><Label htmlFor="g-icon">{t("goals.icon")}</Label><Input id="g-icon" maxLength={8} className="text-center text-lg" value={icon} onChange={(e) => setIcon(e.target.value)} placeholder="🎯" /></div>
            <div className="grid gap-1.5"><Label htmlFor="g-name">{t("goals.name")}</Label><Input id="g-name" required maxLength={80} autoFocus value={name} onChange={(e) => setName(e.target.value)} /></div>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="grid gap-1.5"><Label htmlFor="g-target">{t("goals.target")}</Label><Input id="g-target" required inputMode="decimal" placeholder="0" value={target} onChange={(e) => setTarget(amount(e.target.value))} /></div>
            <div className="grid gap-1.5"><Label htmlFor="g-date">{t("goals.date")}</Label><Input id="g-date" type="date" value={date} onChange={(e) => setDate(e.target.value)} /></div>
          </div>
          <label className="flex items-start gap-2 text-sm">
            <input type="checkbox" className="accent-[var(--primary)] mt-1" checked={todays} onChange={(e) => setTodays(e.target.checked)} />
            <span>{t("goals.todaysMoney")}<span className="text-muted-foreground block text-xs">{t("goals.todaysMoneyHint")}</span></span>
          </label>
          {todays && (
            <div className="grid max-w-[12rem] gap-1.5"><Label htmlFor="g-inf">{t("goals.inflation")}</Label><Input id="g-inf" inputMode="decimal" value={inflation} onChange={(e) => setInflation(amount(e.target.value))} /></div>
          )}

          <fieldset className="grid gap-2">
            <legend className="mb-1 text-sm font-medium">{t("goals.savedTowards")}</legend>
            {links === null ? <Loader2 className="text-muted-foreground size-4 animate-spin" /> : links.length === 0 ? <p className="text-muted-foreground text-xs">{t("goals.noSources")}</p> : (
              <ul className="max-h-48 space-y-1.5 overflow-y-auto rounded-lg border p-2">
                {links.map((l) => (
                  <li key={l.key} className="flex items-center gap-2 text-sm">
                    <input id={`gl-${l.key}`} type="checkbox" className="accent-[var(--primary)]" checked={l.on} onChange={(e) => setLink(l.key, { on: e.target.checked })} />
                    <label htmlFor={`gl-${l.key}`} className="min-w-0 flex-1 truncate">{l.name}</label>
                    {l.on && <><Input aria-label={t("goals.shareLabel", { name: l.name })} inputMode="numeric" className="h-8 w-16 text-end" value={l.share} onChange={(e) => setLink(l.key, { share: e.target.value.replace(/\D/g, "").slice(0, 3) })} /><span className="text-muted-foreground text-xs">%</span></>}
                  </li>
                ))}
              </ul>
            )}
            <p className="text-muted-foreground text-xs">{t("goals.shareHint")}</p>
            <div className="grid max-w-[16rem] gap-1.5"><Label htmlFor="g-saved">{t("goals.savedByHand")}</Label><Input id="g-saved" inputMode="decimal" placeholder="0" value={saved} onChange={(e) => setSaved(amount(e.target.value))} /></div>
          </fieldset>

          <div className="grid gap-3 sm:grid-cols-3">
            <div className="grid gap-1.5"><Label htmlFor="g-monthly">{t("goals.monthly")}</Label><Input id="g-monthly" inputMode="decimal" placeholder="0" value={monthly} onChange={(e) => setMonthly(amount(e.target.value))} /></div>
            <div className="grid gap-1.5"><Label htmlFor="g-rate">{t("goals.rate")}</Label><Input id="g-rate" inputMode="decimal" value={rate} onChange={(e) => setRate(amount(e.target.value))} /></div>
            <div className="grid gap-1.5"><Label htmlFor="g-priority">{t("goals.priority")}</Label>
              <NativeSelect id="g-priority" value={priority} onChange={(e) => setPriority(Number(e.target.value))}>
                {[1, 2, 3].map((p) => <option key={p} value={p}>{t(`goals.priorities.${p}`)}</option>)}
              </NativeSelect>
            </div>
          </div>
          <p className="text-muted-foreground -mt-2 text-xs">{t("goals.rateHint")}</p>
          <div className="grid gap-1.5"><Label htmlFor="g-notes">{t("goals.notes")}</Label><Input id="g-notes" maxLength={1000} value={notes} onChange={(e) => setNotes(e.target.value)} /></div>
          {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            <Button type="button" variant="outline" onClick={() => onClose(false)}>{t("common.cancel")}</Button>
            <Button disabled={busy || !name.trim() || !target}>{busy && <Loader2 className="animate-spin" />}{existing ? t("common.save") : t("goals.create")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
