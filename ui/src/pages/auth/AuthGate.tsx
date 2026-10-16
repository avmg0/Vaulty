import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { ArrowLeft, Fingerprint, KeyRound, Loader2, Plus, ShieldCheck } from "lucide-react";
import { Logo } from "@/components/Logo";
import { api, biometricLabel, errorText, type User } from "@/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { CodeInput } from "@/components/CodeInput";
import { ColorPicker, PROFILE_COLORS, UserAvatar } from "@/components/UserAvatar";
import { cn } from "@/lib/utils";

type Step =
  | { kind: "pick" }
  | { kind: "password"; user: User }
  | { kind: "claim"; user: User }
  | { kind: "totp"; user: User; backup: boolean };

export function AuthGate({ onSignedIn, lastUserId }: { onSignedIn: (u: User) => void; lastUserId?: string }) {
  const [users, setUsers] = useState<User[] | null>(null);
  const [step, setStep] = useState<Step>({ kind: "pick" });

  useEffect(() => {
    api.users.list().then((list) => {
      setUsers(list);
      const last = list.find((u) => u.id === lastUserId && u.active);
      if (last) setStep({ kind: "password", user: last });
      else if (list.length === 1 && list[0].active) setStep({ kind: "password", user: list[0] });
    });
  }, [lastUserId]);

  if (!users) return <Centered><Loader2 className="text-muted-foreground size-6 animate-spin" /></Centered>;
  if (users.length === 0) return <Centered><FirstRun onDone={onSignedIn} /></Centered>;

  const back = users.length > 1 || step.kind !== "password" ? () => setStep({ kind: "pick" }) : undefined;
  return (
    <Centered>
      {step.kind === "pick" && <ProfilePicker users={users} onPick={(u) => setStep(u.active ? { kind: "password", user: u } : { kind: "claim", user: u })} />}
      {step.kind === "password" && (
        <PasswordStep user={step.user} onBack={back} onSignedIn={onSignedIn} onNeedsCode={() => setStep({ kind: "totp", user: step.user, backup: false })} />
      )}
      {step.kind === "claim" && <ClaimStep user={step.user} onBack={() => setStep({ kind: "pick" })} onDone={onSignedIn} />}
      {step.kind === "totp" && (
        <SecondFactorStep user={step.user} backup={step.backup} onToggle={() => setStep({ ...step, backup: !step.backup })}
          onBack={() => setStep({ kind: "password", user: step.user })} onDone={onSignedIn} />
      )}
    </Centered>
  );
}

function Centered({ children }: { children: ReactNode }) {
  return <div className="grid min-h-screen place-items-center p-4"><div className="page-enter w-full max-w-sm">{children}</div></div>;
}

function Brand({ title, desc }: { title: string; desc: ReactNode }) {
  return (
    <CardHeader className="items-center text-center">
      <Logo className="mb-2 w-14" />
      <CardTitle className="text-xl">{title}</CardTitle>
      <CardDescription>{desc}</CardDescription>
    </CardHeader>
  );
}

function BackButton({ onClick }: { onClick?: () => void }) {
  if (!onClick) return null;
  return <Button variant="ghost" size="sm" className="text-muted-foreground mb-2 -ml-2" onClick={onClick}><ArrowLeft />Switch profile</Button>;
}

function useSubmit() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const run = async (fn: () => Promise<void>) => {
    setBusy(true); setError("");
    try { await fn(); } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  return { busy, error, setError, run };
}

function ProfilePicker({ users, onPick }: { users: User[]; onPick: (u: User) => void }) {
  return (
    <Card>
      <Brand title="Who's using Vaulty?" desc="Each family member has their own private space." />
      <CardContent className="grid gap-2">
        {users.map((u) => (
          <button key={u.id} onClick={() => onPick(u)}
            className="hover:bg-accent flex items-center gap-3 rounded-lg border p-3 text-left transition-colors">
            <UserAvatar name={u.name} color={u.color} />
            <div className="min-w-0 flex-1">
              <div className="truncate font-medium">{u.name}</div>
              <div className="text-muted-foreground text-xs capitalize">{u.role}</div>
            </div>
            {!u.active && <Badge variant="secondary">Set up</Badge>}
            {u.active && u.biometric_enabled && <Fingerprint className="text-muted-foreground size-4" />}
          </button>
        ))}
        <p className="text-muted-foreground pt-2 text-center text-xs">New here? Ask an admin to add you from the Family page.</p>
      </CardContent>
    </Card>
  );
}

function PasswordStep({ user, onBack, onSignedIn, onNeedsCode }: { user: User; onBack?: () => void; onSignedIn: (u: User) => void; onNeedsCode: () => void }) {
  const [pw, setPw] = useState("");
  const { busy, error, run } = useSubmit();

  const biometric = () => run(async () => onSignedIn(await api.auth.signInBiometric(user.id)));
  // Offer biometrics straight away; if that prompt is dismissed, quietly fall back to the password field.
  useEffect(() => {
    if (user.biometric_enabled) api.auth.signInBiometric(user.id).then(onSignedIn).catch(() => {});
  }, [user.id]);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    run(async () => {
      const r = await api.auth.signIn(user.id, pw);
      if (r.status === "signed_in") onSignedIn(r.user); else onNeedsCode();
    });
  };

  return (
    <>
      <BackButton onClick={onBack} />
      <Card>
        <CardHeader className="items-center text-center">
          <UserAvatar name={user.name} color={user.color} className="mb-2 size-14 text-lg" />
          <CardTitle className="text-xl">Hi, {user.name.split(" ")[0]}</CardTitle>
          <CardDescription>Enter your password to unlock.</CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-4">
            <Input type="password" autoFocus={!user.biometric_enabled} placeholder="Password" value={pw} onChange={(e) => setPw(e.target.value)} required />
            {error && <p className="text-destructive text-sm">{error}</p>}
            <Button className="w-full" disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <KeyRound />}Unlock</Button>
            {user.biometric_enabled && (
              <Button type="button" variant="outline" className="w-full" onClick={biometric} disabled={busy}>
                <Fingerprint />Use {biometricLabel()}
              </Button>
            )}
          </form>
        </CardContent>
      </Card>
    </>
  );
}

function SecondFactorStep({ user, backup, onToggle, onBack, onDone }: { user: User; backup: boolean; onToggle: () => void; onBack: () => void; onDone: (u: User) => void }) {
  const [code, setCode] = useState("");
  const { busy, error, run, setError } = useSubmit();
  const submit = (c = code) => run(async () => onDone(backup ? await api.auth.signInBackupCode(c) : await api.auth.signInTotp(c)));
  useEffect(() => { setCode(""); setError(""); }, [backup]);

  return (
    <>
      <Button variant="ghost" size="sm" className="text-muted-foreground mb-2 -ml-2" onClick={onBack}><ArrowLeft />Back</Button>
      <Card>
        <CardHeader className="items-center text-center">
          <div className="bg-secondary mb-2 grid size-11 place-items-center rounded-xl"><ShieldCheck className="size-5" /></div>
          <CardTitle className="text-xl">Two-step verification</CardTitle>
          <CardDescription>
            {backup ? "Enter one of your saved backup codes." : <>Enter the 6-digit code from your authenticator app for <b>{user.name}</b>.</>}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={(e) => { e.preventDefault(); submit(); }} className="space-y-4">
            {backup
              ? <Input autoFocus placeholder="XXXX-XXXX" className="h-12 text-center font-mono text-lg tracking-widest uppercase" value={code} onChange={(e) => setCode(e.target.value)} />
              : <CodeInput value={code} onChange={(v) => { setCode(v); if (v.length === 6) submit(v); }} />}
            {error && <p className="text-destructive text-sm">{error}</p>}
            <Button className="w-full" disabled={busy || (!backup && code.length < 6)}>{busy && <Loader2 className="animate-spin" />}Verify</Button>
            <button type="button" onClick={onToggle} className="text-muted-foreground hover:text-foreground w-full text-center text-sm underline-offset-4 hover:underline">
              {backup ? "Use authenticator app instead" : "Lost your phone? Use a backup code"}
            </button>
          </form>
        </CardContent>
      </Card>
    </>
  );
}

function NewPasswordFields({ pw, setPw, confirm, setConfirm }: { pw: string; setPw: (v: string) => void; confirm: string; setConfirm: (v: string) => void }) {
  const strength = Math.min(4, Math.floor(pw.length / 4) + (/[^a-zA-Z0-9]/.test(pw) ? 1 : 0) + (/\d/.test(pw) && /[a-zA-Z]/.test(pw) ? 1 : 0) - 1);
  return (
    <>
      <div className="space-y-2">
        <Label htmlFor="new-pw">Choose a password</Label>
        <Input id="new-pw" type="password" value={pw} onChange={(e) => setPw(e.target.value)} minLength={8} required />
        <div className="flex gap-1" aria-hidden>
          {[0, 1, 2, 3].map((i) => (
            <div key={i} className={cn("h-1 flex-1 rounded-full bg-muted", pw && i <= strength && (strength < 2 ? "bg-destructive" : strength < 3 ? "bg-orange-500" : "bg-success"))} />
          ))}
        </div>
        <p className="text-muted-foreground text-xs">At least 8 characters. A short sentence is easy to remember and hard to guess.</p>
      </div>
      <div className="space-y-2">
        <Label htmlFor="new-pw2">Confirm password</Label>
        <Input id="new-pw2" type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} required />
      </div>
    </>
  );
}

function FirstRun({ onDone }: { onDone: (u: User) => void }) {
  const [name, setName] = useState("");
  const [color, setColor] = useState(PROFILE_COLORS[0]);
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const { busy, error, setError, run } = useSubmit();

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (pw !== confirm) return setError("Passwords don't match.");
    run(async () => onDone(await api.auth.setupFirstAdmin(name, color, pw)));
  };

  return (
    <Card>
      <Brand title="Welcome to Vaulty" desc="Create the first profile. You'll be the admin and can add the rest of your family afterwards." />
      <CardContent>
        <form onSubmit={submit} className="space-y-4">
          <div className="space-y-2"><Label htmlFor="name">Your name</Label><Input id="name" autoFocus value={name} onChange={(e) => setName(e.target.value)} required maxLength={40} /></div>
          <div className="space-y-2"><Label>Profile colour</Label><ColorPicker value={color} onChange={setColor} /></div>
          <NewPasswordFields pw={pw} setPw={setPw} confirm={confirm} setConfirm={setConfirm} />
          <p className="text-muted-foreground text-xs">Your password can't be recovered. You can turn on two-step verification and biometric unlock in Settings.</p>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <Button className="w-full" disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <Plus />}Create profile</Button>
        </form>
      </CardContent>
    </Card>
  );
}

function ClaimStep({ user, onBack, onDone }: { user: User; onBack: () => void; onDone: (u: User) => void }) {
  const [code, setCode] = useState("");
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const { busy, error, setError, run } = useSubmit();

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (pw !== confirm) return setError("Passwords don't match.");
    run(async () => onDone(await api.auth.claimProfile(user.id, code, pw)));
  };

  return (
    <>
      <BackButton onClick={onBack} />
      <Card>
        <CardHeader className="items-center text-center">
          <UserAvatar name={user.name} color={user.color} className="mb-2 size-14 text-lg" />
          <CardTitle className="text-xl">Set up {user.name}'s profile</CardTitle>
          <CardDescription>Enter the setup code an admin gave you, then choose your own password.</CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="code">Setup code</Label>
              <Input id="code" autoFocus placeholder="XXXX-XXXX" className="font-mono tracking-widest uppercase" value={code} onChange={(e) => setCode(e.target.value)} required />
            </div>
            <NewPasswordFields pw={pw} setPw={setPw} confirm={confirm} setConfirm={setConfirm} />
            {error && <p className="text-destructive text-sm">{error}</p>}
            <Button className="w-full" disabled={busy}>{busy && <Loader2 className="animate-spin" />}Create my password</Button>
          </form>
        </CardContent>
      </Card>
    </>
  );
}
