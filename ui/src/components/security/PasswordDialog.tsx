import { useState, type FormEvent, type ReactNode } from "react";
import i18n from "@/i18n";
import { Loader2 } from "lucide-react";
import { errorText } from "@/api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

/** Asks for the current password before a security change. `action` receives it. */
export function PasswordDialog({ open, onOpenChange, title, description, confirmLabel = i18n.t("security.continue"), action, destructive }: {
  open: boolean; onOpenChange: (o: boolean) => void; title: string; description: ReactNode; confirmLabel?: string;
  action: (password: string) => Promise<void>; destructive?: boolean;
}) {
  const [pw, setPw] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    try { await action(pw); setPw(""); onOpenChange(false); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) { setPw(""); setError(""); } onOpenChange(o); }}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader><DialogTitle>{title}</DialogTitle><DialogDescription>{description}</DialogDescription></DialogHeader>
        <form onSubmit={submit} className="space-y-4">
          <Input type="password" autoFocus placeholder={i18n.t("security.currentPassword")} value={pw} onChange={(e) => setPw(e.target.value)} required />
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button variant={destructive ? "destructive" : "default"} disabled={busy}>{busy && <Loader2 className="animate-spin" />}{confirmLabel}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
