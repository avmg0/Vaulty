import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { ChevronLeft, ChevronRight, CornerDownRight, Loader2, Pencil, Plus, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { api, DATA_RESTORED, errorText, type BudgetLine, type BudgetMonth } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ProgressBar } from "@/components/ProgressBar";
import { AnimatedMoney } from "@/components/AnimatedMoney";
import { Segmented } from "@/components/Segmented";
import { Skeleton, SkeletonRows } from "@/components/Skeleton";
import { PROFILE_COLORS } from "@/components/UserAvatar";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate, formatMoney } from "@/lib/utils";

const monthKey = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
const shift = (key: string, by: number) => { const [y, m] = key.split("-").map(Number); return monthKey(new Date(y, m - 1 + by, 1)); };

/** A parent with its sub-categories; the parent's spending includes theirs. */
type Group = { line: BudgetLine; children: BudgetLine[]; spent: number };

export function Budgets() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [month, setMonth] = useState(() => monthKey(new Date()));
  const [data, setData] = useState<BudgetMonth | null>(null);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<{ type: "expense" | "income"; edit?: BudgetLine } | null>(null);

  const load = () => api.budgets.month(month).then((d) => { setData(d); setError(""); }).catch((e) => setError(errorText(e)));
  useEffect(() => { load(); }, [month]);
  useEffect(() => { window.addEventListener(DATA_RESTORED, load); return () => window.removeEventListener(DATA_RESTORED, load); }, [month]);

  const groups = (type: "expense" | "income"): Group[] => {
    const all = (data?.categories ?? []).filter((c) => c.type === type);
    return all.filter((c) => !c.parent_id || !all.some((p) => p.id === c.parent_id)).map((line) => {
      const children = all.filter((c) => c.parent_id === line.id);
      return { line, children, spent: line.spent + children.reduce((s, c) => s + c.spent, 0) };
    });
  };
  const expense = useMemo(() => groups("expense"), [data]);
  const income = useMemo(() => groups("income"), [data]);
  const cur = data?.currency ?? "INR";
  const money = (n: number) => formatMoney(n, cur, hideBalances);

  // Totals over budgeted lines only; a parent's budget covers its children, so they aren't added twice.
  const budgeted = expense.reduce((s, g) => s + (g.line.budget ?? g.children.reduce((x, c) => x + (c.budget ?? 0), 0)), 0);
  const spentInBudgets = expense.reduce((s, g) => s + (g.line.budget != null ? g.spent : g.children.reduce((x, c) => x + (c.budget != null ? c.spent : 0), 0)), 0);
  const spentAll = expense.reduce((s, g) => s + g.spent, 0) + (data?.uncategorised_expense ?? 0);

  // How far through the month we are, to say whether spending is on pace.
  const [y, m] = month.split("-").map(Number);
  const days = new Date(y, m, 0).getDate();
  const now = new Date();
  const current = monthKey(now) === month;
  const elapsed = current ? now.getDate() / days : month < monthKey(now) ? 1 : 0;

  return (
    <div className="max-w-4xl space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" onClick={() => setMonth((k) => shift(k, -1))} aria-label={t("family.prevMonth")}><ChevronLeft /></Button>
          <div className="w-40 text-center text-sm font-medium">{formatDate(new Date(y, m - 1, 1), { month: "long", year: "numeric" })}</div>
          <Button variant="ghost" size="icon" onClick={() => setMonth((k) => shift(k, 1))} aria-label={t("family.nextMonth")}><ChevronRight /></Button>
        </div>
        {!current && <Button variant="link" size="sm" onClick={() => setMonth(monthKey(new Date()))}>{t("budgets.thisMonth")}</Button>}
        <Button className="ml-auto" onClick={() => setDialog({ type: "expense" })}><Plus />{t("budgets.addCategory")}</Button>
      </div>
      {error && <p className="text-destructive text-sm">{error}</p>}

      <Card className="gap-3">
        <CardContent className="grid gap-4 sm:grid-cols-3">
          {!data ? [0, 1, 2].map((i) => <Skeleton key={i} className="h-14" />) : <>
            <Stat label={t("budgets.budgeted")} value={<AnimatedMoney value={budgeted} currency={cur} hidden={hideBalances} />} />
            <Stat label={t("budgets.spentAll")} value={<AnimatedMoney value={spentAll} currency={cur} hidden={hideBalances} />} />
            <Stat label={budgeted - spentInBudgets >= 0 ? t("budgets.left") : t("budgets.over")}
              value={<AnimatedMoney value={Math.abs(budgeted - spentInBudgets)} currency={cur} hidden={hideBalances} />}
              tone={budgeted - spentInBudgets < 0 ? "text-destructive" : undefined} />
          </>}
          {data && budgeted > 0 && (
            <div className="sm:col-span-3">
              <ProgressBar value={spentInBudgets / budgeted} barClassName={tone(spentInBudgets / budgeted, elapsed)} label={t("budgets.budgeted")} />
              {current && <p className="text-muted-foreground mt-1.5 text-xs">{pace(t, spentInBudgets / budgeted, elapsed, days - now.getDate())}</p>}
            </div>
          )}
          {data && budgeted === 0 && <p className="text-muted-foreground text-sm sm:col-span-3">{t("budgets.none")}</p>}
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>{t("budgets.spending")}</CardTitle><CardDescription>{t("budgets.spendingDesc")}</CardDescription></CardHeader>
        <CardContent className="divide-y">
          {!data && <SkeletonRows rows={5} />}
          {expense.map((g) => (
            <div key={g.line.id}>
              <Line line={g.line} spent={g.spent} elapsed={elapsed} money={money} month={month} onSaved={load} onEdit={() => setDialog({ type: "expense", edit: g.line })} />
              {g.children.map((c) => <Line key={c.id} line={c} spent={c.spent} elapsed={elapsed} money={money} month={month} child onSaved={load} onEdit={() => setDialog({ type: "expense", edit: c })} />)}
            </div>
          ))}
          {data && data.uncategorised_expense > 0 && (
            <div className="text-muted-foreground flex justify-between py-3 text-sm"><span>{t("dashboard.uncategorised")}</span><span className="tabular-nums">{money(data.uncategorised_expense)}</span></div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex-row items-start justify-between">
          <div className="space-y-1.5"><CardTitle>{t("budgets.income")}</CardTitle><CardDescription>{t("budgets.incomeDesc")}</CardDescription></div>
          <Button variant="outline" size="sm" onClick={() => setDialog({ type: "income" })}><Plus />{t("budgets.addIncomeCategory")}</Button>
        </CardHeader>
        <CardContent className="divide-y">
          {income.flatMap((g) => [{ l: g.line, s: g.spent, child: false }, ...g.children.map((c) => ({ l: c, s: c.spent, child: true }))]).map(({ l, s, child }) => (
            <div key={l.id} className={cn("flex items-center gap-3 py-2.5 text-sm", child && "ps-6")}>
              {child && <CornerDownRight className="text-muted-foreground size-3.5" />}
              <span className="text-base">{l.icon}</span>
              <button type="button" className="flex-1 truncate text-left hover:underline" onClick={() => setDialog({ type: "income", edit: l })}>{l.name}</button>
              <span className={cn("tabular-nums", s > 0 ? "text-success" : "text-muted-foreground")}>{money(s)}</span>
            </div>
          ))}
        </CardContent>
      </Card>

      {dialog && <CategoryDialog type={dialog.type} edit={dialog.edit} lines={data?.categories ?? []} onClose={() => setDialog(null)} onSaved={() => { setDialog(null); load(); }} />}
    </div>
  );
}

function Stat({ label, value, tone }: { label: string; value: React.ReactNode; tone?: string }) {
  return <div><div className="text-muted-foreground text-sm">{label}</div><div className={cn("text-2xl font-semibold tabular-nums", tone)}>{value}</div></div>;
}

/** Bar colour: fine, getting close (ahead of the month's pace), or over. */
function tone(used: number, elapsed: number) {
  if (used > 1) return "bg-destructive";
  if (used >= 0.9 || (elapsed > 0 && used > elapsed + 0.1)) return "bg-orange-500";
  return "bg-success";
}

function pace(t: TFunction, used: number, elapsed: number, daysLeft: number) {
  if (used > 1) return t("budgets.paceOver");
  if (used > elapsed + 0.1) return t("budgets.paceAhead", { count: daysLeft });
  return t("budgets.paceOk", { count: daysLeft });
}

/** One category: spent against its budget, with the budget editable in place. */
function Line({ line, spent, elapsed, money, child, onSaved, onEdit }: {
  line: BudgetLine; spent: number; elapsed: number; money: (n: number) => string; month: string; child?: boolean; onSaved: () => void; onEdit: () => void;
}) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { if (editing) input.current?.select(); }, [editing]);
  const save = async (e?: FormEvent) => {
    e?.preventDefault();
    setBusy(true); setError("");
    try { await api.budgets.set(line.id, value.trim() || null); setEditing(false); onSaved(); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const used = line.budget ? spent / line.budget : 0;
  return (
    <div className={cn("group py-3", child && "ps-6")}>
      <div className="flex items-center gap-3">
        {child && <CornerDownRight className="text-muted-foreground size-3.5 shrink-0" />}
        <span className="grid size-8 shrink-0 place-items-center rounded-lg text-base" style={{ backgroundColor: `color-mix(in oklab, ${line.color ?? "#94a3b8"} 18%, transparent)` }}>{line.icon}</span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="truncate text-sm font-medium">{line.name}</span>
            <button type="button" onClick={onEdit} className="text-muted-foreground hover:text-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100" aria-label={t("budgets.editCategory")}><Pencil className="size-3" /></button>
          </div>
          {line.budget != null && <ProgressBar value={used} className="mt-1.5" barClassName={tone(used, elapsed)} label={line.name} />}
        </div>
        <div className="text-end text-sm tabular-nums">
          <div className="font-medium">{money(spent)}</div>
          {editing ? (
            <form onSubmit={save} className="mt-1 flex items-center gap-1">
              <Input ref={input} type="number" min="0" step="0.01" className="h-7 w-28 text-end text-xs" value={value} onChange={(e) => setValue(e.target.value)}
                onKeyDown={(e) => e.key === "Escape" && setEditing(false)} aria-label={t("budgets.monthly", { name: line.name })} />
              <Button size="sm" className="h-7" disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : t("common.save")}</Button>
            </form>
          ) : (
            <button type="button" className="text-muted-foreground hover:text-foreground text-xs underline-offset-4 hover:underline"
              onClick={() => { setValue(line.budget != null ? String(line.budget) : ""); setEditing(true); }}>
              {line.budget == null ? t("budgets.set")
                : spent <= line.budget ? t("budgets.leftOf", { left: money(line.budget - spent), budget: money(line.budget) })
                : t("budgets.overOf", { over: money(spent - line.budget), budget: money(line.budget) })}
            </button>
          )}
          {error && <p className="text-destructive text-xs">{error}</p>}
        </div>
      </div>
    </div>
  );
}

const ICONS = ["🍔", "🛒", "🚗", "🏠", "💡", "🎬", "💊", "🎓", "✈️", "👕", "🎁", "🐾", "📱", "💼", "📈", "🧾", "☕", "🍼"];

function CategoryDialog({ type: initialType, edit, lines, onClose, onSaved }: {
  type: "expense" | "income"; edit?: BudgetLine; lines: BudgetLine[]; onClose: () => void; onSaved: () => void;
}) {
  const { t } = useTranslation();
  const [type, setType] = useState<"expense" | "income">(edit?.type ?? initialType);
  const [name, setName] = useState(edit?.name ?? "");
  const [icon, setIcon] = useState(edit?.icon ?? "📁");
  const [color, setColor] = useState(edit?.color ?? PROFILE_COLORS[0]);
  const [parent, setParent] = useState(edit?.parent_id ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const builtIn = !!edit?.is_system;
  // Possible parents: top-level categories of the same type, other than this one.
  const parents = lines.filter((l) => l.type === type && !l.parent_id && l.id !== edit?.id);
  const hasChildren = !!edit && lines.some((l) => l.parent_id === edit.id);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    const body = { name, type, icon, color, parent_id: parent || null };
    try { if (edit) await api.categories.update(edit.id, body); else await api.categories.create(body); onSaved(); }
    catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const remove = async () => {
    if (!edit) return;
    setBusy(true);
    try { await api.categories.delete(edit.id); onSaved(); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader><DialogTitle>{edit ? t("budgets.editTitle", { name: edit.name }) : t("budgets.newTitle")}</DialogTitle></DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          {!edit && <Segmented value={type} onChange={(v) => { setType(v); setParent(""); }} className="flex w-full" itemClassName="py-1.5" label={t("tx.type")}
            options={[{ value: "expense", label: t("tx.expense") }, { value: "income", label: t("tx.income") }]} />}
          <div className="grid gap-2"><Label htmlFor="cat-name">{t("accounts.name")}</Label>
            <Input id="cat-name" value={name} onChange={(e) => setName(e.target.value)} maxLength={40} required disabled={builtIn} autoFocus={!builtIn} />
            {builtIn && <p className="text-muted-foreground text-xs">{t("budgets.builtIn")}</p>}</div>
          <div className="grid gap-2"><Label>{t("budgets.icon")}</Label>
            <div className="flex flex-wrap gap-1">
              {ICONS.map((i) => <button key={i} type="button" onClick={() => setIcon(i)} aria-pressed={icon === i}
                className={cn("grid size-8 place-items-center rounded-md text-base hover:bg-accent", icon === i && "bg-accent ring-ring ring-2")}>{i}</button>)}
              <Input aria-label={t("budgets.ownIcon")} className="h-8 w-14 text-center" maxLength={4} value={icon} onChange={(e) => setIcon(e.target.value)} />
            </div></div>
          <div className="grid gap-2"><Label>{t("members.colour")}</Label>
            <div className="flex gap-1.5">
              {PROFILE_COLORS.map((c) => <button key={c} type="button" onClick={() => setColor(c)} aria-label={c} aria-pressed={color === c}
                className={cn("size-7 rounded-full ring-offset-2 ring-offset-background", color === c && "ring-ring ring-2")} style={{ backgroundColor: c }} />)}
            </div></div>
          <div className="grid gap-2"><Label htmlFor="cat-parent">{t("budgets.parent")}</Label>
            <NativeSelect id="cat-parent" value={parent} onChange={(e) => setParent(e.target.value)} disabled={hasChildren}>
              <option value="">{t("budgets.topLevel")}</option>
              {parents.map((p) => <option key={p.id} value={p.id}>{p.icon} {p.name}</option>)}
            </NativeSelect>
            {hasChildren && <p className="text-muted-foreground text-xs">{t("budgets.hasChildren")}</p>}</div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="sm:justify-between">
            {edit && !builtIn ? <Button type="button" variant="ghost" className="text-destructive" onClick={remove} disabled={busy}><Trash2 />{t("budgets.delete")}</Button> : <span />}
            <div className="flex gap-2">
              <Button type="button" variant="ghost" onClick={onClose}>{t("common.cancel")}</Button>
              <Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}{edit ? t("cards.saveChanges") : t("budgets.create")}</Button>
            </div>
          </DialogFooter>
          {edit && !builtIn && <p className="text-muted-foreground -mt-2 text-xs">{t("budgets.deleteHint")}</p>}
        </form>
      </DialogContent>
    </Dialog>
  );
}

