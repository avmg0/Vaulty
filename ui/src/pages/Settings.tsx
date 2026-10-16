import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { Check, Fingerprint, KeyRound, Loader2, Monitor, Moon, ShieldCheck, Sun } from "lucide-react";
import { api, biometricLabel, errorText, type SecurityStatus } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { Badge } from "@/components/ui/badge";
import { ColorPicker, UserAvatar } from "@/components/UserAvatar";
import { PasswordDialog } from "@/components/security/PasswordDialog";
import { TotpSetupDialog } from "@/components/security/TotpSetupDialog";
import { BackupCodes } from "@/components/security/BackupCodes";
import { FamilySharingCard } from "@/components/FamilySharingCard";
import { useTheme, type Accent, type Theme } from "@/hooks/theme";
import { useSession } from "@/hooks/session";
import { Logo } from "@/components/Logo";
import { cn } from "@/lib/utils";

const THEMES: { id: Theme; label: string; icon: typeof Sun }[] = [
  { id: "system", label: "System", icon: Monitor }, { id: "light", label: "Light", icon: Sun }, { id: "dark", label: "Dark", icon: Moon },
];
const ACCENTS: { id: Accent; swatch: string }[] = [
  { id: "neutral", swatch: "bg-zinc-800 dark:bg-zinc-200" }, { id: "blue", swatch: "bg-blue-600" }, { id: "green", swatch: "bg-green-600" },
  { id: "violet", swatch: "bg-violet-600" }, { id: "orange", swatch: "bg-orange-500" }, { id: "rose", swatch: "bg-rose-600" },
];

export function Settings() {
  return (
    <div className="max-w-3xl space-y-6">
      <ProfileCard />
      <SecurityCard />
      <FamilySharingCard />
      <AppearanceCard />
    </div>
  );
}

function Row({ icon: Icon, label, desc, children }: { icon?: typeof Sun; label: string; desc?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex items-start gap-3">
        {Icon && <Icon className="text-muted-foreground mt-0.5 size-4 shrink-0" />}
        <div><div className="text-sm font-medium">{label}</div>{desc && <div className="text-muted-foreground text-xs">{desc}</div>}</div>
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

function ProfileCard() {
  const { user, setUser } = useSession();
  const [name, setName] = useState(user.name);
  const [color, setColor] = useState(user.color);
  const [saved, setSaved] = useState(false);
  const dirty = name !== user.name || color !== user.color;

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setUser(await api.users.updateProfile(name, color));
    setSaved(true); setTimeout(() => setSaved(false), 1500);
  };

  return (
    <Card>
      <CardHeader><CardTitle>Your profile</CardTitle><CardDescription>Only you can see your accounts and transactions.</CardDescription></CardHeader>
      <CardContent>
        <form onSubmit={save} className="flex flex-wrap items-end gap-4">
          <UserAvatar name={name || "?"} color={color} className="size-12 text-base" />
          <div className="min-w-48 flex-1 space-y-2"><Label htmlFor="pname">Name</Label><Input id="pname" value={name} onChange={(e) => setName(e.target.value)} maxLength={40} /></div>
          <div className="space-y-2"><Label>Colour</Label><ColorPicker value={color} onChange={setColor} /></div>
          <Button disabled={!dirty && !saved} variant={saved ? "outline" : "default"}>{saved ? <><Check />Saved</> : "Save"}</Button>
        </form>
        <div className="text-muted-foreground mt-3 text-xs">Role: <span className="capitalize">{user.role}</span></div>
      </CardContent>
    </Card>
  );
}

function SecurityCard() {
  const { user } = useSession();
  const [status, setStatus] = useState<SecurityStatus | null>(null);
  const [dialog, setDialog] = useState<null | "password" | "totp-on" | "totp-off" | "codes" | "bio-on">(null);
  const [newCodes, setNewCodes] = useState<string[] | null>(null);
  const [bioError, setBioError] = useState("");
  const refresh = () => api.security.status().then(setStatus);
  useEffect(() => { refresh(); }, []);

  if (!status) return <Card><CardContent className="py-8"><Loader2 className="text-muted-foreground mx-auto animate-spin" /></CardContent></Card>;
  const bio = biometricLabel();

  return (
    <Card>
      <CardHeader><CardTitle>Sign-in & security</CardTitle><CardDescription>These settings apply only to your profile.</CardDescription></CardHeader>
      <CardContent className="space-y-4">
        <Row icon={KeyRound} label="Password" desc="Used to unlock your profile.">
          <Button variant="outline" size="sm" onClick={() => setDialog("password")}>Change</Button>
        </Row>
        <Separator />
        <Row icon={ShieldCheck} label="Two-step verification"
          desc={status.totp_enabled ? <>On · {status.backup_codes_remaining} backup codes left</> : "Ask for a code from an authenticator app after your password."}>
          {status.totp_enabled && <Button variant="ghost" size="sm" onClick={() => setDialog("codes")}>New backup codes</Button>}
          <Switch checked={status.totp_enabled} onCheckedChange={(on) => setDialog(on ? "totp-on" : "totp-off")} />
        </Row>
        <Separator />
        <Row icon={Fingerprint} label={`Unlock with ${bio}`}
          desc={!status.biometric_available ? "Not available on this device yet." : status.biometric_enabled ? "On for this device. Skips password and 2FA here." : "Unlock without typing your password on this device."}>
          <Switch disabled={!status.biometric_available} checked={status.biometric_enabled}
            onCheckedChange={async (on) => {
              setBioError("");
              if (on) setDialog("bio-on");
              else { await api.security.biometricDisable(); refresh(); }
            }} />
        </Row>
        {bioError && <p className="text-destructive text-sm">{bioError}</p>}
        <Separator />
        {["Auto-lock after inactivity", "Screenshot protection", "Blur in app switcher"].map((l) => (
          <Row key={l} label={l}><Badge variant="secondary">Coming soon</Badge></Row>
        ))}
      </CardContent>

      <ChangePasswordDialog open={dialog === "password"} onOpenChange={(o) => setDialog(o ? "password" : null)} />
      <TotpSetupDialog open={dialog === "totp-on"} onOpenChange={(o) => setDialog(o ? "totp-on" : null)} userName={user.name} onEnabled={refresh} />
      <PasswordDialog open={dialog === "totp-off"} onOpenChange={(o) => setDialog(o ? "totp-off" : null)} destructive
        title="Turn off two-step verification?" description="Your password alone will unlock your profile, and your backup codes will stop working."
        confirmLabel="Turn off" action={async (pw) => { await api.security.totpDisable(pw); refresh(); }} />
      <PasswordDialog open={dialog === "codes"} onOpenChange={(o) => setDialog(o ? "codes" : null)}
        title="Generate new backup codes" description="Your old backup codes will stop working."
        confirmLabel="Generate" action={async (pw) => { setNewCodes(await api.security.regenerateBackupCodes(pw)); refresh(); }} />
      <PasswordDialog open={dialog === "bio-on"} onOpenChange={(o) => setDialog(o ? "bio-on" : null)}
        title={`Turn on ${bio}`} description={`Confirm your password, then approve the ${bio} prompt. Anyone who can pass ${bio} on this computer will be able to open your profile.`}
        action={async (pw) => {
          try { await api.security.biometricEnable(pw); } catch (e) { setBioError(errorText(e)); throw e; }
          refresh();
        }} />
      <Dialog open={!!newCodes} onOpenChange={(o) => !o && setNewCodes(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader><DialogTitle>Your new backup codes</DialogTitle><DialogDescription>Each works once. They won't be shown again.</DialogDescription></DialogHeader>
          {newCodes && <BackupCodes codes={newCodes} userName={user.name} />}
          <DialogFooter><Button onClick={() => setNewCodes(null)}>Done</Button></DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

function ChangePasswordDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const [cur, setCur] = useState(""), [next, setNext] = useState(""), [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const reset = () => { setCur(""); setNext(""); setConfirm(""); setError(""); };
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (next !== confirm) return setError("New passwords don't match.");
    setBusy(true); setError("");
    try { await api.security.changePassword(cur, next); reset(); onOpenChange(false); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) reset(); onOpenChange(o); }}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader><DialogTitle>Change password</DialogTitle></DialogHeader>
        <form onSubmit={submit} className="space-y-3">
          <Input type="password" autoFocus placeholder="Current password" value={cur} onChange={(e) => setCur(e.target.value)} required />
          <Input type="password" placeholder="New password (8+ characters)" minLength={8} value={next} onChange={(e) => setNext(e.target.value)} required />
          <Input type="password" placeholder="Confirm new password" value={confirm} onChange={(e) => setConfirm(e.target.value)} required />
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}Change password</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function AppearanceCard() {
  const { theme, setTheme, accent, setAccent, hideBalances, setHideBalances } = useTheme();
  return (
    <Card>
      <CardHeader><CardTitle>Appearance</CardTitle><CardDescription>Theme and accent colour for this device</CardDescription></CardHeader>
      <CardContent className="space-y-6">
        <div className="space-y-2"><Label>Theme</Label>
          <div className="grid grid-cols-3 gap-2">
            {THEMES.map((t) => (
              <button key={t.id} onClick={() => setTheme(t.id)}
                className={cn("flex items-center justify-center gap-2 rounded-lg border p-3 text-sm transition-colors hover:bg-accent", theme === t.id && "border-primary ring-primary/30 ring-2")}>
                <t.icon className="size-4" />{t.label}
              </button>
            ))}
          </div>
        </div>
        <div className="space-y-2"><Label>Accent colour</Label>
          <div className="flex items-center gap-2">
            {ACCENTS.map((a) => (
              <button key={a.id} onClick={() => setAccent(a.id)} title={a.id}
                className={cn("grid size-8 place-items-center rounded-full text-white ring-offset-2 ring-offset-background", a.swatch, accent === a.id && "ring-2 ring-ring")}>
                {accent === a.id && <Check className="size-4 mix-blend-difference" />}
              </button>
            ))}
            <div className="ml-auto flex items-center gap-3">
              <span className="text-muted-foreground text-right text-xs">The logo follows<br />your accent</span>
              <Logo className="w-11" />
            </div>
          </div>
        </div>
        <Row label="Hide balances (incognito)" desc="Mask amounts everywhere until toggled off (Ctrl/Cmd+H)">
          <Switch checked={hideBalances} onCheckedChange={setHideBalances} />
        </Row>
      </CardContent>
    </Card>
  );
}
