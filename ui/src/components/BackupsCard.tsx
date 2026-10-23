import { useState, type FormEvent } from "react";
import { ArchiveRestore, FileSpreadsheet, Loader2, Lock, Printer, ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type BackupInfo } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { printRecoverySheet } from "@/components/RecoverySheet";
import { useSession } from "@/hooks/session";
import { formatDate } from "@/lib/utils";


/** Encrypted backups, restoring, spreadsheet export and the printable recovery sheet. */
export function BackupsCard() {
  const { t } = useTranslation();
  const { user } = useSession();
  const [make, setMake] = useState(false);
  const [restore, setRestore] = useState<{ path: string; info: BackupInfo } | null>(null);
  const [note, setNote] = useState("");
  const [error, setError] = useState("");

  const exportCsv = async () => {
    setError(""); setNote("");
    const path = await api.files.pick("save_csv", "CSV");
    if (!path) return;
    try { setNote(t("backups.csvDone", { count: await api.backup.exportCsv(path) })); } catch (e) { setError(errorText(e)); }
  };
  const pickRestore = async () => {
    setError(""); setNote("");
    const path = await api.files.pick("backup", t("backups.files"));
    if (!path) return;
    try { setRestore({ path, info: await api.backup.inspect(path) }); } catch (e) { setError(errorText(e)); }
  };

  return (
    <Card>
      <CardHeader><CardTitle>{t("backups.title")}</CardTitle><CardDescription>{t("backups.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-4">
        <Row icon={ShieldCheck} title={t("backups.makeTitle")} desc={t("backups.makeDesc")}>
          <Button size="sm" onClick={() => { setError(""); setNote(""); setMake(true); }}><Lock />{t("backups.make")}</Button>
        </Row>
        <Separator />
        <Row icon={ArchiveRestore} title={t("backups.restoreTitle")} desc={t("backups.restoreDesc")}>
          <Button variant="outline" size="sm" onClick={pickRestore}>{t("backups.restore")}</Button>
        </Row>
        <Separator />
        <Row icon={FileSpreadsheet} title={t("backups.csvTitle")} desc={t("backups.csvDesc")}>
          <Button variant="outline" size="sm" onClick={exportCsv}>{t("backups.csv")}</Button>
        </Row>
        <Separator />
        <Row icon={Printer} title={t("backups.sheetTitle")} desc={t("backups.sheetDesc")}>
          <Button variant="outline" size="sm" onClick={() => printRecoverySheet({ name: user.name, child: user.role === "child" })}>{t("sheet.print")}</Button>
        </Row>
        {note && <p className="text-success text-sm">{note}</p>}
        {error && <p className="text-destructive text-sm">{error}</p>}
      </CardContent>
      <MakeDialog open={make} onClose={() => setMake(false)} onDone={(n) => { setMake(false); setNote(n); }} />
      {restore && <RestoreDialog {...restore} onClose={() => setRestore(null)} onDone={(n) => { setRestore(null); setNote(n); window.dispatchEvent(new Event(DATA_RESTORED)); }} />}
    </Card>
  );
}

function Row({ icon: Icon, title, desc, children }: { icon: typeof Lock; title: string; desc: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex items-start gap-3"><Icon className="text-muted-foreground mt-0.5 size-4 shrink-0" />
        <div><div className="text-sm font-medium">{title}</div><div className="text-muted-foreground text-xs">{desc}</div></div></div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function MakeDialog({ open: isOpen, onClose, onDone }: { open: boolean; onClose: () => void; onDone: (note: string) => void }) {
  const { t } = useTranslation();
  const [pw, setPw] = useState(""), [confirm, setConfirm] = useState(""), [hint, setHint] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const reset = () => { setPw(""); setConfirm(""); setHint(""); setError(""); };
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (pw !== confirm) return setError(t("security.mismatch"));
    const path = await api.files.pick("save_backup", t("backups.files"));
    if (!path) return;
    setBusy(true); setError("");
    try {
      const out = await api.backup.export(path, pw, hint || undefined);
      reset();
      const made = t("backups.made", { file: path.split(/[\\/]/).pop() });
      onDone(out.document_files_left_out ? `${made} ${t("backups.filesLeftOut")}` : made);
    }
    catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  return (
    <Dialog open={isOpen} onOpenChange={(o) => { if (!o) { reset(); onClose(); } }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader><DialogTitle>{t("backups.makeTitle")}</DialogTitle><DialogDescription>{t("backups.passwordDesc")}</DialogDescription></DialogHeader>
        <form onSubmit={submit} className="grid gap-3">
          <div className="grid gap-2"><Label htmlFor="bk-pw">{t("backups.password")}</Label><Input id="bk-pw" type="password" autoFocus minLength={10} value={pw} onChange={(e) => setPw(e.target.value)} required autoComplete="new-password" /></div>
          <div className="grid gap-2"><Label htmlFor="bk-pw2">{t("security.confirmPassword")}</Label><Input id="bk-pw2" type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} required autoComplete="new-password" /></div>
          <div className="grid gap-2"><Label htmlFor="bk-hint">{t("backups.hint")}</Label><Input id="bk-hint" maxLength={80} value={hint} onChange={(e) => setHint(e.target.value)} placeholder={t("backups.hintPh")} />
            <p className="text-muted-foreground text-xs">{t("backups.hintNote")}</p></div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button type="button" variant="ghost" onClick={() => { reset(); onClose(); }}>{t("common.cancel")}</Button>
            <Button disabled={busy || pw.length < 10}>{busy ? <Loader2 className="animate-spin" /> : <Lock />}{t("backups.saveAs")}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function RestoreDialog({ path, info, onClose, onDone }: { path: string; info: BackupInfo; onClose: () => void; onDone: (note: string) => void }) {
  const { t } = useTranslation();
  const [pw, setPw] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const submit = async (e: FormEvent) => {
    e.preventDefault(); setBusy(true); setError("");
    try {
      const c = await api.backup.import(path, info.encrypted ? pw : undefined);
      onDone(t("backups.restored", { tx: c.transactions, acc: c.accounts }));
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader><DialogTitle>{t("backups.restoreTitle")}</DialogTitle>
          <DialogDescription>{t("backups.restoreWhat", { file: path.split(/[\\/]/).pop(), date: info.created_at ? formatDate(info.created_at) : "—" })}</DialogDescription></DialogHeader>
        <form onSubmit={submit} className="grid gap-3">
          {info.encrypted ? <>
            <div className="grid gap-2"><Label htmlFor="rs-pw">{t("backups.password")}</Label><Input id="rs-pw" type="password" autoFocus value={pw} onChange={(e) => setPw(e.target.value)} required /></div>
            {info.hint && <p className="text-muted-foreground text-xs">{t("backups.yourHint", { hint: info.hint })}</p>}
          </> : <p className="text-orange-600 text-sm">{t("backups.plainWarning")}</p>}
          <p className="text-muted-foreground text-xs">{t("backups.restoreNote")}</p>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button type="button" variant="ghost" onClick={onClose}>{t("common.cancel")}</Button>
            <Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}{t("backups.restore")}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
