import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, FileScan, FileSpreadsheet, FolderKanban, Link2, Paperclip, Pencil, Plus, Printer, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, HIDEABLE_COLUMNS, ITEM_KINDS, PAY_MODES, PROJECT_STATUSES, type CustomValue, type LinkSuggestion, type NameableColumn, type BillDraft, type Project, type ProjectDetail, type ProjectFile, type ProjectItem, type ProjectPayment } from "@/api";
import { BillsPanel } from "@/components/projects/Bills";
import { chooseAndRead, ReadBillDialog } from "@/components/projects/ReadBill";
import { columnName, ItemDialog, LinkDialog, PaymentDialog, ProjectDialog, SectionsEditor } from "@/components/projects/ProjectDialogs";
import { Segmented } from "@/components/Segmented";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { NativeSelect } from "@/components/ui/input";
import { useTheme } from "@/hooks/theme";
import { printPage } from "@/lib/print";
import { cn, formatDate, formatMoney, formatNumber } from "@/lib/utils";

const STATUS_TONE: Record<Project["status"], string> = {
  ongoing: "bg-sky-100 text-sky-900 dark:bg-sky-500/20 dark:text-sky-100",
  planning: "bg-violet-100 text-violet-900 dark:bg-violet-500/20 dark:text-violet-100",
  on_hold: "bg-amber-100 text-amber-900 dark:bg-amber-500/20 dark:text-amber-100",
  done: "bg-emerald-100 text-emerald-900 dark:bg-emerald-500/20 dark:text-emerald-100",
};
const date = (d: string | null) => d ? formatDate(d, { day: "numeric", month: "short", year: "numeric" }) : "";

/** Spent against budget: a bar that turns red past the budget. */
function BudgetBar({ spent, budget, label }: { spent: number; budget: number | null; label: string }) {
  if (!budget) return null;
  const share = spent / budget;
  return (
    <div className="h-1.5 overflow-hidden rounded-full bg-muted" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(Math.min(share, 9.99) * 100)}>
      <div className={cn("h-full rounded-full", share > 1 ? "bg-red-600" : share > 0.85 ? "bg-amber-500" : "bg-sky-600")} style={{ width: `${Math.min(share, 1) * 100}%` }} />
    </div>
  );
}

export function ProjectsPage() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  const reload = () => setVersion((v) => v + 1);
  useEffect(() => { window.addEventListener(DATA_RESTORED, reload); return () => window.removeEventListener(DATA_RESTORED, reload); }, []);
  useEffect(() => { api.projects.list().then((l) => { setProjects(l); setOpen((o) => o && l.some((p) => p.id === o) ? o : null); }).catch((e) => setError(errorText(e))); }, [version]);

  if (open) return <ProjectView id={open} version={version} onBack={() => { setOpen(null); reload(); }} onChanged={reload} />;

  return (
    <div className="max-w-6xl space-y-4">
      {error && <p className="text-destructive text-sm">{error}</p>}
      {projects && projects.length === 0 ? (
        <Card><CardContent className="py-10 text-center">
          <FolderKanban className="text-muted-foreground mx-auto mb-3 size-8" aria-hidden />
          <p className="font-medium">{t("projects.emptyTitle")}</p>
          <p className="text-muted-foreground mx-auto mt-1 max-w-md text-sm">{t("projects.emptyDesc")}</p>
          <Button className="mt-4" onClick={() => setCreating(true)}><Plus />{t("projects.new")}</Button>
        </CardContent></Card>
      ) : (
        <>
          <div className="flex justify-end"><Button onClick={() => setCreating(true)}><Plus />{t("projects.new")}</Button></div>
          <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {(projects ?? []).map((p) => (
              <li key={p.id}>
                <button onClick={() => setOpen(p.id)} className="bg-card hover:border-ring/60 flex h-full w-full flex-col gap-3 rounded-xl border p-4 text-start shadow-sm transition-colors">
                  <div className="flex items-start gap-2">
                    <div className="min-w-0 flex-1">
                      <p className="truncate font-medium">{p.name}</p>
                      <p className="text-muted-foreground text-xs">{t(`projects.kinds.${p.kind}`)}{p.last_date ? ` · ${t("projects.lastActivity", { date: date(p.last_date) })}` : ""}</p>
                    </div>
                    <Badge className={STATUS_TONE[p.status]}>{t(`projects.statuses.${p.status}`)}</Badge>
                  </div>
                  <div>
                    <p className="text-2xl font-semibold tracking-tight tabular-nums">{formatMoney(p.spent, p.currency, hideBalances)}</p>
                    <p className="text-muted-foreground text-xs">{p.budget ? t("projects.ofBudget", { budget: formatMoney(p.budget, p.currency, hideBalances) }) : t("projects.spentSoFar")}</p>
                  </div>
                  <BudgetBar spent={p.spent} budget={p.budget} label={t("projects.budgetUsed")} />
                  <p className="text-muted-foreground mt-auto text-xs">
                    {t("projects.itemCount", { count: p.items })} · {t("projects.paymentCount", { count: p.payments })}
                    {p.owed > 0 && <span className="ms-1 font-medium text-amber-800 dark:text-amber-200">· {t("projects.owedShort", { amount: formatMoney(p.owed, p.currency, hideBalances) })}</span>}
                  </p>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
      {creating && <ProjectDialog project={null} onClose={() => setCreating(false)} onSaved={(id) => { setCreating(false); setOpen(id); reload(); }} />}
    </div>
  );
}

type Tab = "items" | "payments" | "bills" | "people" | "sections";

function ProjectView({ id, version, onBack, onChanged }: { id: string; version: number; onBack: () => void; onChanged: () => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [p, setP] = useState<ProjectDetail | null>(null);
  const [items, setItems] = useState<ProjectItem[]>([]);
  const [payments, setPayments] = useState<ProjectPayment[]>([]);
  const [files, setFiles] = useState<ProjectFile[]>([]);
  const [links, setLinks] = useState<LinkSuggestion[]>([]);
  const [names, setNames] = useState({ vendors: [] as string[], account_names: [] as string[], units: [] as string[] });
  const [tab, setTab] = useState<Tab>("items");
  const [section, setSection] = useState("");
  const [editing, setEditing] = useState(false);
  const [item, setItem] = useState<ProjectItem | "new" | null>(null);
  const [payment, setPayment] = useState<ProjectPayment | "new" | null>(null);
  const [linking, setLinking] = useState(false);
  const [reading, setReading] = useState<{ fileId: string; draft: BillDraft; photo: boolean } | null>(null);
  const [readBusy, setReadBusy] = useState(false);
  const [error, setError] = useState("");
  const [note, setNote] = useState("");
  const [n, setN] = useState(0);
  const load = useCallback(() => setN((x) => x + 1), []);
  useEffect(() => {
    api.projects.get(id).then(setP).catch((e) => setError(errorText(e)));
    api.projects.items(id).then(setItems).catch(() => {});
    api.projects.payments(id).then(setPayments).catch(() => {});
    api.projects.files(id).then(setFiles).catch(() => {});
    api.projects.linkSuggestions(id).then(setLinks).catch(() => setLinks([]));
    api.projects.names().then(setNames).catch(() => {});
  }, [id, n, version]);
  const changed = () => { load(); onChanged(); };

  if (!p) return error ? <p className="text-destructive text-sm">{error}</p> : null;
  const money = (x: number) => formatMoney(x, p.currency, hideBalances);
  const shows = (c: (typeof HIDEABLE_COLUMNS)[number]) => !p.hidden.includes(c);
  const col = (k: NameableColumn) => columnName(p, k, t);
  const shownItems = section ? items.filter((i) => (i.section_id ?? "none") === section) : items;
  const shownPays = section ? payments.filter((y) => (y.section_id ?? "none") === section) : payments;
  const sum = (f: (i: ProjectItem) => number) => shownItems.reduce((a, i) => a + f(i), 0);

  const exportXlsx = async () => {
    setError(""); setNote("");
    try {
      const path = await api.files.pick("save_project", t("projects.excelFile"), p.name.replace(/[\\/:*?"<>|]+/g, " ").trim() || "project");
      if (!path) return;
      const labels: Record<string, string> = {
        summary: t("projects.summary"), items: t("projects.tabs.items"), payments: t("projects.tabs.payments"), sections: t("projects.bySection"), people: t("projects.tabs.people"),
        budget: t("projects.budget"), billed: t("projects.billed"), paid: t("projects.paid"), owed: t("projects.owed"), balance: t("projects.balance"), tax: t("projects.col.tax"),
        currency: t("projects.currency"), status: t("projects.statusLabel"), period: t("projects.dates"), no: t("projects.col.no"), date: t("projects.col.date"),
        section: t("projects.col.section"), item: t("projects.col.item"), kind: t("projects.col.kind"), qty: t("projects.col.qty"), unit: t("projects.col.unit"), rate: t("projects.col.rate"),
        amount: t("projects.col.amount"), total: t("projects.col.total"), vendor: t("projects.col.vendor"), bill_no: t("projects.col.bill_no"), notes: t("projects.col.notes"),
        paidTo: t("projects.paidTo"), accountName: t("projects.accountName"), mode: t("projects.mode"), reference: t("projects.reference"), fromAccount: t("projects.fromAccount"),
        none: t("projects.noSection"), name: t("projects.person"), yes: t("projects.yes"), no_: t("projects.no"),
        ...Object.fromEntries(ITEM_KINDS.map((k) => [`itemKind.${k}`, t(`projects.itemKinds.${k}`)])),
        ...Object.fromEntries(PAY_MODES.map((k) => [`mode.${k}`, t(`projects.modes.${k}`)])),
        ...Object.fromEntries(PROJECT_STATUSES.map((k) => [`status.${k}`, t(`projects.statuses.${k}`)])),
      };
      await api.projects.exportXlsx(p.id, path, labels);
      setNote(t("projects.exported"));
    } catch (e) { setError(errorText(e)); }
  };
  const readNew = async () => {
    setError(""); setReadBusy(true);
    try { const r = await chooseAndRead(p.id, t("projects.billFiles")); if (r) setReading(r); } catch (e) { setError(errorText(e)); load(); } finally { setReadBusy(false); }
  };
  const readKept = async (fileId: string) => {
    setError("");
    try { setReading({ fileId, draft: await api.projects.readBill(fileId), photo: files.find((f) => f.id === fileId)?.mime.startsWith("image/") ?? false }); } catch (e) { setError(errorText(e)); }
  };
  const remove = async () => {
    try { await api.projects.delete(p.id); onBack(); } catch (e) { setError(errorText(e)); }
  };
  const custom = (v: CustomValue | undefined, type: string) =>
    v == null || v === "" ? "" : type === "yes_no" ? (v ? t("projects.yes") : t("projects.no")) : type === "date" ? date(String(v)) : type === "number" ? formatNumber(Number(v)) : String(v);
  const cell = "px-3 py-2";

  return (
    <div className="max-w-6xl space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="ghost" size="sm" className="print-hide" onClick={onBack}><ArrowLeft />{t("projects.all")}</Button>
        <h2 className="order-first w-full min-w-0 truncate text-xl font-semibold sm:order-none sm:w-auto sm:flex-1">{p.name}</h2>
        <Badge className={STATUS_TONE[p.status]}>{t(`projects.statuses.${p.status}`)}</Badge>
        <div className="print-hide flex flex-wrap gap-1">
          <Button variant="ghost" size="icon" aria-label={t("projects.edit")} title={t("projects.edit")} onClick={() => setEditing(true)}><Pencil /></Button>
          <Button variant="ghost" size="icon" aria-label={t("projects.exportExcel")} title={t("projects.exportExcel")} onClick={exportXlsx}><FileSpreadsheet /></Button>
          <Button variant="ghost" size="icon" aria-label={t("projects.print")} title={t("projects.print")} onClick={printPage}><Printer /></Button>
          <Button variant="ghost" size="icon" className="text-destructive" aria-label={t("projects.delete")} title={t("projects.delete")} onClick={remove}><Trash2 /></Button>
        </div>
      </div>
      {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
      {note && <p className="text-sm" role="status">{note}</p>}

      <Card>
        <CardContent className="grid grid-cols-2 gap-4 py-1 sm:grid-cols-4">
          <div><p className="text-muted-foreground text-xs">{t("projects.billed")}</p><p className="text-lg font-semibold tabular-nums sm:text-2xl">{money(p.billed)}</p>
            {p.tax > 0 && <p className="text-muted-foreground text-xs">{t("projects.inclTax", { amount: money(p.tax) })}</p>}</div>
          <div><p className="text-muted-foreground text-xs">{t("projects.paid")}</p><p className="text-lg font-semibold tabular-nums sm:text-2xl">{money(p.paid)}</p></div>
          <div><p className="text-muted-foreground text-xs">{t("projects.owed")}</p>
            <p className={cn("text-lg font-semibold tabular-nums sm:text-2xl", p.owed > 0 && "text-amber-800 dark:text-amber-200")}>{money(p.owed)}</p></div>
          <div><p className="text-muted-foreground text-xs">{t("projects.budget")}</p>
            <p className="text-lg font-semibold tabular-nums sm:text-2xl">{p.budget == null ? "—" : money(p.budget)}</p>
            {p.budget_left != null && <p className={cn("text-xs", p.budget_left < 0 ? "text-red-700 dark:text-red-300" : "text-muted-foreground")}>
              {p.budget_left < 0 ? t("projects.overBy", { amount: money(-p.budget_left) }) : t("projects.left", { amount: money(p.budget_left) })}</p>}</div>
          {p.budget != null && <div className="col-span-2 sm:col-span-4"><BudgetBar spent={p.spent} budget={p.budget} label={t("projects.budgetUsed")} /></div>}
        </CardContent>
      </Card>

      {links.length > 0 && (
        <div className="print-hide flex flex-wrap items-center gap-2 rounded-lg border border-sky-300 bg-sky-50 px-3 py-2 text-sm dark:border-sky-500/40 dark:bg-sky-500/10">
          <Link2 className="size-4 text-sky-800 dark:text-sky-200" aria-hidden />
          <span className="flex-1">{t("projects.linkHint", { count: links.length })}</span>
          <Button size="sm" variant="outline" onClick={() => setLinking(true)}>{t("projects.review")}</Button>
        </div>
      )}

      <div className="print-hide flex flex-wrap items-center gap-2">
        <div className="max-w-full overflow-x-auto"><Segmented role="tablist" label={t("projects.view")} value={tab} onChange={setTab} options={[
          { value: "items", label: t("projects.tabs.itemsN", { count: items.length }) },
          { value: "payments", label: t("projects.tabs.paymentsN", { count: payments.length }) },
          { value: "bills", label: t("projects.tabs.billsN", { count: files.length }) },
          { value: "people", label: t("projects.tabs.people") },
          { value: "sections", label: t("projects.tabs.sections") },
        ]} /></div>
        {(tab === "items" || tab === "payments") && (p.sections.length > 0) && (
          <NativeSelect aria-label={col("section")} className="w-auto" value={section} onChange={(e) => setSection(e.target.value)}>
            <option value="">{t("projects.allSections")}</option>
            {p.sections.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
            <option value="none">{t("projects.noSection")}</option>
          </NativeSelect>
        )}
        <div className="ms-auto flex flex-wrap gap-2">
          {tab === "items" && <Button variant="outline" onClick={readNew} disabled={readBusy}><FileScan />{t("projects.readBill")}</Button>}
          {tab === "items" && <Button onClick={() => setItem("new")}><Plus />{t("projects.addItem")}</Button>}
          {tab === "payments" && <Button onClick={() => setPayment("new")}><Plus />{t("projects.addPayment")}</Button>}
        </div>
      </div>

      {tab === "items" && (
        <Card className="py-0">
          <div className="overflow-x-auto">
            <table className="w-full min-w-[48rem] text-sm">
              <caption className="sr-only">{t("projects.tabs.items")}</caption>
              <thead className="text-muted-foreground border-b text-xs">
                <tr>
                  <th scope="col" className={cn(cell, "w-10 text-end font-medium")}>{t("projects.col.no")}</th>
                  <th scope="col" className={cn(cell, "text-start font-medium")}>{col("date")}</th>
                  <th scope="col" className={cn(cell, "text-start font-medium")}>{col("item")}</th>
                  {shows("qty") && <th scope="col" className={cn(cell, "text-end font-medium")}>{col("qty")}</th>}
                  {shows("rate") && <th scope="col" className={cn(cell, "text-end font-medium")}>{col("rate")}</th>}
                  <th scope="col" className={cn(cell, "text-end font-medium")}>{col("amount")}</th>
                  {shows("tax") && <th scope="col" className={cn(cell, "text-end font-medium")}>{col("tax")}</th>}
                  {shows("tax") && <th scope="col" className={cn(cell, "text-end font-medium")}>{col("total")}</th>}
                  {shows("vendor") && <th scope="col" className={cn(cell, "text-start font-medium")}>{col("vendor")}</th>}
                  {shows("bill_no") && <th scope="col" className={cn(cell, "text-start font-medium")}>{col("bill_no")}</th>}
                  {p.columns.map((c) => <th key={c.id} scope="col" className={cn(cell, c.type === "number" ? "text-end" : "text-start", "font-medium")}>{c.name}</th>)}
                </tr>
              </thead>
              <tbody className="divide-y">
                {shownItems.map((i, n) => (
                  <tr key={i.id} className="hover:bg-accent/50 cursor-pointer align-top" onClick={() => setItem(i)}>
                    <td className={cn(cell, "text-muted-foreground text-end tabular-nums")}>{n + 1}</td>
                    <td className={cn(cell, "whitespace-nowrap tabular-nums")}>{date(i.date)}</td>
                    <td className={cn(cell, "max-w-[18rem]")}>
                      <button className="block w-full truncate text-start font-medium focus-visible:underline focus-visible:outline-none" onClick={(e) => { e.stopPropagation(); setItem(i); }}>{i.item}{i.files > 0 && <Paperclip className="text-muted-foreground ms-1.5 inline size-3.5" aria-label={t("projects.hasBill", { count: i.files })} />}</button>
                      <span className="text-muted-foreground block truncate text-xs">{[shows("section") && i.section_name, shows("kind") && t(`projects.itemKinds.${i.kind}`), shows("notes") && i.notes].filter(Boolean).join(" · ")}</span>
                    </td>
                    {shows("qty") && <td className={cn(cell, "text-end whitespace-nowrap tabular-nums")}>{i.qty == null ? "" : `${formatNumber(i.qty, { maximumFractionDigits: 4 })}${i.unit ? ` ${i.unit}` : ""}`}</td>}
                    {shows("rate") && <td className={cn(cell, "text-end tabular-nums")}>{i.rate == null ? "" : money(i.rate)}</td>}
                    <td className={cn(cell, "text-end tabular-nums")}>{money(i.amount)}</td>
                    {shows("tax") && <td className={cn(cell, "text-end tabular-nums")}>{i.tax ? money(i.tax) : ""}</td>}
                    {shows("tax") && <td className={cn(cell, "text-end font-medium tabular-nums")}>{money(i.total)}</td>}
                    {shows("vendor") && <td className={cn(cell, "min-w-[10rem]")}>{i.vendor}</td>}
                    {shows("bill_no") && <td className={cell}>{i.bill_no}</td>}
                    {p.columns.map((c) => <td key={c.id} className={cn(cell, c.type === "number" && "text-end tabular-nums")}>{custom(i.custom[c.id], c.type)}</td>)}
                  </tr>
                ))}
                {shownItems.length === 0 && <tr><td colSpan={20} className="text-muted-foreground px-4 py-6 text-center">{t("projects.noItems")}</td></tr>}
              </tbody>
              {shownItems.length > 0 && (
                <tfoot className="border-t-2 font-semibold">
                  <tr>
                    <td className={cell} /><td className={cell} />
                    <th scope="row" className={cn(cell, "text-start")}>{col("total")}</th>
                    {shows("qty") && <td className={cell} />}
                    {shows("rate") && <td className={cell} />}
                    <td className={cn(cell, "text-end tabular-nums")}>{money(sum((i) => i.amount))}</td>
                    {shows("tax") && <td className={cn(cell, "text-end tabular-nums")}>{money(sum((i) => i.tax))}</td>}
                    {shows("tax") && <td className={cn(cell, "text-end tabular-nums")}>{money(sum((i) => i.total))}</td>}
                    {shows("vendor") && <td className={cell} />}
                    {shows("bill_no") && <td className={cell} />}
                    {p.columns.map((c) => <td key={c.id} className={cn(cell, "text-end tabular-nums")}>{c.type === "number" ? formatNumber(shownItems.reduce((a, i) => a + (Number(i.custom[c.id]) || 0), 0)) : ""}</td>)}
                  </tr>
                </tfoot>
              )}
            </table>
          </div>
        </Card>
      )}

      {tab === "payments" && (
        <Card className="py-0">
          <div className="overflow-x-auto">
            <table className="w-full min-w-[44rem] text-sm">
              <caption className="sr-only">{t("projects.tabs.payments")}</caption>
              <thead className="text-muted-foreground border-b text-xs">
                <tr>
                  <th scope="col" className={cn(cell, "w-10 text-end font-medium")}>{t("projects.col.no")}</th>
                  <th scope="col" className={cn(cell, "text-start font-medium")}>{t("projects.col.date")}</th>
                  <th scope="col" className={cn(cell, "text-start font-medium")}>{t("projects.paidTo")}</th>
                  <th scope="col" className={cn(cell, "text-start font-medium")}>{t("projects.mode")}</th>
                  <th scope="col" className={cn(cell, "text-start font-medium")}>{t("projects.reference")}</th>
                  <th scope="col" className={cn(cell, "text-end font-medium")}>{t("projects.col.amount")}</th>
                </tr>
              </thead>
              <tbody className="divide-y">
                {shownPays.map((y, n) => (
                  <tr key={y.id} className="hover:bg-accent/50 cursor-pointer align-top" onClick={() => setPayment(y)}>
                    <td className={cn(cell, "text-muted-foreground text-end tabular-nums")}>{n + 1}</td>
                    <td className={cn(cell, "whitespace-nowrap tabular-nums")}>{date(y.date)}</td>
                    <td className={cell}>
                      <button className="text-start font-medium focus-visible:underline focus-visible:outline-none" onClick={(e) => { e.stopPropagation(); setPayment(y); }}>{y.vendor || y.account_name || t("projects.someone")}{y.files > 0 && <Paperclip className="text-muted-foreground ms-1.5 inline size-3.5" aria-label={t("projects.hasBill", { count: y.files })} />}</button>
                      <span className="text-muted-foreground block text-xs">{[y.account_name && y.vendor ? t("projects.asShown", { name: y.account_name }) : null, y.section_name].filter(Boolean).join(" · ")}</span>
                    </td>
                    <td className={cell}>{t(`projects.modes.${y.mode}`)}
                      {y.entry_id && <span className="text-muted-foreground flex items-center gap-1 text-xs"><Link2 className="size-3" aria-hidden />{y.from_account ?? t("projects.linked")}</span>}</td>
                    <td className={cn(cell, "text-muted-foreground")}>{y.reference}</td>
                    <td className={cn(cell, "text-end font-medium tabular-nums")}>{money(y.amount)}</td>
                  </tr>
                ))}
                {shownPays.length === 0 && <tr><td colSpan={6} className="text-muted-foreground px-4 py-6 text-center">{t("projects.noPayments")}</td></tr>}
              </tbody>
              {shownPays.length > 0 && (
                <tfoot className="border-t-2 font-semibold">
                  <tr><td className={cell} /><td className={cell} /><th scope="row" className={cn(cell, "text-start")}>{t("projects.col.total")}</th><td className={cell} /><td className={cell} />
                    <td className={cn(cell, "text-end tabular-nums")}>{money(shownPays.reduce((a, y) => a + y.amount, 0))}</td></tr>
                </tfoot>
              )}
            </table>
          </div>
        </Card>
      )}

      {tab === "people" && (
        <Card className="py-0">
          <div className="overflow-x-auto">
            <table className="w-full min-w-[36rem] text-sm">
              <caption className="sr-only">{t("projects.tabs.people")}</caption>
              <thead className="text-muted-foreground border-b text-xs">
                <tr>
                  <th scope="col" className={cn(cell, "text-start font-medium")}>{t("projects.person")}</th>
                  <th scope="col" className={cn(cell, "text-end font-medium")}>{t("projects.billed")}</th>
                  <th scope="col" className={cn(cell, "text-end font-medium")}>{t("projects.paid")}</th>
                  <th scope="col" className={cn(cell, "text-end font-medium")}>{t("projects.balance")}</th>
                </tr>
              </thead>
              <tbody className="divide-y">
                {p.vendors.map((v) => (
                  <tr key={v.name}>
                    <td className={cell}><span className="font-medium">{v.name}</span>
                      <span className="text-muted-foreground block text-xs">{t("projects.itemCount", { count: v.items })} · {t("projects.paymentCount", { count: v.payments })}</span></td>
                    <td className={cn(cell, "text-end tabular-nums")}>{money(v.billed)}</td>
                    <td className={cn(cell, "text-end tabular-nums")}>{money(v.paid)}</td>
                    <td className={cn(cell, "text-end")}>
                      {v.balance > 0 ? <span className="font-medium text-amber-800 tabular-nums dark:text-amber-200">{t("projects.toPay", { amount: money(v.balance) })}</span>
                        : v.balance < 0 ? <span className="text-sky-800 tabular-nums dark:text-sky-200">{t("projects.advance", { amount: money(-v.balance) })}</span>
                        : <span className="text-muted-foreground">{t("projects.settled")}</span>}
                    </td>
                  </tr>
                ))}
                {p.vendors.length === 0 && <tr><td colSpan={4} className="text-muted-foreground px-4 py-6 text-center">{t("projects.noPeople")}</td></tr>}
              </tbody>
            </table>
          </div>
          <p className="text-muted-foreground border-t px-3 py-2 text-xs">{t("projects.peopleHelp")}</p>
        </Card>
      )}

      {tab === "sections" && <SectionsEditor project={p} onChanged={changed} />}
      {tab === "bills" && <BillsPanel project={p} files={files} items={items} payments={payments} onChanged={changed} onRead={readKept} />}

      {editing && <ProjectDialog project={p} onClose={() => setEditing(false)} onSaved={() => { setEditing(false); changed(); }} />}
      {item && <ItemDialog project={p} item={item === "new" ? null : item} names={names} files={files} onFiles={load} onClose={() => setItem(null)} onSaved={() => { setItem(null); changed(); }} />}
      {payment && <PaymentDialog project={p} payment={payment === "new" ? null : payment} names={names} files={files} onFiles={load} onClose={() => setPayment(null)} onSaved={() => { setPayment(null); changed(); }} />}
      {reading && <ReadBillDialog project={p} fileId={reading.fileId} draft={reading.draft} photo={reading.photo} onClose={() => { setReading(null); load(); }} onAdded={() => { setReading(null); setTab("items"); changed(); }} />}
      {linking && <LinkDialog project={p} suggestions={links} payments={payments} onClose={(c) => { setLinking(false); if (c) changed(); }} />}
    </div>
  );
}
