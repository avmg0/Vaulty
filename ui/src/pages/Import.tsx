import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, CheckCircle2, Copy, FileSpreadsheet, Loader2, RotateCcw, Upload } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type Account, type ImportMapping, type ImportRow, type StatementInfo } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { NativeSelect } from "@/components/ui/input";
import { Segmented } from "@/components/Segmented";
import { Badge } from "@/components/ui/badge";
import { SuccessTick } from "@/components/SuccessTick";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate, formatMoney } from "@/lib/utils";

type Filter = "all" | "new" | "duplicate" | "error";

export function ImportPage({ onOpenHistory }: { onOpenHistory?: () => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [accountId, setAccountId] = useState("");
  const [path, setPath] = useState<string | null>(null);
  const [info, setInfo] = useState<StatementInfo | null>(null);
  const [mapping, setMapping] = useState<ImportMapping | null>(null);
  const [rows, setRows] = useState<ImportRow[] | null>(null);
  // Row indexes whose default choice was flipped: new rows left out, duplicates added.
  const [flipped, setFlipped] = useState<Set<number>>(new Set());
  const [filter, setFilter] = useState<Filter>("all");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [done, setDone] = useState<number | null>(null);

  useEffect(() => { api.accounts.getAll().then((a) => { setAccounts(a); setAccountId((v) => v || a[0]?.id || ""); }); }, []);
  const account = accounts.find((a) => a.id === accountId);

  const choose = async () => {
    setError("");
    const picked = await open({ multiple: false, directory: false, filters: [{ name: t("import.filesName"), extensions: ["csv", "txt", "xlsx", "xls", "xlsm", "ods"] }] });
    if (typeof picked !== "string") return;
    setPath(picked); setInfo(null); setMapping(null); setRows(null); setFlipped(new Set()); setDone(null);
  };

  // Read the file (and any saved mapping) whenever the file or account changes.
  useEffect(() => {
    if (!path || !accountId) return;
    setBusy(true); setError("");
    api.statements.inspect(path, accountId).then((i) => { setInfo(i); setMapping(i.mapping); })
      .catch((e) => { setInfo(null); setError(errorText(e)); }).finally(() => setBusy(false));
  }, [path, accountId]);

  // Re-parse whenever the mapping changes.
  useEffect(() => {
    if (!path || !mapping || !accountId) return;
    const id = setTimeout(() => {
      api.statements.preview(path, accountId, mapping).then((r) => { setRows(r); setError(""); }).catch((e) => { setRows(null); setError(errorText(e)); });
    }, 150);
    return () => clearTimeout(id);
  }, [path, accountId, mapping]);

  const counts = useMemo(() => {
    const c = { new: 0, duplicate: 0, error: 0, skipped: 0 };
    for (const r of rows ?? []) c[r.status]++;
    return c;
  }, [rows]);
  const chosen = (r: ImportRow) => (r.status === "new") !== flipped.has(r.index) && (r.status === "new" || r.status === "duplicate");
  const selected = (rows ?? []).filter(chosen);
  const shown = (rows ?? []).filter((r) => r.status !== "skipped" && (filter === "all" || r.status === filter));
  const toggle = (i: number) => setFlipped((s) => { const n = new Set(s); if (n.has(i)) n.delete(i); else n.add(i); return n; });

  const run = async () => {
    if (!path || !mapping) return;
    setBusy(true); setError("");
    const skip = (rows ?? []).filter((r) => r.status === "new" && flipped.has(r.index)).map((r) => r.index);
    const include = (rows ?? []).filter((r) => r.status === "duplicate" && flipped.has(r.index)).map((r) => r.index);
    try { setDone(await api.statements.import(path, accountId, mapping, skip, include)); } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  const reset = () => { setPath(null); setInfo(null); setMapping(null); setRows(null); setDone(null); setFlipped(new Set()); };
  const fileName = path?.split(/[\\/]/).pop();

  if (done != null) {
    return (
      <Card className="mx-auto max-w-lg">
        <CardContent className="flex flex-col items-center gap-3 py-10 text-center">
          <SuccessTick className="text-success size-10" />
          <div className="text-lg font-semibold">{t("import.doneTitle", { count: done, account: account?.name })}</div>
          <p className="text-muted-foreground max-w-sm text-sm">{t("import.doneBody")}</p>
          <div className="flex gap-2">
            <Button variant="outline" onClick={reset}><Upload />{t("import.another")}</Button>
            {onOpenHistory && <Button variant="ghost" onClick={onOpenHistory}><RotateCcw />{t("import.undoInHistory")}</Button>}
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="max-w-5xl space-y-4">
      <Card>
        <CardHeader><CardTitle>{t("import.step1")}</CardTitle><CardDescription>{t("import.step1Desc")}</CardDescription></CardHeader>
        <CardContent className="flex flex-wrap items-end gap-3">
          <div className="grid min-w-56 gap-2"><Label htmlFor="imp-acc">{t("import.into")}</Label>
            <NativeSelect id="imp-acc" value={accountId} onChange={(e) => setAccountId(e.target.value)}>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></div>
          <Button onClick={choose} variant={path ? "outline" : "default"} disabled={!accounts.length}><FileSpreadsheet />{path ? t("import.otherFile") : t("import.chooseFile")}</Button>
          {fileName && <span className="text-muted-foreground flex items-center gap-1.5 text-sm">{busy && <Loader2 className="size-4 animate-spin" />}{fileName}{info && ` · ${t("import.rows", { count: info.row_count })}`}</span>}
          {!accounts.length && <p className="text-muted-foreground text-sm">{t("tx.needAccount")}</p>}
        </CardContent>
      </Card>

      {info && mapping && (
        <Card>
          <CardHeader className="flex-row items-start justify-between gap-3">
            <div className="space-y-1.5"><CardTitle>{t("import.step2")}</CardTitle><CardDescription>{t("import.step2Desc")}</CardDescription></div>
            {info.saved && <Badge variant="secondary">{t("import.savedMapping")}</Badge>}
          </CardHeader>
          <CardContent className="space-y-4">
            <MappingForm headers={info.headers} mapping={mapping} onChange={setMapping} />
            <div className="overflow-x-auto rounded-md border">
              <table className="w-full text-xs">
                <thead className="bg-muted/50"><tr>{info.headers.map((h, i) => <th key={i} className="px-2 py-1.5 text-start font-medium whitespace-nowrap">{h || `#${i + 1}`}</th>)}</tr></thead>
                <tbody>{info.sample.map((r, i) => <tr key={i} className="border-t">{info.headers.map((_, j) => <td key={j} className="text-muted-foreground max-w-56 truncate px-2 py-1 whitespace-nowrap">{r[j] ?? ""}</td>)}</tr>)}</tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}

      {error && <p className="text-destructive flex items-center gap-1.5 text-sm"><AlertTriangle className="size-4" />{error}</p>}

      {rows && (
        <Card className="gap-3">
          <CardHeader><CardTitle>{t("import.step3")}</CardTitle>
            <CardDescription>{t("import.summary", { new: counts.new, dup: counts.duplicate, err: counts.error })}</CardDescription></CardHeader>
          <CardContent className="space-y-3">
            <Segmented role="tablist" value={filter} onChange={setFilter} itemClassName="py-1"
              options={[{ value: "all", label: t("import.f.all") }, { value: "new", label: `${t("import.f.new")} ${counts.new}` },
                { value: "duplicate", label: `${t("import.f.duplicate")} ${counts.duplicate}` }, { value: "error", label: `${t("import.f.error")} ${counts.error}` }]} />
            <div className="max-h-[28rem] divide-y overflow-y-auto rounded-md border">
              {shown.length === 0 && <p className="text-muted-foreground py-8 text-center text-sm">{t("import.nothingHere")}</p>}
              {shown.map((r) => (
                <label key={r.index} className={cn("flex cursor-pointer items-center gap-3 px-3 py-2 text-sm", r.status === "error" && "cursor-default")}>
                  <input type="checkbox" className="size-4 accent-[var(--primary)]" disabled={r.status === "error"} checked={chosen(r)} onChange={() => toggle(r.index)} />
                  <span className="text-muted-foreground w-24 shrink-0 tabular-nums">{r.date ? formatDate(r.date) : "—"}</span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-medium">{r.merchant || r.description}</span>
                    {r.description !== r.merchant && <span className="text-muted-foreground block truncate text-xs">{r.description}</span>}
                    {r.message && <span className={cn("flex items-center gap-1 text-xs", r.status === "error" ? "text-destructive" : "text-orange-600")}>
                      {r.status === "duplicate" ? <Copy className="size-3" /> : <AlertTriangle className="size-3" />}
                      {r.status === "duplicate" && r.matched_date ? t("import.dupOn", { date: formatDate(r.matched_date) }) : r.message}</span>}
                  </span>
                  <span className={cn("tabular-nums", r.kind === "income" && "text-success")}>
                    {r.amount ? `${r.kind === "income" ? "+" : "−"}${formatMoney(Number(r.amount), account?.currency ?? "INR", hideBalances)}` : ""}
                  </span>
                </label>
              ))}
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <Button onClick={run} disabled={busy || selected.length === 0}>{busy ? <Loader2 className="animate-spin" /> : <CheckCircle2 />}{t("import.go", { count: selected.length })}</Button>
              <span className="text-muted-foreground text-xs">{t("import.safety")}</span>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function MappingForm({ headers, mapping, onChange }: { headers: string[]; mapping: ImportMapping; onChange: (m: ImportMapping) => void }) {
  const { t } = useTranslation();
  const set = (p: Partial<ImportMapping>) => onChange({ ...mapping, ...p });
  const split = mapping.amount == null;
  const col = (label: string, value: number | null | undefined, onPick: (v: number | null) => void, optional = false) => (
    <div className="grid gap-1.5"><Label className="text-xs">{label}</Label>
      <NativeSelect value={value ?? ""} onChange={(e) => onPick(e.target.value === "" ? null : Number(e.target.value))}>
        {optional && <option value="">{t("import.none")}</option>}
        {headers.map((h, i) => <option key={i} value={i}>{h || `#${i + 1}`}</option>)}
      </NativeSelect></div>
  );
  return (
    <div className="grid gap-3">
      <div className="grid gap-3 sm:grid-cols-3">
        {col(t("import.m.date"), mapping.date, (v) => set({ date: v ?? 0 }))}
        <div className="grid gap-1.5"><Label className="text-xs">{t("import.m.order")}</Label>
          <NativeSelect value={mapping.date_order} onChange={(e) => set({ date_order: e.target.value as ImportMapping["date_order"] })}>
            <option value="dmy">{t("import.order.dmy")}</option><option value="mdy">{t("import.order.mdy")}</option><option value="ymd">{t("import.order.ymd")}</option>
          </NativeSelect></div>
        {col(t("import.m.description"), mapping.description, (v) => set({ description: v ?? 0 }))}
      </div>
      <Segmented value={split ? "split" : "one"} onChange={(v) => set(v === "split" ? { amount: null, side: null, debit: mapping.debit ?? 0, credit: mapping.credit ?? 0 } : { amount: mapping.debit ?? 0, debit: null, credit: null })}
        className="w-fit" itemClassName="py-1 text-xs" label={t("import.m.amounts")}
        options={[{ value: "split", label: t("import.m.split") }, { value: "one", label: t("import.m.one") }]} />
      <div className="grid gap-3 sm:grid-cols-3">
        {split ? <>
          {col(t("import.m.debit"), mapping.debit, (v) => set({ debit: v }), true)}
          {col(t("import.m.credit"), mapping.credit, (v) => set({ credit: v }), true)}
        </> : <>
          {col(t("import.m.amount"), mapping.amount, (v) => set({ amount: v ?? 0 }))}
          {col(t("import.m.side"), mapping.side, (v) => set({ side: v }), true)}
          {mapping.side == null && <div className="grid gap-1.5"><Label className="text-xs">{t("import.m.negative")}</Label>
            <NativeSelect value={mapping.negative_is_expense ? "out" : "in"} onChange={(e) => set({ negative_is_expense: e.target.value === "out" })}>
              <option value="out">{t("import.m.negOut")}</option><option value="in">{t("import.m.negIn")}</option>
            </NativeSelect></div>}
        </>}
        {col(t("import.m.reference"), mapping.reference, (v) => set({ reference: v }), true)}
      </div>
    </div>
  );
}
