import { useMemo, useState } from "react";
import { AlertTriangle, CheckCircle2, Loader2, Plus, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, ITEM_KINDS, type BillDraft, type ItemInput, type ItemKind, type ProjectDetail } from "@/api";
import { columnName, today } from "@/components/projects/ProjectDialogs";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { formatMoney } from "@/lib/utils";

type Row = { keep: boolean; item: string; qty: string; unit: string; rate: string; amount: string; tax_percent: string };

/** Pick a PDF bill, keep it with the project, and read it. Returns the file and its draft. */
export async function chooseAndRead(projectId: string, label: string): Promise<{ fileId: string; draft: BillDraft; photo: boolean } | null> {
  const path = await api.files.pick("document", label);
  if (!path) return null;
  const f = await api.projects.addFile(projectId, path, {});
  return { fileId: f.id, draft: await api.projects.readBill(f.id), photo: f.mime.startsWith("image/") };
}

/** The lines read from a bill, to check and correct before they're added as items. */
export function ReadBillDialog({ project, fileId, draft, photo, onClose, onAdded }: { project: ProjectDetail; fileId: string; draft: BillDraft; photo?: boolean; onClose: () => void; onAdded: () => void }) {
  const { t } = useTranslation();
  const [rows, setRows] = useState<Row[]>(() => draft.items.map((i) => ({ keep: true, item: i.item, qty: i.qty ?? "", unit: i.unit ?? "", rate: i.rate ?? "", amount: i.amount, tax_percent: i.tax_percent ?? "" })));
  const [vendor, setVendor] = useState(draft.vendor ?? "");
  const [billNo, setBillNo] = useState(draft.bill_no ?? "");
  const [date, setDate] = useState(draft.date ?? today());
  const [section, setSection] = useState("");
  const [kind, setKind] = useState<ItemKind>("material");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (i: number, p: Partial<Row>) => setRows((r) => r.map((x, j) => j === i ? { ...x, ...p } : x));
  const kept = rows.filter((r) => r.keep && r.item.trim());
  const money = (n: number) => formatMoney(n, project.currency);
  const sum = useMemo(() => kept.reduce((a, r) => {
    const amt = Number(r.amount) || 0;
    return a + amt + Math.round(amt * (Number(r.tax_percent) || 0)) / 100;
  }, 0), [kept]);
  const billTotal = draft.total == null ? null : Number(draft.total);
  const matches = billTotal != null && Math.abs(billTotal - sum) <= 1;

  const add = async () => {
    setBusy(true); setError("");
    try {
      const items: ItemInput[] = kept.map((r) => ({ date, item: r.item.trim(), kind, section_id: section, qty: r.qty, unit: r.unit, rate: r.rate, amount: r.amount,
        tax_percent: r.tax_percent, vendor, bill_no: billNo, custom: {} }));
      await api.projects.addItems(project.id, { items, file_id: fileId });
      onAdded();
    } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  const cell = "h-8 px-2 text-sm";

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="max-h-[92vh] overflow-y-auto sm:max-w-4xl" onOpenAutoFocus={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle>{t("projects.readTitle")}</DialogTitle>
          <DialogDescription>{t("projects.readDesc")}{photo ? ` ${t("projects.readPhoto")}` : ""}</DialogDescription>
        </DialogHeader>
        <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_10rem_10rem]">
          <div className="grid gap-1.5"><Label htmlFor="rb-vendor">{columnName(project, "vendor", t)}</Label><Input id="rb-vendor" maxLength={100} value={vendor} onChange={(e) => setVendor(e.target.value)} /></div>
          <div className="grid gap-1.5"><Label htmlFor="rb-bill">{columnName(project, "bill_no", t)}</Label><Input id="rb-bill" maxLength={60} value={billNo} onChange={(e) => setBillNo(e.target.value)} /></div>
          <div className="grid gap-1.5"><Label htmlFor="rb-date">{columnName(project, "date", t)}</Label><Input id="rb-date" type="date" value={date} onChange={(e) => setDate(e.target.value)} /></div>
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="grid gap-1.5"><Label htmlFor="rb-section">{columnName(project, "section", t)}</Label>
            <NativeSelect id="rb-section" value={section} onChange={(e) => setSection(e.target.value)}>
              <option value="">{t("projects.noSection")}</option>{project.sections.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
            </NativeSelect></div>
          <div className="grid gap-1.5"><Label htmlFor="rb-kind">{columnName(project, "kind", t)}</Label>
            <NativeSelect id="rb-kind" value={kind} onChange={(e) => setKind(e.target.value as ItemKind)}>
              {ITEM_KINDS.map((k) => <option key={k} value={k}>{t(`projects.itemKinds.${k}`)}</option>)}
            </NativeSelect></div>
        </div>
        {draft.gstin && <p className="text-muted-foreground text-xs">{t("projects.gstinOf", { gstin: draft.gstin })}</p>}

        <div className="overflow-x-auto rounded-lg border">
          <table className="w-full min-w-[46rem] text-sm">
            <caption className="sr-only">{t("projects.readLines")}</caption>
            <thead className="bg-muted/50 text-muted-foreground text-xs">
              <tr>
                <th scope="col" className="w-10 px-2 py-2"><span className="sr-only">{t("projects.keepLine")}</span></th>
                <th scope="col" className="px-2 py-2 text-start font-medium">{columnName(project, "item", t)}</th>
                <th scope="col" className="w-20 px-2 py-2 text-end font-medium">{columnName(project, "qty", t)}</th>
                <th scope="col" className="w-20 px-2 py-2 text-start font-medium">{columnName(project, "unit", t)}</th>
                <th scope="col" className="w-28 px-2 py-2 text-end font-medium">{columnName(project, "rate", t)}</th>
                <th scope="col" className="w-28 px-2 py-2 text-end font-medium">{columnName(project, "amount", t)}</th>
                <th scope="col" className="w-20 px-2 py-2 text-end font-medium">{t("projects.gstPercent")}</th>
                <th scope="col" className="w-10 px-2 py-2"><span className="sr-only">{t("projects.actions")}</span></th>
              </tr>
            </thead>
            <tbody className="divide-y">
              {rows.map((r, i) => (
                <tr key={i} className={r.keep ? "" : "opacity-50"}>
                  <td className="px-2 py-1 text-center"><input type="checkbox" className="accent-primary size-4" aria-label={t("projects.keepThis", { name: r.item || "…" })} checked={r.keep} onChange={(e) => set(i, { keep: e.target.checked })} /></td>
                  <td className="px-1 py-1"><Input className={cell} aria-label={columnName(project, "item", t)} maxLength={200} value={r.item} onChange={(e) => set(i, { item: e.target.value })} /></td>
                  <td className="px-1 py-1"><Input className={`${cell} text-end`} aria-label={columnName(project, "qty", t)} inputMode="decimal" value={r.qty} onChange={(e) => set(i, { qty: e.target.value })} /></td>
                  <td className="px-1 py-1"><Input className={cell} aria-label={columnName(project, "unit", t)} maxLength={20} value={r.unit} onChange={(e) => set(i, { unit: e.target.value })} /></td>
                  <td className="px-1 py-1"><Input className={`${cell} text-end`} aria-label={columnName(project, "rate", t)} inputMode="decimal" value={r.rate} onChange={(e) => set(i, { rate: e.target.value })} /></td>
                  <td className="px-1 py-1"><Input className={`${cell} text-end`} aria-label={columnName(project, "amount", t)} inputMode="decimal" value={r.amount} onChange={(e) => set(i, { amount: e.target.value })} /></td>
                  <td className="px-1 py-1"><Input className={`${cell} text-end`} aria-label={t("projects.gstPercent")} inputMode="decimal" value={r.tax_percent} onChange={(e) => set(i, { tax_percent: e.target.value })} /></td>
                  <td className="px-1 py-1"><Button type="button" size="icon" variant="ghost" aria-label={t("projects.removeLine", { name: r.item || "…" })} onClick={() => setRows(rows.filter((_, j) => j !== i))}><Trash2 /></Button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <Button type="button" variant="outline" className="justify-self-start" onClick={() => setRows([...rows, { keep: true, item: "", qty: "", unit: "", rate: "", amount: "", tax_percent: draft.tax_rate ?? "" }])}><Plus />{t("projects.addLine")}</Button>

        <div className="flex flex-wrap items-start gap-2 rounded-lg border p-3 text-sm" role="status">
          {billTotal == null ? <AlertTriangle className="size-4 shrink-0 text-amber-700 dark:text-amber-300" aria-hidden />
            : matches ? <CheckCircle2 className="size-4 shrink-0 text-emerald-700 dark:text-emerald-300" aria-hidden />
            : <AlertTriangle className="size-4 shrink-0 text-amber-700 dark:text-amber-300" aria-hidden />}
          <p className="flex-1">
            {billTotal == null ? t("projects.noBillTotal", { sum: money(sum) })
              : matches ? t("projects.totalMatches", { total: money(billTotal) })
              : t("projects.totalDiffers", { sum: money(sum), total: money(billTotal) })}
          </p>
        </div>
        {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
        <DialogFooter className="gap-2">
          <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
          <Button onClick={add} disabled={busy || kept.length === 0}>{busy && <Loader2 className="animate-spin" />}{t("projects.addNItems", { count: kept.length })}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
