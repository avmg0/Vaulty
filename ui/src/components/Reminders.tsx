import { useEffect, useState } from "react";
import { BellRing } from "lucide-react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type Reminder } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import i18n from "@/i18n";
import { formatDate, formatMoney } from "@/lib/utils";

/** How often reminders are checked while the app is open and unlocked. */
const EVERY_MS = 30 * 60_000;

/** A reminder in the member's language: a title and a short line. */
export function wordReminder(r: Reminder, amounts: boolean): { title: string; body: string } {
  const t = i18n.t.bind(i18n);
  const when = r.days < 0 ? t("reminders.when.overdue", { count: -r.days }) : r.days === 0 ? t("reminders.when.today") : t("reminders.when.in", { count: r.days });
  const money = amounts && r.amount != null ? formatMoney(r.amount, r.currency ?? "INR") : "";
  const date = formatDate(r.date, { weekday: "short", day: "numeric", month: "short" });
  if (r.kind === "price") {
    const was = amounts && r.previous != null ? formatMoney(r.previous, r.currency ?? "INR") : "";
    return { title: t("reminders.price", { name: r.name, percent: r.percent }), body: money && was ? `${was} → ${money}` : date };
  }
  if (r.kind === "budget") return { title: t("reminders.budget.title", { name: r.name, percent: r.percent }), body: money ? t("reminders.budget.spent", { amount: money }) : t("reminders.budget.body") };
  const title = r.days < 0 ? t("reminders.overdue", { name: r.name, when }) : t(`reminders.${r.kind}`, { name: r.name, when });
  return { title, body: money ? `${money} · ${date}` : date };
}

/** While signed in: checks for reminders now and every half hour and shows each one once. */
export function ReminderWatcher() {
  useEffect(() => {
    let stop = false;
    const run = async () => {
      try {
        const [due, settings] = await Promise.all([api.reminders.take(), api.settings.get()]);
        if (stop || !due.length) return;
        if ((await api.reminders.permission()) !== "granted") return;
        const amounts = !!settings.notify_amounts;
        // A few at once read fine; more become one summary.
        if (due.length > 3) {
          await api.reminders.show(i18n.t("reminders.many", { count: due.length }), due.slice(0, 3).map((r) => wordReminder(r, amounts).title).join(" · "));
        } else for (const r of due) { const w = wordReminder(r, amounts); await api.reminders.show(w.title, w.body); }
      } catch { /* locked meanwhile, or notifications unavailable: try again next time */ }
    };
    void run();
    const id = setInterval(run, EVERY_MS);
    return () => { stop = true; clearInterval(id); };
  }, []);
  return null;
}

/** Always mounted: reminders for members who asked to be told while their profile is locked. */
export function LockedReminderListener() {
  useEffect(() => {
    const off = listen<{ name: string }>("vaulty://locked-reminder", async (e) => {
      try {
        if ((await api.reminders.permission()) !== "granted") return;
        await api.reminders.show(i18n.t("reminders.locked.title"), i18n.t("reminders.locked.body", { name: e.payload.name.split(" ")[0] }));
      } catch { /* nothing to do */ }
    }).catch(() => () => {});
    return () => { void off.then((f) => f()); };
  }, []);
  return null;
}

/** Settings: reminders on or off, amounts in notifications, reminders while locked, and a test. */
export function RemindersCard() {
  const { t } = useTranslation();
  const [on, setOn] = useState(true);
  const [amounts, setAmounts] = useState(false);
  const [locked, setLocked] = useState(false);
  const [permission, setPermission] = useState<"granted" | "denied" | "prompt" | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api.settings.get().then((s) => { setOn(s.notifications !== "off"); setAmounts(!!s.notify_amounts); setLocked(!!s.notify_locked); }).catch(() => {});
    api.reminders.permission().then(setPermission).catch(() => setPermission(null));
  }, []);
  const save = async (patch: Parameters<typeof api.settings.update>[0]) => {
    setError("");
    try { await api.settings.update(patch); } catch (e) { setError(errorText(e)); }
  };
  const turn = async (v: boolean) => {
    setOn(v); await save({ notifications: v ? "on" : "off" });
    if (v) setPermission(await api.reminders.permission(true).catch(() => null));
  };
  const test = async () => {
    setError("");
    try {
      const p = await api.reminders.permission(true); setPermission(p);
      if (p !== "granted") return;
      await api.reminders.show(t("reminders.test.title"), t("reminders.test.body"));
    } catch (e) { setError(errorText(e)); }
  };

  return (
    <Card>
      <CardHeader><CardTitle>{t("reminders.title")}</CardTitle><CardDescription>{t("reminders.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-4">
        <Toggle label={t("reminders.on")} desc={t("reminders.onDesc")} checked={on} onChange={turn} />
        <Toggle label={t("reminders.amounts")} desc={t("reminders.amountsDesc")} checked={amounts} disabled={!on} onChange={(v) => { setAmounts(v); void save({ notify_amounts: v }); }} />
        <Toggle label={t("reminders.locked.setting")} desc={t("reminders.locked.settingDesc")} checked={locked} disabled={!on} onChange={(v) => { setLocked(v); void save({ notify_locked: v }); }} />
        {permission === "denied" && <p className="text-sm text-amber-800 dark:text-amber-300">{t("reminders.blocked")}</p>}
        <div className="flex items-center justify-between gap-3">
          <span className="text-muted-foreground text-xs">{t("reminders.testHint")}</span>
          <Button variant="outline" size="sm" disabled={!on} onClick={test}><BellRing />{t("reminders.test.button")}</Button>
        </div>
        {error && <p className="text-destructive text-sm">{error}</p>}
      </CardContent>
    </Card>
  );
}

function Toggle({ label, desc, checked, disabled, onChange }: { label: string; desc: string; checked: boolean; disabled?: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div><div className="text-sm font-medium">{label}</div><div className="text-muted-foreground text-xs">{desc}</div></div>
      <Switch aria-label={label} checked={checked} disabled={disabled} onCheckedChange={onChange} />
    </div>
  );
}
