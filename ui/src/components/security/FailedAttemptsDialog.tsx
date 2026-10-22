import { useState } from "react";
import { HeartHandshake, ShieldAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { FailedAttempts, HelpedNotice } from "@/api";
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

/** Shown once to a helper whose approval let a family member back into their profile. */
export function HelpedDialog({ helped, onDone }: { helped: HelpedNotice[]; onDone: () => void }) {
  const { t } = useTranslation();
  if (!helped.length) return null;
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onDone(); }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2"><HeartHandshake className="size-5" />{t("helpers.notice.title")}</DialogTitle>
          <DialogDescription>{t("helpers.notice.desc")}</DialogDescription>
        </DialogHeader>
        <ul className="space-y-1 text-sm">
          {helped.map((h, i) => <li key={i}>{t("helpers.notice.line", { name: h.owner_name, date: formatDate(new Date(h.at * 1000)) })}</li>)}
        </ul>
        <p className="text-muted-foreground text-sm">{t("helpers.notice.advice")}</p>
        <DialogFooter><Button onClick={onDone}>{t("helpers.notice.ok")}</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
