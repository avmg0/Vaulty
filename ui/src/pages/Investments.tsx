import { useEffect, useMemo, useState, type FormEvent } from "react";
import { AlertTriangle, FileUp, Loader2, Plus, RefreshCw, TrendingUp, Trash2 } from "lucide-react";
import { FundPricesCard } from "@/components/FundPricesCard";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, HOLDING_KINDS, type CasPreview, type Holding, type HoldingInput, type HoldingKind, type HoldingTxn, type InvestmentTotal } from "@/api";
import { Segmented } from "@/components/Segmented";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate, formatMoney, formatNumber } from "@/lib/utils";

/** One colour per kind of investment, for the allocation bar (each with its label beside it). */
const KIND_COLOR: Record<HoldingKind, string> = {
  mutual_fund: "bg-sky-600", stock: "bg-violet-600", etf: "bg-teal-600", bond: "bg-amber-600", deposit: "bg-slate-500", gold: "bg-yellow-500", other: "bg-rose-500",
};

const units = (n: number | null) => n == null ? "" : formatNumber(n, { maximumFractionDigits: 4 });
const date = (d: string | null) => d ? formatDate(d, { day: "numeric", month: "short", year: "numeric" }) : "";

/** A gain or loss, coloured, with its sign. */
function Gain({ amount, percent, currency, hidden, className }: { amount: number; percent?: number | null; currency: string; hidden: boolean; className?: string }) {
  const tone = amount > 0 ? "text-emerald-700 dark:text-emerald-300" : amount < 0 ? "text-red-700 dark:text-red-300" : "text-muted-foreground";
  const sign = amount > 0 ? "+" : amount < 0 ? "−" : "";
  return (
    <span className={cn("tabular-nums", tone, className)}>
      {sign}{formatMoney(Math.abs(amount), currency, hidden)}
      {percent != null && !hidden && <span className="ms-1 text-xs">({sign}{formatNumber(Math.abs(percent), { maximumFractionDigits: 2 })}%)</span>}
    </span>
  );
}

export function InvestmentsPage() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [holdings, setHoldings] = useState<Holding[] | null>(null);
  const [totals, setTotals] = useState<InvestmentTotal[]>([]);
  const [show, setShow] = useState<"held" | "sold">("held");
  const [editing, setEditing] = useState<Holding | "new" | null>(null);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  const reload = () => setVersion((v) => v + 1);
  useEffect(() => { window.addEventListener(DATA_RESTORED, reload); return () => window.removeEventListener(DATA_RESTORED, reload); }, []);
  useEffect(() => {
    api.investments.list().then(setHoldings).catch((e) => setError(errorText(e)));
    api.investments.summary().then(setTotals).catch(() => {});
  }, [version]);
  // Today's fund prices, once a day, when turned on (quietly: a failure just leaves the old ones).
  useEffect(() => { api.investments.fundPricesUpdate(false).then((r) => { if (r.updated) reload(); }).catch(() => {}); }, []);
  const [pricesOpen, setPricesOpen] = useState(false);

  const held = (holdings ?? []).filter((h) => !h.closed);
  const sold = (holdings ?? []).filter((h) => h.closed);
  const shown = show === "held" ? held : sold;
  const main = totals[0];
  const allocation = useMemo(() => {
    if (!main) return [];
    const by = new Map<HoldingKind, number>();
    for (const h of held.filter((x) => x.currency === main.currency)) by.set(h.kind, (by.get(h.kind) ?? 0) + h.value);
    const sum = [...by.values()].reduce((a, b) => a + b, 0);
    return sum > 0 ? [...by.entries()].sort((a, b) => b[1] - a[1]).map(([kind, v]) => ({ kind, share: v / sum })) : [];
  }, [held, main]);

  return (
    <div className="max-w-6xl space-y-4">
      {error && <p className="text-destructive text-sm">{error}</p>}

      {holdings && holdings.length === 0 ? (
        <Card><CardContent className="py-10 text-center">
          <TrendingUp className="text-muted-foreground mx-auto mb-3 size-8" aria-hidden />
          <p className="font-medium">{t("investments.emptyTitle")}</p>
          <p className="text-muted-foreground mx-auto mt-1 max-w-md text-sm">{t("investments.emptyDesc")}</p>
          <div className="mt-4 flex flex-wrap justify-center gap-2">
            <Button onClick={() => setImporting(true)}><FileUp />{t("investments.readStatement")}</Button>
            <Button variant="outline" onClick={() => setEditing("new")}><Plus />{t("investments.add")}</Button>
          </div>
        </CardContent></Card>
      ) : (
        <>
          {totals.map((s, i) => (
            <Card key={s.currency}>
              <CardContent className="grid grid-flow-row-dense grid-cols-2 gap-4 py-1 sm:grid-flow-row sm:grid-cols-[minmax(0,1.4fr)_repeat(3,minmax(0,1fr))] sm:items-end">
                <div className="col-span-2 sm:col-span-1">
                  <p className="text-muted-foreground text-sm">{t("investments.worth", { count: s.holdings })}</p>
                  <p className="text-3xl font-semibold tracking-tight tabular-nums">{formatMoney(s.value, s.currency, hideBalances)}</p>
                </div>
                <div><p className="text-muted-foreground text-xs">{t("investments.invested")}</p><p className="tabular-nums">{formatMoney(s.cost, s.currency, hideBalances)}</p></div>
                <div className="col-span-2 sm:col-span-1"><p className="text-muted-foreground text-xs">{t("investments.gain")}</p><Gain amount={s.gain} percent={s.gain_percent} currency={s.currency} hidden={hideBalances} /></div>
                <div>
                  <p className="text-muted-foreground text-xs" title={t("investments.xirrHelp")}>{t("investments.xirr")}</p>
                  <p className="tabular-nums">{s.xirr == null ? <span className="text-muted-foreground">—</span> : hideBalances ? "••" : `${formatNumber(s.xirr, { maximumFractionDigits: 2 })}%`}</p>
                </div>
                {i === 0 && allocation.length > 1 && (
                  <div className="col-span-2 sm:col-span-4">
                    <div className="flex h-2 overflow-hidden rounded-full" role="img" aria-label={allocation.map((a) => `${t(`investments.kinds.${a.kind}`)} ${Math.round(a.share * 100)}%`).join(", ")}>
                      {allocation.map((a) => <div key={a.kind} className={KIND_COLOR[a.kind]} style={{ width: `${a.share * 100}%` }} />)}
                    </div>
                    <ul className="text-muted-foreground mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs" aria-hidden>
                      {allocation.map((a) => <li key={a.kind} className="flex items-center gap-1.5"><span className={cn("size-2 rounded-full", KIND_COLOR[a.kind])} />{t(`investments.kinds.${a.kind}`)} {Math.round(a.share * 100)}%</li>)}
                    </ul>
                  </div>
                )}
              </CardContent>
            </Card>
          ))}

          <div className="flex flex-wrap items-center gap-2">
            <Segmented label={t("investments.showLabel")} value={show} onChange={setShow}
              options={[{ value: "held", label: t("investments.held", { count: held.length }) }, { value: "sold", label: t("investments.sold", { count: sold.length }) }]} />
            <div className="ms-auto flex flex-wrap gap-2">
              <Button variant="outline" onClick={() => setPricesOpen((o) => !o)} aria-expanded={pricesOpen}><RefreshCw />{t("investments.prices")}</Button>
              <Button variant="outline" onClick={() => setImporting(true)}><FileUp />{t("investments.readStatement")}</Button>
              <Button onClick={() => setEditing("new")}><Plus />{t("investments.add")}</Button>
            </div>
          </div>

          {pricesOpen && <Card><CardContent className="py-1"><FundPricesCard compact onUpdated={reload} /></CardContent></Card>}

          <Card className="py-0">
            <div className="overflow-x-auto">
              <table className="w-full min-w-[44rem] text-sm">
                <caption className="sr-only">{t("investments.tableCaption")}</caption>
                <thead className="text-muted-foreground border-b text-xs">
                  <tr>
                    <th scope="col" className="px-4 py-2.5 text-start font-medium">{t("investments.name")}</th>
                    <th scope="col" className="px-3 py-2.5 text-end font-medium">{t("investments.units")}</th>
                    <th scope="col" className="px-3 py-2.5 text-end font-medium">{t("investments.price")}</th>
                    <th scope="col" className="px-3 py-2.5 text-end font-medium">{t("investments.invested")}</th>
                    <th scope="col" className="px-3 py-2.5 text-end font-medium">{t("investments.value")}</th>
                    <th scope="col" className="px-4 py-2.5 text-end font-medium">{t("investments.gain")}</th>
                  </tr>
                </thead>
                <tbody className="divide-y">
                  {shown.map((h) => (
                    <tr key={h.id} className="hover:bg-accent/50 cursor-pointer" onClick={() => setEditing(h)}>
                      <td className="max-w-[22rem] px-4 py-2.5">
                        <button className="block w-full truncate text-start font-medium focus-visible:underline focus-visible:outline-none" onClick={(e) => { e.stopPropagation(); setEditing(h); }}>{h.name}</button>
                        <span className="text-muted-foreground block truncate text-xs">
                          {[t(`investments.kinds.${h.kind}`), h.provider, h.folio && t("investments.folioShort", { folio: h.folio }), h.symbol].filter(Boolean).join(" · ")}
                        </span>
                      </td>
                      <td className="px-3 py-2.5 text-end tabular-nums">{hideBalances ? "••" : units(h.units)}</td>
                      <td className="px-3 py-2.5 text-end tabular-nums">
                        {formatMoney(h.price, h.currency, false, { maximumFractionDigits: 4 })}
                        {h.price_date && <span className="text-muted-foreground block text-xs">{date(h.price_date)}</span>}
                      </td>
                      <td className="px-3 py-2.5 text-end tabular-nums">{formatMoney(h.cost, h.currency, hideBalances)}</td>
                      <td className="px-3 py-2.5 text-end font-medium tabular-nums">{formatMoney(h.value, h.currency, hideBalances)}</td>
                      <td className="px-4 py-2.5 text-end">
                        {h.closed ? <span className="text-muted-foreground text-xs">{t("investments.soldOut")}</span>
                          : <Gain amount={h.gain} percent={h.gain_percent} currency={h.currency} hidden={hideBalances} />}
                        {h.xirr != null && !h.closed && !hideBalances && <span className="text-muted-foreground block text-xs">{t("investments.xirrShort", { rate: formatNumber(h.xirr, { maximumFractionDigits: 1 }) })}</span>}
                      </td>
                    </tr>
                  ))}
                  {shown.length === 0 && <tr><td colSpan={6} className="text-muted-foreground px-4 py-6 text-center">{show === "held" ? t("investments.noneHeld") : t("investments.noneSold")}</td></tr>}
                </tbody>
              </table>
            </div>
          </Card>
        </>
      )}
      {editing && <HoldingDialog holding={editing === "new" ? null : editing} onClose={() => { setEditing(null); reload(); }} />}
      {importing && <StatementDialog onClose={(changed) => { setImporting(false); if (changed) reload(); }} />}
    </div>
  );
}

const EMPTY: HoldingInput = { kind: "mutual_fund", name: "", isin: "", symbol: "", folio: "", provider: "", units: "", price: "", price_date: "", cost: "", notes: "", nominee: "" };

function HoldingDialog({ holding, onClose }: { holding: Holding | null; onClose: () => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [v, setV] = useState<HoldingInput>(() => holding ? {
    kind: holding.kind, name: holding.name, isin: holding.isin ?? "", symbol: holding.symbol ?? "", folio: holding.folio ?? "", provider: holding.provider ?? "",
    currency: holding.currency, units: String(holding.units), price: String(holding.price), price_date: holding.price_date ?? "", cost: String(holding.cost), notes: holding.notes ?? "", nominee: holding.nominee ?? "",
  } : EMPTY);
  const [txns, setTxns] = useState<HoldingTxn[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (p: Partial<HoldingInput>) => setV((x) => ({ ...x, ...p }));
  useEffect(() => { if (holding && holding.transactions > 0) api.investments.transactions(holding.id).then(setTxns).catch(() => setTxns([])); }, [holding]);
  const isFund = v.kind === "mutual_fund";
  const field = (key: keyof HoldingInput, label: string, props: React.ComponentProps<typeof Input> = {}) => (
    <div className="grid gap-1.5"><Label htmlFor={`inv-${key}`}>{label}</Label>
      <Input id={`inv-${key}`} value={String(v[key] ?? "")} onChange={(e) => set({ [key]: e.target.value } as Partial<HoldingInput>)} {...props} /></div>
  );

  const save = async (e: FormEvent) => {
    e.preventDefault(); setBusy(true); setError("");
    try {
      if (holding) await api.investments.update(holding.id, v); else await api.investments.create(v);
      onClose();
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const remove = async () => { if (!holding) return; try { await api.investments.delete(holding.id); onClose(); } catch (err) { setError(errorText(err)); } };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl" onOpenAutoFocus={(e) => { if (holding) e.preventDefault(); }}>
        <DialogHeader>
          <DialogTitle className="pe-6">{holding ? holding.name : t("investments.newTitle")}</DialogTitle>
          <DialogDescription>{holding?.source === "cas" ? t("investments.fromStatement", { date: date(holding.statement_to) }) : t("investments.dialogDesc")}</DialogDescription>
        </DialogHeader>
        {holding && !holding.closed && (
          <dl className="bg-muted/50 grid grid-cols-2 gap-3 rounded-lg p-3 text-sm sm:grid-cols-4">
            <div><dt className="text-muted-foreground text-xs">{t("investments.value")}</dt><dd className="font-medium tabular-nums">{formatMoney(holding.value, holding.currency, hideBalances)}</dd></div>
            <div><dt className="text-muted-foreground text-xs">{t("investments.gain")}</dt><dd><Gain amount={holding.gain} percent={holding.gain_percent} currency={holding.currency} hidden={hideBalances} /></dd></div>
            <div><dt className="text-muted-foreground text-xs">{t("investments.xirr")}</dt><dd className="tabular-nums">{holding.xirr == null ? "—" : hideBalances ? "••" : `${formatNumber(holding.xirr, { maximumFractionDigits: 2 })}%`}</dd></div>
            <div><dt className="text-muted-foreground text-xs">{t("investments.dividends")}</dt><dd className="tabular-nums">{formatMoney(holding.dividends, holding.currency, hideBalances)}</dd></div>
          </dl>
        )}
        <form onSubmit={save} className="grid gap-3">
          <div className="grid grid-cols-[minmax(0,1fr)_10rem] gap-3">
            {field("name", t("investments.name"), { autoFocus: !holding, required: true, maxLength: 160, placeholder: t("investments.namePlaceholder") })}
            <div className="grid gap-1.5"><Label htmlFor="inv-kind">{t("investments.kindLabel")}</Label>
              <NativeSelect id="inv-kind" value={v.kind} onChange={(e) => set({ kind: e.target.value as HoldingKind })}>
                {HOLDING_KINDS.map((k) => <option key={k} value={k}>{t(`investments.kinds.${k}`)}</option>)}
              </NativeSelect></div>
          </div>
          <div className="grid gap-3 sm:grid-cols-3">
            {field("provider", isFund ? t("investments.amc") : t("investments.broker"), { maxLength: 80 })}
            {isFund ? field("folio", t("investments.folio"), { maxLength: 40, autoComplete: "off", spellCheck: false }) : field("symbol", t("investments.symbol"), { maxLength: 40, autoComplete: "off", spellCheck: false })}
            {field("isin", t("investments.isin"), { maxLength: 12, className: "font-mono uppercase", autoComplete: "off", spellCheck: false })}
          </div>
          <div className="grid gap-3 sm:grid-cols-4">
            {field("units", t("investments.units"), { inputMode: "decimal", autoComplete: "off" })}
            {field("price", isFund ? t("investments.nav") : t("investments.price"), { inputMode: "decimal", autoComplete: "off" })}
            {field("price_date", t("investments.priceDate"), { type: "date" })}
            {field("cost", t("investments.invested"), { inputMode: "decimal", autoComplete: "off" })}
          </div>
          {field("nominee", t("investments.nominee"), { maxLength: 200, placeholder: t("investments.nomineeHint") })}
          <div className="grid gap-1.5"><Label htmlFor="inv-notes">{t("investments.notes")}</Label>
            <textarea id="inv-notes" rows={2} maxLength={2000} value={v.notes ?? ""} onChange={(e) => set({ notes: e.target.value })}
              className="border-input dark:bg-input/30 focus-visible:border-ring focus-visible:ring-ring/50 rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:ring-[3px]" /></div>

          {txns && txns.length > 0 && (
            <section className="grid gap-2">
              <h3 className="text-sm font-medium">{t("investments.transactions", { count: txns.length })}</h3>
              <div className="max-h-64 overflow-auto rounded-lg border">
                <table className="w-full min-w-[34rem] text-xs">
                  <caption className="sr-only">{t("investments.transactions", { count: txns.length })}</caption>
                  <thead className="bg-muted/60 text-muted-foreground sticky top-0">
                    <tr>
                      <th scope="col" className="px-3 py-1.5 text-start font-medium">{t("investments.date")}</th>
                      <th scope="col" className="px-3 py-1.5 text-start font-medium">{t("investments.what")}</th>
                      <th scope="col" className="px-3 py-1.5 text-end font-medium">{t("investments.amount")}</th>
                      <th scope="col" className="px-3 py-1.5 text-end font-medium">{t("investments.units")}</th>
                      <th scope="col" className="px-3 py-1.5 text-end font-medium">{t("investments.nav")}</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y">
                    {txns.map((x) => (
                      <tr key={x.id}>
                        <td className="px-3 py-1.5 whitespace-nowrap tabular-nums">{date(x.date)}</td>
                        <td className="px-3 py-1.5"><span className="font-medium">{t(`investments.txnKinds.${x.kind}`, { defaultValue: t("investments.txnKinds.other") })}</span>
                          <span className="text-muted-foreground block max-w-[16rem] truncate" title={x.description}>{x.description}</span></td>
                        <td className="px-3 py-1.5 text-end tabular-nums">{x.amount == null ? "" : formatMoney(x.amount, holding?.currency ?? "INR", hideBalances)}</td>
                        <td className="px-3 py-1.5 text-end tabular-nums">{hideBalances ? "" : units(x.units)}</td>
                        <td className="px-3 py-1.5 text-end tabular-nums">{x.price == null ? "" : formatNumber(x.price, { maximumFractionDigits: 4 })}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          )}
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            {holding && <Button type="button" variant="ghost" className="text-destructive me-auto" onClick={remove}><Trash2 />{t("common.delete")}</Button>}
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={busy || !v.name.trim()}>{busy && <Loader2 className="animate-spin" />}{t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Choose the CAMS or KFintech statement, give its password, see what it holds, then read it in. */
function StatementDialog({ onClose }: { onClose: (changed: boolean) => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [path, setPath] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [preview, setPreview] = useState<CasPreview | null>(null);
  const [done, setDone] = useState<{ added: number; updated: number; transactions: number } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const name = path?.split(/[\\/]/).pop() ?? "";

  const choose = async () => {
    setError("");
    try { const p = await api.files.pick("fund_statement", t("investments.statementFiles")); if (p) { setPath(p); setPreview(null); } } catch (err) { setError(errorText(err)); }
  };
  const look = async (e: FormEvent) => {
    e.preventDefault();
    if (!path) return;
    setBusy(true); setError("");
    try { setPreview(await api.investments.previewStatement(path, password)); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const read = async () => {
    if (!path) return;
    setBusy(true); setError("");
    try { setDone(await api.investments.importStatement(path, password)); setPassword(""); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const unbalanced = preview?.schemes.filter((s) => !s.balanced).length ?? 0;

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(!!done); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("investments.statementTitle")}</DialogTitle>
          <DialogDescription>{t("investments.statementDesc")}</DialogDescription>
        </DialogHeader>
        {done ? (
          <div className="grid gap-3">
            <p className="text-sm" role="status">{t("investments.readDone", { added: done.added, updated: done.updated, count: done.transactions })}</p>
            <DialogFooter><Button autoFocus onClick={() => onClose(true)}>{t("investments.done")}</Button></DialogFooter>
          </div>
        ) : !preview ? (
          <form onSubmit={look} className="grid gap-3">
            <div className="flex flex-wrap items-center gap-2">
              <Button type="button" variant="outline" onClick={choose} autoFocus><FileUp />{path ? t("investments.chooseOther") : t("investments.choose")}</Button>
              {path && <span className="min-w-0 flex-1 truncate text-sm" title={path}>{name}</span>}
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="cas-password">{t("investments.password")}</Label>
              <Input id="cas-password" type="password" autoComplete="off" value={password} onChange={(e) => setPassword(e.target.value)} disabled={!path} />
              <p className="text-muted-foreground text-xs">{t("investments.passwordHint")}</p>
            </div>
            <p className="text-muted-foreground text-xs">{t("investments.stays")}</p>
            {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
            <DialogFooter className="gap-2">
              <Button type="button" variant="outline" onClick={() => onClose(false)}>{t("common.cancel")}</Button>
              <Button disabled={!path || busy}>{busy && <Loader2 className="animate-spin" />}{t("investments.look")}</Button>
            </DialogFooter>
          </form>
        ) : (
          <div className="grid gap-3">
            <p className="text-sm">
              {t("investments.found", { count: preview.schemes.length, from: date(preview.from), to: date(preview.to) })}
              {preview.investor && <span className="text-muted-foreground"> {t("investments.forInvestor", { name: preview.investor })}</span>}
            </p>
            <ul className="divide-y rounded-lg border">
              {preview.schemes.map((s) => (
                <li key={`${s.isin}-${s.folio}`} className="flex items-start gap-3 px-3 py-2">
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium" title={s.name}>{s.name}</p>
                    <p className="text-muted-foreground truncate text-xs">{[s.amc, t("investments.folioShort", { folio: s.folio }), t("investments.newRows", { count: s.new_transactions })].join(" · ")}</p>
                    {!s.balanced && <p className="mt-0.5 flex items-center gap-1 text-xs text-amber-800 dark:text-amber-200"><AlertTriangle className="size-3" aria-hidden />{t("investments.unbalanced")}</p>}
                  </div>
                  <div className="text-end">
                    <Badge variant="secondary">{t(`investments.status.${s.status}`)}</Badge>
                    <p className="mt-1 text-sm tabular-nums">{s.value == null ? "" : formatMoney(s.value, "INR", hideBalances)}</p>
                  </div>
                </li>
              ))}
            </ul>
            {unbalanced > 0 && <p className="text-muted-foreground text-xs">{t("investments.unbalancedHelp")}</p>}
            {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
            <DialogFooter className="gap-2">
              <Button type="button" variant="outline" onClick={() => setPreview(null)}>{t("investments.back")}</Button>
              <Button onClick={read} disabled={busy} autoFocus>{busy && <Loader2 className="animate-spin" />}{t("investments.readIn", { count: preview.new_transactions })}</Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
