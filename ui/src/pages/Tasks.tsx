import { useEffect, useMemo, useState, type FormEvent } from "react";
import { CalendarDays, Flag, Link2, Plus, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type Task, type TaskInput, type TaskLink } from "@/api";
import { Segmented } from "@/components/Segmented";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn, formatDate } from "@/lib/utils";

const pad = (n: number) => String(n).padStart(2, "0");
const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const LINK_ICON: Record<TaskLink, string> = { recurring: "🧾", transaction: "💳", person: "👤", event: "📌" };
type Group = "overdue" | "today" | "week" | "later" | "undated" | "done";

export function TasksPage() {
  const { t } = useTranslation();
  const [view, setView] = useState<"open" | "done">("open");
  const [tasks, setTasks] = useState<Task[]>([]);
  const [title, setTitle] = useState("");
  const [editing, setEditing] = useState<Partial<Task> | null>(null);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  const reload = () => setVersion((v) => v + 1);
  useEffect(() => { window.addEventListener(DATA_RESTORED, reload); return () => window.removeEventListener(DATA_RESTORED, reload); }, []);
  useEffect(() => { api.tasks.list(view).then(setTasks).catch((e) => setError(errorText(e))); }, [view, version]);

  const today = iso(new Date()), week = iso(new Date(Date.now() + 7 * 86_400_000));
  const groups = useMemo(() => {
    const by: Partial<Record<Group, Task[]>> = {};
    for (const x of tasks) {
      const g: Group = x.status === "done" ? "done" : !x.due_date ? "undated" : x.due_date < today ? "overdue" : x.due_date === today ? "today" : x.due_date <= week ? "week" : "later";
      (by[g] ??= []).push(x);
    }
    return (["overdue", "today", "week", "later", "undated", "done"] as Group[]).filter((g) => by[g]?.length).map((g) => ({ g, list: by[g]! }));
  }, [tasks, today, week]);

  const add = async (e: FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    setError("");
    try { await api.tasks.create({ title, priority: "medium" }); setTitle(""); reload(); } catch (err) { setError(errorText(err)); }
  };
  const toggle = async (x: Task) => {
    // Shown done at once; the list catches up when the save returns.
    setTasks((l) => l.map((y) => y.id === x.id ? { ...y, status: y.status === "done" ? "pending" : "done" } : y));
    try { await api.tasks.setDone(x.id, x.status !== "done"); } catch (e) { setError(errorText(e)); }
    setTimeout(reload, 600);
  };

  return (
    <div className="max-w-3xl space-y-4">
      <div className="flex flex-wrap items-center gap-3">
        <form onSubmit={add} className="flex min-w-64 flex-1 gap-2">
          <Input aria-label={t("tasks.new")} placeholder={t("tasks.new")} maxLength={200} value={title} onChange={(e) => setTitle(e.target.value)} />
          <Button disabled={!title.trim()}><Plus />{t("tasks.add")}</Button>
        </form>
        <Segmented value={view} onChange={setView} label={t("tasks.show")} options={[{ value: "open", label: t("tasks.open") }, { value: "done", label: t("tasks.doneTab") }]} />
      </div>
      {error && <p className="text-destructive text-sm">{error}</p>}
      {tasks.length === 0 && (
        <Card><CardContent className="py-10 text-center">
          <p className="font-medium">{view === "open" ? t("tasks.emptyTitle") : t("tasks.noneDone")}</p>
          {view === "open" && <p className="text-muted-foreground mx-auto mt-1 max-w-md text-sm">{t("tasks.emptyDesc")}</p>}
        </CardContent></Card>
      )}
      {groups.map(({ g, list }) => (
        <Card key={g} className="gap-1 py-2">
          <CardHeader className="pt-3"><CardTitle className={cn("text-sm", g === "overdue" && "text-red-700 dark:text-red-400")}>{t(`tasks.groups.${g}`)}</CardTitle></CardHeader>
          <CardContent className="divide-y">
            {list.map((x) => (
              <div key={x.id} className="flex items-start gap-3 py-2">
                <input type="checkbox" className="accent-primary mt-1 size-4 shrink-0" checked={x.status === "done"} onChange={() => toggle(x)}
                  aria-label={x.status === "done" ? t("tasks.markOpen", { title: x.title }) : t("tasks.markDone", { title: x.title })} />
                <div className="min-w-0 flex-1">
                  <button type="button" onClick={() => setEditing(x)} className={cn("block max-w-full truncate text-left text-sm hover:underline", x.status === "done" && "text-muted-foreground line-through")}>{x.title}</button>
                  <div className="text-muted-foreground flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs">
                    {x.due_date && <span className={cn("flex items-center gap-1", x.overdue && "font-medium text-red-700 dark:text-red-400")}><CalendarDays className="size-3" aria-hidden />{formatDate(x.due_date, { weekday: "short", day: "numeric", month: "short" })}</span>}
                    {x.priority === "high" && <span className="flex items-center gap-1 text-amber-800 dark:text-amber-300"><Flag className="size-3" aria-hidden />{t("tasks.priority.high")}</span>}
                    {x.linked_type && x.linked_name && (
                      <span className={cn("flex items-center gap-1", x.link_missing && "line-through")} title={x.link_missing ? t("tasks.linkGone") : undefined}>
                        <span aria-hidden>{LINK_ICON[x.linked_type]}</span>{x.linked_name}{x.link_missing && <span className="sr-only"> ({t("tasks.linkGone")})</span>}
                      </span>
                    )}
                    {x.notes && <span className="truncate">{x.notes}</span>}
                  </div>
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      ))}
      {editing && <TaskDialog value={editing} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); reload(); }} />}
    </div>
  );
}

type Option = { id: string; name: string };

function TaskDialog({ value, onClose, onSaved }: { value: Partial<Task>; onClose: () => void; onSaved: () => void }) {
  const { t } = useTranslation();
  const [v, setV] = useState<TaskInput>({
    title: value.title ?? "", notes: value.notes ?? "", due_date: value.due_date ?? "", priority: value.priority ?? "medium",
    linked_type: value.linked_type ?? "", linked_id: value.linked_id ?? "",
  });
  const [options, setOptions] = useState<Option[]>([]);
  const [error, setError] = useState("");
  const set = (p: Partial<TaskInput>) => setV((x) => ({ ...x, ...p }));

  // What can be linked, for the chosen kind.
  useEffect(() => {
    const kind = v.linked_type;
    const load: Promise<Option[]> = kind === "recurring" ? api.recurring.list().then((l) => l.map((r) => ({ id: r.id, name: r.name })))
      : kind === "person" ? api.people.list().then((l) => l.map((p) => ({ id: p.id, name: p.name })))
      : kind === "event" ? api.calendar.events().then((l) => l.map((e) => ({ id: e.id, name: `${e.title} · ${formatDate(e.date)}` })))
      : kind === "transaction" ? api.transactions.getAll({ limit: 200, hidden: "exclude" }).then((l) => l.map((x) => ({ id: x.id, name: `${x.merchant || x.description || x.category_name || "—"} · ${formatDate(x.date)}` })))
      : Promise.resolve([]);
    load.then((o) => {
      // Keep a link whose item has gone, so saving doesn't drop it silently.
      if (value.linked_id && kind === value.linked_type && !o.some((x) => x.id === value.linked_id)) o = [{ id: value.linked_id, name: `${value.linked_name ?? "?"} (${t("tasks.linkGone")})` }, ...o];
      setOptions(o);
    }).catch(() => setOptions([]));
  }, [v.linked_type, value.linked_id, value.linked_type, value.linked_name, t]);

  const save = async (e: FormEvent) => {
    e.preventDefault(); setError("");
    const body: TaskInput = { ...v, linked_type: v.linked_type && v.linked_id ? v.linked_type : "", linked_id: v.linked_type ? v.linked_id : "" };
    try { await (value.id ? api.tasks.update(value.id, body) : api.tasks.create(body)); onSaved(); } catch (err) { setError(errorText(err)); }
  };
  const remove = async () => { try { await api.tasks.delete(value.id!); onSaved(); } catch (err) { setError(errorText(err)); } };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader><DialogTitle>{value.id ? t("tasks.edit") : t("tasks.add")}</DialogTitle><DialogDescription>{t("tasks.editDesc")}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="grid gap-3">
          <div className="grid gap-1.5"><Label htmlFor="t-title">{t("tasks.title")}</Label><Input id="t-title" autoFocus maxLength={200} value={v.title} onChange={(e) => set({ title: e.target.value })} /></div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-1.5"><Label htmlFor="t-due">{t("tasks.due")}</Label><Input id="t-due" type="date" value={v.due_date} onChange={(e) => set({ due_date: e.target.value })} /></div>
            <div className="grid gap-1.5"><Label htmlFor="t-pri">{t("tasks.priorityLabel")}</Label>
              <NativeSelect id="t-pri" value={v.priority} onChange={(e) => set({ priority: e.target.value as Task["priority"] })}>
                {(["high", "medium", "low"] as const).map((p) => <option key={p} value={p}>{t(`tasks.priority.${p}`)}</option>)}
              </NativeSelect></div>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="t-link" className="flex items-center gap-1.5"><Link2 className="size-3.5" />{t("tasks.linkTo")}</Label>
            <div className="grid grid-cols-[9rem_1fr] gap-2">
              <NativeSelect id="t-link" value={v.linked_type} onChange={(e) => set({ linked_type: e.target.value as TaskLink | "", linked_id: "" })}>
                <option value="">{t("tasks.links.none")}</option>
                {(["recurring", "person", "event", "transaction"] as TaskLink[]).map((k) => <option key={k} value={k}>{t(`tasks.links.${k}`)}</option>)}
              </NativeSelect>
              {v.linked_type && (
                <NativeSelect aria-label={t(`tasks.links.${v.linked_type}`)} value={v.linked_id} onChange={(e) => set({ linked_id: e.target.value })}>
                  <option value="">{t("tasks.choose")}</option>
                  {options.map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
                </NativeSelect>
              )}
            </div>
            {v.linked_type === "recurring" && <p className="text-muted-foreground text-xs">{t("tasks.billHint")}</p>}
          </div>
          <div className="grid gap-1.5"><Label htmlFor="t-notes">{t("tasks.notes")}</Label><Input id="t-notes" maxLength={2000} value={v.notes} onChange={(e) => set({ notes: e.target.value })} /></div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            {value.id && <Button type="button" variant="ghost" className="text-destructive me-auto" onClick={remove}><Trash2 />{t("common.delete")}</Button>}
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={!v.title.trim()}>{t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
