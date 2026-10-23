import { useState } from "react";
import { Download, FileScan, FileText, ImageIcon, Loader2, Paperclip, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type FileLink, type ProjectDetail, type ProjectFile, type ProjectItem, type ProjectPayment } from "@/api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { NativeSelect } from "@/components/ui/input";
import { formatDate, formatMoney } from "@/lib/utils";

const size = (n: number) => n < 1024 * 1024 ? `${Math.max(1, Math.round(n / 1024))} KB` : `${(n / 1024 / 1024).toFixed(1)} MB`;

/** Choose a PDF or photo and keep it with the project (for an item or payment, if given). */
export function useAttach(projectId: string, onDone: () => void) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const attach = async (link: FileLink) => {
    setError("");
    try {
      const path = await api.files.pick("document", t("projects.billFiles"));
      if (!path) return;
      setBusy(true);
      await api.projects.addFile(projectId, path, link);
      onDone();
    } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  return { attach, busy, error };
}

/** A photo opened in the window. */
function Preview({ preview, onClose }: { preview: { name: string; url: string }; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-3xl">
        <DialogHeader><DialogTitle className="truncate pe-6">{preview.name}</DialogTitle><DialogDescription className="sr-only">{t("projects.viewBill")}</DialogDescription></DialogHeader>
        <img src={preview.url} alt={preview.name} className="max-h-[70vh] w-full rounded-md object-contain" />
      </DialogContent>
    </Dialog>
  );
}

/** One file: open, save a copy, remove; and, in the full list, what it's for. */
function FileRow({ f, project, items, payments, onChanged, onRead, compact }: {
  f: ProjectFile; project: ProjectDetail; items?: ProjectItem[]; payments?: ProjectPayment[]; onChanged: () => void; onRead?: (id: string) => void; compact?: boolean;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState("");
  const [preview, setPreview] = useState<{ name: string; url: string } | null>(null);
  const isImage = f.mime.startsWith("image/");
  const show = async () => { try { const p = await api.projects.previewFile(f.id); setPreview({ name: p.name, url: `data:${p.mime};base64,${p.data}` }); } catch (e) { setError(errorText(e)); } };
  const save = async () => { try { const path = await api.files.pick("save_document", t("projects.billFiles"), f.name); if (path) await api.projects.saveFile(f.id, path); } catch (e) { setError(errorText(e)); } };
  const remove = async () => { try { await api.projects.removeFile(f.id); onChanged(); } catch (e) { setError(errorText(e)); } };
  const link = async (value: string) => {
    const [kind, id] = value.split(":");
    try { await api.projects.linkFile(f.id, kind === "item" ? { item_id: id } : kind === "payment" ? { payment_id: id } : {}); onChanged(); } catch (e) { setError(errorText(e)); }
  };
  const current = f.item_id ? `item:${f.item_id}` : f.payment_id ? `payment:${f.payment_id}` : "";

  return (
    <li className="py-2">
      <div className="flex flex-wrap items-center gap-2">
        {isImage ? <ImageIcon className="text-muted-foreground size-4 shrink-0" aria-hidden /> : <FileText className="text-muted-foreground size-4 shrink-0" aria-hidden />}
        <span className="min-w-0 flex-1 truncate text-sm" title={f.name}>{f.name}</span>
        <span className="text-muted-foreground text-xs tabular-nums">{size(f.size)}</span>
        {isImage && <Button type="button" size="sm" variant="ghost" onClick={show}>{t("projects.viewBill")}</Button>}
        {onRead && <Button type="button" size="sm" variant="ghost" onClick={() => onRead(f.id)}><FileScan />{t("projects.readItems")}</Button>}
        <Button type="button" size="icon" variant="ghost" aria-label={t("projects.saveBill", { name: f.name })} title={t("projects.saveBill", { name: f.name })} onClick={save}><Download /></Button>
        <Button type="button" size="icon" variant="ghost" aria-label={t("projects.removeBill", { name: f.name })} title={t("projects.removeBill", { name: f.name })} onClick={remove}><Trash2 /></Button>
      </div>
      {!compact && items && payments && (
        <div className="mt-1 flex items-center gap-2 ps-6">
          <label className="text-muted-foreground text-xs" htmlFor={`bill-for-${f.id}`}>{t("projects.billFor")}</label>
          <NativeSelect id={`bill-for-${f.id}`} className="h-8 w-auto max-w-full min-w-0 text-xs" value={current} onChange={(e) => link(e.target.value)}>
            <option value="">{t("projects.wholeProject")}</option>
            {items.length > 0 && <optgroup label={t("projects.tabs.items")}>
              {items.map((i) => <option key={i.id} value={`item:${i.id}`}>{formatDate(i.date)} · {i.item}</option>)}
            </optgroup>}
            {payments.length > 0 && <optgroup label={t("projects.tabs.payments")}>
              {payments.map((y) => <option key={y.id} value={`payment:${y.id}`}>{formatDate(y.date)} · {formatMoney(y.amount, project.currency)}{y.vendor ? ` · ${y.vendor}` : ""}</option>)}
            </optgroup>}
          </NativeSelect>
        </div>
      )}
      {error && <p className="text-destructive mt-1 text-xs" role="alert">{error}</p>}
      {preview && <Preview preview={preview} onClose={() => setPreview(null)} />}
    </li>
  );
}

/** Every bill and receipt kept with the project. */
export function BillsPanel({ project, files, items, payments, onChanged, onRead }: {
  project: ProjectDetail; files: ProjectFile[]; items: ProjectItem[]; payments: ProjectPayment[]; onChanged: () => void; onRead: (id: string) => void;
}) {
  const { t } = useTranslation();
  const { attach, busy, error } = useAttach(project.id, onChanged);
  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <p className="text-muted-foreground min-w-0 flex-1 text-sm">{t("projects.billsHelp")}</p>
        <Button onClick={() => attach({})} disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <Paperclip />}{t("projects.attachBill")}</Button>
      </div>
      {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
      {files.length === 0
        ? <p className="text-muted-foreground rounded-lg border px-4 py-6 text-center text-sm">{t("projects.noBills")}</p>
        : <ul className="divide-y rounded-lg border px-3">{files.map((f) => <FileRow key={f.id} f={f} project={project} items={items} payments={payments} onChanged={onChanged} onRead={onRead} />)}</ul>}
      <p className="text-muted-foreground text-xs">{t("projects.billsEncrypted")}</p>
    </div>
  );
}

/** The bills for one item or payment, inside its form. */
export function AttachedBills({ project, files, link, onChanged }: { project: ProjectDetail; files: ProjectFile[]; link: FileLink; onChanged: () => void }) {
  const { t } = useTranslation();
  const { attach, busy, error } = useAttach(project.id, onChanged);
  return (
    <section className="grid gap-1 rounded-lg border p-3">
      <div className="flex items-center justify-between gap-2">
        <h3 className="text-sm font-medium">{t("projects.bills")}</h3>
        <Button type="button" size="sm" variant="outline" onClick={() => attach(link)} disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <Paperclip />}{t("projects.attachBill")}</Button>
      </div>
      {files.length === 0 ? <p className="text-muted-foreground text-xs">{t("projects.noBillsHere")}</p>
        : <ul className="divide-y">{files.map((f) => <FileRow key={f.id} f={f} project={project} onChanged={onChanged} compact />)}</ul>}
      {error && <p className="text-destructive text-xs" role="alert">{error}</p>}
    </section>
  );
}
