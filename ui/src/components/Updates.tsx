import { useEffect, useState } from "react";
import { Download, Loader2, RefreshCw, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type UpdateInfo } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { NativeSelect } from "@/components/ui/input";
import { useSession } from "@/hooks/session";
import { useAppInfo } from "@/hooks/appInfo";

/** Settings: whether to check for new versions, a "Check now" button, and installing. */
export function UpdatesCard() {
  const { t } = useTranslation();
  const info = useAppInfo();
  const [mode, setMode] = useState<"off" | "weekly">("off");
  const [state, setState] = useState<"idle" | "checking" | "none" | "found">("idle");
  const [found, setFound] = useState<UpdateInfo | null>(null);
  const [error, setError] = useState("");

  useEffect(() => { api.settings.get().then((s) => setMode(s.update_check === "weekly" ? "weekly" : "off")).catch(() => {}); }, []);

  const changeMode = async (m: "off" | "weekly") => {
    setMode(m); setError("");
    try { await api.settings.update({ update_check: m }); } catch (e) { setError(errorText(e)); }
  };
  const check = async () => {
    setState("checking"); setError("");
    try { const u = await api.updates.check(); setFound(u); setState(u ? "found" : "none"); }
    catch (e) { setError(errorText(e)); setState("idle"); }
  };

  return (
    <Card>
      <CardHeader><CardTitle>{t("updates.title")}</CardTitle><CardDescription>{t("updates.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <div className="text-sm font-medium">{t("updates.version", { version: info?.version ?? "…" })}</div>
            <div className="text-muted-foreground text-xs">
              {state === "none" ? t("updates.latest") : state === "found" && found ? t("updates.available", { version: found.version }) : t("updates.privacy")}
            </div>
          </div>
          <Button variant="outline" size="sm" onClick={check} disabled={state === "checking"}>
            {state === "checking" ? <Loader2 className="animate-spin" /> : <RefreshCw />}{t("updates.check")}
          </Button>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <label htmlFor="update-mode" className="text-sm">{t("updates.automatic")}</label>
          <NativeSelect id="update-mode" className="w-auto" value={mode} onChange={(e) => changeMode(e.target.value as "off" | "weekly")}>
            <option value="off">{t("updates.off")}</option>
            <option value="weekly">{t("updates.weekly")}</option>
          </NativeSelect>
        </div>
        {state === "found" && found && <Install update={found} />}
        {error && <p className="text-destructive text-sm">{error}</p>}
      </CardContent>
    </Card>
  );
}

/** What's new, and Install for the admin (others are told to ask). */
function Install({ update, compact }: { update: UpdateInfo; compact?: boolean }) {
  const { t } = useTranslation();
  const { user } = useSession();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const install = async () => {
    setBusy(true); setError("");
    try { await api.updates.install(); } catch (e) { setError(errorText(e)); setBusy(false); }
  };
  return (
    <div className="space-y-2">
      {update.notes && !compact && <div className="bg-muted/50 max-h-40 overflow-y-auto rounded-md p-3 text-xs whitespace-pre-wrap">{update.notes}</div>}
      {user.role === "admin" ? (
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" onClick={install} disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <Download />}{busy ? t("updates.installing") : t("updates.install")}</Button>
          <span className="text-muted-foreground text-xs">{t("updates.restartNote")}</span>
        </div>
      ) : <p className="text-muted-foreground text-xs">{t("updates.askAdmin")}</p>}
      {error && <p className="text-destructive text-sm">{error}</p>}
    </div>
  );
}

/** After signing in: if automatic checks are on and a week has passed, quietly checks and shows a small notice. */
export function UpdateNotice() {
  const { t } = useTranslation();
  const [found, setFound] = useState<UpdateInfo | null>(null);
  useEffect(() => { api.updates.auto().then(setFound).catch(() => {}); }, []);
  if (!found) return null;
  return (
    <div role="status" className="bg-popover text-popover-foreground fixed right-4 bottom-4 z-40 w-80 rounded-lg border p-4 shadow-lg">
      <div className="mb-2 flex items-start justify-between gap-2">
        <div className="text-sm font-medium">{t("updates.available", { version: found.version })}</div>
        <button type="button" onClick={() => setFound(null)} aria-label={t("updates.later")} className="text-muted-foreground hover:text-foreground"><X className="size-4" /></button>
      </div>
      <Install update={found} compact />
    </div>
  );
}
