import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { Skeleton } from "@/components/Skeleton";
import { Check, Fingerprint, KeyRound, Loader2, LockKeyhole, Monitor, Moon, ShieldCheck, Sun, Timer } from "lucide-react";
import { useTranslation } from "react-i18next";
import { LANGUAGES } from "@/i18n";
import { COUNTRIES } from "@/i18n/regions";
import { currencyFor, useDisplayNames, useLocale, type DisplayPrefs } from "@/hooks/locale";
import { MONTH_IDS, resolveFiscalStart, resolveWeekStart } from "@/lib/formats";
import { CurrencyPicker } from "@/components/CurrencyPicker";
import { MarketCard } from "@/components/MarketCard";
import { SnapshotsCard } from "@/components/SnapshotsCard";
import { BackupsCard } from "@/components/BackupsCard";
import { SuccessTick } from "@/components/SuccessTick";
import { Segmented } from "@/components/Segmented";
import { motion as m } from "motion/react";
import { useMotion } from "@/lib/motion";
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
import { RecoveryKeyDialog } from "@/components/security/RecoveryKeyDialog";
import { AUTO_LOCK_CHOICES, getAutoLockMinutes, setAutoLockMinutes } from "@/hooks/idle";
import { FamilySharingCard } from "@/components/FamilySharingCard";
import { useTheme, type Accent, type Theme } from "@/hooks/theme";
import { useSession } from "@/hooks/session";
import { Logo } from "@/components/Logo";
import { cn, formatDate, formatDateAs, formatMoney } from "@/lib/utils";

const THEMES: { id: Theme; icon: typeof Sun }[] = [{ id: "system", icon: Monitor }, { id: "light", icon: Sun }, { id: "dark", icon: Moon }];
/** Names are `settings.accent.<id>` in the translation files. */
const ACCENTS: { id: Accent; swatch: string }[] = [
  { id: "neutral", swatch: "bg-[linear-gradient(135deg,#52525b_50%,#235BAA_50%)]" },
  { id: "black", swatch: "bg-black dark:bg-white border border-zinc-300 dark:border-zinc-600" },
  { id: "blue", swatch: "bg-blue-600" }, { id: "green", swatch: "bg-green-600" },
  { id: "violet", swatch: "bg-violet-600" }, { id: "orange", swatch: "bg-orange-500" },
  { id: "rose", swatch: "bg-rose-600" },
];

export function Settings() {
  return (
    <div className="max-w-3xl space-y-6">
      <ProfileCard />
      <LanguageRegionCard />
      <FormatsCard />
      <SecurityCard />
      <BackupsCard />
      <SnapshotsCard />
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
          <Button disabled={!dirty && !saved} variant={saved ? "outline" : "default"}>{saved ? <><SuccessTick />{t("common.saved")}</> : t("common.save")}</Button>
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

/** Dates, negative amounts, decimals, week start and financial year. Saved to the profile. */
function FormatsCard() {
  const { t } = useTranslation();
  const { locale, country, currency, display, setDisplay } = useLocale();
  const [error, setError] = useState("");
  const sample = new Date(2026, 9, 17);
  // A Sunday, so weekday names line up: index 0 = Sunday … 6 = Saturday.
  const weekday = (i: number) => new Intl.DateTimeFormat(locale, { weekday: "long" }).format(new Date(2026, 9, 18 + i));
  const month = (i: number) => new Intl.DateTimeFormat(locale, { month: "long" }).format(new Date(2026, i, 1));
  const WEEK_INDEX = { sunday: 0, monday: 1, saturday: 6 } as const;
  const autoWeek = resolveWeekStart("auto", locale), autoFiscal = resolveFiscalStart("auto", country);

  const save = (patch: Partial<DisplayPrefs>) => {
    const next = { ...display, ...patch };
    setDisplay(next); setError("");
    api.settings.update({ date_format: next.dateFormat, negative_format: next.negative, decimals: next.decimals, start_of_week: next.weekStart, fiscal_year_start: next.fiscalStart })
      .catch((e) => setError(errorText(e)));
  };
  const field = (id: string, label: string, value: string, onChange: (v: string) => void, options: [string, string][]) => (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <select id={id} className={SELECT} value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
      </select>
    </div>
  );
  // Examples are shown as each choice would format them.
  const ex = (order: DisplayPrefs["dateFormat"]) => formatDateAs(sample, order);
  const autoLabel = (shown: string) => t("formats.auto", { example: shown });

  return (
    <Card>
      <CardHeader><CardTitle>{t("formats.title")}</CardTitle><CardDescription>{t("formats.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-4 sm:grid-cols-2">
          {field("date-order", t("formats.date"), display.dateFormat, (v) => save({ dateFormat: v as DisplayPrefs["dateFormat"] }), [
            ["auto", autoLabel(ex("auto"))],
            ["dmy", ex("dmy")], ["mdy", ex("mdy")], ["ymd", ex("ymd")],
          ])}
          {field("negatives", t("formats.negative"), display.negative, (v) => save({ negative: v as DisplayPrefs["negative"] }), [
            ["minus", t("formats.minus", { example: `−${formatMoney(1234.5, currency).replace(/^[-−]/, "")}` })],
            ["parentheses", t("formats.parentheses", { example: `(${formatMoney(1234.5, currency)})` })],
          ])}
          {field("decimals", t("formats.decimals"), display.decimals, (v) => save({ decimals: v as DisplayPrefs["decimals"] }), [
            ["auto", t("formats.decimalsAuto")], ["0", t("formats.decimals0")], ["2", t("formats.decimals2")],
          ])}
          {field("week-start", t("formats.weekStart"), display.weekStart, (v) => save({ weekStart: v as DisplayPrefs["weekStart"] }), [
            ["auto", autoLabel(weekday(WEEK_INDEX[autoWeek]))],
            ["sunday", weekday(0)], ["monday", weekday(1)], ["saturday", weekday(6)],
          ])}
          {field("fiscal-start", t("formats.fiscal"), display.fiscalStart, (v) => save({ fiscalStart: v as DisplayPrefs["fiscalStart"] }), [
            ["auto", autoLabel(month(MONTH_IDS.indexOf(autoFiscal)))],
            ...MONTH_IDS.map((m, i) => [m, month(i)] as [string, string]),
          ])}
        </div>
        <div className="bg-muted/50 rounded-lg px-3 py-2 text-sm tabular-nums">
          {t("formats.preview", { date: formatDate(sample), amount: formatMoney(-1234.5, currency) })}
        </div>
        {error && <p className="text-destructive text-sm">{error}</p>}
      </CardContent>
    </Card>
  );
}

function SecurityCard() {
  const { user } = useSession();
  const [status, setStatus] = useState<SecurityStatus | null>(null);
  const [dialog, setDialog] = useState<null | "password" | "totp-on" | "totp-off" | "codes" | "bio-on" | "recovery">(null);
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null);
  const [autoLock, setAutoLock] = useState(getAutoLockMinutes);
  const [newCodes, setNewCodes] = useState<string[] | null>(null);
  const [bioError, setBioError] = useState("");
  const { t } = useTranslation();
  const refresh = () => api.security.status().then(setStatus);
  useEffect(() => { refresh(); }, []);

  if (!status) return <Card><CardContent className="space-y-4 py-2">{[0, 1, 2].map((i) => <div key={i} className="flex items-center gap-3"><Skeleton className="size-4" /><div className="flex-1 space-y-1.5"><Skeleton className="h-3.5 w-1/3" /><Skeleton className="h-3 w-1/2" /></div><Skeleton className="h-5 w-9 rounded-full" /></div>)}</CardContent></Card>;
  const bio = biometricLabel();

  return (
    <Card>
      <CardHeader><CardTitle>{t("security.title")}</CardTitle><CardDescription>{t("security.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-4">
        <Row icon={KeyRound} label={t("security.password")} desc={t("security.passwordDesc")}>
          <Button variant="outline" size="sm" onClick={() => setDialog("password")}>{t("common.change")}</Button>
        </Row>
        <Separator />
        <Row icon={ShieldCheck} label={t("security.twoStep")}
          desc={status.totp_enabled ? t("security.twoStepOn", { count: status.backup_codes_remaining }) : t("security.twoStepOff")}>
          {status.totp_enabled && <Button variant="ghost" size="sm" onClick={() => setDialog("codes")}>{t("security.newCodes")}</Button>}
          <Switch checked={status.totp_enabled} onCheckedChange={(on) => setDialog(on ? "totp-on" : "totp-off")} />
        </Row>
        <Separator />
        <Row icon={Fingerprint} label={t("security.unlockWith", { method: bio })}
          desc={!status.biometric_available ? t("security.bioUnavailable") : status.biometric_enabled ? t("security.bioOn") : t("security.bioOff")}>
          <Switch disabled={!status.biometric_available} checked={status.biometric_enabled}
            onCheckedChange={async (on) => {
              setBioError("");
              if (on) setDialog("bio-on");
              else { await api.security.biometricDisable(); refresh(); }
            }} />
        </Row>
        {bioError && <p className="text-destructive text-sm">{bioError}</p>}
        <Separator />
        <Row icon={LockKeyhole} label={t("security.recoveryKey")}
          desc={user.role === "child"
            ? t("security.recoveryChild")
            : t("security.recoveryAdult")}>
          <Button variant="outline" size="sm" onClick={() => setDialog("recovery")}>{t("security.newKey")}</Button>
        </Row>
        <Separator />
        <Row icon={Timer} label={t("security.autoLock")} desc={t("security.autoLockDesc")}>
          <select className="border-input dark:bg-input/30 h-8 rounded-md border bg-transparent px-2 text-sm" value={autoLock}
            onChange={(e) => { const m = Number(e.target.value); setAutoLock(m); setAutoLockMinutes(m); }} aria-label={t("security.autoLock")}>
            {AUTO_LOCK_CHOICES.map((m) => <option key={m} value={m}>{m === 0 ? t("security.never") : t("security.minutes", { count: m })}</option>)}
          </select>
        </Row>
        <Separator />
        {[t("security.screenshot"), t("security.blur")].map((l) => (
          <Row key={l} label={l}><Badge variant="secondary">{t("common.comingSoon")}</Badge></Row>
        ))}
      </CardContent>

      <ChangePasswordDialog open={dialog === "password"} onOpenChange={(o) => setDialog(o ? "password" : null)} />
      <PasswordDialog open={dialog === "recovery"} onOpenChange={(o) => setDialog(o ? "recovery" : null)}
        title={t("security.newKeyTitle")} description={t("security.newKeyDesc")}
        confirmLabel={t("security.newKeyButton")} action={async (pw) => setRecoveryKey(await api.security.newRecoveryKey(pw))} />
      <RecoveryKeyDialog recoveryKey={recoveryKey} isNew child={user.role === "child"} onDone={() => setRecoveryKey(null)} />
      <TotpSetupDialog open={dialog === "totp-on"} onOpenChange={(o) => setDialog(o ? "totp-on" : null)} userName={user.name} onEnabled={refresh} />
      <PasswordDialog open={dialog === "totp-off"} onOpenChange={(o) => setDialog(o ? "totp-off" : null)} destructive
        title={t("security.totpOffTitle")} description={t("security.totpOffDesc")}
        confirmLabel={t("security.turnOff")} action={async (pw) => { await api.security.totpDisable(pw); refresh(); }} />
      <PasswordDialog open={dialog === "codes"} onOpenChange={(o) => setDialog(o ? "codes" : null)}
        title={t("security.genTitle")} description={t("security.genDesc")}
        confirmLabel={t("security.generate")} action={async (pw) => { setNewCodes(await api.security.regenerateBackupCodes(pw)); refresh(); }} />
      <PasswordDialog open={dialog === "bio-on"} onOpenChange={(o) => setDialog(o ? "bio-on" : null)}
        title={t("security.bioTitle", { method: bio })} description={t("security.bioDesc", { method: bio })}
        action={async (pw) => {
          try { await api.security.biometricEnable(pw); } catch (e) { setBioError(errorText(e)); throw e; }
          refresh();
        }} />
      <Dialog open={!!newCodes} onOpenChange={(o) => !o && setNewCodes(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader><DialogTitle>{t("security.newCodesTitle")}</DialogTitle><DialogDescription>{t("security.newCodesDesc")}</DialogDescription></DialogHeader>
          {newCodes && <BackupCodes codes={newCodes} userName={user.name} />}
          <DialogFooter><Button onClick={() => setNewCodes(null)}>{t("members.done")}</Button></DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

function ChangePasswordDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const [cur, setCur] = useState(""), [next, setNext] = useState(""), [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const reset = () => { setCur(""); setNext(""); setConfirm(""); setError(""); };
  const { t } = useTranslation();
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (next !== confirm) return setError(t("security.mismatch"));
    setBusy(true); setError("");
    try { await api.security.changePassword(cur, next); reset(); onOpenChange(false); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };
  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) reset(); onOpenChange(o); }}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader><DialogTitle>{t("security.changePassword")}</DialogTitle></DialogHeader>
        <form onSubmit={submit} className="space-y-3">
          <Input type="password" autoFocus placeholder={t("security.currentPassword")} value={cur} onChange={(e) => setCur(e.target.value)} required />
          <Input type="password" placeholder={t("security.newPassword")} minLength={8} value={next} onChange={(e) => setNext(e.target.value)} required />
          <Input type="password" placeholder={t("security.confirmPassword")} value={confirm} onChange={(e) => setConfirm(e.target.value)} required />
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}{t("security.changePassword")}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function AppearanceCard() {
  const { theme, setTheme, accent, setAccent, hideBalances, setHideBalances, hiddenInMain, setHiddenInMain, motionPref, setMotionPref, motion } = useTheme();
  const { t } = useTranslation();
  const { move } = useMotion();
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
        <div className="space-y-2"><Label>{t("settings.appearance.motion")}</Label>
          <p className="text-muted-foreground text-xs">{t("settings.appearance.motionDesc")}</p>
          <div><Segmented value={motionPref} onChange={setMotionPref} label={t("settings.appearance.motion")}
            options={(["full", "reduced", "off"] as const).map((x) => ({ value: x, label: t(`settings.appearance.motion${x[0].toUpperCase()}${x.slice(1)}`) }))} /></div>
          {motionPref === "full" && motion === "reduced" && <p className="text-muted-foreground text-xs">{t("settings.appearance.motionSystem")}</p>}
        </div>
        <div className="space-y-2"><Label>{t("settings.appearance.accent")}</Label>
          <div className="flex items-center gap-2">
            {ACCENTS.map((a) => (
              <m.button key={a.id} onClick={() => setAccent(a.id)} title={t(`settings.accent.${a.id}`)} aria-label={t(`settings.accent.${a.id}`)} aria-pressed={accent === a.id}
                animate={accent === a.id ? { scale: [1, 1.18, 1] } : { scale: 1 }} transition={move({ duration: 0.3 })}
                whileHover={{ scale: 1.08 }} whileTap={{ scale: 0.92 }}
                className={cn("grid size-8 place-items-center rounded-full text-white ring-offset-2 ring-offset-background transition-shadow", a.swatch, accent === a.id && "ring-2 ring-ring")}>
                {accent === a.id && <SuccessTick className="size-4 mix-blend-difference" />}
              </m.button>
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
        <Row label={t("hidden.pref")} desc={t("hidden.prefDesc")}>
          <Segmented value={hiddenInMain} onChange={setHiddenInMain} label={t("hidden.pref")}
            options={[{ value: "masked", label: t("hidden.prefMasked") }, { value: "off", label: t("hidden.prefOff") }]} />
        </Row>
      </CardContent>
    </Card>
  );
}
