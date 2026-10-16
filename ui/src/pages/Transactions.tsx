import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { ArrowDownRight, ArrowUpRight, Info, Plus, Search, Trash2 } from "lucide-react";
import { AnimatedItem, AnimatedList } from "@/components/AnimatedList";
import { Segmented } from "@/components/Segmented";
import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";
import { AnimatedMoney } from "@/components/AnimatedMoney";
import { api, DATA_RESTORED, errorText, type Account, type Category, type SharingPrefs, type Transaction, type Visibility } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { VisibilityBadge, VisibilityPicker } from "@/components/Visibility";
import { cn, formatMoney } from "@/lib/utils";
import { useTheme } from "@/hooks/theme";

type RowProps = {
  t: Pick<Transaction, "type" | "amount" | "date" | "merchant" | "description" | "category_name" | "account_name" | "currency" | "visibility">;
  hidden: boolean;
  /** Shown before the date line, e.g. who the item belongs to in the family view. */
  lead?: ReactNode;
  onDelete?: () => void;
  onVisibilityChange?: (v: Visibility) => void;
};

export function TransactionRow({ t, hidden, lead, onDelete, onVisibilityChange }: RowProps) {
  const income = t.type === "income";
  return (
    <div className="group flex items-center gap-3 py-2.5">
      <div className={cn("grid size-8 shrink-0 place-items-center rounded-full", income ? "bg-success/15 text-success" : "bg-secondary text-muted-foreground")}>
        {income ? <ArrowUpRight className="size-4" /> : <ArrowDownRight className="size-4" />}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{t.merchant || t.description || t.category_name || "Untitled"}</div>
        <div className="text-muted-foreground flex items-center gap-1.5 truncate text-xs">
          {lead}
          <span className="truncate">{[t.date, t.category_name, t.account_name].filter(Boolean).join(" · ")}</span>
        </div>
      </div>
      <div className={cn("text-sm font-medium tabular-nums", income && "text-success")}>
        {income ? "+" : "−"}<AnimatedMoney value={t.amount} currency={t.currency || "INR"} hidden={hidden} />
      </div>
      <VisibilityBadge value={t.visibility ?? "private"} onChange={onVisibilityChange} />
      {onDelete && <Button variant="ghost" size="icon" className="size-7 opacity-0 group-hover:opacity-100" onClick={onDelete} title="Delete"><Trash2 /></Button>}
    </div>
  );
}

export function Transactions() {
  const { hideBalances } = useTheme();
  const { fade } = useMotion();
  const [rows, setRows] = useState<Transaction[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [prefs, setPrefs] = useState<SharingPrefs | null>(null);
  const [q, setQ] = useState("");
  const [type, setType] = useState("");
  const [accountId, setAccountId] = useState("");
  const [visibility, setVisibility] = useState<"" | Visibility>("");
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState<"expense" | "income">("expense");
  const [newVisibility, setNewVisibility] = useState<Visibility>("private");
  const [error, setError] = useState("");

  const load = () =>
    api.transactions.getAll({ type: type || undefined, accountId: accountId || undefined, visibility: visibility || undefined }).then(setRows);
  useEffect(() => {
    load();
    // After an undo or restore, reload in place so the restored row slides back in.
    window.addEventListener(DATA_RESTORED, load);
    return () => window.removeEventListener(DATA_RESTORED, load);
  }, [type, accountId, visibility]);
  useEffect(() => {
    api.accounts.getAll().then(setAccounts);
    api.categories.getAll().then(setCategories);
    api.sharing.get().then((s) => setPrefs(s.prefs));
  }, []);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    return s ? rows.filter((t) => [t.merchant, t.description, t.category_name, t.account_name].some((v) => v?.toLowerCase().includes(s))) : rows;
  }, [rows, q]);

  const openDialog = (o: boolean) => {
    if (o) { setNewVisibility(prefs?.default_visibility ?? "private"); setError(""); }
    setOpen(o);
  };

  async function create(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const f = new FormData(e.currentTarget);
    try {
      await api.transactions.create({
        type: kind, amount: String(f.get("amount")), date: String(f.get("date")),
        account_id: String(f.get("account")), category_id: String(f.get("category")) || undefined,
        merchant: String(f.get("merchant")) || undefined, visibility: newVisibility,
      });
      setOpen(false); load();
    } catch (err) { setError(errorText(err)); }
  }

  const changeVisibility = async (id: string, v: Visibility) => {
    await api.transactions.setVisibility(id, v);
    load();
  };

  const today = new Date().toISOString().slice(0, 10);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-56 flex-1">
          <Search className="text-muted-foreground absolute top-2.5 left-2.5 size-4" />
          <Input placeholder="Search merchant, category, account…" className="pl-8" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <NativeSelect className="w-32" value={type} onChange={(e) => setType(e.target.value)}>
          <option value="">All types</option><option value="expense">Expenses</option><option value="income">Income</option>
        </NativeSelect>
        <NativeSelect className="w-40" value={accountId} onChange={(e) => setAccountId(e.target.value)}>
          <option value="">All accounts</option>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
        </NativeSelect>
        <NativeSelect className="w-32" value={visibility} onChange={(e) => setVisibility(e.target.value as "" | Visibility)} aria-label="Visibility">
          <option value="">Any visibility</option><option value="private">Private</option><option value="shared">Shared</option><option value="public">Public</option>
        </NativeSelect>
        <Dialog open={open} onOpenChange={openDialog}>
          <DialogTrigger asChild><Button disabled={!accounts.length} title={accounts.length ? "" : "Add an account first"}><Plus />Add</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>New transaction</DialogTitle></DialogHeader>
            <form onSubmit={create} className="grid gap-4">
              <Segmented value={kind} onChange={setKind} className="flex w-full" itemClassName="py-1.5"
                options={[{ value: "expense", label: "Expense" }, { value: "income", label: "Income" }]} label="Type" />
              <div className="grid grid-cols-2 gap-4">
                <div className="grid gap-2"><Label htmlFor="amount">Amount</Label>
                  {/* The amount box flashes the colour of the chosen type. */}
                  <motion.div key={kind} className="rounded-md"
                    initial={{ boxShadow: `0 0 0 0px ${kind === "income" ? "var(--success)" : "var(--destructive)"}` }}
                    animate={{ boxShadow: [`0 0 0 3px color-mix(in oklab, ${kind === "income" ? "var(--success)" : "var(--destructive)"} 45%, transparent)`, "0 0 0 0px transparent"] }}
                    transition={fade({ duration: 0.7, ease: "easeOut" })}>
                    <Input id="amount" name="amount" type="number" step="0.01" min="0" required autoFocus />
                  </motion.div>
                </div>
                <div className="grid gap-2"><Label htmlFor="date">Date</Label><Input id="date" name="date" type="date" defaultValue={today} required /></div>
              </div>
              <div className="grid gap-2"><Label htmlFor="merchant">Merchant / description</Label><Input id="merchant" name="merchant" /></div>
              <div className="grid grid-cols-2 gap-4">
                <div className="grid gap-2"><Label htmlFor="account">Account</Label>
                  <NativeSelect id="account" name="account">{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></div>
                <div className="grid gap-2"><Label htmlFor="category">Category</Label>
                  <NativeSelect id="category" name="category"><option value="">Uncategorised</option>
                    {categories.filter((c) => c.type === kind).map((c) => <option key={c.id} value={c.id}>{c.icon} {c.name}</option>)}</NativeSelect></div>
              </div>
              <div className="grid gap-2">
                <Label>Who can see this</Label>
                <VisibilityPicker value={newVisibility} onChange={setNewVisibility} />
                <SharingHint prefs={prefs} visibility={newVisibility} />
              </div>
              {error && <p className="text-destructive text-sm">{error}</p>}
              <DialogFooter><Button type="submit">Save</Button></DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </div>
      <Card className="py-2">
        <CardContent className="divide-y">
          {filtered.length === 0 && <p className="text-muted-foreground py-10 text-center text-sm">No transactions match.</p>}
          <AnimatedList>
            {filtered.map((t) => (
              <AnimatedItem key={t.id}>
                <TransactionRow t={t} hidden={hideBalances}
                  onVisibilityChange={(v) => changeVisibility(t.id, v)}
                  onDelete={() => api.transactions.delete(t.id).then(load)} />
              </AnimatedItem>
            ))}
          </AnimatedList>
        </CardContent>
      </Card>
    </div>
  );
}

/** Explains what the chosen visibility will actually do given the member's sharing settings. */
function SharingHint({ prefs, visibility }: { prefs: SharingPrefs | null; visibility: Visibility }) {
  if (visibility === "private" || !prefs) return null;
  let text: string | null = null;
  if (!prefs.enabled) text = "Family sharing is off, so this stays private until you turn it on in Settings.";
  else if (visibility === "shared" && prefs.shared_with.length === 0) text = "You haven't picked anyone to share with yet (Settings → Family sharing).";
  if (!text) return null;
  return <p className="text-muted-foreground flex items-start gap-1.5 text-xs"><Info className="mt-0.5 size-3.5 shrink-0" />{text}</p>;
}
