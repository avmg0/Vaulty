import { useEffect, useRef, useState } from "react";
import { Check, Copy, KeyRound, Printer } from "lucide-react";
import { printRecoverySheet } from "@/components/RecoverySheet";
import { useOptionalSession } from "@/hooks/session";
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";

/**
 * Shows a recovery key exactly once. The person must confirm they saved it before continuing,
 * and the dialog can't be dismissed by clicking outside or pressing Escape.
 */
export function RecoveryKeyDialog({ recoveryKey, child, isNew, onDone }: { recoveryKey: string | null; child?: boolean; isNew?: boolean; onDone: () => void }) {
  const { t } = useTranslation();
  const { move, fade } = useMotion();
  const [saved, setSaved] = useState(false);
  const [copied, setCopied] = useState(false);
  const clearTimer = useRef<ReturnType<typeof setTimeout>>();
  const userName = useOptionalSession()?.user.name;
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
          {recoveryKey.split("-").map((g, i) => (
            // Groups appear one by one, which invites reading the key rather than skimming past it.
            <motion.span key={`${recoveryKey}-${i}`} className="inline-block px-1.5"
              initial={{ opacity: 0, y: 6, filter: "blur(4px)" }} animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
              transition={{ ...fade({ duration: 0.3 }), delay: 0.15 + i * 0.12 }}>{g}</motion.span>
          ))}
        </div>
        <div className="flex items-center justify-between gap-2">
          <p className="text-muted-foreground text-xs">{t("recovery.where")} {t("recovery.clipboardNote")}</p>
          <div className="flex shrink-0 gap-1.5">
            <Button variant="outline" size="sm" onClick={() => printRecoverySheet({ name: userName ?? "", recoveryKey, child })}><Printer />{t("sheet.print")}</Button>
            <Button variant="outline" size="sm" onClick={copy}>{copied ? <Check /> : <Copy />}{copied ? t("recovery.copied") : t("recovery.copy")}</Button>
          </div>
        </div>
        <label className="flex items-start gap-2 text-sm">
          <input type="checkbox" className="mt-0.5 accent-[var(--primary)]" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
          {t("recovery.confirm")}
        </label>
        <DialogFooter>
          <motion.div className="w-full" animate={saved ? { scale: [1, 1.03, 1] } : { scale: 1 }} transition={move({ duration: 0.3 })}>
            <Button className="w-full" disabled={!saved} onClick={onDone}>{t("recovery.continue")}</Button>
          </motion.div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
