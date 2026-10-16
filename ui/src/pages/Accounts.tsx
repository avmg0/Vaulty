import { useEffect, useState, type FormEvent } from "react";
import { Banknote, Building2, CreditCard, Landmark, LineChart, Plus, Trash2, Wallet } from "lucide-react";
import { api, errorText, type Account } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { formatMoney } from "@/lib/utils";
import { useTheme } from "@/hooks/theme";

export const ACCOUNT_TYPES = {
  checking: { label: "Checking / Savings", icon: Landmark },
  cash: { label: "Cash", icon: Banknote },
  credit: { label: "Credit card", icon: CreditCard },
  investment: { label: "Investment", icon: LineChart },
  business: { label: "Business", icon: Building2 },
  other: { label: "Other", icon: Wallet },
} as const;

export function Accounts() {
  const { hideBalances } = useTheme();
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [open, setOpen] = useState(false);
  const [newType, setNewType] = useState<keyof typeof ACCOUNT_TYPES>("checking");
  const [error, setError] = useState("");
  const load = () => api.accounts.getAll().then(setAccounts);
  const isCard = newType === "credit";
  useEffect(() => { load(); }, []);

  async function create(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const f = new FormData(e.currentTarget);
    try {
      await api.accounts.create({
        name: String(f.get("name")), type: String(f.get("type")),
        currency: String(f.get("currency")).toUpperCase(), initial_balance: String(f.get("balance") || "0"),
      });
      setOpen(false); setError(""); setNewType("checking");
      load();
    } catch (err) { setError(errorText(err)); }
  }

  return (
    <div className="space-y-4">
      <div className="flex justify-end">
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogTrigger asChild><Button><Plus />Add account</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>New account</DialogTitle></DialogHeader>
            <form onSubmit={create} className="grid gap-4">
              <div className="grid gap-2"><Label htmlFor="name">Name</Label><Input id="name" name="name" placeholder="HDFC Savings" required /></div>
              <div className="grid grid-cols-2 gap-4">
                <div className="grid gap-2"><Label htmlFor="type">Type</Label>
                  <NativeSelect id="type" name="type" value={newType} onChange={(e) => setNewType(e.target.value as keyof typeof ACCOUNT_TYPES)}>{Object.entries(ACCOUNT_TYPES).map(([k, v]) => <option key={k} value={k}>{v.label}</option>)}</NativeSelect>
                </div>
                <div className="grid gap-2"><Label htmlFor="currency">Currency</Label><Input id="currency" name="currency" defaultValue="INR" maxLength={3} required /></div>
              </div>
              <div className="grid gap-2">
                <Label htmlFor="balance">{isCard ? "Amount currently owed" : "Opening balance"}</Label>
                <Input id="balance" name="balance" type="number" step="0.01" min={isCard ? "0" : undefined} defaultValue="0" />
                {isCard && <p className="text-muted-foreground text-xs">Card balances count against your net worth.</p>}
              </div>
              {error && <p className="text-destructive text-sm">{error}</p>}
              <DialogFooter><Button type="submit">Create account</Button></DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </div>
      {accounts.length === 0 && (
        <Card><CardContent className="text-muted-foreground py-10 text-center text-sm">No accounts yet. Add your bank, cash or card accounts to start tracking.</CardContent></Card>
      )}
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {accounts.map((a) => {
          const Icon = ACCOUNT_TYPES[a.type as keyof typeof ACCOUNT_TYPES]?.icon ?? Wallet;
          return (
            <Card key={a.id} className="group">
              <CardHeader className="flex-row items-center gap-3">
                <div className="bg-secondary grid size-9 place-items-center rounded-lg"><Icon className="size-4" /></div>
                <div className="min-w-0 flex-1">
                  <CardTitle className="truncate">{a.name}</CardTitle>
                  <CardDescription>{ACCOUNT_TYPES[a.type as keyof typeof ACCOUNT_TYPES]?.label ?? a.type}</CardDescription>
                </div>
                <Button variant="ghost" size="icon" className="opacity-0 group-hover:opacity-100" title="Archive"
                  onClick={() => api.accounts.delete(a.id).then(load)}><Trash2 /></Button>
              </CardHeader>
              <CardContent>
                {a.ledger_kind === "liability" ? (
                  <>
                    <div className="text-2xl font-semibold tabular-nums">{formatMoney(Math.max(0, -a.current_balance), a.currency, hideBalances)}</div>
                    <div className="text-muted-foreground text-xs">{a.current_balance > 0 ? `In credit by ${formatMoney(a.current_balance, a.currency, hideBalances)}` : "Owed"}</div>
                  </>
                ) : (
                  <div className="text-2xl font-semibold tabular-nums">{formatMoney(a.current_balance, a.currency, hideBalances)}</div>
                )}
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
