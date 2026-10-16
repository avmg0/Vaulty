import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { ArrowLeft, Fingerprint, KeyRound, Languages, Loader2, Plus, ShieldCheck } from "lucide-react";
import { Trans, useTranslation } from "react-i18next";
import { LANGUAGES } from "@/i18n";
import { useLocale } from "@/hooks/locale";
import { Logo } from "@/components/Logo";
import { api, biometricLabel, errorText, type SignedInUser, type User } from "@/api";
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
  | { kind: "totp"; user: User; backup: boolean }
  | { kind: "recover"; user: User };

/** `fresh` is true when the profile was created just now, so it can inherit the device language. */
export function AuthGate({ onSignedIn, lastUserId }: { onSignedIn: (u: SignedInUser, fresh?: boolean) => void; lastUserId?: string }) {
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
  if (users.length === 0) return <Centered><FirstRun onDone={(u) => onSignedIn(u, true)} /></Centered>;

  const back = users.length > 1 || step.kind !== "password" ? () => setStep({ kind: "pick" }) : undefined;
  return (
    <Centered>
      {step.kind === "pick" && <ProfilePicker users={users} onPick={(u) => setStep(u.active ? { kind: "password", user: u } : { kind: "claim", user: u })} />}
      {step.kind === "password" && (
        <PasswordStep user={step.user} onBack={back} onSignedIn={onSignedIn} onNeedsCode={() => setStep({ kind: "totp", user: step.user, backup: false })}
          onForgot={() => setStep({ kind: "recover", user: step.user })} />
      )}
      {step.kind === "recover" && <RecoverStep user={step.user} onBack={() => setStep({ kind: "password", user: step.user })} onDone={onSignedIn} />}
      {step.kind === "claim" && <ClaimStep user={step.user} onBack={() => setStep({ kind: "pick" })} onDone={(u) => onSignedIn(u, true)} />}
      {step.kind === "totp" && (
        <SecondFactorStep user={step.user} backup={step.backup} onToggle={() => setStep({ ...step, backup: !step.backup })}
          onBack={() => setStep({ kind: "password", user: step.user })} onDone={onSignedIn} />
      )}
    </Centered>
  );
}

function Centered({ children }: { children: ReactNode }) {
  return (
    <div className="grid min-h-screen place-items-center p-4">
      <div className="page-enter w-full max-w-sm">{children}<LanguageSwitch /></div>
    </div>
  );
}

/** Lets each family member read the sign-in screens in their own language. */
function LanguageSwitch() {
  const { t } = useTranslation();
  const { language, country, setLocale } = useLocale();
  return (
    <label className="text-muted-foreground mt-4 flex items-center justify-center gap-1.5 text-xs">
      <Languages className="size-3.5" /><span className="sr-only">{t("common.language")}</span>
      <select value={language} onChange={(e) => setLocale(`${e.target.value}-${country}`)}
        className="hover:text-foreground cursor-pointer rounded bg-transparent py-0.5 outline-none focus-visible:ring-2 focus-visible:ring-ring">
        {LANGUAGES.map((l) => <option key={l.code} value={l.code}>{l.native}</option>)}
      </select>
    </label>
  );
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
  const { t } = useTranslation();
  if (!onClick) return null;
  return <Button variant="ghost" size="sm" className="text-muted-foreground mb-2 -ml-2" onClick={onClick}><ArrowLeft />{t("auth.switchProfile")}</Button>;
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
  const { t } = useTranslation();
  return (
    <Card>
      <Brand title={t("auth.whoTitle")} desc={t("auth.whoDesc")} />
      <CardContent className="grid gap-2">
        {users.map((u) => (
          <button key={u.id} onClick={() => onPick(u)}
            className="hover:bg-accent flex items-center gap-3 rounded-lg border p-3 text-left transition-colors">
            <UserAvatar name={u.name} color={u.color} />
            <div className="min-w-0 flex-1">
              <div className="truncate font-medium">{u.name}</div>
              <div className="text-muted-foreground text-xs">{t(`role.${u.role}`)}</div>
            </div>
            {!u.active && <Badge variant="secondary">{t("auth.setUp")}</Badge>}
            {u.active && u.biometric_enabled && <Fingerprint className="text-muted-foreground size-4" />}
          </button>
        ))}
        <p className="text-muted-foreground pt-2 text-center text-xs">{t("auth.newHere")}</p>
      </CardContent>
    </Card>
  );
}

function PasswordStep({ user, onBack, onSignedIn, onNeedsCode, onForgot }: { user: User; onBack?: () => void; onSignedIn: (u: SignedInUser) => void; onNeedsCode: () => void; onForgot: () => void }) {
  const { t } = useTranslation();
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
          <CardTitle className="text-xl">{t("auth.hi", { name: user.name.split(" ")[0] })}</CardTitle>
          <CardDescription>{t("auth.enterPassword")}</CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-4">
            <Input type="password" autoFocus={!user.biometric_enabled} placeholder={t("auth.password")} aria-label={t("auth.password")} value={pw} onChange={(e) => setPw(e.target.value)} required />
            {error && <p className="text-destructive text-sm">{error}</p>}
            <Button className="w-full" disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <KeyRound />}{t("auth.unlock")}</Button>
            {user.biometric_enabled && (
              <Button type="button" variant="outline" className="w-full" onClick={biometric} disabled={busy}>
                <Fingerprint />{t("auth.useBiometric", { method: biometricLabel() })}
              </Button>
            )}
            <button type="button" onClick={onForgot} className="text-muted-foreground hover:text-foreground w-full text-center text-sm underline-offset-4 hover:underline">
              {t("recovery.forgot")}
            </button>
          </form>
        </CardContent>
      </Card>
    </>
  );
}

function SecondFactorStep({ user, backup, onToggle, onBack, onDone }: { user: User; backup: boolean; onToggle: () => void; onBack: () => void; onDone: (u: SignedInUser) => void }) {
  const { t } = useTranslation();
  const [code, setCode] = useState("");
  const { busy, error, run, setError } = useSubmit();
  const submit = (c = code) => run(async () => onDone(backup ? await api.auth.signInBackupCode(c) : await api.auth.signInTotp(c)));
  useEffect(() => { setCode(""); setError(""); }, [backup]);

  return (
    <>
      <Button variant="ghost" size="sm" className="text-muted-foreground mb-2 -ml-2" onClick={onBack}><ArrowLeft />{t("common.back")}</Button>
      <Card>
        <CardHeader className="items-center text-center">
          <div className="bg-secondary mb-2 grid size-11 place-items-center rounded-xl"><ShieldCheck className="size-5" /></div>
          <CardTitle className="text-xl">{t("auth.twoStep")}</CardTitle>
          <CardDescription>
            {backup ? t("auth.enterBackup") : <Trans i18nKey="auth.enterCode" values={{ name: user.name }} components={{ b: <b /> }} />}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={(e) => { e.preventDefault(); submit(); }} className="space-y-4">
            {backup
              ? <Input autoFocus placeholder="XXXX-XXXX" className="h-12 text-center font-mono text-lg tracking-widest uppercase" value={code} onChange={(e) => setCode(e.target.value)} />
              : <CodeInput value={code} onChange={(v) => { setCode(v); if (v.length === 6) submit(v); }} />}
            {error && <p className="text-destructive text-sm">{error}</p>}
            <Button className="w-full" disabled={busy || (!backup && code.length < 6)}>{busy && <Loader2 className="animate-spin" />}{t("auth.verify")}</Button>
            <button type="button" onClick={onToggle} className="text-muted-foreground hover:text-foreground w-full text-center text-sm underline-offset-4 hover:underline">
              {backup ? t("auth.useApp") : t("auth.lostPhone")}
            </button>
          </form>
        </CardContent>
      </Card>
    </>
  );
}

function NewPasswordFields({ pw, setPw, confirm, setConfirm }: { pw: string; setPw: (v: string) => void; confirm: string; setConfirm: (v: string) => void }) {
  const { t } = useTranslation();
  const strength = Math.min(4, Math.floor(pw.length / 4) + (/[^a-zA-Z0-9]/.test(pw) ? 1 : 0) + (/\d/.test(pw) && /[a-zA-Z]/.test(pw) ? 1 : 0) - 1);
  return (
    <>
      <div className="space-y-2">
        <Label htmlFor="new-pw">{t("auth.choosePassword")}</Label>
        <Input id="new-pw" type="password" value={pw} onChange={(e) => setPw(e.target.value)} minLength={8} required />
        <div className="flex gap-1" aria-hidden>
          {[0, 1, 2, 3].map((i) => (
            <div key={i} className={cn("h-1 flex-1 rounded-full bg-muted", pw && i <= strength && (strength < 2 ? "bg-destructive" : strength < 3 ? "bg-orange-500" : "bg-success"))} />
          ))}
        </div>
        <p className="text-muted-foreground text-xs">{t("auth.passwordHint")}</p>
      </div>
      <div className="space-y-2">
        <Label htmlFor="new-pw2">{t("auth.confirmPassword")}</Label>
        <Input id="new-pw2" type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} required />
      </div>
    </>
  );
}

function FirstRun({ onDone }: { onDone: (u: SignedInUser) => void }) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [color, setColor] = useState(PROFILE_COLORS[0]);
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const { busy, error, setError, run } = useSubmit();

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (pw !== confirm) return setError(t("auth.mismatch"));
    run(async () => onDone(await api.auth.setupFirstAdmin(name, color, pw)));
  };

  return (
    <Card>
      <Brand title={t("auth.welcome")} desc={t("auth.welcomeDesc")} />
      <CardContent>
        <form onSubmit={submit} className="space-y-4">
          <div className="space-y-2"><Label htmlFor="name">{t("auth.yourName")}</Label><Input id="name" autoFocus value={name} onChange={(e) => setName(e.target.value)} required maxLength={40} /></div>
          <div className="space-y-2"><Label>{t("auth.profileColour")}</Label><ColorPicker value={color} onChange={setColor} /></div>
          <NewPasswordFields pw={pw} setPw={setPw} confirm={confirm} setConfirm={setConfirm} />
          <p className="text-muted-foreground text-xs">{t("auth.noRecovery")}</p>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <Button className="w-full" disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <Plus />}{t("auth.createProfile")}</Button>
        </form>
      </CardContent>
    </Card>
  );
}

function ClaimStep({ user, onBack, onDone }: { user: User; onBack: () => void; onDone: (u: SignedInUser) => void }) {
  const { t } = useTranslation();
  const [code, setCode] = useState("");
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const { busy, error, setError, run } = useSubmit();

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (pw !== confirm) return setError(t("auth.mismatch"));
    run(async () => onDone(await api.auth.claimProfile(user.id, code, pw)));
  };

  return (
    <>
      <BackButton onClick={onBack} />
      <Card>
        <CardHeader className="items-center text-center">
          <UserAvatar name={user.name} color={user.color} className="mb-2 size-14 text-lg" />
          <CardTitle className="text-xl">{t("auth.setupTitle", { name: user.name })}</CardTitle>
          <CardDescription>{t("auth.setupDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="code">{t("auth.setupCode")}</Label>
              <Input id="code" autoFocus placeholder="XXXX-XXXX" className="font-mono tracking-widest uppercase" value={code} onChange={(e) => setCode(e.target.value)} required />
            </div>
            <NewPasswordFields pw={pw} setPw={setPw} confirm={confirm} setConfirm={setConfirm} />
            {error && <p className="text-destructive text-sm">{error}</p>}
            <Button className="w-full" disabled={busy}>{busy && <Loader2 className="animate-spin" />}{t("auth.createMyPassword")}</Button>
          </form>
        </CardContent>
      </Card>
    </>
  );
}

/** Forgot password: the recovery key unlocks the data and a new password is set. */
function RecoverStep({ user, onBack, onDone }: { user: User; onBack: () => void; onDone: (u: SignedInUser) => void }) {
  const { t } = useTranslation();
  const [key, setKey] = useState("");
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const { busy, error, setError, run } = useSubmit();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (pw !== confirm) return setError(t("auth.mismatch"));
    run(async () => onDone(await api.auth.signInRecovery(user.id, key, pw)));
  };
  return (
    <>
      <Button variant="ghost" size="sm" className="text-muted-foreground mb-2 -ml-2" onClick={onBack}><ArrowLeft />{t("common.back")}</Button>
      <Card>
        <CardHeader className="items-center text-center">
          <div className="bg-secondary mb-2 grid size-11 place-items-center rounded-xl"><KeyRound className="size-5" /></div>
          <CardTitle className="text-xl">{t("recovery.recoverTitle")}</CardTitle>
          <CardDescription>{t("recovery.recoverDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="rk">{t("recovery.recoveryKey")}</Label>
              <Input id="rk" autoFocus autoComplete="off" spellCheck={false} placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
                className="font-mono tracking-wider uppercase" value={key} onChange={(e) => setKey(e.target.value)} required />
            </div>
            <NewPasswordFields pw={pw} setPw={setPw} confirm={confirm} setConfirm={setConfirm} />
            {error && <p className="text-destructive text-sm">{error}</p>}
            <Button className="w-full" disabled={busy}>{busy && <Loader2 className="animate-spin" />}{t("recovery.recover")}</Button>
            <p className="text-muted-foreground text-center text-xs">{user.role === "child" ? t("recovery.childHint") : t("recovery.adultHint")}</p>
          </form>
        </CardContent>
      </Card>
    </>
  );
}
