import { useState } from "react";
import { ShieldAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { FailedAttempts } from "@/api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ChangePasswordDialog } from "@/pages/Settings";
import { formatDate } from "@/lib/utils";

/** Shown once after signing in when someone got the password or code wrong in the meantime. */
export function FailedAttemptsDialog({ attempts, onDone }: { attempts: FailedAttempts | null; onDone: () => void }) {
  const { t } = useTranslation();
  const [change, setChange] = useState(false);
  if (!attempts) return null;
  const when = new Date(attempts.last_at * 1000);
  const time = when.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  return (
    <>
      <Dialog open={!change} onOpenChange={(o) => { if (!o) onDone(); }}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2"><ShieldAlert className="size-5 text-amber-600 dark:text-amber-400" />{t("attempts.title", { count: attempts.count })}</DialogTitle>
            <DialogDescription>{t("attempts.desc", { count: attempts.count, date: formatDate(when), time })}</DialogDescription>
          </DialogHeader>
          <p className="text-muted-foreground text-sm">{t("attempts.advice")}</p>
          <DialogFooter>
            <Button variant="outline" onClick={onDone}>{t("attempts.wasMe")}</Button>
            <Button onClick={() => setChange(true)}>{t("attempts.change")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ChangePasswordDialog open={change} onOpenChange={(o) => { if (!o) { setChange(false); onDone(); } }} />
    </>
  );
}
