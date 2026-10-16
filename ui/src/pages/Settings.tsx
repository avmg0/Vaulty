import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { Check, Fingerprint, KeyRound, Loader2, Monitor, Moon, ShieldCheck, Sun } from "lucide-react";
import { useTranslation } from "react-i18next";
import { LANGUAGES } from "@/i18n";
import { COUNTRIES } from "@/i18n/regions";
import { currencyFor, useDisplayNames, useLocale } from "@/hooks/locale";
import { CurrencyPicker } from "@/components/CurrencyPicker";
import { MarketCard } from "@/components/MarketCard";
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
import { cn, formatDate, formatMoney } from "@/lib/utils";

const THEMES: { id: Theme; icon: typeof Sun }[] = [{ id: "system", icon: Monitor }, { id: "light", icon: Sun }, { id: "dark", icon: Moon }];
const ACCENTS: { id: Accent; label: string; swatch: string }[] = [
  { id: "neutral", label: "Default (grey with Vaulty-blue logo)", swatch: "bg-[linear-gradient(135deg,#52525b_50%,#235BAA_50%)]" },
  { id: "black", label: "Black (white in dark mode)", swatch: "bg-black dark:bg-white border border-zinc-300 dark:border-zinc-600" },
  { id: "blue", label: "Blue", swatch: "bg-blue-600" }, { id: "green", label: "Green", swatch: "bg-green-600" },
  { id: "violet", label: "Violet", swatch: "bg-violet-600" }, { id: "orange", label: "Orange", swatch: "bg-orange-500" },
  { id: "rose", label: "Rose", swatch: "bg-rose-600" },
];

export function Settings() {
  return (
    <div className="max-w-3xl space-y-6">
      <ProfileCard />
      <LanguageRegionCard />
      <SecurityCard />
      <FamilySharingCard />
      <AppearanceCard />
      <MarketCard />
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
  const { t } = useTranslation();
  const dirty = name !== user.name || color !== user.color;

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setUser(await api.users.updateProfile(name, color));
    setSaved(true); setTimeout(() => setSaved(false), 1500);
  };

  return (
    <Card>
      <CardHeader><CardTitle>{t("settings.profile.title")}</CardTitle><CardDescription>{t("settings.profile.desc")}</CardDescription></CardHeader>
      <CardContent>
        <form onSubmit={save} className="flex flex-wrap items-end gap-4">
          <UserAvatar name={name || "?"} color={color} className="size-12 text-base" />
          <div className="min-w-48 flex-1 space-y-2"><Label htmlFor="pname">{t("settings.profile.name")}</Label><Input id="pname" value={name} onChange={(e) => setName(e.target.value)} maxLength={40} /></div>
          <div className="space-y-2"><Label>{t("settings.profile.colour")}</Label><ColorPicker value={color} onChange={setColor} /></div>
          <Button disabled={!dirty && !saved} variant={saved ? "outline" : "default"}>{saved ? <><Check />{t("common.saved")}</> : t("common.save")}</Button>
        </form>
        <div className="text-muted-foreground mt-3 text-xs">{t("settings.profile.role", { role: t(`role.${user.role}`) })}</div>
      </CardContent>
    </Card>
  );
}

const SELECT = "border-input dark:bg-input/30 h-9 w-full rounded-md border bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]";

/** Language, country and main currency. Saved to the profile and remembered on this device. */
function LanguageRegionCard() {
  const { t } = useTranslation();
  const { locale, language, country, currency, setLocale, setCurrency } = useLocale();
  const names = useDisplayNames();
  const [error, setError] = useState("");
  const countries = useMemo(() => {
    const coll = new Intl.Collator(locale);
    return COUNTRIES.map((c) => ({ c, n: names.region(c) })).sort((a, b) => coll.compare(a.n, b.n));
  }, [locale, names]);

  const save = (patch: { locale?: string; currency?: string }) => {
    setError("");
    if (patch.locale) setLocale(patch.locale);
    if (patch.currency) setCurrency(patch.currency);
    api.settings.update(patch).catch((e) => setError(errorText(e)));
  };
  // Changing country also offers that country's currency, unless the user picked something else on purpose.
  const changeCountry = (c: string) => {
    const follow = currency === currencyFor(country);
    save({ locale: `${language}-${c}`, ...(follow ? { currency: currencyFor(c) } : {}) });
  };
  const lang = LANGUAGES.find((l) => l.code === language);

  return (
    <Card>
      <CardHeader><CardTitle>{t("settings.language.title")}</CardTitle><CardDescription>{t("settings.language.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-4 sm:grid-cols-3">
          <div className="space-y-2"><Label htmlFor="lang">{t("settings.language.language")}</Label>
            <select id="lang" className={SELECT} value={language} onChange={(e) => save({ locale: `${e.target.value}-${country}` })}>
              {LANGUAGES.map((l) => <option key={l.code} value={l.code}>{l.native}{l.code !== language ? ` (${names.language(l.code)})` : ""}</option>)}
            </select>
          </div>
          <div className="space-y-2"><Label htmlFor="country">{t("settings.language.country")}</Label>
            <select id="country" className={SELECT} value={country} onChange={(e) => changeCountry(e.target.value)}>
              {countries.map(({ c, n }) => <option key={c} value={c}>{n}</option>)}
            </select>
          </div>
          <div className="space-y-2"><Label htmlFor="main-currency">{t("settings.language.currency")}</Label>
            <CurrencyPicker id="main-currency" value={currency} onChange={(c) => save({ currency: c })} />
          </div>
        </div>
        <p className="text-muted-foreground text-xs">{t("settings.language.currencyHint")}</p>
        <div className="bg-muted/50 rounded-lg px-3 py-2 text-sm tabular-nums">
          {t("settings.language.preview", { amount: formatMoney(1234567.5, currency), date: formatDate(new Date(), { dateStyle: "long" }) })}
        </div>
        {lang && !lang.complete && <p className="text-muted-foreground text-xs">{t("settings.language.partial")} {t("settings.language.contribute")}</p>}
        {error && <p className="text-destructive text-sm">{error}</p>}
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
  const { t } = useTranslation();
  return (
    <Card>
      <CardHeader><CardTitle>{t("settings.appearance.title")}</CardTitle><CardDescription>{t("settings.appearance.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-6">
        <div className="space-y-2"><Label>{t("settings.appearance.theme")}</Label>
          <div className="grid grid-cols-3 gap-2">
            {THEMES.map((m) => (
              <button key={m.id} onClick={() => setTheme(m.id)}
                className={cn("flex items-center justify-center gap-2 rounded-lg border p-3 text-sm transition-colors hover:bg-accent", theme === m.id && "border-primary ring-primary/30 ring-2")}>
                <m.icon className="size-4" />{t(`settings.appearance.${m.id}`)}
              </button>
            ))}
          </div>
        </div>
        <div className="space-y-2"><Label>{t("settings.appearance.accent")}</Label>
          <div className="flex items-center gap-2">
            {ACCENTS.map((a) => (
              <button key={a.id} onClick={() => setAccent(a.id)} title={a.label} aria-label={a.label}
                className={cn("grid size-8 place-items-center rounded-full text-white ring-offset-2 ring-offset-background", a.swatch, accent === a.id && "ring-2 ring-ring")}>
                {accent === a.id && <Check className="size-4 mix-blend-difference" />}
              </button>
            ))}
            <div className="ml-auto flex items-center gap-3">
              <span className="text-muted-foreground max-w-28 text-end text-xs">{t("settings.appearance.logoFollows")}</span>
              <Logo className="w-11" />
            </div>
          </div>
        </div>
        <Row label={t("settings.appearance.hide")} desc={t("settings.appearance.hideDesc")}>
          <Switch checked={hideBalances} onCheckedChange={setHideBalances} />
        </Row>
      </CardContent>
    </Card>
  );
}
