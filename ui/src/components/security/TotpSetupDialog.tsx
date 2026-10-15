import { useState, type FormEvent } from "react";
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
              <DialogTitle>Turn on two-step verification</DialogTitle>
              <DialogDescription>You'll need a code from an authenticator app (Google Authenticator, Microsoft Authenticator, Aegis, 1Password…) each time you sign in with your password.</DialogDescription>
            </DialogHeader>
            <form onSubmit={begin} className="space-y-4">
              <Input type="password" autoFocus placeholder="Current password" value={pw} onChange={(e) => setPw(e.target.value)} required />
              {error && <p className="text-destructive text-sm">{error}</p>}
              <DialogFooter><Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}Continue</Button></DialogFooter>
            </form>
          </>
        )}
        {stage.kind === "scan" && (
          <>
            <DialogHeader>
              <DialogTitle>Scan with your authenticator app</DialogTitle>
              <DialogDescription>Then enter the 6-digit code it shows.</DialogDescription>
            </DialogHeader>
            <form onSubmit={confirm} className="space-y-4">
              <div className="flex justify-center">
                <img src={`data:image/png;base64,${stage.enrollment.qr_png_base64}`} alt="QR code for your authenticator app"
                  className="size-48 rounded-lg bg-white p-2" style={{ imageRendering: "pixelated" }} />
              </div>
              <div className="space-y-1">
                <Label className="text-muted-foreground text-xs">Can't scan? Enter this key manually</Label>
                <div className="bg-muted rounded-md px-3 py-2 font-mono text-xs break-all select-all">{stage.enrollment.secret_base32.match(/.{1,4}/g)?.join(" ")}</div>
              </div>
              <CodeInput value={code} onChange={setCode} />
              {error && <p className="text-destructive text-sm">{error}</p>}
              <DialogFooter><Button disabled={busy || code.length < 6}>{busy && <Loader2 className="animate-spin" />}Verify & turn on</Button></DialogFooter>
            </form>
          </>
        )}
        {stage.kind === "codes" && (
          <>
            <DialogHeader>
              <DialogTitle>Save your backup codes</DialogTitle>
              <DialogDescription>If you lose your phone, each of these codes lets you sign in once. They won't be shown again.</DialogDescription>
            </DialogHeader>
            <BackupCodes codes={stage.codes} userName={userName} />
            <DialogFooter><Button onClick={() => close(false)}>I've saved them</Button></DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
