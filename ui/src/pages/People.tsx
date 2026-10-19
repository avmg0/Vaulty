import { useEffect, useMemo, useState, type FormEvent } from "react";
import { ArrowDownLeft, ArrowUpRight, Copy, Pencil, Plus, Receipt, Trash2, UserPlus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type Account, type Category, type Person, type PersonEvent } from "@/api";
import { UserAvatar } from "@/components/UserAvatar";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate, formatMoney } from "@/lib/utils";

/** The four things that happen with a person, and which way the money moves for each. */
type Kind = "lent" | "borrowed" | "repaid_me" | "i_repaid";
const FLOW: Record<Kind, "out" | "in"> = { lent: "out", i_repaid: "out", borrowed: "in", repaid_me: "in" };
/** Accounts money can come from or go into. */
const SPENDABLE = ["checking", "savings", "cash", "business", "credit", "prepaid", "other"];
/** Avatar colours dark enough for white initials (4.5:1 or more). */
const COLORS = ["#1d4ed8", "#6d28d9", "#be185d", "#c2410c", "#15803d", "#0e7490", "#475569"];
const colorOf = (name: string) => COLORS[[...name].reduce((s, c) => s + c.charCodeAt(0), 0) % COLORS.length];
const today = () => { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; };

export function PeoplePage() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [people, setPeople] = useState<Person[]>([]);
  const [totals, setTotals] = useState<{ currency: string; owed_to_me: number; i_owe: number }[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [history, setHistory] = useState<PersonEvent[]>([]);
  const [editing, setEditing] = useState<Partial<Person> | null>(null);
  const [recording, setRecording] = useState<{ kind: Kind; amount?: string } | null>(null);
  const [splitting, setSplitting] = useState(false);
  const [note, setNote] = useState("");
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  const reload = () => setVersion((v) => v + 1);

  useEffect(() => { window.addEventListener(DATA_RESTORED, reload); return () => window.removeEventListener(DATA_RESTORED, reload); }, []);
  useEffect(() => {
    api.people.list().then((l) => { setPeople(l); setSelected((s) => s && l.some((p) => p.id === s) ? s : l[0]?.id ?? null); }).catch((e) => setError(errorText(e)));
    api.people.totals().then(setTotals).catch(() => {});
    api.accounts.getAll().then((a) => setAccounts(a.filter((x) => SPENDABLE.includes(x.type)))).catch(() => {});
    api.categories.getAll().then(setCategories).catch(() => {});
  }, [version]);
  useEffect(() => { if (selected) api.people.history(selected).then(setHistory).catch(() => setHistory([])); else setHistory([]); }, [selected, version]);

  const person = people.find((p) => p.id === selected) ?? null;
  const money = (n: number, cur: string) => formatMoney(n, cur, hideBalances);
  const main = person?.balances[0];

  const copyReminder = async () => {
    if (!person || !main) return;
    const text = t("people.reminderText", { name: person.name.split(" ")[0], amount: formatMoney(main.amount, main.currency) });
    try { await navigator.clipboard.writeText(text); setNote(t("people.copied")); } catch { setNote(text); }
  };
  const remove = async () => {
    if (!person) return;
    try { await api.people.delete(person.id); setSelected(null); reload(); } catch (e) { setError(errorText(e)); }
  };
  const removeEvent = async (id: string) => {
    try { await api.transactions.delete(id); window.dispatchEvent(new Event(DATA_RESTORED)); } catch (e) { setError(errorText(e)); }
  };

  return (
    <div className="max-w-6xl space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex flex-wrap gap-6">
          {(totals.length ? totals : [{ currency: accounts[0]?.currency ?? "INR", owed_to_me: 0, i_owe: 0 }]).map((x) => (
            <div key={x.currency} className="flex gap-6">
              <div><div className="text-muted-foreground text-xs">{t("people.owedToMe")}</div><div className="text-success text-2xl font-semibold tabular-nums">{money(x.owed_to_me, x.currency)}</div></div>
              <div><div className="text-muted-foreground text-xs">{t("people.iOwe")}</div><div className="text-2xl font-semibold tabular-nums">{money(x.i_owe, x.currency)}</div></div>
            </div>
          ))}
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" disabled={!people.length || !accounts.length} onClick={() => setSplitting(true)}><Receipt />{t("people.split")}</Button>
          <Button size="sm" onClick={() => setEditing({ name: "" })}><UserPlus />{t("people.add")}</Button>
        </div>
      </div>
      {error && <p className="text-destructive text-sm">{error}</p>}

      {people.length === 0 ? (
        <Card><CardContent className="space-y-3 py-10 text-center">
          <p className="font-medium">{t("people.emptyTitle")}</p>
          <p className="text-muted-foreground mx-auto max-w-md text-sm">{t("people.emptyDesc")}</p>
          <Button onClick={() => setEditing({ name: "" })}><Plus />{t("people.add")}</Button>
        </CardContent></Card>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[20rem_minmax(0,1fr)]">
          <Card className="gap-0 py-2">
            <ul aria-label={t("nav.people")}>
              {people.map((p) => {
                const b = p.balances[0];
                return (
                  <li key={p.id}>
                    <button type="button" onClick={() => setSelected(p.id)} aria-current={selected === p.id}
                      className={cn("hover:bg-accent/60 flex w-full items-center gap-3 px-4 py-2.5 text-left", selected === p.id && "bg-accent")}>
                      <UserAvatar name={p.name} color={colorOf(p.name)} />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm font-medium">{p.name}</span>
                        <BalanceText b={b} money={money} className="text-xs" />
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </Card>

          {person && (
            <Card>
              <CardHeader className="flex-row flex-wrap items-start justify-between gap-3">
                <div className="flex items-center gap-3">
                  <UserAvatar name={person.name} color={colorOf(person.name)} className="size-12 text-base" />
                  <div>
                    <CardTitle className="text-lg">{person.name}</CardTitle>
                    {person.phone && <div className="text-muted-foreground text-xs">{person.phone}</div>}
                  </div>
                </div>
                <div className="flex gap-1">
                  <Button variant="ghost" size="icon" aria-label={t("people.edit")} title={t("people.edit")} onClick={() => setEditing(person)}><Pencil /></Button>
                  <Button variant="ghost" size="icon" aria-label={t("people.remove")} title={t("people.remove")} onClick={remove}><Trash2 /></Button>
                </div>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="flex flex-wrap items-end justify-between gap-3">
                  <BalanceText b={main} money={money} large />
                  {main && main.amount > 0 && <Button variant="ghost" size="sm" onClick={copyReminder}><Copy />{t("people.copyReminder")}</Button>}
                </div>
                {note && <p className="text-muted-foreground text-xs">{note}</p>}
                {person.notes && <p className="text-muted-foreground text-sm">{person.notes}</p>}
                <div className="grid grid-cols-2 gap-2 xl:grid-cols-4">
                  {(["lent", "repaid_me", "borrowed", "i_repaid"] as Kind[]).map((k) => {
                    // Settling up: pre-fill the whole balance on the action that clears it.
                    const settles = main && ((k === "repaid_me" && main.amount > 0) || (k === "i_repaid" && main.amount < 0));
                    return (
                      <Button key={k} variant={settles ? "default" : "outline"} size="sm" disabled={!accounts.length}
                        onClick={() => setRecording({ kind: k, amount: settles ? String(Math.abs(main!.amount)) : undefined })}>
                        {FLOW[k] === "out" ? <ArrowUpRight /> : <ArrowDownLeft />}{t(`people.kinds.${k}`)}
                      </Button>
                    );
                  })}
                </div>
                <div>
                  <h2 className="mb-1 text-sm font-medium">{t("people.history")}</h2>
                  {history.length === 0 ? <p className="text-muted-foreground text-sm">{t("people.noHistory")}</p> : (
                    <ul className="divide-y">
                      {history.map((h) => (
                        <li key={h.entry_id} className="group flex items-center gap-3 py-2 text-sm">
                          <span className="text-muted-foreground w-16 shrink-0 text-xs tabular-nums">{formatDate(h.date, { day: "numeric", month: "short" })}</span>
                          <span className="min-w-0 flex-1 truncate">{t(`people.what.${h.what}`)}{(h.description || h.merchant) && <span className="text-muted-foreground"> · {h.description || h.merchant}</span>}</span>
                          <span className={cn("shrink-0 tabular-nums", h.amount > 0 ? "text-success" : "")}>{h.amount > 0 ? "+" : "−"}{money(Math.abs(h.amount), h.currency)}</span>
                          <span className="text-muted-foreground hidden w-24 shrink-0 text-end text-xs tabular-nums sm:block">{money(h.balance, h.currency)}</span>
                          <Button variant="ghost" size="icon" className="size-7 opacity-0 group-hover:opacity-100 focus-visible:opacity-100" aria-label={t("people.deleteEvent")} onClick={() => removeEvent(h.entry_id)}><Trash2 className="size-3.5" /></Button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </CardContent>
            </Card>
          )}
        </div>
      )}

      {editing && <PersonDialog value={editing} onClose={() => setEditing(null)} onSaved={(p) => { setEditing(null); setSelected(p.id); reload(); }} />}
      {recording && person && <RecordDialog person={person} kind={recording.kind} amount={recording.amount} accounts={accounts} onClose={() => setRecording(null)} onSaved={() => { setRecording(null); window.dispatchEvent(new Event(DATA_RESTORED)); }} />}
      {splitting && <SplitDialog people={people} accounts={accounts} categories={categories} onClose={() => setSplitting(false)} onSaved={() => { setSplitting(false); window.dispatchEvent(new Event(DATA_RESTORED)); }} />}
    </div>
  );
}

function BalanceText({ b, money, large, className }: { b?: { amount: number; currency: string }; money: (n: number, c: string) => string; large?: boolean; className?: string }) {
  const { t } = useTranslation();
  if (!b || b.amount === 0) return <span className={cn("text-muted-foreground", large && "text-lg", className)}>{t("people.settled")}</span>;
  const amount = money(Math.abs(b.amount), b.currency);
  return b.amount > 0
    ? <span className={cn("text-success", large && "text-2xl font-semibold", className)}>{t("people.owesYou", { amount })}</span>
    : <span className={cn("text-amber-800 dark:text-amber-300", large && "text-2xl font-semibold", className)}>{t("people.youOwe", { amount })}</span>;
}

function PersonDialog({ value, onClose, onSaved }: { value: Partial<Person>; onClose: () => void; onSaved: (p: Person) => void }) {
  const { t } = useTranslation();
  const [name, setName] = useState(value.name ?? "");
  const [phone, setPhone] = useState(value.phone ?? "");
  const [notes, setNotes] = useState(value.notes ?? "");
  const [error, setError] = useState("");
  const save = async (e: FormEvent) => {
    e.preventDefault(); setError("");
    const body = { name, phone: phone || undefined, notes: notes || undefined };
    try { onSaved(await (value.id ? api.people.update(value.id, body) : api.people.create(body))); } catch (err) { setError(errorText(err)); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader><DialogTitle>{value.id ? t("people.edit") : t("people.add")}</DialogTitle><DialogDescription>{t("people.addDesc")}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="grid gap-3">
          <div className="grid gap-1.5"><Label htmlFor="p-name">{t("people.name")}</Label><Input id="p-name" autoFocus maxLength={60} value={name} onChange={(e) => setName(e.target.value)} /></div>
          <div className="grid gap-1.5"><Label htmlFor="p-phone">{t("people.phone")}</Label><Input id="p-phone" type="tel" maxLength={30} value={phone} onChange={(e) => setPhone(e.target.value)} /></div>
          <div className="grid gap-1.5"><Label htmlFor="p-notes">{t("people.notes")}</Label><Input id="p-notes" maxLength={500} value={notes} onChange={(e) => setNotes(e.target.value)} /></div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button><Button disabled={!name.trim()}>{t("common.save")}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function RecordDialog({ person, kind: initialKind, amount: initialAmount, accounts, onClose, onSaved }: {
  person: Person; kind: Kind; amount?: string; accounts: Account[]; onClose: () => void; onSaved: () => void;
}) {
  const { t } = useTranslation();
  const [kind, setKind] = useState(initialKind);
  const [amount, setAmount] = useState(initialAmount ?? "");
  const [date, setDate] = useState(today());
  const [accountId, setAccountId] = useState(accounts[0]?.id ?? "");
  const [what, setWhat] = useState("");
  const [error, setError] = useState("");
  const save = async (e: FormEvent) => {
    e.preventDefault(); setError("");
    try {
      await api.transactions.create({ type: "transfer", person_id: person.id, flow: FLOW[kind], account_id: accountId, amount, date, description: what.trim() || undefined, visibility: "private" });
      onSaved();
    } catch (err) { setError(errorText(err)); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader><DialogTitle>{t(`people.titles.${kind}`, { name: person.name })}</DialogTitle><DialogDescription>{t(`people.descs.${kind}`)}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="grid gap-3">
          <div className="grid gap-1.5"><Label htmlFor="r-kind">{t("people.what.label")}</Label>
            <NativeSelect id="r-kind" value={kind} onChange={(e) => setKind(e.target.value as Kind)}>
              {(["lent", "repaid_me", "borrowed", "i_repaid"] as Kind[]).map((k) => <option key={k} value={k}>{t(`people.kinds.${k}`)}</option>)}
            </NativeSelect></div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-1.5"><Label htmlFor="r-amount">{t("tx.amount")}</Label><Input id="r-amount" autoFocus inputMode="decimal" value={amount} onChange={(e) => setAmount(e.target.value)} /></div>
            <div className="grid gap-1.5"><Label htmlFor="r-date">{t("calendar.date")}</Label><Input id="r-date" type="date" value={date} onChange={(e) => setDate(e.target.value)} /></div>
          </div>
          <div className="grid gap-1.5"><Label htmlFor="r-account">{FLOW[kind] === "out" ? t("people.fromAccount") : t("people.intoAccount")}</Label>
            <NativeSelect id="r-account" value={accountId} onChange={(e) => setAccountId(e.target.value)}>
              {accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
            </NativeSelect></div>
          <div className="grid gap-1.5"><Label htmlFor="r-what">{t("people.forWhat")}</Label><Input id="r-what" maxLength={200} placeholder={t("people.forWhatHint")} value={what} onChange={(e) => setWhat(e.target.value)} /></div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button><Button disabled={!amount || !accountId}>{t("common.save")}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** "Dinner ₹3,000, I paid, split with John and Jane": your share is spending; theirs is owed to you. */
function SplitDialog({ people, accounts, categories, onClose, onSaved }: {
  people: Person[]; accounts: Account[]; categories: Category[]; onClose: () => void; onSaved: () => void;
}) {
  const { t } = useTranslation();
  const [what, setWhat] = useState("");
  const [total, setTotal] = useState("");
  const [date, setDate] = useState(today());
  const [accountId, setAccountId] = useState(accounts[0]?.id ?? "");
  const [categoryId, setCategoryId] = useState("");
  const [withMe, setWithMe] = useState(true);
  const [chosen, setChosen] = useState<string[]>([]);
  const [custom, setCustom] = useState<Record<string, string> | null>(null);
  const [error, setError] = useState("");
  const cents = Math.round(Number(total) * 100) || 0;
  // Equal shares in whole paise; any leftover paisa goes to you (or the first person).
  const shares = useMemo(() => {
    const n = chosen.length + (withMe ? 1 : 0);
    if (!n || !cents) return { me: 0, each: {} as Record<string, number> };
    const base = Math.floor(cents / n), extra = cents - base * n;
    const each = Object.fromEntries(chosen.map((id, i) => [id, base + (!withMe && i === 0 ? extra : 0)]));
    return { me: withMe ? base + extra : 0, each };
  }, [chosen, withMe, cents]);
  const partOf = (id: string) => custom ? Math.round(Number(custom[id] || 0) * 100) : shares.each[id] ?? 0;
  const othersTotal = chosen.reduce((s, id) => s + partOf(id), 0);
  const mine = custom ? cents - othersTotal : shares.me;
  const toggle = (id: string) => setChosen((c) => c.includes(id) ? c.filter((x) => x !== id) : [...c, id]);
  const fmt = (c: number) => (c / 100).toFixed(2);
  const ready = cents > 0 && chosen.length > 0 && mine >= 0 && accountId && (!custom || mine >= 0);

  const save = async (e: FormEvent) => {
    e.preventDefault(); setError("");
    const parts = [
      ...(mine > 0 ? [{ category_id: categoryId || null, amount: fmt(mine) }] : []),
      ...chosen.filter((id) => partOf(id) > 0).map((id) => ({ person_id: id, amount: fmt(partOf(id)) })),
    ];
    try {
      // Paying entirely for one person is simply lending to them.
      if (parts.length === 1 && "person_id" in parts[0]) {
        await api.transactions.create({ type: "transfer", person_id: parts[0].person_id, flow: "out", account_id: accountId, amount: fmt(cents), date, description: what.trim() || undefined, visibility: "private" });
      } else {
        await api.transactions.create({ type: "expense", account_id: accountId, amount: fmt(cents), date, merchant: what.trim() || undefined, visibility: "private", splits: parts });
      }
      onSaved();
    } catch (err) { setError(errorText(err)); }
  };
  const cur = accounts.find((a) => a.id === accountId)?.currency ?? "INR";
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader><DialogTitle>{t("people.split")}</DialogTitle><DialogDescription>{t("people.splitDesc")}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="grid gap-3">
          <div className="grid grid-cols-[1fr_8rem] gap-3">
            <div className="grid gap-1.5"><Label htmlFor="s-what">{t("people.splitWhat")}</Label><Input id="s-what" autoFocus maxLength={120} placeholder={t("people.splitWhatHint")} value={what} onChange={(e) => setWhat(e.target.value)} /></div>
            <div className="grid gap-1.5"><Label htmlFor="s-total">{t("people.splitTotal")}</Label><Input id="s-total" inputMode="decimal" value={total} onChange={(e) => setTotal(e.target.value)} /></div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-1.5"><Label htmlFor="s-account">{t("people.paidFrom")}</Label>
              <NativeSelect id="s-account" value={accountId} onChange={(e) => setAccountId(e.target.value)}>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></div>
            <div className="grid gap-1.5"><Label htmlFor="s-date">{t("calendar.date")}</Label><Input id="s-date" type="date" value={date} onChange={(e) => setDate(e.target.value)} /></div>
          </div>
          <fieldset className="grid gap-1.5">
            <legend className="mb-1 text-sm font-medium">{t("people.splitWith")}</legend>
            <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={withMe} onChange={(e) => setWithMe(e.target.checked)} />{t("people.me")}
              <span className="text-muted-foreground ms-auto tabular-nums">{formatMoney(mine / 100, cur)}</span></label>
            {people.map((p) => (
              <label key={p.id} className="flex items-center gap-2 text-sm">
                <input type="checkbox" checked={chosen.includes(p.id)} onChange={() => toggle(p.id)} />{p.name}
                {chosen.includes(p.id) && (custom
                  ? <Input aria-label={t("people.shareOf", { name: p.name })} className="ms-auto h-7 w-24 text-end" inputMode="decimal" value={custom[p.id] ?? ""} onChange={(e) => setCustom({ ...custom, [p.id]: e.target.value })} />
                  : <span className="text-muted-foreground ms-auto tabular-nums">{formatMoney(partOf(p.id) / 100, cur)}</span>)}
              </label>
            ))}
            <button type="button" className="text-muted-foreground hover:text-foreground justify-self-start text-xs underline-offset-2 hover:underline"
              onClick={() => setCustom(custom ? null : Object.fromEntries(chosen.map((id) => [id, fmt(partOf(id))])))}>
              {custom ? t("people.splitEqually") : t("people.splitCustom")}
            </button>
          </fieldset>
          {mine > 0 && (
            <div className="grid gap-1.5"><Label htmlFor="s-cat">{t("people.myShareCategory")}</Label>
              <NativeSelect id="s-cat" value={categoryId} onChange={(e) => setCategoryId(e.target.value)}>
                <option value="">{t("dashboard.uncategorised")}</option>
                {categories.filter((c) => c.type === "expense").map((c) => <option key={c.id} value={c.id}>{c.icon} {c.name}</option>)}
              </NativeSelect></div>
          )}
          {mine < 0 && <p className="text-destructive text-sm">{t("people.splitOver")}</p>}
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button><Button disabled={!ready}>{t("common.save")}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
