import { useEffect, useState, type FormEvent } from "react";
import { ArrowDownLeft, ArrowUpRight, Briefcase, Building2, Loader2, LogIn, Pencil, Plus, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, BUSINESS_KINDS, errorText, type Account, type BookInfo, type Business, type BusinessInput, type BusinessKind } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Segmented } from "@/components/Segmented";
import { useBook } from "@/hooks/book";
import { useLocale } from "@/hooks/locale";
import { useTheme } from "@/hooks/theme";
import { today } from "@/components/projects/ProjectDialogs";
import { formatMoney } from "@/lib/utils";

/** Indian states and union territories by GST state code. */
const STATES: Record<string, string> = {
  "01": "Jammu and Kashmir", "02": "Himachal Pradesh", "03": "Punjab", "04": "Chandigarh", "05": "Uttarakhand", "06": "Haryana", "07": "Delhi", "08": "Rajasthan",
  "09": "Uttar Pradesh", "10": "Bihar", "11": "Sikkim", "12": "Arunachal Pradesh", "13": "Nagaland", "14": "Manipur", "15": "Mizoram", "16": "Tripura", "17": "Meghalaya",
  "18": "Assam", "19": "West Bengal", "20": "Jharkhand", "21": "Odisha", "22": "Chhattisgarh", "23": "Madhya Pradesh", "24": "Gujarat", "26": "Dadra and Nagar Haveli and Daman and Diu",
  "27": "Maharashtra", "29": "Karnataka", "30": "Goa", "31": "Lakshadweep", "32": "Kerala", "33": "Tamil Nadu", "34": "Puducherry", "35": "Andaman and Nicobar Islands",
  "36": "Telangana", "37": "Andhra Pradesh", "38": "Ladakh",
};
const MONTHS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

export function BusinessPage() {
  const { book } = useBook();
  return book ? <InBusiness book={book} /> : <Businesses />;
}

/** In the personal book: the member's businesses, and starting one. */
function Businesses() {
  const { t } = useTranslation();
  const { businesses, switchTo, refresh } = useBook();
  const [creating, setCreating] = useState(false);
  const [showArchived, setShowArchived] = useState(false);
  const open = businesses.filter((b) => !b.archived_at);
  const archived = businesses.filter((b) => b.archived_at);
  return (
    <div className="mx-auto max-w-4xl space-y-6">
      {businesses.length === 0 ? (
        <Card><CardContent className="space-y-4 py-10 text-center">
          <div className="mx-auto grid size-12 place-items-center rounded-xl bg-indigo-500/10 text-indigo-700 dark:text-indigo-300"><Building2 className="size-5" /></div>
          <div className="space-y-1">
            <p className="font-semibold">{t("business.emptyTitle")}</p>
            <p className="text-muted-foreground mx-auto max-w-lg text-sm">{t("business.emptyBody")}</p>
          </div>
          <Button onClick={() => setCreating(true)}><Plus />{t("business.start")}</Button>
        </CardContent></Card>
      ) : (<>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-muted-foreground max-w-xl text-sm">{t("business.intro")}</p>
          <Button onClick={() => setCreating(true)}><Plus />{t("business.start")}</Button>
        </div>
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {open.map((b) => <BusinessCard key={b.id} b={b} onOpen={() => void switchTo(b.id)} onArchive={() => api.business.archive(b.id, true).then(refresh)} />)}
        </div>
        {archived.length > 0 && (
          <section className="space-y-2">
            <button type="button" className="text-muted-foreground text-sm underline-offset-2 hover:underline" onClick={() => setShowArchived(!showArchived)}>{t("business.archivedN", { count: archived.length })}</button>
            {showArchived && <div className="grid grid-cols-1 gap-4 md:grid-cols-2">{archived.map((b) => <BusinessCard key={b.id} b={b} onRestore={() => api.business.archive(b.id, false).then(refresh)} />)}</div>}
          </section>
        )}
      </>)}
      {creating && <BusinessDialog onClose={() => setCreating(false)} />}
    </div>
  );
}

function BusinessCard({ b, onOpen, onArchive, onRestore }: { b: Business; onOpen?: () => void; onArchive?: () => void; onRestore?: () => void }) {
  const { t } = useTranslation();
  return (
    <Card className="gap-0 py-5">
      <CardContent className="space-y-3">
        <div className="flex items-start gap-3">
          <div className="grid size-10 shrink-0 place-items-center rounded-lg bg-indigo-500/10 text-indigo-700 dark:text-indigo-300"><Briefcase className="size-5" /></div>
          <div className="min-w-0 flex-1">
            <h2 className="truncate font-semibold">{b.name}</h2>
            <p className="text-muted-foreground text-xs">{[t(`business.kinds.${b.kind}`), b.gstin && `GSTIN ${b.gstin}`, b.state_code && STATES[b.state_code]].filter(Boolean).join(" · ")}</p>
          </div>
        </div>
        {b.capital_account_name && <p className="text-muted-foreground text-xs">{t("business.capitalIn", { account: b.capital_account_name })}</p>}
        <div className="flex gap-2 border-t pt-3">
          {onOpen && <Button size="sm" onClick={onOpen}><LogIn />{t("business.open")}</Button>}
          {onArchive && <Button size="sm" variant="ghost" className="ms-auto" onClick={onArchive}>{t("business.archive")}</Button>}
          {onRestore && <Button size="sm" variant="outline" onClick={onRestore}>{t("business.restore")}</Button>}
        </div>
      </CardContent>
    </Card>
  );
}

/** Start a business, or change the open one's details. */
function BusinessDialog({ current, onClose }: { current?: BookInfo; onClose: () => void }) {
  const { t } = useTranslation();
  const { currency: mine, country } = useLocale();
  const { refresh } = useBook();
  const [v, setV] = useState<BusinessInput>(() => current
    ? { name: current.name, kind: current.kind, gstin: current.gstin ?? "", pan: current.pan ?? "", address: current.address ?? "", state_code: current.state_code ?? "", fy_start_month: current.fy_start_month }
    : { name: "", kind: "sole_proprietor", currency: mine, gstin: "", pan: "", address: "", state_code: "", fy_start_month: country === "IN" ? 4 : 1 });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (patch: Partial<BusinessInput>) => setV((o) => ({ ...o, ...patch }));
  const india = (current ? true : v.currency === "INR");
  const gstState = v.gstin && v.gstin.length >= 2 ? v.gstin.slice(0, 2) : "";
  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    try {
      if (current) await api.business.update(v); else await api.business.create(v);
      await refresh();
      onClose();
    } catch (err) { setError(errorText(err)); setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{current ? t("business.editTitle") : t("business.newTitle")}</DialogTitle>
          <DialogDescription>{current ? t("business.editDesc") : t("business.newDesc")}</DialogDescription>
        </DialogHeader>
        <form onSubmit={save} className="grid gap-4">
          <div className="grid gap-1.5"><Label htmlFor="b-name">{t("business.name")}</Label><Input id="b-name" required autoFocus maxLength={100} value={v.name} onChange={(e) => set({ name: e.target.value })} /></div>
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="grid gap-1.5"><Label htmlFor="b-kind">{t("business.kind")}</Label>
              <NativeSelect id="b-kind" value={v.kind} onChange={(e) => set({ kind: e.target.value as BusinessKind })}>
                {BUSINESS_KINDS.map((k) => <option key={k} value={k}>{t(`business.kinds.${k}`)}</option>)}
              </NativeSelect>
            </div>
            <div className="grid gap-1.5"><Label htmlFor="b-fy">{t("business.fyStart")}</Label>
              <NativeSelect id="b-fy" value={v.fy_start_month} onChange={(e) => set({ fy_start_month: Number(e.target.value) })}>
                {MONTHS.map((m) => <option key={m} value={m}>{new Date(2026, m - 1, 1).toLocaleDateString(undefined, { month: "long" })}</option>)}
              </NativeSelect>
            </div>
          </div>
          {!current && (
            <div className="grid gap-1.5"><Label htmlFor="b-currency">{t("business.currency")}</Label>
              <Input id="b-currency" maxLength={3} className="w-28 uppercase" value={v.currency ?? ""} onChange={(e) => set({ currency: e.target.value.toUpperCase() })} />
              <p className="text-muted-foreground text-xs">{t("business.currencyHint")}</p>
            </div>
          )}
          {india && (
            <div className="grid gap-3 sm:grid-cols-2">
              <div className="grid gap-1.5"><Label htmlFor="b-gstin">{t("business.gstin")}</Label>
                <Input id="b-gstin" maxLength={15} className="font-mono uppercase" placeholder="27ABCDE1234F1Z5" value={v.gstin ?? ""} onChange={(e) => set({ gstin: e.target.value.toUpperCase().replace(/\s/g, "") })} />
                {gstState && STATES[gstState] && <p className="text-muted-foreground text-xs">{STATES[gstState]}</p>}
              </div>
              <div className="grid gap-1.5"><Label htmlFor="b-pan">{t("business.pan")}</Label>
                <Input id="b-pan" maxLength={10} className="font-mono uppercase" placeholder="ABCDE1234F" value={v.pan ?? ""} onChange={(e) => set({ pan: e.target.value.toUpperCase().replace(/\s/g, "") })} />
              </div>
              {!v.gstin && (
                <div className="grid gap-1.5 sm:col-span-2"><Label htmlFor="b-state">{t("business.state")}</Label>
                  <NativeSelect id="b-state" value={v.state_code ?? ""} onChange={(e) => set({ state_code: e.target.value })}>
                    <option value="">{t("business.noState")}</option>
                    {Object.entries(STATES).sort((a, b) => a[1].localeCompare(b[1])).map(([c, n]) => <option key={c} value={c}>{n}</option>)}
                  </NativeSelect>
                </div>
              )}
            </div>
          )}
          <div className="grid gap-1.5"><Label htmlFor="b-address">{t("business.address")}</Label>
            <textarea id="b-address" rows={2} maxLength={300} value={v.address ?? ""} onChange={(e) => set({ address: e.target.value })}
              className="border-input dark:bg-input/30 focus-visible:border-ring focus-visible:ring-ring/50 rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:ring-[3px]" />
          </div>
          {!current && <p className="text-muted-foreground text-xs">{t("business.separateNote")}</p>}
          {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={busy || !v.name.trim()}>{busy && <Loader2 className="animate-spin" />}{current ? t("common.save") : t("business.create")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Inside a business book: its details, and money between owner and business. */
function InBusiness({ book }: { book: BookInfo }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const { switchTo } = useBook();
  const [editing, setEditing] = useState(false);
  const [totals, setTotals] = useState<{ currency: string; capital: number; drawings: number; net: number } | null>(null);
  const load = () => { api.business.ownerTotals().then(setTotals).catch(() => {}); };
  useEffect(load, []);
  const m = (n: number) => formatMoney(n, totals?.currency ?? "INR", hideBalances, { maximumFractionDigits: 0, minimumFractionDigits: 0 });
  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <Card>
        <CardHeader className="flex-row flex-wrap items-start justify-between gap-3">
          <div className="space-y-1">
            <CardTitle>{book.name}</CardTitle>
            <CardDescription>{[t(`business.kinds.${book.kind}`), book.gstin && `GSTIN ${book.gstin}`, book.pan && `PAN ${book.pan}`, book.state_code && STATES[book.state_code]].filter(Boolean).join(" · ")}</CardDescription>
            {book.address && <p className="text-muted-foreground text-sm whitespace-pre-line">{book.address}</p>}
          </div>
          <div className="flex gap-2">
            <Button size="sm" variant="outline" onClick={() => setEditing(true)}><Pencil />{t("business.editDetails")}</Button>
            <Button size="sm" variant="ghost" onClick={() => void switchTo(null)}><User />{t("business.backToPersonal")}</Button>
          </div>
        </CardHeader>
      </Card>

      <Card>
        <CardHeader><CardTitle>{t("business.ownerTitle")}</CardTitle><CardDescription>{t("business.ownerDesc")}</CardDescription></CardHeader>
        <CardContent className="space-y-5">
          {totals && (
            <dl className="grid gap-3 sm:grid-cols-3">
              <div className="rounded-lg border p-3"><dt className="text-muted-foreground text-xs">{t("business.capital")}</dt><dd className="text-lg font-semibold tabular-nums">{m(totals.capital)}</dd></div>
              <div className="rounded-lg border p-3"><dt className="text-muted-foreground text-xs">{t("business.drawings")}</dt><dd className="text-lg font-semibold tabular-nums">{m(totals.drawings)}</dd></div>
              <div className="bg-muted/40 rounded-lg border p-3"><dt className="text-muted-foreground text-xs">{t("business.netIn")}</dt><dd className="text-lg font-semibold tabular-nums">{m(totals.net)}</dd></div>
            </dl>
          )}
          <OwnerMoneyForm onDone={load} />
        </CardContent>
      </Card>
      {editing && <BusinessDialog current={book} onClose={() => setEditing(false)} />}
    </div>
  );
}

function OwnerMoneyForm({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation();
  const [direction, setDirection] = useState<"in" | "out">("in");
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [personal, setPersonal] = useState<Account[]>([]);
  const [account, setAccount] = useState("");
  const [personalAccount, setPersonalAccount] = useState("");
  const [amount, setAmount] = useState("");
  const [date, setDate] = useState(today());
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    api.accounts.getAll().then((a) => { const ok = a.filter((x) => x.ledger_kind === "asset"); setAccounts(ok); setAccount(ok[0]?.id ?? ""); }).catch(() => {});
    api.business.personalAccounts().then((a) => setPersonal(a.filter((x) => x.ledger_kind === "asset" && x.type !== "business"))).catch(() => {});
  }, []);
  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError(""); setMsg("");
    try {
      const r = await api.business.ownerMoney({ direction, account_id: account, amount, date, note, ...(personalAccount ? { personal_account_id: personalAccount } : {}) });
      setMsg(r.personal_entry ? t("business.recordedBoth") : t("business.recorded"));
      setAmount(""); setNote(""); onDone();
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  if (accounts.length === 0) return <p className="text-muted-foreground text-sm">{t("business.addAccountFirst")}</p>;
  return (
    <form onSubmit={save} className="grid min-w-0 grid-cols-1 gap-3 rounded-lg border border-dashed p-4">
      <Segmented label={t("business.direction")} value={direction} onChange={setDirection} className="max-w-full"
        itemClassName="whitespace-normal" options={[{ value: "in", label: <><ArrowDownLeft className="size-4" />{t("business.putIn")}</> }, { value: "out", label: <><ArrowUpRight className="size-4" />{t("business.tookOut")}</> }]} />
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <div className="grid gap-1.5"><Label htmlFor="om-account">{direction === "in" ? t("business.intoAccount") : t("business.fromAccount")}</Label>
          <NativeSelect id="om-account" value={account} onChange={(e) => setAccount(e.target.value)}>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect>
        </div>
        <div className="grid gap-1.5"><Label htmlFor="om-amount">{t("business.amount")}</Label><Input id="om-amount" inputMode="decimal" required value={amount} onChange={(e) => setAmount(e.target.value.replace(/[^\d.]/g, ""))} /></div>
        <div className="grid gap-1.5"><Label htmlFor="om-date">{t("business.date")}</Label><Input id="om-date" type="date" required value={date} onChange={(e) => setDate(e.target.value)} /></div>
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5"><Label htmlFor="om-personal">{direction === "in" ? t("business.fromPersonal") : t("business.toPersonal")}</Label>
          <NativeSelect id="om-personal" value={personalAccount} onChange={(e) => setPersonalAccount(e.target.value)}>
            <option value="">{t("business.notRecordedPersonal")}</option>
            {personal.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
          </NativeSelect>
        </div>
        <div className="grid gap-1.5"><Label htmlFor="om-note">{t("business.note")}</Label><Input id="om-note" maxLength={200} value={note} onChange={(e) => setNote(e.target.value)} /></div>
      </div>
      <p className="text-muted-foreground text-xs">{t("business.ownerHint")}</p>
      {msg && <p role="status" className="text-sm text-emerald-700 dark:text-emerald-400">{msg}</p>}
      {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
      <div><Button disabled={busy || !amount || !account}>{busy && <Loader2 className="animate-spin" />}{t("business.record")}</Button></div>
    </form>
  );
}
