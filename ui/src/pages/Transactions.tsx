import { useEffect, useMemo, useRef, useState, type FormEvent, type ReactNode } from "react";
import { ArrowDownRight, ArrowLeftRight, ArrowUpRight, Eye, EyeOff, Fingerprint, Loader2, Lock, NotebookText, Plus, Search, Trash2 } from "lucide-react";
import { TransactionDialog } from "@/components/TransactionDialog";
import { splitTags } from "@/components/TagInput";
import { useTranslation } from "react-i18next";
import { AnimatedItem, AnimatedList } from "@/components/AnimatedList";
import { Segmented } from "@/components/Segmented";
import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";
import { AnimatedMoney } from "@/components/AnimatedMoney";
import { api, DATA_RESTORED, errorText, type Account, type Category, type HiddenStatus, type SharingPrefs, type Transaction, type Visibility } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input, NativeSelect } from "@/components/ui/input";
import { VisibilityBadge } from "@/components/Visibility";
import { cn, formatDate } from "@/lib/utils";
import { useTheme } from "@/hooks/theme";

type RowProps = {
  t: Pick<Transaction, "type" | "amount" | "date" | "merchant" | "description" | "category_name" | "account_name" | "currency" | "visibility" | "hidden" | "masked">
    & Partial<Pick<Transaction, "to_account_name" | "tags" | "notes" | "splits">>;
  hidden: boolean;
  /** Shown before the date line, e.g. who the item belongs to in the family view. */
  lead?: ReactNode;
  onDelete?: () => void;
  onVisibilityChange?: (v: Visibility) => void;
  /** Moves the transaction into the Hidden section, or back out of it. */
  onToggleHidden?: () => void;
  /** Keeps a masked row's amount in line with rows that show action buttons. */
  reserveActions?: boolean;
  /** Opens the transaction for editing when its name is clicked. */
  onEdit?: () => void;
};

export function TransactionRow({ t, hidden, lead, onDelete, onVisibilityChange, onToggleHidden, reserveActions, onEdit }: RowProps) {
  const { t: tr } = useTranslation();
  const income = t.type === "income", transfer = t.type === "transfer";
  const isHidden = t.hidden === 1 || !!t.masked;
  const tags = splitTags(t.tags);
  // Masked rows keep amount, date and account; everything that says what it was is withheld.
  if (t.masked) {
    return (
      <div className="group flex items-center gap-3 py-2.5" title={tr("hidden.maskedHint")}>
        <div className="bg-muted text-muted-foreground grid size-8 shrink-0 place-items-center rounded-full"><EyeOff className="size-4" /></div>
        <div className="min-w-0 flex-1">
          <div className="text-muted-foreground truncate text-sm font-medium tracking-[0.25em]" aria-label={tr("hidden.category")}>{tr("hidden.mask")}</div>
          <div className="text-muted-foreground flex items-center gap-1.5 truncate text-xs">
            {lead}
            <span className="truncate">{[formatDate(t.date), tr("hidden.category"), t.account_name].filter(Boolean).join(" · ")}</span>
          </div>
        </div>
        <div className={cn("text-sm font-medium tabular-nums", income && "text-success")}>
          {income ? "+" : "−"}<AnimatedMoney value={t.amount} currency={t.currency || "INR"} hidden={hidden} />
        </div>
        <span className="text-muted-foreground grid size-7 place-items-center"><EyeOff className="size-3.5" /></span>
        {reserveActions && <span className="w-[4.25rem] shrink-0" aria-hidden />}
      </div>
    );
  }
  return (
    <div className="group flex items-center gap-3 py-2.5">
      <div className={cn("grid size-8 shrink-0 place-items-center rounded-full", income ? "bg-success/15 text-success" : transfer ? "bg-primary/10 text-primary" : "bg-secondary text-muted-foreground")}>
        {income ? <ArrowUpRight className="size-4" /> : transfer ? <ArrowLeftRight className="size-4" /> : <ArrowDownRight className="size-4" />}
      </div>
      <div className="min-w-0 flex-1">
        {(() => {
          const title = t.merchant || t.description || (transfer ? tr("tx.transferLabel") : t.category_name) || tr("tx.untitled");
          return onEdit
            ? <button type="button" onClick={onEdit} className="block max-w-full truncate text-left text-sm font-medium underline-offset-4 hover:underline" title={tr("tx.edit")}>{title}</button>
            : <div className="truncate text-sm font-medium">{title}</div>;
        })()}
        <div className="text-muted-foreground flex items-center gap-1.5 truncate text-xs">
          {lead}
          <span className="truncate">{[formatDate(t.date),
            transfer ? `${t.account_name} → ${t.to_account_name ?? "?"}`
              : t.splits?.length ? tr("split.label", { names: [...new Set(t.splits.map((x) => x.category_name ?? tr("dashboard.uncategorised")))].join(", ") })
              : t.category_name,
            transfer ? null : t.account_name].filter(Boolean).join(" · ")}</span>
          {tags.slice(0, 3).map((g) => <span key={g} className="bg-secondary shrink-0 rounded px-1 text-[10px]">#{g}</span>)}
          {t.notes && <NotebookText className="size-3 shrink-0" aria-label={tr("tx.notes")}><title>{t.notes}</title></NotebookText>}
        </div>
      </div>
      <div className={cn("text-sm font-medium tabular-nums", income && "text-success")}>
        {income ? "+" : transfer ? "" : "−"}<AnimatedMoney value={t.amount} currency={t.currency || "INR"} hidden={hidden} />
      </div>
      {/* Hidden transactions are never shared, so their visibility doesn't apply. */}
      {!isHidden && <VisibilityBadge value={t.visibility ?? "private"} onChange={onVisibilityChange} />}
      {onToggleHidden && (
        <Button variant="ghost" size="icon" className="size-7 opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onClick={onToggleHidden}
          title={isHidden ? tr("hidden.unhide") : tr("hidden.hideTitle")} aria-label={isHidden ? tr("hidden.unhide") : tr("hidden.hideTitle")}>
          {isHidden ? <Eye /> : <EyeOff />}
        </Button>
      )}
      {onDelete && <Button variant="ghost" size="icon" className="size-7 opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onClick={onDelete} title={tr("tx.delete")} aria-label={tr("tx.delete")}><Trash2 /></Button>}
    </div>
  );
}

export function Transactions() {
  const { t: tr } = useTranslation();
  const { hideBalances, hiddenInMain } = useTheme();
  const [tab, setTab] = useState<"all" | "hidden">("all");
  const [hiddenCount, setHiddenCount] = useState(0);
  const [editing, setEditing] = useState<Transaction | null>(null);
  const [rows, setRows] = useState<Transaction[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [prefs, setPrefs] = useState<SharingPrefs | null>(null);
  const [q, setQ] = useState("");
  const [type, setType] = useState("");
  const [accountId, setAccountId] = useState("");
  const [visibility, setVisibility] = useState<"" | Visibility>("");
  const [open, setOpen] = useState(false);
  const edit = (t: Transaction) => { setEditing(t); setOpen(true); };

  const load = () => {
    api.hidden.status().then((h) => setHiddenCount(h.count)).catch(() => {});
    return api.transactions.getAll({ type: type || undefined, accountId: accountId || undefined, visibility: visibility || undefined,
      hidden: hiddenInMain === "off" ? "exclude" : undefined }).then(setRows);
  };
  useEffect(() => {
    load();
    // After an undo or restore, reload in place so the restored row slides back in.
    window.addEventListener(DATA_RESTORED, load);
    return () => window.removeEventListener(DATA_RESTORED, load);
  }, [type, accountId, visibility, hiddenInMain, tab]);
  useEffect(() => {
    api.accounts.getAll().then(setAccounts);
    api.categories.getAll().then(setCategories);
    api.sharing.get().then((s) => setPrefs(s.prefs));
  }, []);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    const tag = s.startsWith("#") ? s.slice(1) : null;
    if (tag) return rows.filter((t) => splitTags(t.tags).some((g) => g.toLowerCase().startsWith(tag)));
    return s ? rows.filter((t) => [t.merchant, t.description, t.category_name, t.account_name, t.to_account_name, t.notes, t.tags].some((v) => v?.toLowerCase().includes(s))) : rows;
  }, [rows, q]);
  const allTags = useMemo(() => [...new Set(rows.flatMap((r) => splitTags(r.tags)))].sort(), [rows]);

  const [reloadHidden, setReloadHidden] = useState(0);

  const toggleHidden = async (row: Transaction) => {
    try { await api.transactions.setHidden(row.id, !(row.hidden === 1 || row.masked)); } catch { /* the Hidden tab explains locking */ }
    load(); setReloadHidden((n) => n + 1);
  };

  const changeVisibility = async (id: string, v: Visibility) => {
    await api.transactions.setVisibility(id, v);
    load();
  };

  return (
    <div className="space-y-4">
      <Segmented role="tablist" value={tab} onChange={setTab} label={tr("hidden.lockedTitle")}
        options={[{ value: "all", label: tr("hidden.tabAll") },
          { value: "hidden", label: <><EyeOff className="size-3.5" />{tr("hidden.tabHidden")}{hiddenCount > 0 && <span className="text-muted-foreground tabular-nums">{hiddenCount}</span>}</> }]} />
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-56 flex-1">
          <Search className="text-muted-foreground absolute top-2.5 left-2.5 size-4" />
          <Input placeholder={tr("tx.search")} className="pl-8" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        {tab === "all" && <>
        <NativeSelect className="w-32" value={type} onChange={(e) => setType(e.target.value)}>
          <option value="">{tr("tx.allTypes")}</option><option value="expense">{tr("tx.expenses")}</option><option value="income">{tr("tx.incomes")}</option><option value="transfer">{tr("tx.transfers")}</option>
        </NativeSelect>
        <NativeSelect className="w-40" value={accountId} onChange={(e) => setAccountId(e.target.value)}>
          <option value="">{tr("tx.allAccounts")}</option>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
        </NativeSelect>
        <NativeSelect className="w-32" value={visibility} onChange={(e) => setVisibility(e.target.value as "" | Visibility)} aria-label={tr("tx.visibility")}>
          <option value="">{tr("tx.anyVisibility")}</option><option value="private">{tr("visibility.private")}</option><option value="shared">{tr("visibility.shared")}</option><option value="public">{tr("visibility.public")}</option>
        </NativeSelect>
        </>}
        <Button disabled={!accounts.length} title={accounts.length ? "" : tr("tx.needAccount")} onClick={() => { setEditing(null); setOpen(true); }}><Plus />{tr("tx.add")}</Button>
        <TransactionDialog open={open} onOpenChange={setOpen} initial={editing} accounts={accounts} categories={categories} prefs={prefs}
          startHidden={tab === "hidden"} tagSuggestions={allTags} onSaved={() => { load(); setReloadHidden((n) => n + 1); }} />
      </div>
      {tab === "hidden" ? <HiddenSection query={q} reload={reloadHidden} onChange={load} hideBalances={hideBalances} onToggle={toggleHidden} onEdit={edit} /> : (
      <Card className="py-2">
        <CardContent className="divide-y">
          {filtered.length === 0 && <p className="text-muted-foreground py-10 text-center text-sm">{tr("tx.noMatch")}</p>}
          <AnimatedList>
            {filtered.map((t) => (
              <AnimatedItem key={t.id}>
                <TransactionRow t={t} hidden={hideBalances}
                  onVisibilityChange={(v) => changeVisibility(t.id, v)}
                  onToggleHidden={t.masked ? undefined : () => toggleHidden(t)} reserveActions
                  onEdit={t.masked ? undefined : () => edit(t)}
                  onDelete={t.masked ? undefined : () => api.transactions.delete(t.id).then(load)} />
              </AnimatedItem>
            ))}
          </AnimatedList>
        </CardContent>
      </Card>)}
    </div>
  );
}

const clock = (secs: number) => `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;

/**
 * The Hidden tab. Details arrive from the app only after the member confirms it's them, and it
 * locks again after a few minutes, when they leave the tab, or when the window is hidden.
 */
function HiddenSection({ query, reload, onChange, hideBalances, onToggle, onEdit }: {
  query: string; reload: number; onChange: () => void; hideBalances: boolean; onToggle: (t: Transaction) => void; onEdit: (t: Transaction) => void;
}) {
  const { t } = useTranslation();
  const { fade } = useMotion();
  const [status, setStatus] = useState<HiddenStatus | null>(null);
  const [rows, setRows] = useState<Transaction[]>([]);
  const [left, setLeft] = useState(0);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState<"" | "password" | "system">("");
  const [error, setError] = useState("");
  const input = useRef<HTMLInputElement>(null);

  const apply = (s: HiddenStatus) => {
    setStatus(s); setLeft(s.seconds_left);
    if (s.unlocked) api.transactions.getAll({ hidden: "only", reveal_hidden: true }).then(setRows);
    else setRows([]);
  };
  const refresh = () => api.hidden.status().then(apply);
  const lock = () => { setRows([]); setStatus((s) => s && { ...s, unlocked: false, seconds_left: 0 }); api.hidden.lock().then(refresh); };

  useEffect(() => { refresh(); }, [reload]);
  // Leaving the tab or hiding the window locks it.
  useEffect(() => {
    const onHide = () => { if (document.hidden) lock(); };
    document.addEventListener("visibilitychange", onHide);
    return () => { document.removeEventListener("visibilitychange", onHide); api.hidden.lock(); };
  }, []);
  // Countdown to the automatic lock.
  useEffect(() => {
    if (!status?.unlocked) return;
    const id = setInterval(() => setLeft((n) => { if (n <= 1) { clearInterval(id); refresh(); return 0; } return n - 1; }), 1000);
    return () => clearInterval(id);
  }, [status?.unlocked]);
  useEffect(() => { if (status && !status.unlocked) input.current?.focus(); }, [status?.unlocked]);

  const unlock = async (how: "password" | "system", e?: FormEvent) => {
    e?.preventDefault();
    setBusy(how); setError("");
    try { apply(how === "password" ? await api.hidden.unlock(password) : await api.hidden.unlockBiometric()); setPassword(""); }
    catch (err) { setError(errorText(err)); }
    finally { setBusy(""); }
  };

  const shown = useMemo(() => {
    const s = query.trim().toLowerCase();
    return s ? rows.filter((r) => [r.merchant, r.description, r.category_name, r.account_name].some((v) => v?.toLowerCase().includes(s))) : rows;
  }, [rows, query]);

  if (!status) return null;
  if (!status.unlocked) {
    return (
      <Card>
        <CardContent>
          <motion.form onSubmit={(e) => unlock("password", e)} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} transition={fade({ duration: 0.25 })}
            className="mx-auto grid max-w-sm justify-items-center gap-4 py-8 text-center">
            <div className="bg-muted grid size-12 place-items-center rounded-2xl"><Lock className="size-5" /></div>
            <div className="grid gap-1">
              <h2 className="text-base font-semibold">{t("hidden.lockedTitle")}</h2>
              <p className="text-muted-foreground text-sm text-pretty">{t("hidden.lockedDesc")}</p>
              <p className="text-muted-foreground text-xs">{t("hidden.count", { count: status.count })}</p>
            </div>
            <div className="grid w-full gap-2">
              <Input ref={input} type="password" autoComplete="current-password" placeholder={t("hidden.password")} aria-label={t("hidden.password")}
                value={password} onChange={(e) => setPassword(e.target.value)} />
              <Button type="submit" disabled={!password || !!busy}>{busy === "password" ? <Loader2 className="animate-spin" /> : <Eye />}{t("hidden.unlock")}</Button>
              {status.biometric && (
                <Button type="button" variant="outline" disabled={!!busy} onClick={() => unlock("system")}>
                  {busy === "system" ? <Loader2 className="animate-spin" /> : <Fingerprint />}{t("hidden.unlockSystem")}
                </Button>
              )}
            </div>
            {error && <p className="text-destructive text-sm">{error}</p>}
          </motion.form>
        </CardContent>
      </Card>
    );
  }
  return (
    <Card className="py-2">
      <CardContent>
        <div className="flex items-center justify-between gap-3 border-b pb-2">
          <span className="text-muted-foreground flex items-center gap-1.5 text-xs tabular-nums"><Eye className="size-3.5" />{t("hidden.locksIn", { time: clock(left) })}</span>
          <Button variant="outline" size="sm" onClick={lock}><Lock />{t("hidden.lockNow")}</Button>
        </div>
        <div className="divide-y">
          {shown.length === 0 && <p className="text-muted-foreground py-10 text-center text-sm">{query ? t("tx.noMatch") : t("hidden.empty")}</p>}
          <AnimatedList>
            {shown.map((r) => (
              <AnimatedItem key={r.id}>
                <TransactionRow t={r} hidden={hideBalances} onToggleHidden={() => onToggle(r)} onEdit={() => onEdit(r)}
                  onDelete={() => api.transactions.delete(r.id).then(() => { onChange(); refresh(); })} />
              </AnimatedItem>
            ))}
          </AnimatedList>
        </div>
      </CardContent>
    </Card>
  );
}
