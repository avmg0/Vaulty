import { useEffect, useRef, useState, type FormEvent } from "react";
import { ArrowDownRight, ArrowUpRight, CalendarDays, Loader2, SlidersHorizontal, Tag, Wallet, Zap } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type Account, type Category, type QuickParsed, type SharingPrefs } from "@/api";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { NativeSelect } from "@/components/ui/input";
import { TransactionDialog } from "@/components/TransactionDialog";
import { useTheme } from "@/hooks/theme";
import { rememberFocus, returnFocus } from "@/lib/focus";
import { cn, formatDate, formatMoney } from "@/lib/utils";

const LAST_ACCOUNT = "vaulty.quick.account";
export const QUICK_ADD_EVENT = "vaulty:quick-add";

/**
 * One line, one transaction: "250 swiggy yesterday". Opened with Ctrl/Cmd+J or the header button.
 * What was understood is shown before saving; "Add details" opens the full form pre-filled.
 */
export function QuickAdd() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [parsed, setParsed] = useState<QuickParsed | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [prefs, setPrefs] = useState<SharingPrefs | null>(null);
  const [accountId, setAccountId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState<string | null>(null);
  const [details, setDetails] = useState(false);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const show = () => { rememberFocus(); setOpen(true); };
    const onKey = (e: KeyboardEvent) => { if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "j") { e.preventDefault(); show(); } };
    window.addEventListener("keydown", onKey);
    window.addEventListener(QUICK_ADD_EVENT, show);
    return () => { window.removeEventListener("keydown", onKey); window.removeEventListener(QUICK_ADD_EVENT, show); };
  }, []);
  useEffect(() => {
    if (!open) return;
    setText(""); setParsed(null); setError(""); setSaved(null);
    Promise.all([api.accounts.getAll(), api.categories.getAll(), api.sharing.get()]).then(([a, c, s]) => {
      setAccounts(a); setCategories(c); setPrefs(s.prefs);
      let last = ""; try { last = localStorage.getItem(LAST_ACCOUNT) ?? ""; } catch { /* private mode */ }
      setAccountId(a.some((x) => x.id === last) ? last : a[0]?.id ?? "");
    });
  }, [open]);
  useEffect(() => {
    if (!text.trim()) { setParsed(null); return; }
    const id = setTimeout(() => api.quick.parse(text).then(setParsed).catch(() => setParsed(null)), 120);
    return () => clearTimeout(id);
  }, [text]);

  const account = accounts.find((a) => a.id === (parsed?.account_id ?? accountId));
  const category = categories.find((c) => c.id === parsed?.category_id);
  const ready = !!parsed?.amount && !!account;

  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (!parsed || !ready) return;
    setBusy(true); setError("");
    try {
      await api.transactions.create({ type: parsed.kind, amount: parsed.amount!, date: parsed.date, account_id: account!.id,
        merchant: parsed.merchant || undefined, category_id: parsed.category_id ?? undefined, visibility: prefs?.default_visibility ?? "private" });
      try { localStorage.setItem(LAST_ACCOUNT, account!.id); } catch { /* ignore */ }
      window.dispatchEvent(new Event(DATA_RESTORED));
      setSaved(t("quick.saved", { what: parsed.merchant || formatMoney(Number(parsed.amount), account!.currency, hideBalances) }));
      setText(""); setParsed(null);
      input.current?.focus();
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  return (
    <>
      <Dialog open={open && !details} onOpenChange={setOpen}>
        <DialogContent className="top-[22%] translate-y-0 gap-3 sm:max-w-lg" onCloseAutoFocus={(e) => { e.preventDefault(); returnFocus(); }}>
          <DialogHeader className="sr-only"><DialogTitle>{t("quick.title")}</DialogTitle><DialogDescription>{t("quick.hint")}</DialogDescription></DialogHeader>
          <form onSubmit={save} className="grid gap-3">
            <div className="flex items-center gap-2 border-b pb-2">
              <Zap className="text-primary size-4 shrink-0" />
              <input ref={input} autoFocus value={text} onChange={(e) => { setText(e.target.value); setSaved(null); }} maxLength={200}
                placeholder={t("quick.placeholder")} aria-label={t("quick.title")} className="placeholder:text-muted-foreground flex-1 bg-transparent py-1 text-base outline-none" />
              {busy && <Loader2 className="size-4 animate-spin" />}
            </div>
            {parsed ? (
              <div className="flex flex-wrap items-center gap-1.5 text-xs">
                <Chip icon={parsed.kind === "income" ? ArrowUpRight : ArrowDownRight} tone={parsed.kind === "income" ? "text-success" : undefined}>
                  {parsed.amount ? formatMoney(Number(parsed.amount), account?.currency ?? "INR", hideBalances) : <span className="text-destructive">{t("quick.noAmount")}</span>}
                </Chip>
                {parsed.merchant && <Chip>{parsed.merchant}</Chip>}
                <Chip icon={Tag} muted={!category}>{category ? `${category.icon ?? ""} ${category.name}` : t("dashboard.uncategorised")}</Chip>
                <Chip icon={CalendarDays}>{formatDate(parsed.date)}</Chip>
                {parsed.account_id ? <Chip icon={Wallet}>{account?.name}</Chip> : (
                  <span className="flex items-center gap-1"><Wallet className="text-muted-foreground size-3.5" />
                    <NativeSelect aria-label={t("tx.account")} className="h-7 py-0 text-xs" value={accountId} onChange={(e) => setAccountId(e.target.value)}>
                      {accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
                    </NativeSelect></span>
                )}
              </div>
            ) : <p className="text-muted-foreground text-xs">{saved ?? t("quick.examples")}</p>}
            {error && <p className="text-destructive text-sm">{error}</p>}
            <div className="flex items-center justify-between gap-2">
              <span className="text-muted-foreground text-xs">{t("quick.keys")}</span>
              <div className="flex gap-2">
                <Button type="button" variant="ghost" size="sm" disabled={!accounts.length} onClick={() => setDetails(true)}><SlidersHorizontal />{t("quick.details")}</Button>
                <Button size="sm" disabled={!ready || busy}>{t("common.save")}</Button>
              </div>
            </div>
          </form>
        </DialogContent>
      </Dialog>
      <TransactionDialog open={details} onOpenChange={(o) => { setDetails(o); if (!o) setOpen(false); }} initial={null} accounts={accounts} categories={categories} prefs={prefs}
        prefill={parsed ? { type: parsed.kind, amount: parsed.amount ?? "", date: parsed.date, merchant: parsed.merchant, account_id: account?.id, category_id: parsed.category_id ?? undefined } : { account_id: accountId }}
        onSaved={() => window.dispatchEvent(new Event(DATA_RESTORED))} />
    </>
  );
}

function Chip({ icon: Icon, children, tone, muted }: { icon?: typeof Tag; children: React.ReactNode; tone?: string; muted?: boolean }) {
  return (
    <span className={cn("bg-secondary flex items-center gap-1 rounded-md px-2 py-1", tone, muted && "text-muted-foreground")}>
      {Icon && <Icon className="size-3.5" />}{children}
    </span>
  );
}
