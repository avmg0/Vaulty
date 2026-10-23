import { Bell, DatabaseBackup, Keyboard, Languages, LineChart, Monitor, Palette, ShieldCheck, UserRound, Users, type LucideIcon } from "lucide-react";

/** The sections of Settings, in order. Labels are `settings.tabs.<id>` in the translation files. */
export const SETTINGS_TABS = ["profile", "security", "language", "appearance", "reminders", "backups", "family", "shortcuts", "computer", "market"] as const;
export type SettingsTab = (typeof SETTINGS_TABS)[number];

export const SETTINGS_TAB_ICON: Record<SettingsTab, LucideIcon> = {
  profile: UserRound, security: ShieldCheck, language: Languages, appearance: Palette, reminders: Bell,
  backups: DatabaseBackup, family: Users, shortcuts: Keyboard, computer: Monitor, market: LineChart,
};

/** Sent when another part of the app asks Settings to show a section. */
export const SETTINGS_TAB_EVENT = "vaulty:settings-tab";
const ASKED = "vaulty.settings.open";
const LAST = "vaulty.settings.tab";

const isTab = (v: string | null): v is SettingsTab => !!v && (SETTINGS_TABS as readonly string[]).includes(v);

/** Ask Settings to open at a section (before navigating there, or while it's showing). */
export function requestSettingsTab(tab: SettingsTab) {
  try { sessionStorage.setItem(ASKED, tab); } catch { /* storage unavailable: the event still works */ }
  window.dispatchEvent(new CustomEvent(SETTINGS_TAB_EVENT, { detail: tab }));
}

/** The section to show when Settings opens: one asked for, else the last one seen. */
export function initialSettingsTab(): SettingsTab {
  try {
    const asked = sessionStorage.getItem(ASKED);
    sessionStorage.removeItem(ASKED);
    if (isTab(asked)) return asked;
    const last = localStorage.getItem(LAST);
    if (isTab(last)) return last;
  } catch { /* storage unavailable */ }
  return "profile";
}

export function rememberSettingsTab(tab: SettingsTab) {
  try { localStorage.setItem(LAST, tab); } catch { /* storage unavailable */ }
}
