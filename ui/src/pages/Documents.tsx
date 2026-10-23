import { useEffect, useMemo, useState, type FormEvent } from "react";
import { AlertTriangle, Award, BadgeCheck, BookUser, Car, Download, FileText, HeartPulse, House, IdCard, ImageIcon, Loader2, Paperclip, Plus, Receipt, ScrollText, Search, ShieldCheck, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, DOC_KINDS, errorText, type Doc, type DocFile, type DocInput, type DocKind } from "@/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate } from "@/lib/utils";

export const DOC_ICON: Record<DocKind, typeof FileText> = {
  id: IdCard, passport: BookUser, licence: IdCard, insurance: ShieldCheck, property: House, vehicle: Car, will: ScrollText,
  certificate: Award, warranty: BadgeCheck, medical: HeartPulse, tax: Receipt, other: FileText,
};

const size = (n: number) => n < 1024 * 1024 ? `${Math.max(1, Math.round(n / 1024))} KB` : `${(n / 1024 / 1024).toFixed(1)} MB`;

/** Expired, expiring soon, or valid until a date, in words. */
function ExpiryBadge({ doc }: { doc: Doc }) {
  const { t } = useTranslation();
  if (!doc.expires_on || doc.days_left == null) return null;
  const date = formatDate(doc.expires_on, { day: "numeric", month: "short", year: "numeric" });
  if (doc.status === "expired") return <Badge className="bg-red-100 text-red-900 dark:bg-red-500/20 dark:text-red-200">{t("documents.expired", { date })}</Badge>;
  if (doc.status === "expiring") return <Badge className="bg-amber-100 text-amber-900 dark:bg-amber-500/20 dark:text-amber-200">{t("documents.expiresIn", { count: doc.days_left })}</Badge>;
  return <span className="text-muted-foreground text-xs">{t("documents.validTill", { date })}</span>;
}

export function DocumentsPage() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [docs, setDocs] = useState<Doc[] | null>(null);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<DocKind | "">("");
  const [editing, setEditing] = useState<Doc | "new" | null>(null);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  const reload = () => setVersion((v) => v + 1);
  useEffect(() => { window.addEventListener(DATA_RESTORED, reload); return () => window.removeEventListener(DATA_RESTORED, reload); }, []);
  useEffect(() => { api.documents.list().then(setDocs).catch((e) => setError(errorText(e))); }, [version]);

  const shown = useMemo(() => (docs ?? []).filter((d) => (!kind || d.kind === kind) &&
    (!query.trim() || [d.title, d.holder, d.issuer, d.number, d.kept_at, d.notes].some((x) => x?.toLowerCase().includes(query.trim().toLowerCase())))), [docs, query, kind]);
  const attention = (docs ?? []).filter((d) => d.status === "expired" || d.status === "expiring");

  return (
    <div className="max-w-5xl space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-56 flex-1">
          <Search className="text-muted-foreground pointer-events-none absolute top-2.5 left-2.5 size-4" aria-hidden />
          <Input className="ps-8" aria-label={t("documents.search")} placeholder={t("documents.search")} value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        <NativeSelect aria-label={t("documents.kindLabel")} className="w-auto" value={kind} onChange={(e) => setKind(e.target.value as DocKind | "")}>
          <option value="">{t("documents.allKinds")}</option>
          {DOC_KINDS.map((k) => <option key={k} value={k}>{t(`documents.kinds.${k}`)}</option>)}
        </NativeSelect>
        <Button onClick={() => setEditing("new")}><Plus />{t("documents.add")}</Button>
      </div>
      {error && <p className="text-destructive text-sm">{error}</p>}

      {attention.length > 0 && !kind && !query && (
        <Card className="border-amber-300 dark:border-amber-500/40">
          <CardContent className="space-y-2 py-1">
            <p className="flex items-center gap-2 text-sm font-medium"><AlertTriangle className="size-4 text-amber-700 dark:text-amber-300" aria-hidden />{t("documents.attention", { count: attention.length })}</p>
            <ul className="flex flex-wrap gap-2">
              {attention.map((d) => (
                <li key={d.id}><button onClick={() => setEditing(d)} className="hover:bg-accent flex items-center gap-2 rounded-md border px-2.5 py-1 text-sm">{d.title}<ExpiryBadge doc={d} /></button></li>
              ))}
            </ul>
          </CardContent>
        </Card>
      )}

      {docs && docs.length === 0 && (
        <Card><CardContent className="py-10 text-center">
          <FileText className="text-muted-foreground mx-auto mb-3 size-8" aria-hidden />
          <p className="font-medium">{t("documents.emptyTitle")}</p>
          <p className="text-muted-foreground mx-auto mt-1 max-w-md text-sm">{t("documents.emptyDesc")}</p>
          <Button className="mt-4" onClick={() => setEditing("new")}><Plus />{t("documents.add")}</Button>
        </CardContent></Card>
      )}
      {docs && docs.length > 0 && shown.length === 0 && <p className="text-muted-foreground text-sm">{t("documents.noMatch")}</p>}

      <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {shown.map((d) => {
          const Icon = DOC_ICON[d.kind] ?? FileText;
          return (
            <li key={d.id}>
              <button onClick={() => setEditing(d)} className="bg-card hover:border-ring/60 flex h-full w-full flex-col gap-2 rounded-xl border p-4 text-left shadow-sm transition-colors">
                <div className="flex items-start gap-3">
                  <div className="bg-secondary grid size-9 shrink-0 place-items-center rounded-lg"><Icon className="size-4" aria-hidden /></div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate font-medium">{d.title}</div>
                    <div className="text-muted-foreground truncate text-xs">{[t(`documents.kinds.${d.kind}`), d.holder, d.issuer].filter(Boolean).join(" · ")}</div>
                  </div>
                </div>
                {d.number && <div className={cn("font-mono text-sm tracking-wide", hideBalances && "blur-sm")}>{d.number}</div>}
                <div className="mt-auto flex flex-wrap items-center gap-x-3 gap-y-1 pt-1">
                  <ExpiryBadge doc={d} />
                  {d.files.length > 0 && <span className="text-muted-foreground flex items-center gap-1 text-xs"><Paperclip className="size-3" aria-hidden />{t("documents.files", { count: d.files.length })}</span>}
                </div>
              </button>
            </li>
          );
        })}
      </ul>
      {editing && <DocumentDialog doc={editing === "new" ? null : editing} onClose={() => { setEditing(null); reload(); }} />}
    </div>
  );
}

const EMPTY: DocInput = { title: "", kind: "other", holder: "", number: "", issuer: "", issued_on: "", expires_on: "", remind_days: 30, kept_at: "", notes: "" };

function DocumentDialog({ doc: initial, onClose }: { doc: Doc | null; onClose: () => void }) {
  const { t } = useTranslation();
  const [doc, setDoc] = useState<Doc | null>(initial);
  const [v, setV] = useState<DocInput>(() => initial ? { ...EMPTY, ...Object.fromEntries(Object.entries(initial).map(([k, x]) => [k, x ?? ""])) } as DocInput : EMPTY);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [preview, setPreview] = useState<{ name: string; url: string } | null>(null);
  const set = (p: Partial<DocInput>) => setV((x) => ({ ...x, ...p }));
  const field = (key: keyof DocInput, label: string, props: React.ComponentProps<typeof Input> = {}) => (
    <div className="grid gap-1.5"><Label htmlFor={`doc-${key}`}>{label}</Label>
      <Input id={`doc-${key}`} value={String(v[key] ?? "")} onChange={(e) => set({ [key]: e.target.value } as Partial<DocInput>)} {...props} /></div>
  );

  const save = async (e: FormEvent) => {
    e.preventDefault(); setBusy(true); setError("");
    try {
      const saved = doc ? await api.documents.update(doc.id, v) : await api.documents.create(v);
      // A new document stays open, so files can be added straight away.
      if (doc) onClose(); else setDoc(saved);
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const refresh = async () => { if (doc) setDoc((await api.documents.list()).find((d) => d.id === doc.id) ?? doc); };
  const addFile = async () => {
    if (!doc) return;
    setError("");
    try {
      const path = await api.files.pick("document", t("documents.fileTypes"));
      if (!path) return;
      setBusy(true);
      await api.documents.addFile(doc.id, path);
      await refresh();
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const saveCopy = async (f: DocFile) => {
    try { const path = await api.files.pick("save_document", t("documents.fileTypes"), f.name); if (path) await api.documents.saveFile(f.id, path); } catch (err) { setError(errorText(err)); }
  };
  const show = async (f: DocFile) => {
    try { const p = await api.documents.preview(f.id); setPreview({ name: p.name, url: `data:${p.mime};base64,${p.data}` }); } catch (err) { setError(errorText(err)); }
  };
  const remove = async () => { if (!doc) return; try { await api.documents.delete(doc.id); onClose(); } catch (err) { setError(errorText(err)); } };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{doc ? doc.title : t("documents.newTitle")}</DialogTitle>
          <DialogDescription>{doc ? t("documents.editDesc") : t("documents.newDesc")}</DialogDescription>
        </DialogHeader>
        <form onSubmit={save} className="grid gap-3">
          <div className="grid grid-cols-[1fr_11rem] gap-3">
            {field("title", t("documents.title"), { autoFocus: !doc, required: true, maxLength: 120, placeholder: t("documents.titlePlaceholder") })}
            <div className="grid gap-1.5"><Label htmlFor="doc-kind">{t("documents.kindLabel")}</Label>
              <NativeSelect id="doc-kind" value={v.kind} onChange={(e) => set({ kind: e.target.value as DocKind })}>
                {DOC_KINDS.map((k) => <option key={k} value={k}>{t(`documents.kinds.${k}`)}</option>)}
              </NativeSelect></div>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            {field("holder", t("documents.holder"), { maxLength: 80 })}
            {field("number", t("documents.number"), { maxLength: 60, className: "font-mono", autoComplete: "off", spellCheck: false })}
            {field("issuer", t("documents.issuer"), { maxLength: 80 })}
            {field("issued_on", t("documents.issued"), { type: "date" })}
            {field("expires_on", t("documents.expires"), { type: "date" })}
            <div className="grid gap-1.5"><Label htmlFor="doc-remind">{t("documents.remind")}</Label>
              <NativeSelect id="doc-remind" value={v.remind_days} disabled={!v.expires_on} onChange={(e) => set({ remind_days: Number(e.target.value) })}>
                {[7, 15, 30, 60, 90, 180].map((d) => <option key={d} value={d}>{t("documents.daysBefore", { count: d })}</option>)}
              </NativeSelect></div>
          </div>
          {field("kept_at", t("documents.keptAt"), { maxLength: 200, placeholder: t("documents.keptAtPlaceholder") })}
          <div className="grid gap-1.5"><Label htmlFor="doc-notes">{t("documents.notes")}</Label>
            <textarea id="doc-notes" rows={2} maxLength={2000} value={v.notes ?? ""} onChange={(e) => set({ notes: e.target.value })}
              className="border-input dark:bg-input/30 focus-visible:border-ring focus-visible:ring-ring/50 rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:ring-[3px]" /></div>

          {doc && (
            <section className="grid gap-2 rounded-lg border p-3">
              <div className="flex items-center justify-between gap-2">
                <h3 className="text-sm font-medium">{t("documents.scans")}</h3>
                <Button type="button" size="sm" variant="outline" onClick={addFile} disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <Paperclip />}{t("documents.addFile")}</Button>
              </div>
              {doc.files.length === 0 ? <p className="text-muted-foreground text-xs">{t("documents.noFiles")}</p> : (
                <ul className="divide-y">
                  {doc.files.map((f) => (
                    <li key={f.id} className="flex items-center gap-2 py-1.5">
                      {f.mime.startsWith("image/") ? <ImageIcon className="text-muted-foreground size-4 shrink-0" aria-hidden /> : <FileText className="text-muted-foreground size-4 shrink-0" aria-hidden />}
                      <span className="min-w-0 flex-1 truncate text-sm">{f.name}</span>
                      <span className="text-muted-foreground text-xs tabular-nums">{size(f.size)}</span>
                      {f.mime.startsWith("image/") && <Button type="button" size="sm" variant="ghost" onClick={() => show(f)}>{t("documents.view")}</Button>}
                      <Button type="button" size="icon" variant="ghost" aria-label={t("documents.saveCopy", { name: f.name })} title={t("documents.saveCopy", { name: f.name })} onClick={() => saveCopy(f)}><Download /></Button>
                      <Button type="button" size="icon" variant="ghost" aria-label={t("documents.removeFile", { name: f.name })} title={t("documents.removeFile", { name: f.name })}
                        onClick={() => api.documents.removeFile(f.id).then(refresh).catch((e) => setError(errorText(e)))}><Trash2 /></Button>
                    </li>
                  ))}
                </ul>
              )}
              <p className="text-muted-foreground text-xs">{t("documents.encrypted")}</p>
            </section>
          )}
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            {doc && <Button type="button" variant="ghost" className="text-destructive me-auto" onClick={remove}><Trash2 />{t("common.delete")}</Button>}
            <Button type="button" variant="outline" onClick={onClose}>{doc && !initial ? t("documents.done") : t("common.cancel")}</Button>
            <Button disabled={busy || !v.title.trim()}>{busy && <Loader2 className="animate-spin" />}{t("common.save")}</Button>
          </DialogFooter>
        </form>
        {preview && (
          <Dialog open onOpenChange={(o) => { if (!o) setPreview(null); }}>
            <DialogContent className="sm:max-w-3xl">
              <DialogHeader><DialogTitle className="truncate">{preview.name}</DialogTitle><DialogDescription className="sr-only">{t("documents.view")}</DialogDescription></DialogHeader>
              <img src={preview.url} alt={preview.name} className="max-h-[70vh] w-full rounded-md object-contain" />
            </DialogContent>
          </Dialog>
        )}
      </DialogContent>
    </Dialog>
  );
}
