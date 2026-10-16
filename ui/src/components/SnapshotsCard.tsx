import { useEffect, useState } from "react";
import { DatabaseBackup, Loader2, RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type Snapshot } from "@/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { formatDate, formatNumber } from "@/lib/utils";

const when = (s: Snapshot) => formatDate(s.at, { dateStyle: "medium", timeStyle: "short" });
const size = (b: number) => (b >= 1048576 ? `${formatNumber(b / 1048576, { maximumFractionDigits: 1 })} MB` : `${formatNumber(Math.max(1, Math.round(b / 1024)))} KB`);

/** Encrypted copies of the member's data on this device, with restore. */
export function SnapshotsCard() {
  const { t } = useTranslation();
  const [list, setList] = useState<Snapshot[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [confirm, setConfirm] = useState<Snapshot | null>(null);
  const [done, setDone] = useState(false);
  const load = () => api.snapshots.list().then(setList).catch((e) => setError(errorText(e)));
  useEffect(() => { load(); }, []);

  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true); setError("");
    try { await fn(); await load(); } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };

  return (
    <Card>
      <CardHeader>
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1.5"><CardTitle>{t("snapshots.title")}</CardTitle><CardDescription>{t("snapshots.desc")}</CardDescription></div>
          <Button variant="outline" size="sm" className="shrink-0" disabled={busy} onClick={() => run(api.snapshots.create)}>
            {busy ? <Loader2 className="animate-spin" /> : <DatabaseBackup />}{t("snapshots.now")}
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {list && list.length === 0 && <p className="text-muted-foreground text-sm">{t("snapshots.empty")}</p>}
        {list && list.length > 0 && (
          <ul className="divide-y rounded-lg border">
            {list.map((s) => (
              <li key={s.name} className="flex items-center gap-3 px-3 py-2 text-sm">
                <div className="min-w-0 flex-1">
                  <div className="tabular-nums">{when(s)}</div>
                  <div className="text-muted-foreground text-xs">{t(`snapshots.reasons.${s.reason}`)} · {size(s.size)}</div>
                </div>
                <Button size="sm" variant="ghost" disabled={busy} onClick={() => { setDone(false); setConfirm(s); }}><RotateCcw />{t("snapshots.restore")}</Button>
              </li>
            ))}
          </ul>
        )}
        <p className="text-muted-foreground text-xs">{t("snapshots.keep")} {t("snapshots.note")}</p>
        {done && <p className="text-sm">{t("snapshots.restored")}</p>}
        {error && <p className="text-destructive text-sm">{error}</p>}
      </CardContent>

      <Dialog open={!!confirm} onOpenChange={(o) => !o && setConfirm(null)}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>{t("snapshots.confirmTitle")}</DialogTitle>
            <DialogDescription>{confirm && t("snapshots.confirmBody", { date: when(confirm) })}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirm(null)}>{t("common.cancel")}</Button>
            <Button disabled={busy} onClick={() => { const c = confirm!; setConfirm(null); void run(async () => { await api.snapshots.restore(c.name); setDone(true); }); }}>
              {busy && <Loader2 className="animate-spin" />}{t("snapshots.restore")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
