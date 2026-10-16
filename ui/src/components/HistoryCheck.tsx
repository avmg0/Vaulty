import { useState } from "react";
import { AlertTriangle, Check, Copy, Loader2, ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import { api, errorText, type HistoryCheck as Result } from "@/api";
import { Button } from "@/components/ui/button";
import { SuccessTick } from "@/components/SuccessTick";
import { useMotion } from "@/lib/motion";
import { cn, formatDate } from "@/lib/utils";

/** "Verify history": re-checks the fingerprint chain and shows the current fingerprint. */
export function HistoryCheck() {
  const { t } = useTranslation();
  const { move, fade } = useMotion();
  const [busy, setBusy] = useState(false);
  const [res, setRes] = useState<Result | null>(null);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);

  const run = async () => {
    setBusy(true); setError("");
    try { setRes(await api.history.verify()); } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  const copy = async () => {
    if (!res) return;
    try { await navigator.clipboard.writeText(res.head); setCopied(true); setTimeout(() => setCopied(false), 1500); } catch { /* blocked */ }
  };

  const outside = res?.changed_outside.length ?? 0;
  const tone = !res ? "" : res.ok ? "border-success/40 bg-success/5" : "border-destructive/40 bg-destructive/5";
  return (
    <div className="space-y-3">
      <Button variant="outline" size="sm" onClick={run} disabled={busy}>
        {busy ? <Loader2 className="animate-spin" /> : <ShieldCheck />}{busy ? t("history.check.verifying") : t("history.check.verify")}
      </Button>
      {error && <p className="text-destructive text-sm">{error}</p>}
      {res && (
        <motion.div key={res.head + String(res.ok)} className={cn("space-y-3 rounded-xl border p-4", tone)}
          initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} transition={{ opacity: fade(), y: move() }} role="status">
          <div className="flex items-start gap-3">
            {res.ok ? <SuccessTick className="text-success mt-0.5 size-5" /> : <AlertTriangle className="text-destructive mt-0.5 size-5 shrink-0" />}
            <div className="space-y-1 text-sm">
              {res.ok && <div className="font-medium">{t("history.check.intactTitle")}</div>}
              {res.ok && <p className="text-muted-foreground">{res.checked === 0 ? t("history.check.intactEmpty")
                : t("history.check.intactBody", { count: res.checked, date: res.since ? formatDate(res.since, { dateStyle: "medium" }) : "" })}</p>}
              {res.broken_at != null && <>
                <div className="font-medium">{t("history.check.brokenTitle")}</div>
                <p className="text-muted-foreground">{t("history.check.brokenBody", { id: res.broken_at, reason: res.reason })}</p>
              </>}
              {outside > 0 && <>
                <div className="font-medium">{t("history.check.outsideTitle")}</div>
                <p className="text-muted-foreground">{t("history.check.outsideBody", { count: outside })}</p>
              </>}
              {res.unchained > 0 && <p className="text-muted-foreground text-xs">{t("history.check.unchained", { count: res.unchained })}</p>}
            </div>
          </div>
          {res.broken_at == null && <div className="space-y-1">
            <div className="text-muted-foreground text-xs">{t("history.check.fingerprint")}</div>
            <div className="flex items-center gap-2">
              <code className="bg-background min-w-0 flex-1 truncate rounded border px-2 py-1 font-mono text-xs" title={res.head}>{res.head}</code>
              <Button variant="ghost" size="sm" onClick={copy}>{copied ? <Check /> : <Copy />}{copied ? t("history.check.copied") : t("history.check.copy")}</Button>
            </div>
            <p className="text-muted-foreground text-xs">{t("history.check.fingerprintHint")}</p>
          </div>}
        </motion.div>
      )}
    </div>
  );
}
