import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { ArrowDown, ArrowUp, Link2, Loader2, Paperclip, Pencil, Plus, Trash2, Unlink, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  api, COLUMN_TYPES, errorText, HIDEABLE_COLUMNS, NAMEABLE_COLUMNS, ITEM_KINDS, PAY_MODES, PROJECT_KINDS, PROJECT_STATUSES,
  type Account, type Category, type ColumnType, type CustomValue, type HideableColumn, type ItemInput, type ItemKind, type LinkSuggestion, type NameableColumn, type PaymentInput,
  type PayMode, type ProjectColumn, type ProjectFile, type ProjectMember, type ProjectDetail, type ProjectInput, type ProjectItem, type ProjectKind, type ProjectPayment, type ProjectStatus,
} from "@/api";
import { AttachedBills } from "@/components/projects/Bills";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useTheme } from "@/hooks/theme";
import { formatDate, formatMoney } from "@/lib/utils";

export const TEMPLATES = ["renovation", "construction", "wedding", "trip", "vehicle", "event", "business", "blank"] as const;
export type Template = (typeof TEMPLATES)[number];

export const today = () => { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; };
const str = (n: number | null | undefined) => n == null ? "" : String(n);
const textarea = "border-input dark:bg-input/30 focus-visible:border-ring focus-visible:ring-ring/50 rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:ring-[3px]";

/** A standard column's heading: the project's own name for it, or the app's word. */
export function columnName(p: { labels?: Partial<Record<NameableColumn, string>> }, key: NameableColumn, t: (k: string) => string) {
  return p.labels?.[key]?.trim() || t(`projects.col.${key}`);
}

function Field({ id, label, children, className }: { id: string; label: string; children: ReactNode; className?: string }) {
  return <div className={`grid gap-1.5 ${className ?? ""}`}><Label htmlFor={id}>{label}</Label>{children}</div>;
}

// ------------------------------------------------------------------ project

export function ProjectDialog({ project, onClose, onSaved }: { project: ProjectDetail | null; onClose: () => void; onSaved: (id: string) => void }) {
  const { t } = useTranslation();
  const [template, setTemplate] = useState<Template>("renovation");
  const sectionsOf = (k: Template) => k === "blank" ? [] : t(`projects.templates.${k}.sections`).split("|").map((s) => s.trim()).filter(Boolean);
  const [v, setV] = useState<ProjectInput>(() => project ? {
    name: project.name, kind: project.kind, status: project.status, budget: str(project.budget), start_date: project.start_date ?? "", end_date: project.end_date ?? "",
    notes: project.notes ?? "", columns: project.columns, hidden: project.hidden, labels: project.labels ?? {}, improvement: project.improvement === 1,
  } : { name: "", kind: "personal", status: "ongoing", budget: "", start_date: today(), end_date: "", notes: "", columns: [], hidden: ["bill_no"], labels: {}, sections: sectionsOf("renovation") });
  const [newSection, setNewSection] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (p: Partial<ProjectInput>) => setV((x) => ({ ...x, ...p }));
  const setColumn = (i: number, p: Partial<ProjectColumn>) => set({ columns: v.columns.map((c, j) => j === i ? { ...c, ...p } : c) });
  const pickTemplate = (k: Template) => {
    setTemplate(k);
    set({ sections: sectionsOf(k), kind: k === "business" ? "business" : k === "wedding" || k === "renovation" || k === "construction" ? "family" : v.kind,
      name: v.name || (k === "blank" ? "" : t(`projects.templates.${k}.name`)) });
  };
  const addSection = () => {
    const n = newSection.trim();
    if (n && !(v.sections ?? []).some((s) => s.toLowerCase() === n.toLowerCase())) set({ sections: [...(v.sections ?? []), n] });
    setNewSection("");
  };

  const save = async (e: FormEvent) => {
    e.preventDefault(); setBusy(true); setError("");
    try {
      const body = { ...v, columns: v.columns.map((c) => ({ ...c, options: c.type === "choice" ? c.options : [] })) };
      const saved = project ? await api.projects.update(project.id, body) : await api.projects.create(body);
      onSaved(saved.id);
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{project ? t("projects.editTitle") : t("projects.newTitle")}</DialogTitle>
          <DialogDescription>{t("projects.dialogDesc")}</DialogDescription>
        </DialogHeader>
        <form onSubmit={save} className="grid gap-4">
          {!project && (
            <fieldset className="grid gap-2">
              <legend className="mb-1.5 text-sm font-medium">{t("projects.startFrom")}</legend>
              <div className="flex flex-wrap gap-2">
                {TEMPLATES.map((k) => (
                  <label key={k} className="has-[:checked]:border-primary has-[:checked]:bg-primary/5 has-[:focus-visible]:ring-ring/50 cursor-pointer rounded-md border px-3 py-1.5 text-sm has-[:focus-visible]:ring-[3px]">
                    <input type="radio" name="template" value={k} checked={template === k} onChange={() => pickTemplate(k)} className="sr-only" />
                    {t(`projects.templates.${k}.label`)}
                  </label>
                ))}
              </div>
            </fieldset>
          )}
          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_10rem_10rem]">
            <Field id="pj-name" label={t("projects.name")}>
              <Input id="pj-name" required maxLength={100} value={v.name} onChange={(e) => set({ name: e.target.value })} autoFocus={!project} placeholder={t("projects.namePlaceholder")} />
            </Field>
            <Field id="pj-kind" label={t("projects.kindLabel")}>
              <NativeSelect id="pj-kind" value={v.kind} onChange={(e) => set({ kind: e.target.value as ProjectKind })}>
                {PROJECT_KINDS.map((k) => <option key={k} value={k}>{t(`projects.kinds.${k}`)}</option>)}
              </NativeSelect>
            </Field>
            <Field id="pj-status" label={t("projects.statusLabel")}>
              <NativeSelect id="pj-status" value={v.status} onChange={(e) => set({ status: e.target.value as ProjectStatus })}>
                {PROJECT_STATUSES.map((k) => <option key={k} value={k}>{t(`projects.statuses.${k}`)}</option>)}
              </NativeSelect>
            </Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-3">
            <Field id="pj-budget" label={t("projects.budget")}><Input id="pj-budget" inputMode="decimal" autoComplete="off" value={v.budget ?? ""} onChange={(e) => set({ budget: e.target.value })} /></Field>
            <Field id="pj-start" label={t("projects.start")}><Input id="pj-start" type="date" value={v.start_date ?? ""} onChange={(e) => set({ start_date: e.target.value })} /></Field>
            <Field id="pj-end" label={t("projects.end")}><Input id="pj-end" type="date" value={v.end_date ?? ""} onChange={(e) => set({ end_date: e.target.value })} /></Field>
          </div>

          <label className="flex items-start gap-2 text-sm">
            <input type="checkbox" className="accent-primary mt-0.5 size-4" checked={!!v.improvement} onChange={(e) => set({ improvement: e.target.checked })} aria-describedby="pj-improve-hint" />
            <span>{t("projects.improvement")}<span id="pj-improve-hint" className="text-muted-foreground block text-xs">{t("projects.improvementHelp")}</span></span>
          </label>

          {!project && (
            <section className="grid gap-2">
              <h3 className="text-sm font-medium">{t("projects.sections")}</h3>
              <p className="text-muted-foreground text-xs">{t("projects.sectionsHelp")}</p>
              <ul className="flex flex-wrap gap-1.5">
                {(v.sections ?? []).map((s) => (
                  <li key={s} className="bg-secondary flex items-center gap-1 rounded-full py-0.5 ps-3 pe-1 text-sm">
                    {s}
                    <button type="button" className="hover:bg-background/70 rounded-full p-1" aria-label={t("projects.removeSection", { name: s })} onClick={() => set({ sections: (v.sections ?? []).filter((x) => x !== s) })}><X className="size-3" /></button>
                  </li>
                ))}
              </ul>
              <div className="flex gap-2">
                <Input aria-label={t("projects.newSection")} placeholder={t("projects.newSection")} maxLength={60} value={newSection} onChange={(e) => setNewSection(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addSection(); } }} />
                <Button type="button" variant="outline" onClick={addSection} disabled={!newSection.trim()}><Plus />{t("projects.add")}</Button>
              </div>
            </section>
          )}

          <section className="grid gap-2">
            <h3 className="text-sm font-medium">{t("projects.standardColumns")}</h3>
            <p className="text-muted-foreground text-xs">{t("projects.standardColumnsHelp")}</p>
            <div className="grid gap-x-4 gap-y-2 sm:grid-cols-2">
              {NAMEABLE_COLUMNS.map((c) => {
                const hideable = (HIDEABLE_COLUMNS as readonly string[]).includes(c);
                const shown = !hideable || !v.hidden.includes(c as HideableColumn);
                const word = t(`projects.col.${c}`);
                return (
                  <div key={c} className="flex items-center gap-2">
                    {hideable
                      ? <input type="checkbox" className="accent-primary size-4 shrink-0" aria-label={t("projects.showColumn", { name: v.labels[c] || word })} checked={shown}
                          onChange={(e) => set({ hidden: e.target.checked ? v.hidden.filter((h) => h !== c) : [...v.hidden, c as HideableColumn] })} />
                      : <span className="size-4 shrink-0" aria-hidden />}
                    <Input className="h-8" aria-label={t("projects.columnNameFor", { name: word })} placeholder={word} maxLength={40} disabled={!shown}
                      value={v.labels[c] ?? ""} onChange={(e) => set({ labels: { ...v.labels, [c]: e.target.value } })} />
                  </div>
                );
              })}
            </div>
          </section>

          <section className="grid gap-2">
            <h3 className="text-sm font-medium">{t("projects.ownColumns")}</h3>
            <p className="text-muted-foreground text-xs">{t("projects.ownColumnsHelp")}</p>
            {v.columns.map((c, i) => (
              <div key={c.id ?? `new-${i}`} className="grid gap-2 rounded-md border p-2 sm:grid-cols-[minmax(0,1fr)_9rem_auto]">
                <Input aria-label={t("projects.columnName")} placeholder={t("projects.columnName")} maxLength={40} value={c.name} onChange={(e) => setColumn(i, { name: e.target.value })} />
                <NativeSelect aria-label={t("projects.columnType")} value={c.type} onChange={(e) => setColumn(i, { type: e.target.value as ColumnType })}>
                  {COLUMN_TYPES.map((k) => <option key={k} value={k}>{t(`projects.columnTypes.${k}`)}</option>)}
                </NativeSelect>
                <Button type="button" variant="ghost" size="icon" aria-label={t("projects.removeColumn", { name: c.name || "…" })} onClick={() => set({ columns: v.columns.filter((_, j) => j !== i) })}><Trash2 /></Button>
                {c.type === "choice" && (
                  <Input className="sm:col-span-3" aria-label={t("projects.choices")} placeholder={t("projects.choices")} value={c.options.join(", ")}
                    onChange={(e) => setColumn(i, { options: e.target.value.split(",").map((o) => o.trimStart()) })} onBlur={() => setColumn(i, { options: c.options.map((o) => o.trim()).filter(Boolean) })} />
                )}
              </div>
            ))}
            {v.columns.length < 12 && <Button type="button" variant="outline" className="justify-self-start" onClick={() => set({ columns: [...v.columns, { name: "", type: "text", options: [] }] })}><Plus />{t("projects.addColumn")}</Button>}
          </section>

          <Field id="pj-notes" label={t("projects.notes")}><textarea id="pj-notes" rows={2} maxLength={2000} className={textarea} value={v.notes ?? ""} onChange={(e) => set({ notes: e.target.value })} /></Field>
          {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
          <DialogFooter className="gap-2">
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={busy || !v.name.trim()}>{busy && <Loader2 className="animate-spin" />}{project ? t("common.save") : t("projects.create")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// ------------------------------------------------------------------ sections

/** The sections, with budgets, in order. Changes are saved as they're made. */
export function SectionsEditor({ project, onChanged }: { project: ProjectDetail; onChanged: () => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [name, setName] = useState("");
  const [budget, setBudget] = useState("");
  const [editing, setEditing] = useState<{ id: string; name: string; budget: string } | null>(null);
  const [error, setError] = useState("");
  const money = (n: number) => formatMoney(n, project.currency, hideBalances);
  const run = async (f: () => Promise<unknown>) => { setError(""); try { await f(); onChanged(); } catch (e) { setError(errorText(e)); } };
  const move = (i: number, d: -1 | 1) => {
    const ids = project.sections.map((s) => s.id);
    [ids[i], ids[i + d]] = [ids[i + d], ids[i]];
    run(() => api.projects.orderSections(project.id, ids));
  };

  return (
    <div className="grid gap-3">
      <div className="overflow-x-auto rounded-lg border">
        <table className="w-full min-w-[36rem] text-sm">
          <caption className="sr-only">{t("projects.sections")}</caption>
          <thead className="bg-muted/50 text-muted-foreground text-xs">
            <tr>
              <th scope="col" className="px-3 py-2 text-start font-medium">{columnName(project, "section", t)}</th>
              <th scope="col" className="px-3 py-2 text-end font-medium">{t("projects.budget")}</th>
              <th scope="col" className="px-3 py-2 text-end font-medium">{t("projects.billed")}</th>
              <th scope="col" className="px-3 py-2 text-end font-medium">{t("projects.paid")}</th>
              <th scope="col" className="px-3 py-2"><span className="sr-only">{t("projects.actions")}</span></th>
            </tr>
          </thead>
          <tbody className="divide-y">
            {project.sections.map((s, i) => editing?.id === s.id ? (
              <tr key={s.id}>
                <td className="px-3 py-1.5"><Input aria-label={t("projects.col.section")} maxLength={60} value={editing.name} onChange={(e) => setEditing({ ...editing, name: e.target.value })} autoFocus /></td>
                <td className="px-3 py-1.5"><Input aria-label={t("projects.budget")} inputMode="decimal" className="text-end" value={editing.budget} onChange={(e) => setEditing({ ...editing, budget: e.target.value })} /></td>
                <td colSpan={3} className="px-3 py-1.5 text-end">
                  <Button size="sm" variant="ghost" onClick={() => setEditing(null)}>{t("common.cancel")}</Button>
                  <Button size="sm" onClick={() => run(async () => { await api.projects.updateSection(s.id, { name: editing.name, budget: editing.budget }); setEditing(null); })}>{t("common.save")}</Button>
                </td>
              </tr>
            ) : (
              <tr key={s.id}>
                <td className="px-3 py-2"><button className="text-start hover:underline" onClick={() => setEditing({ id: s.id, name: s.name, budget: str(s.budget) })}>{s.name}</button>
                  <span className="text-muted-foreground block text-xs">{t("projects.itemCount", { count: s.items })}</span></td>
                <td className="px-3 py-2 text-end tabular-nums">{s.budget == null ? <span className="text-muted-foreground">—</span> : money(s.budget)}</td>
                <td className={`px-3 py-2 text-end tabular-nums ${s.budget != null && s.billed > s.budget ? "text-red-700 dark:text-red-300" : ""}`}>{money(s.billed)}</td>
                <td className="px-3 py-2 text-end tabular-nums">{money(s.paid)}</td>
                <td className="px-2 py-1 text-end whitespace-nowrap">
                  <Button size="icon" variant="ghost" aria-label={t("projects.editSection", { name: s.name })} onClick={() => setEditing({ id: s.id, name: s.name, budget: str(s.budget) })}><Pencil /></Button>
                  <Button size="icon" variant="ghost" disabled={i === 0} aria-label={t("projects.moveUp", { name: s.name })} onClick={() => move(i, -1)}><ArrowUp /></Button>
                  <Button size="icon" variant="ghost" disabled={i === project.sections.length - 1} aria-label={t("projects.moveDown", { name: s.name })} onClick={() => move(i, 1)}><ArrowDown /></Button>
                  <Button size="icon" variant="ghost" aria-label={t("projects.removeSection", { name: s.name })} onClick={() => run(() => api.projects.deleteSection(s.id))}><Trash2 /></Button>
                </td>
              </tr>
            ))}
            {project.unsectioned.items > 0 && (
              <tr className="text-muted-foreground">
                <td className="px-3 py-2">{t("projects.noSection")}<span className="block text-xs">{t("projects.itemCount", { count: project.unsectioned.items })}</span></td>
                <td /><td className="px-3 py-2 text-end tabular-nums">{money(project.unsectioned.billed)}</td><td /><td />
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <form className="flex flex-wrap gap-2" onSubmit={(e) => { e.preventDefault(); run(async () => { await api.projects.addSection(project.id, { name, budget }); setName(""); setBudget(""); }); }}>
        <Input className="min-w-48 flex-1" aria-label={t("projects.newSection")} placeholder={t("projects.newSection")} maxLength={60} value={name} onChange={(e) => setName(e.target.value)} />
        <Input className="w-36" aria-label={t("projects.budget")} placeholder={t("projects.budgetOptional")} inputMode="decimal" value={budget} onChange={(e) => setBudget(e.target.value)} />
        <Button disabled={!name.trim()}><Plus />{t("projects.addSection")}</Button>
      </form>
      <p className="text-muted-foreground text-xs">{t("projects.removeSectionHelp")}</p>
      {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
    </div>
  );
}

// ------------------------------------------------------------------ items

/** For a row not saved yet: a bill chosen now and kept once the row is saved. */
function PendingBill({ path, setPath }: { path: string; setPath: (p: string) => void }) {
  const { t } = useTranslation();
  const choose = async () => { try { const p = await api.files.pick("document", t("projects.billFiles")); if (p) setPath(p); } catch { /* the picker reports nothing useful */ } };
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-lg border p-3 text-sm">
      <Paperclip className="text-muted-foreground size-4" aria-hidden />
      <span className="min-w-0 flex-1 truncate">{path ? path.split(/[\\/]/).pop() : t("projects.noBillYet")}</span>
      {path && <Button type="button" size="sm" variant="ghost" onClick={() => setPath("")}>{t("projects.dontAttach")}</Button>}
      <Button type="button" size="sm" variant="outline" onClick={choose}>{path ? t("projects.chooseOther") : t("projects.attachBill")}</Button>
    </div>
  );
}

type Names = { vendors: string[]; account_names: string[]; units: string[] };

function CustomInput({ column, value, onChange }: { column: ProjectDetail["columns"][number]; value: CustomValue | undefined; onChange: (v: CustomValue | "") => void }) {
  const { t } = useTranslation();
  const id = `pi-c-${column.id}`;
  const input = (() => {
    switch (column.type) {
      case "number": return <Input id={id} inputMode="decimal" value={value == null ? "" : String(value)} onChange={(e) => onChange(e.target.value)} />;
      case "date": return <Input id={id} type="date" value={typeof value === "string" ? value : ""} onChange={(e) => onChange(e.target.value)} />;
      case "choice": return (
        <NativeSelect id={id} value={typeof value === "string" ? value : ""} onChange={(e) => onChange(e.target.value)}>
          <option value="">—</option>{column.options.map((o) => <option key={o} value={o}>{o}</option>)}
        </NativeSelect>);
      case "yes_no": return (
        <NativeSelect id={id} value={value === true ? "yes" : value === false ? "no" : ""} onChange={(e) => onChange(e.target.value === "" ? "" : e.target.value === "yes")}>
          <option value="">—</option><option value="yes">{t("projects.yes")}</option><option value="no">{t("projects.no")}</option>
        </NativeSelect>);
      default: return <Input id={id} maxLength={200} value={typeof value === "string" ? value : ""} onChange={(e) => onChange(e.target.value)} />;
    }
  })();
  return <Field id={id} label={column.name}>{input}</Field>;
}

const TAX_RATES = ["0", "5", "12", "18", "28"];

export function ItemDialog({ project, item, names, files = [], onFiles, onClose, onSaved }: {
  project: ProjectDetail; item: ProjectItem | null; names: Names; files?: ProjectFile[]; onFiles?: () => void; onClose: () => void; onSaved: () => void;
}) {
  const [bill, setBill] = useState("");
  const { t } = useTranslation();
  const hides = (c: string) => project.hidden.includes(c as never);
  const blank = (prev?: ItemInput): ItemInput => ({ date: prev?.date ?? today(), item: "", kind: prev?.kind ?? "material", section_id: prev?.section_id ?? "", qty: "", unit: "", rate: "", amount: "",
    tax_percent: prev?.tax_percent ?? "", tax: "", vendor: prev?.vendor ?? "", bill_no: prev?.bill_no ?? "", notes: "", custom: {} });
  const [v, setV] = useState<ItemInput>(() => item ? {
    date: item.date, item: item.item, kind: item.kind, section_id: item.section_id ?? "", qty: str(item.qty), unit: item.unit ?? "", rate: str(item.rate),
    amount: str(item.amount), tax: item.tax ? str(item.tax) : "", tax_percent: "", vendor: item.vendor ?? "", bill_no: item.bill_no ?? "", notes: item.notes ?? "", custom: item.custom,
    warranty_until: item.warranty_until ?? "",
  } : blank());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [added, setAdded] = useState(0);
  const set = (p: Partial<ItemInput>) => setV((x) => ({ ...x, ...p }));
  const computed = useMemo(() => {
    const q = Number(v.qty), r = Number(v.rate);
    return v.qty && v.rate && Number.isFinite(q * r) ? (Math.round(q * r * 100) / 100).toString() : "";
  }, [v.qty, v.rate]);
  const money = (n: number) => formatMoney(n, project.currency);
  const base = Number(v.amount || computed || 0);
  const taxNow = v.tax ? Number(v.tax) : v.tax_percent ? Math.round(base * Number(v.tax_percent)) / 100 : 0;

  const save = async (another: boolean) => {
    setBusy(true); setError("");
    try {
      const body: ItemInput = { ...v, custom: Object.fromEntries(Object.entries(v.custom).filter(([, x]) => x !== "")) };
      const saved = item ? await api.projects.updateItem(item.id, body) : await api.projects.addItem(project.id, body);
      if (bill) { await api.projects.addFile(project.id, bill, { item_id: saved.id }); setBill(""); }
      if (another) { setV((x) => blank(x)); setAdded((n) => n + 1); document.getElementById("pi-item")?.focus(); } else onSaved();
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const remove = async () => { if (!item) return; try { await api.projects.deleteItem(item.id); onSaved(); } catch (err) { setError(errorText(err)); } };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) { if (added) onSaved(); else onClose(); } }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl" onOpenAutoFocus={(e) => { if (item) e.preventDefault(); }}>
        <DialogHeader>
          <DialogTitle>{item ? t("projects.editItem") : t("projects.addItem")}</DialogTitle>
          <DialogDescription>{added ? t("projects.itemsAdded", { count: added }) : t("projects.itemDesc")}</DialogDescription>
        </DialogHeader>
        <form className="grid gap-3" onSubmit={(e) => { e.preventDefault(); save(false); }}>
          <datalist id="pi-vendors">{names.vendors.map((n) => <option key={n} value={n} />)}</datalist>
          <datalist id="pi-units">{names.units.map((n) => <option key={n} value={n} />)}</datalist>
          <div className="grid gap-3 sm:grid-cols-[9.5rem_minmax(0,1fr)]">
            <Field id="pi-date" label={columnName(project, "date", t)}><Input id="pi-date" type="date" required value={v.date} onChange={(e) => set({ date: e.target.value })} /></Field>
            <Field id="pi-item" label={columnName(project, "item", t)}><Input id="pi-item" required maxLength={200} autoFocus={!item} value={v.item} onChange={(e) => set({ item: e.target.value })} placeholder={t("projects.itemPlaceholder")} /></Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            {!hides("section") && (
              <Field id="pi-section" label={columnName(project, "section", t)}>
                <NativeSelect id="pi-section" value={v.section_id ?? ""} onChange={(e) => set({ section_id: e.target.value })}>
                  <option value="">{t("projects.noSection")}</option>
                  {project.sections.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
                </NativeSelect>
              </Field>
            )}
            {!hides("kind") && (
              <Field id="pi-kind" label={columnName(project, "kind", t)}>
                <NativeSelect id="pi-kind" value={v.kind} onChange={(e) => set({ kind: e.target.value as ItemKind })}>
                  {ITEM_KINDS.map((k) => <option key={k} value={k}>{t(`projects.itemKinds.${k}`)}</option>)}
                </NativeSelect>
              </Field>
            )}
          </div>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {!hides("qty") && <>
              <Field id="pi-qty" label={columnName(project, "qty", t)}><Input id="pi-qty" inputMode="decimal" autoComplete="off" value={v.qty ?? ""} onChange={(e) => set({ qty: e.target.value })} /></Field>
              <Field id="pi-unit" label={columnName(project, "unit", t)}><Input id="pi-unit" list="pi-units" maxLength={20} value={v.unit ?? ""} onChange={(e) => set({ unit: e.target.value })} placeholder={t("projects.unitPlaceholder")} /></Field>
            </>}
            {!hides("rate") && <Field id="pi-rate" label={columnName(project, "rate", t)}><Input id="pi-rate" inputMode="decimal" autoComplete="off" value={v.rate ?? ""} onChange={(e) => set({ rate: e.target.value })} /></Field>}
            <Field id="pi-amount" label={columnName(project, "amount", t)}>
              <Input id="pi-amount" inputMode="decimal" autoComplete="off" value={v.amount ?? ""} placeholder={computed} onChange={(e) => set({ amount: e.target.value })} aria-describedby={computed && !v.amount ? "pi-amount-hint" : undefined} />
            </Field>
          </div>
          {computed && !v.amount && <p id="pi-amount-hint" className="text-muted-foreground -mt-2 text-xs">{t("projects.amountWorkedOut", { amount: money(Number(computed)) })}</p>}
          {!hides("tax") && (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <Field id="pi-taxp" label={t("projects.taxRate")}>
                <NativeSelect id="pi-taxp" value={v.tax ? "custom" : v.tax_percent ?? ""} onChange={(e) => set(e.target.value === "custom" ? { tax_percent: "", tax: String(taxNow || "") } : { tax_percent: e.target.value, tax: "" })}>
                  <option value="">{t("projects.noTax")}</option>
                  {TAX_RATES.slice(1).map((r) => <option key={r} value={r}>{r}%</option>)}
                  <option value="custom">{t("projects.taxAmount")}</option>
                </NativeSelect>
              </Field>
              {v.tax ? <Field id="pi-tax" label={columnName(project, "tax", t)}><Input id="pi-tax" inputMode="decimal" value={v.tax} onChange={(e) => set({ tax: e.target.value })} /></Field>
                : <div className="text-muted-foreground self-end pb-2 text-sm tabular-nums" aria-live="polite">{taxNow ? t("projects.taxIs", { amount: money(taxNow) }) : ""}</div>}
              <div className="col-span-2 self-end pb-2 text-end text-sm" aria-live="polite">{t("projects.totalIs")} <strong className="tabular-nums">{money(base + taxNow)}</strong></div>
            </div>
          )}
          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_10rem]">
            {!hides("vendor") && <Field id="pi-vendor" label={columnName(project, "vendor", t)}><Input id="pi-vendor" list="pi-vendors" maxLength={100} value={v.vendor ?? ""} onChange={(e) => set({ vendor: e.target.value })} /></Field>}
            {!hides("bill_no") && <Field id="pi-bill" label={columnName(project, "bill_no", t)}><Input id="pi-bill" maxLength={60} value={v.bill_no ?? ""} onChange={(e) => set({ bill_no: e.target.value })} /></Field>}
          </div>
          <div className="grid gap-3 sm:grid-cols-[10rem_minmax(0,1fr)] sm:items-end">
            <Field id="pi-warranty" label={t("projects.warrantyUntil")}><Input id="pi-warranty" type="date" value={v.warranty_until ?? ""} min={v.date} onChange={(e) => set({ warranty_until: e.target.value })} aria-describedby="pi-warranty-hint" /></Field>
            <p id="pi-warranty-hint" className="text-muted-foreground pb-2 text-xs">{item?.document_id ? t("projects.warrantyInDocuments") : t("projects.warrantyHelp")}</p>
          </div>
          {project.columns.length > 0 && (
            <div className="grid gap-3 sm:grid-cols-2">
              {project.columns.map((c) => <CustomInput key={c.id} column={c} value={v.custom[c.id]} onChange={(x) => set({ custom: { ...v.custom, [c.id]: x } })} />)}
            </div>
          )}
          {!hides("notes") && <Field id="pi-notes" label={columnName(project, "notes", t)}><textarea id="pi-notes" rows={2} maxLength={1000} className={textarea} value={v.notes ?? ""} onChange={(e) => set({ notes: e.target.value })} /></Field>}
          {item ? <AttachedBills project={project} files={files.filter((f) => f.item_id === item.id)} link={{ item_id: item.id }} onChanged={() => onFiles?.()} />
            : <PendingBill path={bill} setPath={setBill} />}
          {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
          <DialogFooter className="gap-2">
            {item && <Button type="button" variant="ghost" className="text-destructive me-auto" onClick={remove}><Trash2 />{t("common.delete")}</Button>}
            <Button type="button" variant="outline" onClick={() => added ? onSaved() : onClose()}>{added ? t("projects.done") : t("common.cancel")}</Button>
            {!item && <Button type="button" variant="outline" disabled={busy || !v.item.trim()} onClick={() => save(true)}>{t("projects.saveAndNext")}</Button>}
            <Button disabled={busy || !v.item.trim()}>{busy && <Loader2 className="animate-spin" />}{t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// ------------------------------------------------------------------ payments

export function PaymentDialog({ project, payment, names, files = [], onFiles, onClose, onSaved }: {
  project: ProjectDetail; payment: ProjectPayment | null; names: Names; files?: ProjectFile[]; onFiles?: () => void; onClose: () => void; onSaved: () => void;
}) {
  const [bill, setBill] = useState("");
  const { t } = useTranslation();
  const [v, setV] = useState<PaymentInput>(() => payment ? {
    date: payment.date, amount: str(payment.amount), vendor: payment.vendor ?? "", mode: payment.mode, account_name: payment.account_name ?? "", reference: payment.reference ?? "",
    section_id: payment.section_id ?? "", notes: payment.notes ?? "", paid_by: payment.paid_by ?? "",
  } : { date: today(), amount: "", vendor: "", mode: "upi", account_name: "", reference: "", section_id: "", notes: "", paid_by: "" });
  const [members, setMembers] = useState<ProjectMember[]>([]);
  useEffect(() => { api.projects.members(project.id).then(setMembers).catch(() => {}); }, [project.id]);
  const [record, setRecord] = useState(false);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [account, setAccount] = useState("");
  const [category, setCategory] = useState("");
  const [unlink, setUnlink] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (p: Partial<PaymentInput>) => setV((x) => ({ ...x, ...p }));
  useEffect(() => {
    if (payment) return;
    api.accounts.getAll().then((a) => { const ok = a.filter((x) => x.currency === project.currency); setAccounts(ok); setAccount((c) => c || ok[0]?.id || ""); }).catch(() => {});
    api.categories.getAll().then((c) => setCategories(c.filter((x) => x.type === "expense"))).catch(() => {});
  }, [payment, project.currency]);

  const save = async (e: FormEvent) => {
    e.preventDefault(); setBusy(true); setError("");
    try {
      if (payment) await api.projects.updatePayment(payment.id, { ...v, ...(unlink ? { entry_id: "" } : {}) });
      else {
        const saved = await api.projects.addPayment(project.id, { ...v, ...(record && account ? { record: { account_id: account, category_id: category || undefined } } : {}) });
        if (bill) await api.projects.addFile(project.id, bill, { payment_id: saved.id });
      }
      onSaved();
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const remove = async () => { if (!payment) return; try { await api.projects.deletePayment(payment.id); onSaved(); } catch (err) { setError(errorText(err)); } };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl" onOpenAutoFocus={(e) => { if (payment) e.preventDefault(); }}>
        <DialogHeader>
          <DialogTitle>{payment ? t("projects.editPayment") : t("projects.addPayment")}</DialogTitle>
          <DialogDescription>{t("projects.paymentDesc")}</DialogDescription>
        </DialogHeader>
        <form className="grid gap-3" onSubmit={save}>
          <datalist id="pp-vendors">{names.vendors.map((n) => <option key={n} value={n} />)}</datalist>
          <datalist id="pp-accnames">{names.account_names.map((n) => <option key={n} value={n} />)}</datalist>
          <div className="grid grid-cols-2 gap-3">
            <Field id="pp-date" label={t("projects.col.date")}><Input id="pp-date" type="date" required value={v.date} onChange={(e) => set({ date: e.target.value })} /></Field>
            <Field id="pp-amount" label={t("projects.col.amount")}><Input id="pp-amount" required inputMode="decimal" autoComplete="off" autoFocus={!payment} value={v.amount} onChange={(e) => set({ amount: e.target.value })} /></Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field id="pp-vendor" label={t("projects.paidTo")}><Input id="pp-vendor" list="pp-vendors" maxLength={100} value={v.vendor ?? ""} onChange={(e) => set({ vendor: e.target.value })} /></Field>
            <Field id="pp-accname" label={t("projects.accountName")}><Input id="pp-accname" list="pp-accnames" maxLength={100} value={v.account_name ?? ""} onChange={(e) => set({ account_name: e.target.value })} aria-describedby="pp-accname-hint" /></Field>
          </div>
          <p id="pp-accname-hint" className="text-muted-foreground -mt-2 text-xs">{t("projects.accountNameHelp")}</p>
          <div className="grid gap-3 sm:grid-cols-3">
            <Field id="pp-mode" label={t("projects.mode")}>
              <NativeSelect id="pp-mode" value={v.mode} onChange={(e) => set({ mode: e.target.value as PayMode })}>
                {PAY_MODES.map((m) => <option key={m} value={m}>{t(`projects.modes.${m}`)}</option>)}
              </NativeSelect>
            </Field>
            <Field id="pp-ref" label={t("projects.reference")}><Input id="pp-ref" maxLength={60} value={v.reference ?? ""} onChange={(e) => set({ reference: e.target.value })} /></Field>
            <Field id="pp-section" label={columnName(project, "section", t)}>
              <NativeSelect id="pp-section" value={v.section_id ?? ""} onChange={(e) => set({ section_id: e.target.value })}>
                <option value="">{t("projects.noSection")}</option>
                {project.sections.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
              </NativeSelect>
            </Field>
          </div>
          {members.length > 0 && (
            <Field id="pp-paidby" label={t("projects.paidBy")}>
              <NativeSelect id="pp-paidby" value={v.paid_by ?? ""} onChange={(e) => { set({ paid_by: e.target.value }); if (e.target.value) setRecord(false); }}>
                <option value="">{t("projects.you")}</option>
                {members.filter((m) => !m.is_me).map((m) => <option key={m.id} value={m.id}>{m.name}</option>)}
              </NativeSelect>
            </Field>
          )}
          <Field id="pp-notes" label={t("projects.col.notes")}><textarea id="pp-notes" rows={2} maxLength={1000} className={textarea} value={v.notes ?? ""} onChange={(e) => set({ notes: e.target.value })} /></Field>
          {payment ? <AttachedBills project={project} files={files.filter((f) => f.payment_id === payment.id)} link={{ payment_id: payment.id }} onChanged={() => onFiles?.()} />
            : <PendingBill path={bill} setPath={setBill} />}

          {!payment && (
            <section className="grid gap-2 rounded-lg border p-3">
              <label className="flex items-center justify-between gap-3 text-sm font-medium" htmlFor="pp-record">
                {t("projects.alsoRecord")}
                <Switch id="pp-record" checked={record} onCheckedChange={setRecord} disabled={accounts.length === 0 || !!v.paid_by} />
              </label>
              <p className="text-muted-foreground text-xs">{accounts.length === 0 ? t("projects.noAccounts", { currency: project.currency }) : t("projects.alsoRecordHelp")}</p>
              {record && (
                <div className="grid gap-3 sm:grid-cols-2">
                  <Field id="pp-account" label={t("projects.fromAccount")}>
                    <NativeSelect id="pp-account" value={account} onChange={(e) => setAccount(e.target.value)}>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect>
                  </Field>
                  <Field id="pp-category" label={t("projects.category")}>
                    <NativeSelect id="pp-category" value={category} onChange={(e) => setCategory(e.target.value)}>
                      <option value="">{t("projects.uncategorised")}</option>
                      {categories.map((c) => <option key={c.id} value={c.id}>{c.icon ? `${c.icon} ` : ""}{c.name}</option>)}
                    </NativeSelect>
                  </Field>
                </div>
              )}
            </section>
          )}
          {payment?.entry_id && (
            <div className="bg-muted/50 flex flex-wrap items-center gap-2 rounded-lg p-3 text-sm">
              <Link2 className="text-muted-foreground size-4" aria-hidden />
              <span className="flex-1">{unlink ? t("projects.willUnlink") : t("projects.linkedTo", { account: payment.from_account ?? "—" })}</span>
              <Button type="button" size="sm" variant="ghost" onClick={() => setUnlink(!unlink)}><Unlink />{unlink ? t("projects.keepLink") : t("projects.unlink")}</Button>
            </div>
          )}
          {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
          <DialogFooter className="gap-2">
            {payment && <Button type="button" variant="ghost" className="text-destructive me-auto" onClick={remove}><Trash2 />{t("common.delete")}</Button>}
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={busy || !v.amount.trim()}>{busy && <Loader2 className="animate-spin" />}{t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Payments that look like transactions already in Vaulty (e.g. from a bank statement): link them. */
export function LinkDialog({ project, suggestions, payments, onClose }: { project: ProjectDetail; suggestions: LinkSuggestion[]; payments: ProjectPayment[]; onClose: (changed: boolean) => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [pick, setPick] = useState<Record<string, string>>(() => Object.fromEntries(suggestions.map((s) => [s.payment_id, s.candidates[0].named ? s.candidates[0].id : ""])));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const chosen = Object.values(pick).filter(Boolean).length;
  const link = async () => {
    setBusy(true); setError("");
    try {
      for (const [pid, entry] of Object.entries(pick)) {
        if (!entry) continue;
        const p = payments.find((x) => x.id === pid);
        if (!p) continue;
        await api.projects.updatePayment(pid, { date: p.date, amount: String(p.amount), vendor: p.vendor ?? "", mode: p.mode, account_name: p.account_name ?? "", reference: p.reference ?? "", section_id: p.section_id ?? "", notes: p.notes ?? "", entry_id: entry });
      }
      onClose(true);
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(false); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("projects.linkTitle")}</DialogTitle>
          <DialogDescription>{t("projects.linkDesc")}</DialogDescription>
        </DialogHeader>
        <div className="grid gap-3">
          {suggestions.map((s) => {
            const p = payments.find((x) => x.id === s.payment_id);
            if (!p) return null;
            return (
              <fieldset key={s.payment_id} className="rounded-lg border p-3">
                <legend className="px-1 text-sm font-medium">
                  {formatDate(p.date)} · {formatMoney(p.amount, project.currency, hideBalances)}{p.vendor ? ` · ${p.vendor}` : ""}{p.account_name ? ` (${p.account_name})` : ""}
                </legend>
                <div className="grid gap-1">
                  {s.candidates.map((c) => (
                    <label key={c.id} className="hover:bg-accent/50 flex cursor-pointer items-start gap-2 rounded-md px-2 py-1.5 text-sm">
                      <input type="radio" className="accent-primary mt-1" name={`link-${s.payment_id}`} checked={pick[s.payment_id] === c.id} onChange={() => setPick({ ...pick, [s.payment_id]: c.id })} />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate">{c.merchant || c.description || "—"}</span>
                        <span className="text-muted-foreground block text-xs">{formatDate(c.date)}{c.account ? ` · ${c.account}` : ""}{c.named ? ` · ${t("projects.nameMatches")}` : ""}</span>
                      </span>
                    </label>
                  ))}
                  <label className="hover:bg-accent/50 flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-sm">
                    <input type="radio" className="accent-primary" name={`link-${s.payment_id}`} checked={!pick[s.payment_id]} onChange={() => setPick({ ...pick, [s.payment_id]: "" })} />
                    {t("projects.dontLink")}
                  </label>
                </div>
              </fieldset>
            );
          })}
          {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
          <DialogFooter className="gap-2">
            <Button type="button" variant="outline" onClick={() => onClose(false)}>{t("common.cancel")}</Button>
            <Button onClick={link} disabled={busy || chosen === 0}>{busy && <Loader2 className="animate-spin" />}{t("projects.linkN", { count: chosen })}</Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
}
