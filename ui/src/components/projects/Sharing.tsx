import { useEffect, useState, type FormEvent } from "react";
import { ArrowRight, BookUser, Copy, FileUp, Loader2, Plus, Send, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type Category, type Person, type ProjectDetail, type ProjectMember, type ProjectSplit, type SettleLine, type SharePeek } from "@/api";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTheme } from "@/hooks/theme";
import { cn, formatMoney, formatNumber } from "@/lib/utils";

// ------------------------------------------------------------------ splitting

/** The people sharing the costs, what each paid and owes, and who pays whom to settle up. */
export function SplitPanel({ project, onChanged }: { project: ProjectDetail; onChanged: () => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [split, setSplit] = useState<ProjectSplit | null>(null);
  const [editing, setEditing] = useState<ProjectMember[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [note, setNote] = useState("");
  const [recording, setRecording] = useState(false);
  const [n, setN] = useState(0);
  useEffect(() => { api.projects.split(project.id).then(setSplit).catch((e) => setError(errorText(e))); }, [project.id, n]);
  const money = (x: number) => formatMoney(x, project.currency, hideBalances);
  const startEditing = async () => {
    const ms = await api.projects.members(project.id).catch(() => []);
    setEditing(ms.length ? ms.map((m) => ({ ...m, is_me: !!m.is_me })) : [{ name: t("projects.me"), share: 1, is_me: true }, { name: "", share: 1, is_me: false }]);
  };
  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (!editing) return;
    setBusy(true); setError("");
    try {
      await api.projects.setMembers(project.id, editing.filter((m) => m.name.trim()).map((m) => ({ ...m, share: Number(m.share) })));
      setEditing(null); setN((x) => x + 1); onChanged();
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const set = (i: number, p: Partial<ProjectMember>) => setEditing((ms) => ms && ms.map((m, j) => j === i ? { ...m, ...p } : p.is_me ? { ...m, is_me: false } : m));
  const who = (name: string, me: boolean) => me ? t("projects.you") : name;
  const copy = async () => {
    if (!split) return;
    const text = [t("projects.splitTextTitle", { name: project.name, total: formatMoney(split.total, project.currency) }),
      ...split.members.map((m) => t("projects.splitTextLine", { name: m.name, paid: formatMoney(m.paid, project.currency), owes: formatMoney(m.owes, project.currency) })),
      "", ...split.settle.map((s) => t("projects.settleLine", { from: s.from, to: s.to, amount: formatMoney(s.amount, project.currency) }))].join("\n");
    try { await navigator.clipboard.writeText(text); setNote(t("projects.copied")); } catch { setNote(text); }
  };

  if (editing) {
    return (
      <form onSubmit={save} className="grid gap-3">
        <p className="text-muted-foreground text-sm">{t("projects.membersHelp")}</p>
        <div className="overflow-x-auto rounded-lg border">
          <table className="w-full min-w-[28rem] text-sm">
            <caption className="sr-only">{t("projects.members")}</caption>
            <thead className="bg-muted/50 text-muted-foreground text-xs">
              <tr><th scope="col" className="px-3 py-2 text-start font-medium">{t("projects.person")}</th><th scope="col" className="w-28 px-3 py-2 text-end font-medium">{t("projects.share")}</th>
                <th scope="col" className="w-20 px-3 py-2 text-center font-medium">{t("projects.thisIsMe")}</th><th scope="col" className="w-12"><span className="sr-only">{t("projects.actions")}</span></th></tr>
            </thead>
            <tbody className="divide-y">
              {editing.map((m, i) => (
                <tr key={m.id ?? `new-${i}`}>
                  <td className="px-2 py-1"><Input aria-label={t("projects.person")} maxLength={60} value={m.name} onChange={(e) => set(i, { name: e.target.value })} autoFocus={i === editing.length - 1 && !m.name} /></td>
                  <td className="px-2 py-1"><Input aria-label={t("projects.shareOf", { name: m.name || "…" })} className="text-end" inputMode="decimal" value={String(m.share)} onChange={(e) => set(i, { share: e.target.value as unknown as number })} /></td>
                  <td className="px-2 py-1 text-center"><input type="radio" name="me" className="accent-primary size-4" aria-label={t("projects.isMe", { name: m.name || "…" })} checked={!!m.is_me} onChange={() => set(i, { is_me: true })} /></td>
                  <td className="px-1 py-1"><Button type="button" size="icon" variant="ghost" aria-label={t("projects.removePerson", { name: m.name || "…" })} onClick={() => setEditing(editing.filter((_, j) => j !== i))}><Trash2 /></Button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <Button type="button" variant="outline" className="justify-self-start" onClick={() => setEditing([...editing, { name: "", share: 1, is_me: false }])}><Plus />{t("projects.addPerson")}</Button>
        {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={() => setEditing(null)}>{t("common.cancel")}</Button>
          <Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}{t("common.save")}</Button>
        </div>
      </form>
    );
  }

  if (!split) return error ? <p className="text-destructive text-sm">{error}</p> : null;
  if (split.members.length === 0) {
    return (
      <Card className="items-center gap-2 px-6 py-8 text-center">
        <p className="font-medium">{t("projects.splitEmptyTitle")}</p>
        <p className="text-muted-foreground max-w-md text-sm">{t("projects.splitEmptyDesc")}</p>
        <Button className="mt-2" onClick={startEditing}><Plus />{t("projects.addPeople")}</Button>
      </Card>
    );
  }
  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <p className="text-muted-foreground min-w-0 flex-1 text-sm">{t("projects.splitBasis", { total: money(split.total) })}</p>
        <Button variant="outline" onClick={copy}><Copy />{t("projects.copySummary")}</Button>
        <Button variant="outline" onClick={() => setRecording(true)}><BookUser />{t("projects.recordLend")}</Button>
        <Button variant="outline" onClick={startEditing}>{t("projects.editPeople")}</Button>
      </div>
      {note && <p className="text-sm whitespace-pre-line" role="status">{note}</p>}
      <Card className="py-0">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[34rem] text-sm">
            <caption className="sr-only">{t("projects.members")}</caption>
            <thead className="text-muted-foreground border-b text-xs">
              <tr>
                <th scope="col" className="px-3 py-2 text-start font-medium">{t("projects.person")}</th>
                <th scope="col" className="px-3 py-2 text-end font-medium">{t("projects.share")}</th>
                <th scope="col" className="px-3 py-2 text-end font-medium">{t("projects.paid")}</th>
                <th scope="col" className="px-3 py-2 text-end font-medium">{t("projects.theirShare")}</th>
                <th scope="col" className="px-3 py-2 text-end font-medium">{t("projects.balance")}</th>
              </tr>
            </thead>
            <tbody className="divide-y">
              {split.members.map((m) => (
                <tr key={m.id}>
                  <td className="px-3 py-2 font-medium">{m.name}{m.is_me && <span className="text-muted-foreground ms-1 text-xs font-normal">({t("projects.you")})</span>}</td>
                  <td className="px-3 py-2 text-end tabular-nums">{formatNumber(m.share)}</td>
                  <td className="px-3 py-2 text-end tabular-nums">{money(m.paid)}</td>
                  <td className="px-3 py-2 text-end tabular-nums">{money(m.owes)}</td>
                  <td className={cn("px-3 py-2 text-end tabular-nums", m.balance > 0.004 ? "text-emerald-700 dark:text-emerald-300" : m.balance < -0.004 ? "text-amber-800 dark:text-amber-200" : "text-muted-foreground")}>
                    {Math.abs(m.balance) < 0.005 ? t("projects.settled") : m.balance > 0 ? t("projects.getsBack", { amount: money(m.balance) }) : t("projects.owesAmount", { amount: money(-m.balance) })}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
      <section className="grid gap-2">
        <h3 className="text-sm font-medium">{t("projects.toSettle")}</h3>
        {split.settle.length === 0 ? <p className="text-muted-foreground text-sm">{t("projects.allSquare")}</p> : (
          <ul className="grid gap-2">
            {split.settle.map((s, i) => (
              <li key={i} className="flex flex-wrap items-center gap-2 rounded-lg border px-3 py-2 text-sm">
                <span className={cn("font-medium", s.from_me && "text-amber-800 dark:text-amber-200")}>{who(s.from, s.from_me)}</span>
                <ArrowRight className="text-muted-foreground size-4" aria-label={t("projects.pays")} />
                <span className={cn("font-medium", s.to_me && "text-emerald-700 dark:text-emerald-300")}>{who(s.to, s.to_me)}</span>
                <span className="ms-auto font-semibold tabular-nums">{money(s.amount)}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
      {recording && <SettleDialog project={project} onClose={(done) => { setRecording(false); if (done) { setNote(t("projects.recorded", { count: done })); onChanged(); } }} />}
    </div>
  );
}

/** Puts what each person owes you (or you owe them) from the split into Lend & borrow. */
function SettleDialog({ project, onClose }: { project: ProjectDetail; onClose: (done: number) => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [lines, setLines] = useState<SettleLine[] | null>(null);
  const [people, setPeople] = useState<Person[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [pick, setPick] = useState<Record<string, string>>({});
  const [category, setCategory] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    Promise.all([api.projects.settlePreview(project.id), api.people.list(), api.categories.getAll()]).then(([l, ps, cs]) => {
      setLines(l); setPeople(ps); setCategories(cs.filter((c) => c.type === "expense"));
      setPick(Object.fromEntries(l.map((x) => [x.member_id, x.person_id ?? ps.find((p) => p.name.toLowerCase() === x.name.toLowerCase())?.id ?? "new"])));
    }).catch((e) => setError(errorText(e)));
  }, [project.id]);
  const money = (x: number) => formatMoney(x, project.currency, hideBalances);
  const todo = (lines ?? []).filter((l) => Math.abs(l.change) >= 0.005);
  const save = async () => {
    setBusy(true); setError("");
    try { onClose(await api.projects.settleRecord(project.id, { category_id: category || undefined, people: pick })); } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(0); }}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t("projects.recordTitle")}</DialogTitle>
          <DialogDescription>{t("projects.recordDesc")}</DialogDescription>
        </DialogHeader>
        {lines && todo.length === 0 ? <p className="text-sm">{t("projects.nothingToRecord")}</p> : (
          <ul className="grid gap-2">
            {todo.map((l) => (
              <li key={l.member_id} className="grid gap-2 rounded-lg border p-3 sm:grid-cols-[minmax(0,1fr)_14rem] sm:items-center">
                <div className="text-sm">
                  <p className="font-medium">{l.change > 0 ? t("projects.theyOweYou", { name: l.name, amount: money(l.change) }) : t("projects.youOweThem", { name: l.name, amount: money(-l.change) })}</p>
                  {Math.abs(l.recorded) >= 0.005 && <p className="text-muted-foreground text-xs">{t("projects.alreadyRecorded", {
                    what: l.recorded > 0 ? t("projects.theyOweYou", { name: l.name, amount: money(l.recorded) }) : t("projects.youOweThem", { name: l.name, amount: money(-l.recorded) }) })}</p>}
                </div>
                <NativeSelect aria-label={t("projects.inLendAs", { name: l.name })} value={pick[l.member_id] ?? "new"} onChange={(e) => setPick({ ...pick, [l.member_id]: e.target.value })}>
                  <option value="new">{t("projects.addAsNew", { name: l.name })}</option>
                  {people.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                </NativeSelect>
              </li>
            ))}
          </ul>
        )}
        {todo.length > 0 && (
          <div className="grid gap-1.5"><Label htmlFor="st-cat">{t("projects.spendingCategory")}</Label>
            <NativeSelect id="st-cat" value={category} onChange={(e) => setCategory(e.target.value)} aria-describedby="st-cat-hint">
              <option value="">{t("projects.uncategorised")}</option>
              {categories.map((c) => <option key={c.id} value={c.id}>{c.icon ? `${c.icon} ` : ""}{c.name}</option>)}
            </NativeSelect>
            <p id="st-cat-hint" className="text-muted-foreground text-xs">{t("projects.spendingCategoryHelp")}</p></div>
        )}
        {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
        <DialogFooter className="gap-2">
          <Button type="button" variant="outline" onClick={() => onClose(0)}>{t("common.cancel")}</Button>
          <Button onClick={save} disabled={busy || todo.length === 0}>{busy && <Loader2 className="animate-spin" />}{t("projects.recordN", { count: todo.length })}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ------------------------------------------------------------------ sharing

/** Save the project as a password-protected file to send to someone who uses Vaulty. */
export function ShareDialog({ project, onClose }: { project: ProjectDetail; onClose: () => void }) {
  const { t } = useTranslation();
  const [password, setPassword] = useState("");
  const [again, setAgain] = useState("");
  const [withFiles, setWithFiles] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [done, setDone] = useState<{ files: number; files_left_out: number } | null>(null);
  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (password !== again) { setError(t("projects.passwordsDiffer")); return; }
    setError("");
    try {
      const path = await api.files.pick("save_project_share", t("projects.shareFile"), project.name.replace(/[\\/:*?"<>|]+/g, " ").trim() || "project");
      if (!path) return;
      setBusy(true);
      setDone(await api.projects.shareExport(project.id, path, password, withFiles));
      setPassword(""); setAgain("");
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("projects.shareTitle")}</DialogTitle>
          <DialogDescription>{t("projects.shareDesc")}</DialogDescription>
        </DialogHeader>
        {done ? (
          <div className="grid gap-3">
            <p className="text-sm" role="status">{t("projects.shared")}</p>
            {done.files_left_out > 0 && <p className="text-muted-foreground text-sm">{t("projects.billsLeftOut", { count: done.files_left_out })}</p>}
            <DialogFooter><Button autoFocus onClick={onClose}>{t("projects.done")}</Button></DialogFooter>
          </div>
        ) : (
          <form onSubmit={save} className="grid gap-3">
            <div className="grid gap-1.5"><Label htmlFor="sh-pw">{t("projects.sharePassword")}</Label>
              <Input id="sh-pw" type="password" autoComplete="new-password" minLength={8} required value={password} onChange={(e) => setPassword(e.target.value)} aria-describedby="sh-pw-hint" /></div>
            <div className="grid gap-1.5"><Label htmlFor="sh-pw2">{t("projects.sharePasswordAgain")}</Label>
              <Input id="sh-pw2" type="password" autoComplete="new-password" required value={again} onChange={(e) => setAgain(e.target.value)} /></div>
            <p id="sh-pw-hint" className="text-muted-foreground -mt-1 text-xs">{t("projects.sharePasswordHint")}</p>
            <label className="flex items-center gap-2 text-sm"><input type="checkbox" className="accent-primary size-4" checked={withFiles} onChange={(e) => setWithFiles(e.target.checked)} />{t("projects.includeBills")}</label>
            {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
            <DialogFooter className="gap-2">
              <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
              <Button disabled={busy || password.length < 8}>{busy ? <Loader2 className="animate-spin" /> : <Send />}{t("projects.saveShare")}</Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}

/** Open a project someone shared: its password, a look at what's in it, which person you are. */
export function OpenSharedDialog({ onClose, onOpened }: { onClose: () => void; onOpened: (id: string) => void }) {
  const { t } = useTranslation();
  const [path, setPath] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [peek, setPeek] = useState<SharePeek | null>(null);
  const [me, setMe] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const choose = async () => { setError(""); try { const p = await api.files.pick("project_share", t("projects.shareFile")); if (p) { setPath(p); setPeek(null); } } catch (e) { setError(errorText(e)); } };
  const look = async (e: FormEvent) => {
    e.preventDefault();
    if (!path) return;
    setBusy(true); setError("");
    try { const p = await api.projects.sharePeek(path, password); setPeek(p); setMe(""); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const open = async () => {
    if (!path) return;
    setBusy(true); setError("");
    try { const r = await api.projects.shareImport(path, password, me || null); setPassword(""); onOpened(r.id); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  const money = (x: number) => formatMoney(x, peek?.currency ?? "INR");
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("projects.openSharedTitle")}</DialogTitle>
          <DialogDescription>{t("projects.openSharedDesc")}</DialogDescription>
        </DialogHeader>
        {!peek ? (
          <form onSubmit={look} className="grid gap-3">
            <div className="flex flex-wrap items-center gap-2">
              <Button type="button" variant="outline" onClick={choose} autoFocus><FileUp />{path ? t("projects.chooseOther") : t("projects.chooseShareFile")}</Button>
              {path && <span className="min-w-0 flex-1 truncate text-sm" title={path}>{path.split(/[\\/]/).pop()}</span>}
            </div>
            <div className="grid gap-1.5"><Label htmlFor="os-pw">{t("projects.sharePassword")}</Label>
              <Input id="os-pw" type="password" autoComplete="off" value={password} disabled={!path} onChange={(e) => setPassword(e.target.value)} /></div>
            {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
            <DialogFooter className="gap-2">
              <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
              <Button disabled={!path || !password || busy}>{busy && <Loader2 className="animate-spin" />}{t("projects.look")}</Button>
            </DialogFooter>
          </form>
        ) : (
          <div className="grid gap-3">
            <div className="bg-muted/50 grid gap-1 rounded-lg p-3 text-sm">
              <p className="font-medium">{peek.name}</p>
              <p className="text-muted-foreground">{t("projects.peekCounts", { items: peek.items, payments: peek.payments, files: peek.files })}</p>
              <p className="text-muted-foreground">{t("projects.peekTotals", { billed: money(peek.billed), paid: money(peek.paid) })}</p>
            </div>
            {peek.updates && <p className="text-sm">{t("projects.willUpdate", { name: peek.updates.name })}</p>}
            {peek.members.length > 0 && (
              <div className="grid gap-1.5"><Label htmlFor="os-me">{t("projects.whichAreYou")}</Label>
                <NativeSelect id="os-me" value={me} onChange={(e) => setMe(e.target.value)}>
                  <option value="">{t("projects.choosePerson")}</option>
                  {peek.members.filter((m) => m.id !== peek.sent_by).map((m) => <option key={m.id} value={m.id}>{m.name}</option>)}
                </NativeSelect></div>
            )}
            {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
            <DialogFooter className="gap-2">
              <Button type="button" variant="outline" onClick={() => setPeek(null)}>{t("projects.back")}</Button>
              <Button onClick={open} disabled={busy || (peek.members.length > 0 && !me)}>{busy && <Loader2 className="animate-spin" />}{peek.updates ? t("projects.update") : t("projects.open")}</Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
