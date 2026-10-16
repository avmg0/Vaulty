import { useEffect, useRef, useState } from "react";
import { Check, Copy, KeyRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";

/**
 * Shows a recovery key exactly once. The person must confirm they saved it before continuing,
 * and the dialog can't be dismissed by clicking outside or pressing Escape.
 */
export function RecoveryKeyDialog({ recoveryKey, child, isNew, onDone }: { recoveryKey: string | null; child?: boolean; isNew?: boolean; onDone: () => void }) {
  const { t } = useTranslation();
  const [saved, setSaved] = useState(false);
  const [copied, setCopied] = useState(false);
  const clearTimer = useRef<ReturnType<typeof setTimeout>>();
  useEffect(() => { setSaved(false); setCopied(false); }, [recoveryKey]);
  useEffect(() => () => clearTimeout(clearTimer.current), []);
  if (!recoveryKey) return null;

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(recoveryKey);
      setCopied(true);
      clearTimeout(clearTimer.current);
      // Don't leave the key on the clipboard for other apps to read.
      clearTimer.current = setTimeout(() => { void navigator.clipboard.writeText("").catch(() => {}); setCopied(false); }, 60_000);
    } catch { /* clipboard blocked; the key is still on screen */ }
  };

  return (
    <Dialog open>
      <DialogContent className="sm:max-w-md" onEscapeKeyDown={(e) => e.preventDefault()} onPointerDownOutside={(e) => e.preventDefault()} onInteractOutside={(e) => e.preventDefault()}
        showCloseButton={false}>
        <DialogHeader>
          <div className="bg-secondary mb-1 grid size-11 place-items-center rounded-xl"><KeyRound className="size-5" /></div>
          <DialogTitle>{isNew ? t("recovery.titleNew") : t("recovery.title")}</DialogTitle>
          <DialogDescription>{child ? t("recovery.bodyChild") : t("recovery.body")}</DialogDescription>
        </DialogHeader>
        <div className="bg-muted rounded-lg p-4 text-center font-mono text-lg leading-relaxed tracking-wider select-all" aria-label={t("recovery.recoveryKey")}>
          {recoveryKey.split("-").map((g, i) => <span key={i} className="inline-block px-1.5">{g}</span>)}
        </div>
        <div className="flex items-center justify-between gap-2">
          <p className="text-muted-foreground text-xs">{t("recovery.where")} {t("recovery.clipboardNote")}</p>
          <Button variant="outline" size="sm" onClick={copy} className="shrink-0">{copied ? <Check /> : <Copy />}{copied ? t("recovery.copied") : t("recovery.copy")}</Button>
        </div>
        <label className="flex items-start gap-2 text-sm">
          <input type="checkbox" className="mt-0.5 accent-[var(--primary)]" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
          {t("recovery.confirm")}
        </label>
        <DialogFooter><Button className="w-full" disabled={!saved} onClick={onDone}>{t("recovery.continue")}</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
