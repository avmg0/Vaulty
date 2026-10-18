import { useEffect, useState, type FormEvent } from "react";
import { EyeOff, Info, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import { api, errorText, type Account, type Category, type SharingPrefs, type Transaction, type Visibility } from "@/api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Segmented } from "@/components/Segmented";
import { VisibilityPicker } from "@/components/Visibility";
import { TagInput, splitTags } from "@/components/TagInput";
import { useMotion } from "@/lib/motion";

type Kind = "expense" | "income" | "transfer";
const localToday = () => { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; };

/**
 * Adds a transaction, or edits one when `initial` is given. Editing sends the whole transaction
 * (the core rewrites its ledger postings), so changing type, amount or accounts is safe.
 */
export function TransactionDialog({ open, onOpenChange, initial, accounts, categories, prefs, startHidden, tagSuggestions, onSaved }: {
  open: boolean; onOpenChange: (o: boolean) => void; initial: Transaction | null; accounts: Account[]; categories: Category[];
  prefs: SharingPrefs | null; startHidden?: boolean; tagSuggestions?: string[]; onSaved: (t: Transaction) => void;
}) {
  const { t } = useTranslation();
  const { fade } = useMotion();
  const editing = !!initial;
  const [kind, setKind] = useState<Kind>("expense");
  const [amount, setAmount] = useState("");
  const [date, setDate] = useState(localToday());
  const [merchant, setMerchant] = useState("");
  const [accountId, setAccountId] = useState("");
  const [toAccountId, setToAccountId] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const [notes, setNotes] = useState("");
  const [tags, setTags] = useState<string[]>([]);
  const [visibility, setVisibility] = useState<Visibility>("private");
  const [hide, setHide] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  // Fill the form each time it opens.
  useEffect(() => {
    if (!open) return;
    const first = accounts[0]?.id ?? "";
    setKind(initial?.type ?? "expense");
    setAmount(initial ? String(initial.amount) : "");
    setDate(initial?.date ?? localToday());
    setMerchant(initial?.merchant ?? "");
    setAccountId(initial?.account_id ?? first);
    setToAccountId(initial?.to_account_id ?? accounts.find((a) => a.id !== (initial?.account_id ?? first))?.id ?? "");
    setCategoryId(initial?.category_id ?? "");
    setNotes(initial?.notes ?? "");
    setTags(splitTags(initial?.tags));
    setVisibility(initial?.visibility ?? prefs?.default_visibility ?? "private");
    setHide(!!startHidden);
    setError("");
  }, [open, initial]);

  const isHidden = editing ? initial?.hidden === 1 : hide;
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    const body = {
      type: kind, amount, date, account_id: accountId, merchant: merchant.trim() || undefined,
      to_account_id: kind === "transfer" ? toAccountId : undefined,
      category_id: kind === "transfer" ? undefined : categoryId || undefined,
      notes: notes.trim() || undefined, tags: tags.length ? tags.join(", ") : undefined,
      visibility: isHidden ? "private" as const : visibility,
    };
    try {
      const saved = initial ? await api.transactions.update(initial.id, body) : await api.transactions.create({ ...body, hidden: hide });
      onSaved(saved);
      onOpenChange(false);
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  const tone = kind === "income" ? "var(--success)" : kind === "expense" ? "var(--destructive)" : "var(--primary)";
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92vh] overflow-y-auto">
        <DialogHeader><DialogTitle>{editing ? t("tx.editTitle") : t("tx.newTitle")}</DialogTitle></DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <Segmented value={kind} onChange={setKind} className="flex w-full" itemClassName="py-1.5" label={t("tx.type")}
            options={[{ value: "expense" as Kind, label: t("tx.expense") }, { value: "income" as Kind, label: t("tx.income") },
              // A transfer needs two accounts.
              ...(accounts.length > 1 || kind === "transfer" ? [{ value: "transfer" as Kind, label: t("tx.transfer") }] : [])]} />
          <div className="grid grid-cols-2 gap-4">
            <div className="grid gap-2"><Label htmlFor="amount">{t("tx.amount")}</Label>
              {/* The amount box flashes the colour of the chosen type. */}
              <motion.div key={kind} className="rounded-md"
                initial={{ boxShadow: `0 0 0 0px ${tone}` }}
                animate={{ boxShadow: [`0 0 0 3px color-mix(in oklab, ${tone} 45%, transparent)`, "0 0 0 0px transparent"] }}
                transition={fade({ duration: 0.7, ease: "easeOut" })}>
                <Input id="amount" type="number" step="0.01" min="0" required autoFocus value={amount} onChange={(e) => setAmount(e.target.value)} />
              </motion.div>
            </div>
            <div className="grid gap-2"><Label htmlFor="date">{t("tx.date")}</Label><Input id="date" type="date" value={date} onChange={(e) => setDate(e.target.value)} required /></div>
          </div>
          <div className="grid gap-2"><Label htmlFor="merchant">{kind === "transfer" ? t("tx.transferNote") : t("tx.merchant")}</Label>
            <Input id="merchant" value={merchant} onChange={(e) => setMerchant(e.target.value)} placeholder={kind === "transfer" ? t("tx.transferPh") : undefined} /></div>
          <div className="grid grid-cols-2 gap-4">
            <div className="grid gap-2"><Label htmlFor="account">{kind === "transfer" ? t("tx.from") : t("tx.account")}</Label>
              <NativeSelect id="account" value={accountId} onChange={(e) => setAccountId(e.target.value)}>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></div>
            {kind === "transfer" ? (
              <div className="grid gap-2"><Label htmlFor="to-account">{t("tx.to")}</Label>
                <NativeSelect id="to-account" value={toAccountId} onChange={(e) => setToAccountId(e.target.value)} required>
                  <option value="" disabled>{t("tx.chooseAccount")}</option>
                  {accounts.filter((a) => a.id !== accountId).map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
                </NativeSelect></div>
            ) : (
              <div className="grid gap-2"><Label htmlFor="category">{t("tx.category")}</Label>
                <NativeSelect id="category" value={categoryId} onChange={(e) => setCategoryId(e.target.value)}><option value="">{t("dashboard.uncategorised")}</option>
                  {categories.filter((c) => c.type === kind).map((c) => <option key={c.id} value={c.id}>{c.icon} {c.name}</option>)}</NativeSelect></div>
            )}
          </div>
          <div className="grid gap-2"><Label htmlFor="tags">{t("tags.label")}</Label><TagInput id="tags" value={tags} onChange={setTags} suggestions={tagSuggestions} /></div>
          <div className="grid gap-2"><Label htmlFor="notes">{t("tx.notes")}</Label>
            <textarea id="notes" rows={2} maxLength={1000} value={notes} onChange={(e) => setNotes(e.target.value)}
              className="border-input placeholder:text-muted-foreground dark:bg-input/30 w-full rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50" /></div>
          {!editing && (
            <label className="flex items-start justify-between gap-4 rounded-lg border p-3">
              <span className="grid gap-0.5"><span className="flex items-center gap-1.5 text-sm font-medium"><EyeOff className="size-3.5" />{t("hidden.hideSwitch")}</span>
                <span className="text-muted-foreground text-xs">{t("hidden.hideSwitchDesc")}</span></span>
              <Switch checked={hide} onCheckedChange={setHide} />
            </label>
          )}
          {!isHidden && (
            <div className="grid gap-2">
              <Label>{t("tx.whoCanSee")}</Label>
              <VisibilityPicker value={visibility} onChange={setVisibility} />
              <SharingHint prefs={prefs} visibility={visibility} />
            </div>
          )}
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>{t("common.cancel")}</Button>
            <Button type="submit" disabled={busy || (kind === "transfer" && !toAccountId)}>{busy && <Loader2 className="animate-spin" />}{editing ? t("cards.saveChanges") : t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Explains what the chosen visibility will actually do given the member's sharing settings. */
function SharingHint({ prefs, visibility }: { prefs: SharingPrefs | null; visibility: Visibility }) {
  const { t } = useTranslation();
  if (visibility === "private" || !prefs) return null;
  let text: string | null = null;
  if (!prefs.enabled) text = t("tx.sharingOff");
  else if (visibility === "shared" && prefs.shared_with.length === 0) text = t("tx.noOneYet");
  if (!text) return null;
  return <p className="text-muted-foreground flex items-start gap-1.5 text-xs"><Info className="mt-0.5 size-3.5 shrink-0" />{text}</p>;
}
