import { useEffect, useMemo, useState, type FormEvent } from "react";
import { CalendarClock, ChevronLeft, ChevronRight, Download, Plus, Upload } from "lucide-react";
import { useTranslation } from "react-i18next";
import i18n from "@/i18n";
import { api, DATA_RESTORED, errorText, type CalendarEventInput, type CalendarItem, type CalendarKind, type IcsEvent } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useLocale } from "@/hooks/locale";
import { useTheme } from "@/hooks/theme";
import { resolveWeekStart } from "@/lib/formats";
import { cn, formatDate, formatMoney } from "@/lib/utils";

const pad = (n: number) => String(n).padStart(2, "0");
/** A local date as YYYY-MM-DD (no time-zone shift). */
const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const parse = (s: string) => { const [y, m, d] = s.split("-").map(Number); return new Date(y, m - 1, d); };
const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);

/** Colour and default icon per kind; the dot carries the colour, the words carry the meaning. */
const KIND: Record<CalendarKind, { dot: string; icon: string }> = {
  card_due: { dot: "bg-rose-500", icon: "💳" },
  bill: { dot: "bg-amber-500", icon: "🧾" },
  transfer: { dot: "bg-slate-400", icon: "🔁" },
  income: { dot: "bg-emerald-500", icon: "💰" },
  statement: { dot: "bg-slate-300 dark:bg-slate-600", icon: "📄" },
  paid: { dot: "bg-emerald-300 dark:bg-emerald-700", icon: "✓" },
  holiday: { dot: "bg-violet-500", icon: "🏖️" },
  event: { dot: "bg-sky-500", icon: "📌" },
  reminder: { dot: "bg-teal-500", icon: "⏰" },
  task: { dot: "bg-indigo-500", icon: "☑️" },
  expiry: { dot: "bg-orange-500", icon: "🪪" },
  tax: { dot: "bg-red-600", icon: "🏛️" },
};
/** Advance tax items carry the tax year as their title. */
const itemTitle = (it: { kind: CalendarKind; title: string }) => it.kind === "tax" ? i18n.t("calendar.taxTitle", { year: it.title }) : it.title;
const OWN: CalendarKind[] = ["event", "reminder", "holiday"];

export function CalendarPage() {
  const { t } = useTranslation();
  const { locale, display } = useLocale();
  const today = useMemo(() => new Date(), []);
  const [month, setMonth] = useState(() => new Date(today.getFullYear(), today.getMonth(), 1));
  const [items, setItems] = useState<CalendarItem[]>([]);
  const [soon, setSoon] = useState<CalendarItem[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState<CalendarEventInput & { id?: string } | null>(null);
  const [importing, setImporting] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  const reload = () => setVersion((v) => v + 1);

  // Six weeks starting on the member's first day of the week.
  const weekStart = ({ sunday: 0, monday: 1, saturday: 6 } as const)[resolveWeekStart(display.weekStart, locale)];
  const gridStart = addDays(month, -((month.getDay() - weekStart + 7) % 7));
  const days = Array.from({ length: 42 }, (_, i) => addDays(gridStart, i));

  useEffect(() => {
    setError("");
    api.calendar.range(iso(days[0]), iso(days[41])).then(setItems).catch((e) => setError(errorText(e)));
    api.calendar.range(iso(today), iso(addDays(today, 30))).then(setSoon).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [month, version]);
  useEffect(() => { window.addEventListener(DATA_RESTORED, reload); return () => window.removeEventListener(DATA_RESTORED, reload); }, []);

  const byDay = useMemo(() => {
    const m = new Map<string, CalendarItem[]>();
    for (const it of items) m.set(it.date, [...(m.get(it.date) ?? []), it]);
    return m;
  }, [items]);
  // Only the earliest unpaid date of each bill can be marked paid (that's the one due next).
  const payable = useMemo(() => {
    const first = new Map<string, string>();
    for (const it of [...items, ...soon].filter((i) => ["bill", "income", "transfer"].includes(i.kind) && i.source_id)) {
      const k = it.source_id!;
      if (!first.has(k) || it.date < first.get(k)!) first.set(k, it.date);
    }
    return first;
  }, [items, soon]);

  const overdue = items.filter((i) => i.overdue);
  // Not yet paid comes first, then the next 30 days.
  const agenda = selected ? byDay.get(selected) ?? [] : [...overdue, ...soon.filter((i) => i.kind !== "statement" && !i.overdue)];
  const monthName = month.toLocaleDateString(locale, { month: "long", year: "numeric" });
  const weekdays = days.slice(0, 7).map((d) => d.toLocaleDateString(locale, { weekday: "short" }));

  const open = (it: CalendarItem) => {
    if (!OWN.includes(it.kind) || !it.source_id || it.source_id.startsWith("fixed:")) return;
    api.calendar.events().then((all) => {
      const e = all.find((x) => x.id === it.source_id);
      if (e) setEditing({ id: e.id, title: e.title, date: e.date, time: e.time ?? "", type: e.type, repeat: e.repeat ?? "none", amount: e.amount != null ? String(e.amount) : "", icon: e.icon ?? "", notes: e.notes ?? "" });
    });
  };
  const pay = async (it: CalendarItem) => {
    try { await api.recurring.pay(it.source_id!); window.dispatchEvent(new Event(DATA_RESTORED)); } catch (e) { setError(errorText(e)); }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <div className="flex items-center gap-1">
          <Button variant="outline" size="icon" aria-label={t("calendar.previous")} onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() - 1, 1))}><ChevronLeft /></Button>
          <Button variant="outline" size="sm" onClick={() => { setMonth(new Date(today.getFullYear(), today.getMonth(), 1)); setSelected(null); }}>{t("calendar.today")}</Button>
          <Button variant="outline" size="icon" aria-label={t("calendar.next")} onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() + 1, 1))}><ChevronRight /></Button>
        </div>
        <h2 className="ms-2 text-lg font-semibold capitalize" aria-live="polite">{monthName}</h2>
        <div className="ms-auto flex flex-wrap gap-2">
          <Button variant="ghost" size="sm" onClick={() => setImporting(true)}><Upload />{t("calendar.import")}</Button>
          <Button variant="ghost" size="sm" onClick={() => setExporting(true)}><Download />{t("calendar.export")}</Button>
          <Button size="sm" onClick={() => setEditing({ title: "", date: selected ?? iso(today), time: "", type: "custom", repeat: "none", amount: "", icon: "", notes: "" })}><Plus />{t("calendar.add")}</Button>
        </div>
      </div>
      {error && <p className="text-destructive text-sm">{error}</p>}

      <div className="grid gap-4 lg:grid-cols-[1fr_20rem]">
        <Card className="gap-0 overflow-hidden py-0">
          <div className="text-muted-foreground grid grid-cols-7 border-b text-center text-xs font-medium">
            {weekdays.map((w, i) => <div key={i} className="py-2">{w}</div>)}
          </div>
          <div className="grid grid-cols-7" role="group" aria-label={monthName}>
            {days.map((d) => {
              const key = iso(d);
              const list = byDay.get(key) ?? [];
              const holiday = list.find((i) => i.kind === "holiday");
              const inMonth = d.getMonth() === month.getMonth();
              const isToday = key === iso(today);
              const shown = list.filter((i) => i.kind !== "holiday");
              return (
                <button key={key} type="button" aria-pressed={selected === key} onClick={() => setSelected(selected === key ? null : key)}
                  className={cn("hover:bg-accent/60 flex min-h-24 flex-col items-stretch gap-0.5 border-r border-b p-1.5 text-left align-top [&:nth-child(7n)]:border-r-0",
                    !inMonth && "bg-muted/30 text-muted-foreground", holiday && "bg-violet-500/5", selected === key && "ring-primary ring-2 ring-inset")}>
                  <span className="flex items-center gap-1">
                    {/* Screen readers hear the full date; the grid shows the day number. */}
                    <span className="sr-only">{formatDate(key, { weekday: "long", day: "numeric", month: "long" })}{isToday ? `, ${t("calendar.today")}` : ""}</span>
                    <span aria-hidden className={cn("grid size-6 place-items-center rounded-full text-xs tabular-nums", isToday && "bg-primary text-primary-foreground font-semibold")}>{d.getDate()}</span>
                    {holiday && <span className="truncate text-[10px] text-violet-700 dark:text-violet-300">{holiday.title}</span>}
                  </span>
                  {shown.slice(0, 3).map((it, i) => (
                    <span key={i} className={cn("flex items-center gap-1 truncate text-[11px] leading-tight", it.kind === "paid" && "text-muted-foreground line-through", it.overdue && "font-medium text-red-700 dark:text-red-400")}>
                      <span className={cn("size-1.5 shrink-0 rounded-full", KIND[it.kind].dot)} />{itemTitle(it)}
                    </span>
                  ))}
                  {shown.length > 3 && <span className="text-muted-foreground text-[10px]">{t("calendar.more", { count: shown.length - 3 })}</span>}
                </button>
              );
            })}
          </div>
        </Card>

        <Card className="gap-3">
          <CardHeader className="pb-0">
            <CardTitle className="flex items-center gap-2 text-base"><CalendarClock className="size-4" />
              {selected ? formatDate(selected, { weekday: "long", day: "numeric", month: "long" }) : t("calendar.comingUp")}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-1">
            {!selected && overdue.length > 0 && <p className="text-destructive pb-1 text-xs font-medium">{t("calendar.overdue", { count: overdue.length })}</p>}
            {agenda.length === 0 && <p className="text-muted-foreground py-6 text-center text-sm">{selected ? t("calendar.nothingDay") : t("calendar.nothingSoon")}</p>}
            {agenda.map((it, i) => (
              <AgendaRow key={i} item={it} showDate={!selected} canPay={payable.get(it.source_id ?? "") === it.date} onPay={() => pay(it)} onOpen={() => open(it)} />
            ))}
          </CardContent>
        </Card>
      </div>

      {editing && <EventDialog value={editing} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); reload(); }} />}
      {importing && <ImportDialog onClose={() => setImporting(false)} onDone={() => { setImporting(false); reload(); }} />}
      {exporting && <ExportDialog onClose={() => setExporting(false)} />}
    </div>
  );
}

function AgendaRow({ item: it, showDate, canPay, onPay, onOpen }: { item: CalendarItem; showDate: boolean; canPay: boolean; onPay: () => void; onOpen: () => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const editable = OWN.includes(it.kind) && !!it.source_id && !it.source_id.startsWith("fixed:");
  return (
    <div className="flex items-start gap-2.5 rounded-md py-2">
      <span className="bg-secondary grid size-8 shrink-0 place-items-center rounded-md text-sm" aria-hidden>{it.icon || KIND[it.kind].icon}</span>
      <div className="min-w-0 flex-1">
        {editable ? <button type="button" onClick={onOpen} className="block max-w-full truncate text-left text-sm font-medium hover:underline">{itemTitle(it)}</button>
          : <div className={cn("truncate text-sm font-medium", it.kind === "paid" && "text-muted-foreground")}>{itemTitle(it)}</div>}
        <div className="text-muted-foreground text-xs">
          {showDate && `${formatDate(it.date, { weekday: "short", day: "numeric", month: "short" })}${it.time ? ` ${it.time}` : ""} · `}
          <span className={cn(it.overdue && "text-destructive font-medium")}>{it.overdue ? t("calendar.overdueOne") : t(`calendar.kinds.${it.kind}`)}</span>
        </div>
        {it.pay_by && <div className="text-xs text-amber-800 dark:text-amber-300">{t("calendar.payBy", { date: formatDate(it.pay_by, { weekday: "short", day: "numeric", month: "short" }) })}</div>}
      </div>
      {(it.amount != null || canPay) && (
        <div className="flex shrink-0 flex-col items-end gap-1">
          {it.amount != null && <span className={cn("text-sm tabular-nums", it.kind === "income" && "text-success")}>{formatMoney(it.amount, it.currency ?? "INR", hideBalances)}</span>}
          {canPay && <Button variant="outline" size="sm" className="h-6 px-2 text-xs" onClick={onPay}>{it.kind === "income" ? t("calendar.received") : t("calendar.markPaid")}</Button>}
        </div>
      )}
    </div>
  );
}

function EventDialog({ value, onClose, onSaved }: { value: CalendarEventInput & { id?: string }; onClose: () => void; onSaved: () => void }) {
  const { t } = useTranslation();
  const [v, setV] = useState(value);
  const [error, setError] = useState("");
  const set = (p: Partial<CalendarEventInput>) => setV((x) => ({ ...x, ...p }));
  const save = async (e: FormEvent) => {
    e.preventDefault(); setError("");
    const { id, ...body } = v;
    try { await (id ? api.calendar.update(id, body) : api.calendar.create(body)); onSaved(); } catch (err) { setError(errorText(err)); }
  };
  const remove = async () => { try { await api.calendar.delete(v.id!); onSaved(); } catch (err) { setError(errorText(err)); } };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader><DialogTitle>{v.id ? t("calendar.editTitle") : t("calendar.addTitle")}</DialogTitle><DialogDescription>{t("calendar.addDesc")}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="grid gap-3">
          <div className="grid gap-1.5"><Label htmlFor="ev-title">{t("calendar.name")}</Label><Input id="ev-title" autoFocus value={v.title} maxLength={80} onChange={(e) => set({ title: e.target.value })} /></div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-1.5"><Label htmlFor="ev-date">{t("calendar.date")}</Label><Input id="ev-date" type="date" value={v.date} onChange={(e) => set({ date: e.target.value })} /></div>
            <div className="grid gap-1.5"><Label htmlFor="ev-time">{t("calendar.time")}</Label><Input id="ev-time" type="time" value={v.time} onChange={(e) => set({ time: e.target.value })} /></div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-1.5"><Label htmlFor="ev-type">{t("calendar.type")}</Label>
              <NativeSelect id="ev-type" value={v.type} onChange={(e) => set({ type: e.target.value as CalendarEventInput["type"] })}>
                <option value="custom">{t("calendar.kinds.event")}</option><option value="reminder">{t("calendar.kinds.reminder")}</option><option value="holiday">{t("calendar.kinds.holiday")}</option>
              </NativeSelect></div>
            <div className="grid gap-1.5"><Label htmlFor="ev-repeat">{t("calendar.repeat")}</Label>
              <NativeSelect id="ev-repeat" value={v.repeat} onChange={(e) => set({ repeat: e.target.value as CalendarEventInput["repeat"] })}>
                <option value="none">{t("calendar.repeatNone")}</option><option value="monthly">{t("calendar.repeatMonthly")}</option><option value="yearly">{t("calendar.repeatYearly")}</option>
              </NativeSelect></div>
          </div>
          <div className="grid grid-cols-[1fr_5rem] gap-3">
            <div className="grid gap-1.5"><Label htmlFor="ev-amount">{t("calendar.amount")}</Label><Input id="ev-amount" inputMode="decimal" value={v.amount} onChange={(e) => set({ amount: e.target.value })} /></div>
            <div className="grid gap-1.5"><Label htmlFor="ev-icon">{t("calendar.icon")}</Label><Input id="ev-icon" value={v.icon} maxLength={4} placeholder="📌" onChange={(e) => set({ icon: e.target.value })} /></div>
          </div>
          <div className="grid gap-1.5"><Label htmlFor="ev-notes">{t("calendar.notes")}</Label><Input id="ev-notes" value={v.notes} onChange={(e) => set({ notes: e.target.value })} /></div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            {v.id && <Button type="button" variant="ghost" className="text-destructive me-auto" onClick={remove}>{t("common.delete")}</Button>}
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={!v.title.trim() || !v.date}>{t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ImportDialog({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const { t } = useTranslation();
  const [file, setFile] = useState<{ path: string; events: IcsEvent[] } | null>(null);
  const [asHolidays, setAsHolidays] = useState(true);
  const [error, setError] = useState("");
  const [done, setDone] = useState<number | null>(null);
  const choose = async () => {
    setError("");
    const path = await api.files.pick("calendar", t("calendar.files"));
    if (!path) return;
    try { setFile({ path, events: await api.calendar.importPreview(path) }); } catch (e) { setError(errorText(e)); }
  };
  const go = async () => {
    try { setDone(await api.calendar.import(file!.path, asHolidays)); } catch (e) { setError(errorText(e)); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) (done != null ? onDone : onClose)(); }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader><DialogTitle>{t("calendar.importTitle")}</DialogTitle><DialogDescription>{t("calendar.importDesc")}</DialogDescription></DialogHeader>
        {done != null ? <p className="text-sm">{t("calendar.imported", { count: done })}</p> : !file ? (
          <Button variant="outline" onClick={choose}><Upload />{t("calendar.chooseFile")}</Button>
        ) : (
          <div className="space-y-3">
            <p className="text-sm">{t("calendar.found", { count: file.events.length })}</p>
            <ul className="text-muted-foreground max-h-32 space-y-0.5 overflow-y-auto text-xs">
              {file.events.slice(0, 8).map((e, i) => <li key={i}>{formatDate(e.date)} · {e.title}</li>)}
              {file.events.length > 8 && <li>…</li>}
            </ul>
            <fieldset className="space-y-1.5 text-sm">
              <legend className="mb-1 font-medium">{t("calendar.importAs")}</legend>
              <label className="flex items-center gap-2"><input type="radio" checked={asHolidays} onChange={() => setAsHolidays(true)} />{t("calendar.asHolidays")}</label>
              <label className="flex items-center gap-2"><input type="radio" checked={!asHolidays} onChange={() => setAsHolidays(false)} />{t("calendar.asEvents")}</label>
            </fieldset>
          </div>
        )}
        {error && <p className="text-destructive text-sm">{error}</p>}
        <DialogFooter>
          {done != null ? <Button onClick={onDone}>{t("common.done")}</Button> : <>
            <Button variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={!file?.events.length} onClick={go}>{t("calendar.importButton")}</Button>
          </>}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ExportDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const [months, setMonths] = useState(12);
  const [amounts, setAmounts] = useState(false);
  const [note, setNote] = useState("");
  const [error, setError] = useState("");
  const go = async () => {
    setError("");
    const path = await api.files.pick("save_calendar", t("calendar.files"));
    if (!path) return;
    const now = new Date();
    const to = new Date(now.getFullYear(), now.getMonth() + months, now.getDate());
    try { setNote(t("calendar.exported", { count: await api.calendar.export(path, iso(now), iso(to), amounts) })); } catch (e) { setError(errorText(e)); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader><DialogTitle>{t("calendar.exportTitle")}</DialogTitle><DialogDescription>{t("calendar.exportDesc")}</DialogDescription></DialogHeader>
        <div className="grid gap-3 text-sm">
          <div className="grid gap-1.5"><Label htmlFor="ex-range">{t("calendar.exportRange")}</Label>
            <NativeSelect id="ex-range" value={months} onChange={(e) => setMonths(Number(e.target.value))}>
              <option value={3}>{t("calendar.nextMonths", { count: 3 })}</option><option value={12}>{t("calendar.nextMonths", { count: 12 })}</option>
            </NativeSelect></div>
          <label className="flex items-start gap-2"><input type="checkbox" className="mt-1" checked={amounts} onChange={(e) => setAmounts(e.target.checked)} />
            <span>{t("calendar.includeAmounts")}<span className="text-muted-foreground block text-xs">{t("calendar.includeAmountsDesc")}</span></span></label>
          {note && <p className="text-success">{note}</p>}
          {error && <p className="text-destructive">{error}</p>}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{note ? t("common.done") : t("common.cancel")}</Button>
          {!note && <Button onClick={go}><Download />{t("calendar.exportButton")}</Button>}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
