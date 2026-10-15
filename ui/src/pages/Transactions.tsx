import { useEffect, useMemo, useState, type FormEvent } from "react";
import { ArrowDownRight, ArrowUpRight, Plus, Search, Trash2 } from "lucide-react";
import { api, type Account, type Category, type Transaction } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn, formatMoney } from "@/lib/utils";
import { useTheme } from "@/hooks/theme";

export function TransactionRow({ t, hidden, onDelete }: { t: Transaction; hidden: boolean; onDelete?: () => void }) {
  const income = t.type === "income";
  return (
    <div className="group flex items-center gap-3 py-2.5">
      <div className={cn("grid size-8 place-items-center rounded-full", income ? "bg-success/15 text-success" : "bg-secondary text-muted-foreground")}>
        {income ? <ArrowUpRight className="size-4" /> : <ArrowDownRight className="size-4" />}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{t.merchant || t.description || t.category_name || "Untitled"}</div>
        <div className="text-muted-foreground truncate text-xs">{[t.date, t.category_name, t.account_name].filter(Boolean).join(" · ")}</div>
      </div>
      <div className={cn("text-sm font-medium tabular-nums", income && "text-success")}>
        {income ? "+" : "−"}{formatMoney(t.amount, "INR", hidden)}
      </div>
      {onDelete && <Button variant="ghost" size="icon" className="opacity-0 group-hover:opacity-100" onClick={onDelete} title="Delete"><Trash2 /></Button>}
    </div>
  );
}

export function Transactions() {
  const { hideBalances } = useTheme();
  const [rows, setRows] = useState<Transaction[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [q, setQ] = useState("");
  const [type, setType] = useState("");
  const [accountId, setAccountId] = useState("");
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState<"expense" | "income">("expense");
  const [error, setError] = useState("");

  const load = () => api.transactions.getAll({ type: type || undefined, accountId: accountId || undefined }).then(setRows);
  useEffect(() => { load(); }, [type, accountId]);
  useEffect(() => { api.accounts.getAll().then(setAccounts); api.categories.getAll().then(setCategories); }, []);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    return s ? rows.filter((t) => [t.merchant, t.description, t.category_name, t.account_name].some((v) => v?.toLowerCase().includes(s))) : rows;
  }, [rows, q]);

  async function create(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const f = new FormData(e.currentTarget);
    try {
      await api.transactions.create({
        type: kind, amount: Number(f.get("amount")), date: String(f.get("date")),
        account_id: String(f.get("account")), category_id: String(f.get("category")) || undefined,
        merchant: String(f.get("merchant")) || undefined,
      });
      setOpen(false); setError(""); load();
    } catch (err) { setError(String(err)); }
  }

  const today = new Date().toISOString().slice(0, 10);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-56 flex-1">
          <Search className="text-muted-foreground absolute top-2.5 left-2.5 size-4" />
          <Input placeholder="Search merchant, category, account…" className="pl-8" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <NativeSelect className="w-36" value={type} onChange={(e) => setType(e.target.value)}>
          <option value="">All types</option><option value="expense">Expenses</option><option value="income">Income</option>
        </NativeSelect>
        <NativeSelect className="w-44" value={accountId} onChange={(e) => setAccountId(e.target.value)}>
          <option value="">All accounts</option>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
        </NativeSelect>
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogTrigger asChild><Button disabled={!accounts.length} title={accounts.length ? "" : "Add an account first"}><Plus />Add</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>New transaction</DialogTitle></DialogHeader>
            <form onSubmit={create} className="grid gap-4">
              <div className="bg-muted grid grid-cols-2 gap-1 rounded-lg p-1">
                {(["expense", "income"] as const).map((k) => (
                  <button type="button" key={k} onClick={() => setKind(k)}
                    className={cn("rounded-md py-1.5 text-sm capitalize transition-colors", kind === k ? "bg-background shadow-sm font-medium" : "text-muted-foreground")}>{k}</button>
                ))}
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div className="grid gap-2"><Label htmlFor="amount">Amount</Label><Input id="amount" name="amount" type="number" step="0.01" min="0" required autoFocus /></div>
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
              {error && <p className="text-destructive text-sm">{error}</p>}
              <DialogFooter><Button type="submit">Save</Button></DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </div>
      <Card className="py-2">
        <CardContent className="divide-y">
          {filtered.length === 0 && <p className="text-muted-foreground py-10 text-center text-sm">No transactions match.</p>}
          {filtered.map((t) => <TransactionRow key={t.id} t={t} hidden={hideBalances} onDelete={() => api.transactions.delete(t.id).then(load)} />)}
        </CardContent>
      </Card>
    </div>
  );
}
