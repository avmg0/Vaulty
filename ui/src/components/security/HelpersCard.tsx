import { useEffect, useState, type FormEvent } from "react";
import { AlertTriangle, Eye, FileText, Loader2, Printer, ScrollText, UserRound, UsersRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type Estate, type HelperCard, type HelperChoice, type Helpers, type NomineeStatus, type User } from "@/api";
import { EstateView } from "@/components/EstateView";
import { useSession } from "@/hooks/session";
import { PasswordDialog } from "@/components/security/PasswordDialog";
import { printHelperCards } from "@/components/security/HelperCards";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Label } from "@/components/ui/label";
import { formatDate } from "@/lib/utils";

/** Settings: the three people who can help this member back in (any two together). */
export function HelpersCard() {
  const { t } = useTranslation();
  const [data, setData] = useState<{ helpers: Helpers; candidates: User[] } | null>(null);
  const [choosing, setChoosing] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [cards, setCards] = useState<HelperCard[] | null>(null);
  const load = () => api.helpers.mine().then(setData).catch(() => {});
  useEffect(() => { load(); }, []);
  if (!data) return null;
  const h = data.helpers;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2"><UsersRound className="size-4" />{t("helpers.title")}</CardTitle>
        <CardDescription>{t("helpers.desc")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        {h.set_up ? (
          <>
            <ul className="divide-y rounded-lg border">
              {h.slots.map((s) => (
                <li key={s.slot} className="flex items-center gap-3 px-3 py-2.5">
                  {s.kind === "card" ? <FileText className="text-muted-foreground size-4 shrink-0" aria-hidden /> : <UserRound className="text-muted-foreground size-4 shrink-0" aria-hidden />}
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">{s.name}</div>
                    <div className="text-muted-foreground text-xs">{s.kind === "card" ? t("helpers.kindCard") : t("helpers.kindMember")}</div>
                  </div>
                  {!s.usable && <span className="text-xs font-medium text-amber-800 dark:text-amber-300">{t("helpers.cantHelp")}</span>}
                </li>
              ))}
            </ul>
            {h.usable < 3 && (
              <p className="flex items-start gap-1.5 text-sm text-amber-800 dark:text-amber-300">
                <AlertTriangle className="mt-0.5 size-4 shrink-0" aria-hidden />{h.usable < 2 ? t("helpers.broken") : t("helpers.weak")}
              </p>
            )}
            <div className="flex flex-wrap items-center gap-2">
              {h.created_at && <span className="text-muted-foreground me-auto text-xs">{t("helpers.since", { date: formatDate(h.created_at.slice(0, 10)) })}</span>}
              <Button variant="ghost" size="sm" className="text-destructive" onClick={() => setRemoving(true)}>{t("helpers.turnOff")}</Button>
              <Button variant="outline" size="sm" onClick={() => setChoosing(true)}>{t("helpers.change")}</Button>
            </div>
            <Separator />
            <NomineeSection onCard={(c) => setCards([c])} />
          </>
        ) : (
          <div className="flex flex-wrap items-center justify-between gap-3">
            <p className="text-muted-foreground max-w-md text-sm">{t("helpers.howItWorks")}</p>
            <Button onClick={() => setChoosing(true)}>{t("helpers.choose")}</Button>
          </div>
        )}
      </CardContent>
      {choosing && <ChooseDialog current={h} candidates={data.candidates} onClose={() => setChoosing(false)}
        onSaved={(c) => { setChoosing(false); load(); if (c.length) setCards(c); }} />}
      <PasswordDialog open={removing} onOpenChange={setRemoving} destructive title={t("helpers.offTitle")} description={t("helpers.offDesc")}
        confirmLabel={t("helpers.turnOff")} action={async (pw) => { await api.helpers.remove(pw); load(); }} />
      <CardsDialog cards={cards} onDone={() => setCards(null)} />
    </Card>
  );
}

/** The nominee: who may see a read-only summary if something happens to this member. */
function NomineeSection({ onCard }: { onCard: (c: HelperCard) => void }) {
  const { t } = useTranslation();
  const { user } = useSession();
  const [data, setData] = useState<{ nominee: NomineeStatus; letter: string | null } | null>(null);
  const [editing, setEditing] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [preview, setPreview] = useState<Estate | null>(null);
  const load = () => api.nominee.mine().then(setData).catch(() => {});
  useEffect(() => { load(); }, []);
  if (!data) return null;
  const n = data.nominee;
  return (
    <div className="space-y-3">
      <div className="flex items-start gap-3">
        <ScrollText className="text-muted-foreground mt-0.5 size-4 shrink-0" aria-hidden />
        <div className="min-w-0 flex-1">
          <div className="text-sm font-medium">{t("nominee.title")}</div>
          <p className="text-muted-foreground text-xs">{n.set_up ? t("nominee.set", { name: n.name, count: n.wait_days }) : t("nominee.desc")}</p>
          {n.set_up && <p className="text-muted-foreground text-xs">{n.has_letter ? t("nominee.withLetter") : t("nominee.noLetter")}</p>}
        </div>
      </div>
      <div className="flex flex-wrap justify-end gap-2">
        <Button variant="ghost" size="sm" onClick={() => api.nominee.preview().then(setPreview).catch(() => {})}><Eye />{t("nominee.preview")}</Button>
        {n.set_up && <Button variant="ghost" size="sm" className="text-destructive" onClick={() => setRemoving(true)}>{t("nominee.remove")}</Button>}
        <Button variant="outline" size="sm" onClick={() => setEditing(true)}>{n.set_up ? t("nominee.edit") : t("nominee.add")}</Button>
      </div>
      {editing && <NomineeDialog current={n} letter={data.letter} onClose={() => setEditing(false)} onSaved={(c) => { setEditing(false); load(); onCard(c); }} />}
      <PasswordDialog open={removing} onOpenChange={setRemoving} destructive title={t("nominee.removeTitle")} description={t("nominee.removeDesc")}
        confirmLabel={t("nominee.remove")} action={async (pw) => { await api.nominee.remove(pw); load(); }} />
      {preview && <EstateView estate={preview} owner={user.name} letter={data.letter} preview onClose={() => setPreview(null)} />}
    </div>
  );
}

function NomineeDialog({ current, letter: currentLetter, onClose, onSaved }: { current: NomineeStatus; letter: string | null; onClose: () => void; onSaved: (c: HelperCard) => void }) {
  const { t } = useTranslation();
  const [name, setName] = useState(current.name ?? "");
  const [wait, setWait] = useState(current.wait_days || 7);
  const [letter, setLetter] = useState(currentLetter ?? "");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const save = async (e: FormEvent) => {
    e.preventDefault(); setBusy(true); setError("");
    try { onSaved(await api.nominee.set(password, name, wait, letter)); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader><DialogTitle>{t("nominee.dialogTitle")}</DialogTitle><DialogDescription>{t("nominee.dialogDesc")}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="grid gap-4">
          <div className="grid grid-cols-[1fr_9rem] gap-3">
            <div className="grid gap-1.5"><Label htmlFor="nom-name">{t("nominee.name")}</Label><Input id="nom-name" autoFocus maxLength={60} value={name} onChange={(e) => setName(e.target.value)} required /></div>
            <div className="grid gap-1.5"><Label htmlFor="nom-wait">{t("nominee.wait")}</Label>
              <NativeSelect id="nom-wait" value={wait} onChange={(e) => setWait(Number(e.target.value))}>
                {[3, 7, 14, 30].map((d) => <option key={d} value={d}>{t("nominee.days", { count: d })}</option>)}
              </NativeSelect></div>
          </div>
          <p className="text-muted-foreground -mt-2 text-xs">{t("nominee.waitHint")}</p>
          <div className="grid gap-1.5">
            <Label htmlFor="nom-letter">{t("nominee.letter")}</Label>
            <textarea id="nom-letter" rows={5} maxLength={5000} value={letter} onChange={(e) => setLetter(e.target.value)} placeholder={t("nominee.letterPlaceholder")}
              className="border-input dark:bg-input/30 focus-visible:border-ring focus-visible:ring-ring/50 rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:ring-[3px]" />
            <p className="text-muted-foreground text-xs">{t("nominee.letterHint")}</p>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="nom-pw">{t("security.currentPassword")}</Label>
            <Input id="nom-pw" type="password" value={password} onChange={(e) => setPassword(e.target.value)} required />
          </div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}{t("nominee.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

type Slot = { kind: "member"; id: string } | { kind: "card"; label: string };

function ChooseDialog({ current, candidates, onClose, onSaved }: { current: Helpers; candidates: User[]; onClose: () => void; onSaved: (cards: HelperCard[]) => void }) {
  const { t } = useTranslation();
  // Start from the current helpers, or from family members first and cards for the rest.
  const [slots, setSlots] = useState<Slot[]>(() => {
    if (current.set_up && current.slots.length === 3) {
      return current.slots.map((s) => s.kind === "member" && s.helper_id && candidates.some((c) => c.id === s.helper_id)
        ? { kind: "member", id: s.helper_id } : { kind: "card", label: s.kind === "card" ? s.name : "" });
    }
    return [0, 1, 2].map((i) => candidates[i] ? { kind: "member", id: candidates[i].id } : { kind: "card", label: "" });
  });
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (i: number, s: Slot) => setSlots((l) => l.map((x, j) => (j === i ? s : x)));
  const taken = (i: number) => slots.filter((s, j) => j !== i && s.kind === "member").map((s) => (s as { id: string }).id);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    try { onSaved(await api.helpers.set(password, slots as HelperChoice[])); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader><DialogTitle>{t("helpers.chooseTitle")}</DialogTitle><DialogDescription>{t("helpers.chooseDesc")}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="grid gap-4">
          {slots.map((s, i) => (
            <fieldset key={i} className="grid gap-2 rounded-lg border p-3">
              <legend className="px-1 text-sm font-medium">{t("helpers.helperN", { n: i + 1 })}</legend>
              <NativeSelect aria-label={t("helpers.helperN", { n: i + 1 })} value={s.kind === "member" ? s.id : "card"}
                onChange={(e) => set(i, e.target.value === "card" ? { kind: "card", label: "" } : { kind: "member", id: e.target.value })}>
                {candidates.filter((c) => !taken(i).includes(c.id)).map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
                <option value="card">{t("helpers.printedCard")}</option>
              </NativeSelect>
              {s.kind === "card" && (
                <div className="grid gap-1.5">
                  <Label htmlFor={`holder-${i}`} className="text-xs">{t("helpers.holder")}</Label>
                  <Input id={`holder-${i}`} maxLength={60} placeholder={t("helpers.holderPlaceholder")} value={s.label} onChange={(e) => set(i, { kind: "card", label: e.target.value })} required />
                </div>
              )}
            </fieldset>
          ))}
          {candidates.length === 0 && <p className="text-muted-foreground text-xs">{t("helpers.noFamily")}</p>}
          {current.set_up && <p className="text-muted-foreground text-xs">{t("helpers.replaces")}</p>}
          <div className="grid gap-1.5">
            <Label htmlFor="helpers-pw">{t("security.currentPassword")}</Label>
            <Input id="helpers-pw" type="password" value={password} onChange={(e) => setPassword(e.target.value)} required />
          </div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}{t("helpers.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Right after choosing: the printed cards' codes, shown once. */
function CardsDialog({ cards, onDone }: { cards: HelperCard[] | null; onDone: () => void }) {
  const { t } = useTranslation();
  const [printed, setPrinted] = useState(false);
  useEffect(() => { setPrinted(false); }, [cards]);
  if (!cards) return null;
  return (
    <Dialog open onOpenChange={(o) => { if (!o && printed) onDone(); }}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader><DialogTitle>{cards.every((c) => c.nominee) ? t("nominee.cardTitle") : t("helpers.cardsTitle", { count: cards.length })}</DialogTitle><DialogDescription>{t("helpers.cardsDesc")}</DialogDescription></DialogHeader>
        <ul className="grid gap-2">
          {cards.map((c) => (
            <li key={c.slot} className="rounded-lg border p-3">
              <div className="text-sm font-medium">{c.nominee ? t("helpers.nomineeCardFor", { holder: c.label }) : t("helpers.cardFor", { holder: c.label })}</div>
              <code className="mt-1 block font-mono text-sm tracking-wider break-all">{c.code}</code>
            </li>
          ))}
        </ul>
        <DialogFooter>
          <Button variant="outline" onClick={() => { printHelperCards(cards); setPrinted(true); }}><Printer />{t("helpers.print")}</Button>
          <Button onClick={onDone} disabled={!printed}>{t("helpers.printedDone")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
