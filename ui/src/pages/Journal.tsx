import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { BookOpen, CalendarCheck, Link2, Loader2, Pencil, Plus, RefreshCw, Search, Trash2, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, MOODS, type JournalEntry, type JournalInput, type MonthFacts, type Mood, type Transaction } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTheme } from "@/hooks/theme";
import { today } from "@/components/projects/ProjectDialogs";
import { cn, formatDate, formatMoney, formatNumber } from "@/lib/utils";

const FACE: Record<Mood, string> = { great: "😄", good: "🙂", okay: "😐", worried: "😟", stressed: "😣" };
const WHOLE: Intl.NumberFormatOptions = { maximumFractionDigits: 0, minimumFractionDigits: 0 };
const monthName = (m: string) => formatDate(`${m}-01`, { month: "long", year: "numeric" });

export function JournalPage() {
  const { t } = useTranslation();
  const [data, setData] = useState<{ entries: JournalEntry[]; tags: string[]; review_due: string | null } | null>(null);
  const [search, setSearch] = useState("");
  const [tag, setTag] = useState("");
  const [editing, setEditing] = useState<JournalEntry | { review_of?: string } | null>(null);
  const [error, setError] = useState("");
  const load = useCallback(() => { api.journal.list(search, tag).then((d) => { setData(d); setError(""); }).catch((e) => setError(errorText(e))); }, [search, tag]);
  useEffect(() => { const h = setTimeout(load, 200); return () => clearTimeout(h); }, [load]);
  useEffect(() => { window.addEventListener(DATA_RESTORED, load); return () => window.removeEventListener(DATA_RESTORED, load); }, [load]);

  const groups = useMemo(() => {
    const m = new Map<string, JournalEntry[]>();
    for (const e of data?.entries ?? []) { const k = e.date.slice(0, 7); m.set(k, [...(m.get(k) ?? []), e]); }
    return [...m];
  }, [data]);
  if (!data) return error ? <p role="alert" className="text-destructive text-sm">{error}</p> : null;
  const filtering = !!search.trim() || !!tag;

  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-48 flex-1">
          <Search className="text-muted-foreground pointer-events-none absolute top-2.5 left-2.5 size-4" aria-hidden />
          <Input className="ps-8" aria-label={t("journal.search")} placeholder={t("journal.search")} value={search} onChange={(e) => setSearch(e.target.value)} />
        </div>
        <Button onClick={() => setEditing({})}><Plus />{t("journal.new")}</Button>
      </div>
      {data.tags.length > 0 && (
        <div className="flex flex-wrap gap-1.5" role="group" aria-label={t("journal.tags")}>
          {data.tags.map((x) => (
            <button key={x} type="button" aria-pressed={tag === x} onClick={() => setTag(tag === x ? "" : x)}
              className={cn("rounded-full border px-2.5 py-0.5 text-xs", tag === x ? "bg-primary text-primary-foreground border-primary" : "hover:bg-accent")}>#{x}</button>
          ))}
        </div>
      )}
      {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
      {data.review_due && !filtering && (
        <div className="flex flex-wrap items-center gap-3 rounded-xl border border-dashed p-4">
          <CalendarCheck className="text-muted-foreground size-5" />
          <p className="min-w-0 flex-1 text-sm">{t("journal.reviewDue", { month: monthName(data.review_due) })}</p>
          <Button variant="outline" onClick={() => setEditing({ review_of: data.review_due! })}>{t("journal.writeReview")}</Button>
        </div>
      )}
      {data.entries.length === 0 ? (
        <Card><CardContent className="space-y-3 py-10 text-center">
          <div className="bg-secondary mx-auto grid size-12 place-items-center rounded-xl"><BookOpen className="size-5" /></div>
          <p className="font-semibold">{filtering ? t("journal.noMatch") : t("journal.emptyTitle")}</p>
          {!filtering && <p className="text-muted-foreground mx-auto max-w-md text-sm">{t("journal.emptyBody")}</p>}
        </CardContent></Card>
      ) : groups.map(([month, list]) => (
        <section key={month} aria-labelledby={`m-${month}`} className="space-y-3">
          <h2 id={`m-${month}`} className="text-muted-foreground text-sm font-medium">{monthName(month)}</h2>
          {list.map((e) => <EntryCard key={e.id} e={e} onEdit={() => setEditing(e)} onChange={load} onTag={setTag} />)}
        </section>
      ))}
      {editing && <EntryDialog start={editing} onClose={(changed) => { setEditing(null); if (changed) load(); }} />}
    </div>
  );
}

function Facts({ f }: { f: MonthFacts }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const m = (n: number) => formatMoney(n, f.currency, hideBalances, WHOLE);
  return (
    <div className="bg-muted/40 space-y-3 rounded-lg p-3 text-sm">
      <dl className="grid grid-cols-3 gap-2">
        <div><dt className="text-muted-foreground text-xs">{t("journal.facts.income")}</dt><dd className="font-medium tabular-nums">{m(f.income)}</dd></div>
        <div><dt className="text-muted-foreground text-xs">{t("journal.facts.spending")}</dt><dd className="font-medium tabular-nums">{m(f.spending)}</dd>
          {f.change != null && <dd className="text-muted-foreground text-xs">{t(f.change >= 0 ? "journal.facts.up" : "journal.facts.down", { pct: formatNumber(Math.abs(f.change)) })}</dd>}</div>
        <div><dt className="text-muted-foreground text-xs">{t("journal.facts.kept")}</dt><dd className={cn("font-medium tabular-nums", f.kept < 0 && "text-rose-700 dark:text-rose-400")}>{m(f.kept)}</dd>
          {f.savings_rate != null && <dd className="text-muted-foreground text-xs">{t("journal.facts.rate", { pct: formatNumber(Math.round(f.savings_rate * 100)) })}</dd>}</div>
      </dl>
      {f.categories.length > 0 && <p className="text-muted-foreground text-xs">{t("journal.facts.most")} {f.categories.map((c) => `${c.icon ? c.icon + " " : ""}${c.name} ${m(c.amount)}`).join(" · ")}</p>}
      {f.biggest.length > 0 && <p className="text-muted-foreground text-xs">{t("journal.facts.biggest")} {f.biggest.map((b) => `${b.name} ${m(b.amount)}`).join(" · ")}</p>}
    </div>
  );
}

function EntryCard({ e, onEdit, onChange, onTag }: { e: JournalEntry; onEdit: () => void; onChange: () => void; onTag: (t: string) => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  return (
    <Card className="gap-0 py-4">
      <CardContent className="space-y-3">
        <div className="flex items-start gap-3">
          <div className="w-12 shrink-0 text-center">
            <div className="text-xl font-semibold leading-none tabular-nums">{formatDate(e.date, { day: "numeric" })}</div>
            <div className="text-muted-foreground text-xs">{formatDate(e.date, { weekday: "short" })}</div>
          </div>
          <div className="min-w-0 flex-1">
            <h3 className="font-semibold">{e.title}{e.mood && <span className="ms-2" role="img" aria-label={t(`journal.moods.${e.mood}`)}>{FACE[e.mood]}</span>}</h3>
            {e.review_of && <p className="text-muted-foreground text-xs">{t("journal.reviewOf", { month: monthName(e.review_of) })}</p>}
          </div>
          <Button size="icon" variant="ghost" className="size-8" onClick={onEdit} aria-label={t("journal.editNamed", { title: e.title })}><Pencil className="size-4" /></Button>
          <Button size="icon" variant="ghost" className="text-destructive size-8" onClick={() => api.journal.delete(e.id).then(onChange)} aria-label={t("journal.deleteNamed", { title: e.title })}><Trash2 className="size-4" /></Button>
        </div>
        {e.facts && <Facts f={e.facts} />}
        {e.content && <p className="text-sm leading-relaxed whitespace-pre-wrap">{e.content}</p>}
        {e.linked.length > 0 && (
          <ul className="space-y-1 text-xs">
            {e.linked.map((x) => (
              <li key={x.id} className="text-muted-foreground flex items-center gap-2"><Link2 className="size-3.5 shrink-0" />
                <span className="min-w-0 flex-1 truncate">{formatDate(x.date)} · {x.name}</span>
                {x.amount != null && <span className="tabular-nums">{formatMoney(x.amount, x.currency, hideBalances)}</span>}
              </li>
            ))}
          </ul>
        )}
        {e.tags.length > 0 && <div className="flex flex-wrap gap-1">{e.tags.map((x) => <button key={x} type="button" onClick={() => onTag(x)} className="text-muted-foreground hover:text-foreground text-xs">#{x}</button>)}</div>}
      </CardContent>
    </Card>
  );
}

function EntryDialog({ start, onClose }: { start: JournalEntry | { review_of?: string }; onClose: (changed: boolean) => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const existing = "id" in start ? start as JournalEntry : null;
  const review = existing?.review_of ?? (start as { review_of?: string }).review_of ?? null;
  const [date, setDate] = useState(existing?.date ?? today());
  const [title, setTitle] = useState(existing?.title ?? (review ? monthName(review) : ""));
  const [content, setContent] = useState(existing?.content ?? "");
  const [mood, setMood] = useState<Mood | null>(existing?.mood ?? null);
  const [tags, setTags] = useState((existing?.tags ?? []).join(", "));
  const [linked, setLinked] = useState<{ id: string; label: string }[]>((existing?.linked ?? []).map((x) => ({ id: x.id, label: `${formatDate(x.date)} · ${x.name}` })));
  const [facts, setFacts] = useState<MonthFacts | null>(existing?.facts ?? null);
  const [refresh, setRefresh] = useState(false);
  const [txns, setTxns] = useState<Transaction[]>([]);
  const [find, setFind] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => { if (review && !existing) api.journal.monthFacts(review).then(setFacts).catch(() => {}); }, [review, existing]);
  useEffect(() => { api.transactions.getAll({ limit: 400, hidden: "exclude" }).then(setTxns).catch(() => {}); }, []);
  const matches = find.trim().length < 2 ? [] : txns.filter((x) => !linked.some((l) => l.id === x.id) && [x.merchant, x.description, x.category_name].some((f) => f?.toLowerCase().includes(find.trim().toLowerCase()))).slice(0, 6);

  const save = async (ev: FormEvent) => {
    ev.preventDefault();
    setBusy(true); setError("");
    const entry: JournalInput = { date, title, content, mood, tags: tags.split(",").map((x) => x.trim()).filter(Boolean), linked: linked.map((l) => l.id), ...(review && !existing ? { review_of: review } : {}) };
    try { if (existing) await api.journal.update(existing.id, { ...entry, refresh_facts: refresh }); else await api.journal.create(entry); onClose(true); }
    catch (err) { setError(errorText(err)); setBusy(false); }
  };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(false); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{review ? t("journal.reviewTitle", { month: monthName(review) }) : existing ? t("journal.editTitle") : t("journal.newTitle")}</DialogTitle>
          <DialogDescription>{review ? t("journal.reviewDesc") : t("journal.dialogDesc")}</DialogDescription>
        </DialogHeader>
        <form onSubmit={save} className="grid gap-4">
          {facts && <Facts f={facts} />}
          {existing?.review_of && (
            <label className="flex items-center gap-2 text-xs"><input type="checkbox" className="accent-[var(--primary)]" checked={refresh} onChange={(e) => setRefresh(e.target.checked)} />
              <RefreshCw className="size-3.5" />{t("journal.refreshFacts")}</label>
          )}
          <div className="grid gap-3 sm:grid-cols-[10rem_minmax(0,1fr)]">
            <div className="grid gap-1.5"><Label htmlFor="j-date">{t("journal.date")}</Label><Input id="j-date" type="date" required value={date} onChange={(e) => setDate(e.target.value)} /></div>
            <div className="grid gap-1.5"><Label htmlFor="j-title">{t("journal.titleLabel")}</Label><Input id="j-title" required maxLength={120} autoFocus value={title} onChange={(e) => setTitle(e.target.value)} /></div>
          </div>
          <fieldset className="grid gap-1.5">
            <legend className="mb-1 text-sm font-medium">{t("journal.mood")}</legend>
            <div className="flex flex-wrap gap-1.5" role="radiogroup" aria-label={t("journal.mood")}>
              {MOODS.map((m) => (
                <button key={m} type="button" role="radio" aria-checked={mood === m} onClick={() => setMood(mood === m ? null : m)}
                  className={cn("flex items-center gap-1.5 rounded-full border px-3 py-1 text-sm", mood === m ? "border-primary bg-primary/10" : "hover:bg-accent")}>
                  <span aria-hidden>{FACE[m]}</span>{t(`journal.moods.${m}`)}
                </button>
              ))}
            </div>
          </fieldset>
          <div className="grid gap-1.5"><Label htmlFor="j-content">{t("journal.content")}</Label>
            <textarea id="j-content" rows={6} maxLength={20000} value={content} onChange={(e) => setContent(e.target.value)} placeholder={review ? t("journal.reviewPrompt") : t("journal.prompt")}
              className="border-input dark:bg-input/30 focus-visible:border-ring focus-visible:ring-ring/50 rounded-md border bg-transparent px-3 py-2 text-sm leading-relaxed shadow-xs outline-none focus-visible:ring-[3px]" />
          </div>
          <div className="grid gap-1.5"><Label htmlFor="j-tags">{t("journal.tags")}</Label><Input id="j-tags" value={tags} onChange={(e) => setTags(e.target.value)} placeholder={t("journal.tagsHint")} /></div>
          <div className="grid gap-1.5">
            <Label htmlFor="j-find">{t("journal.linked")}</Label>
            {linked.length > 0 && (
              <ul className="space-y-1">{linked.map((l) => (
                <li key={l.id} className="bg-muted/50 flex items-center gap-2 rounded-md px-2 py-1 text-xs"><Link2 className="size-3.5" /><span className="min-w-0 flex-1 truncate">{l.label}</span>
                  <button type="button" aria-label={t("journal.unlink")} onClick={() => setLinked(linked.filter((x) => x.id !== l.id))}><X className="size-3.5" /></button></li>
              ))}</ul>
            )}
            <Input id="j-find" value={find} onChange={(e) => setFind(e.target.value)} placeholder={t("journal.findTxn")} />
            {matches.length > 0 && (
              <ul className="divide-y rounded-md border text-sm">{matches.map((x) => (
                <li key={x.id}><button type="button" className="hover:bg-accent flex w-full items-center gap-2 px-2 py-1.5 text-start"
                  onClick={() => { setLinked([...linked, { id: x.id, label: `${formatDate(x.date)} · ${x.merchant || x.description || ""}` }]); setFind(""); }}>
                  <span className="min-w-0 flex-1 truncate">{formatDate(x.date)} · {x.merchant || x.description}</span>
                  <span className="tabular-nums">{formatMoney(x.amount, x.currency, hideBalances)}</span>
                </button></li>
              ))}</ul>
            )}
          </div>
          {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            <Button type="button" variant="outline" onClick={() => onClose(false)}>{t("common.cancel")}</Button>
            <Button disabled={busy || !title.trim()}>{busy && <Loader2 className="animate-spin" />}{t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
