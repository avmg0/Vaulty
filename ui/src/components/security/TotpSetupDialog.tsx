import { useState, type FormEvent } from "react";
import i18n from "@/i18n";
import { Loader2 } from "lucide-react";
import { api, errorText, type TotpEnrollment } from "@/api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { CodeInput } from "@/components/CodeInput";
import { BackupCodes } from "./BackupCodes";

type Stage = { kind: "password" } | { kind: "scan"; password: string; enrollment: TotpEnrollment } | { kind: "codes"; codes: string[] };

export function TotpSetupDialog({ open, onOpenChange, userName, onEnabled }: { open: boolean; onOpenChange: (o: boolean) => void; userName: string; onEnabled: () => void }) {
  const [stage, setStage] = useState<Stage>({ kind: "password" });
  const [pw, setPw] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const close = (o: boolean) => {
    if (!o) { setStage({ kind: "password" }); setPw(""); setCode(""); setError(""); }
    onOpenChange(o);
  };
  const run = async (fn: () => Promise<void>) => {
    setBusy(true); setError("");
    try { await fn(); } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };

  const begin = (e: FormEvent) => { e.preventDefault(); run(async () => setStage({ kind: "scan", password: pw, enrollment: await api.security.totpBegin(pw) })); };
  const confirm = (e: FormEvent) => {
    e.preventDefault();
    if (stage.kind !== "scan") return;
    run(async () => {
      const codes = await api.security.totpConfirm(stage.password, stage.enrollment.secret_base32, code);
      setStage({ kind: "codes", codes });
      onEnabled();
    });
  };

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="sm:max-w-md">
        {stage.kind === "password" && (
          <>
            <DialogHeader>
              <DialogTitle>{i18n.t("security.totpTitle")}</DialogTitle>
              <DialogDescription>{i18n.t("security.totpDesc")}</DialogDescription>
            </DialogHeader>
            <form onSubmit={begin} className="space-y-4">
              <Input type="password" autoFocus placeholder={i18n.t("security.currentPassword")} value={pw} onChange={(e) => setPw(e.target.value)} required />
              {error && <p className="text-destructive text-sm">{error}</p>}
              <DialogFooter><Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}{i18n.t("security.continue")}</Button></DialogFooter>
            </form>
          </>
        )}
        {stage.kind === "scan" && (
          <>
            <DialogHeader>
              <DialogTitle>{i18n.t("security.scanTitle")}</DialogTitle>
              <DialogDescription>{i18n.t("security.scanDesc")}</DialogDescription>
            </DialogHeader>
            <form onSubmit={confirm} className="space-y-4">
              <div className="flex justify-center">
                <img src={`data:image/png;base64,${stage.enrollment.qr_png_base64}`} alt={i18n.t("security.qrAlt")}
                  className="size-48 rounded-lg bg-white p-2" style={{ imageRendering: "pixelated" }} />
              </div>
              <div className="space-y-1">
                <Label className="text-muted-foreground text-xs">{i18n.t("security.manualKey")}</Label>
                <div className="bg-muted rounded-md px-3 py-2 font-mono text-xs break-all select-all">{stage.enrollment.secret_base32.match(/.{1,4}/g)?.join(" ")}</div>
              </div>
              <CodeInput value={code} onChange={setCode} />
              {error && <p className="text-destructive text-sm">{error}</p>}
              <DialogFooter><Button disabled={busy || code.length < 6}>{busy && <Loader2 className="animate-spin" />}{i18n.t("security.verifyOn")}</Button></DialogFooter>
            </form>
          </>
        )}
        {stage.kind === "codes" && (
          <>
            <DialogHeader>
              <DialogTitle>{i18n.t("security.saveCodes")}</DialogTitle>
              <DialogDescription>{i18n.t("security.saveCodesDesc")}</DialogDescription>
            </DialogHeader>
            <BackupCodes codes={stage.codes} userName={userName} />
            <DialogFooter><Button onClick={() => close(false)}>{i18n.t("security.savedThem")}</Button></DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
