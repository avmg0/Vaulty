import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import i18n from "@/i18n";
import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { DEFAULT_DISPLAY, displayFrom, useLocale } from "@/hooks/locale";
import { api, type FailedAttempts, type HelpedNotice, type NomineeRequest, type SignedInUser, type User } from "@/api";
import { RecoveryKeyDialog } from "@/components/security/RecoveryKeyDialog";
import { FailedAttemptsDialog, HelpedDialog, NomineeRequestDialog } from "@/components/security/FailedAttemptsDialog";
import { useIdleLock } from "@/hooks/idle";
import { ALL_ITEMS, type PageId } from "@/nav";
import { Sidebar } from "@/components/layout/Sidebar";
import { Header } from "@/components/layout/Header";
import { useTheme } from "@/hooks/theme";
import { SessionProvider } from "@/hooks/session";
import { AuthGate } from "@/pages/auth/AuthGate";
import { Dashboard } from "@/pages/Dashboard";
import { Accounts } from "@/pages/Accounts";
import { Transactions } from "@/pages/Transactions";
import { Settings } from "@/pages/Settings";
import { Family } from "@/pages/Family";
import { Cards } from "@/pages/Cards";
import { ComingSoon } from "@/pages/ComingSoon";
import { Budgets } from "@/pages/Budgets";
import { RecurringPage } from "@/pages/Recurring";
import { ImportPage } from "@/pages/Import";
import { QUICK_ADD_EVENT, QuickAdd } from "@/components/QuickAdd";
import { requestSettingsTab } from "@/lib/settingsTabs";
import { CommandPalette } from "@/components/CommandPalette";
import { TaxPlanner } from "@/pages/TaxPlanner";
import { GoalsPage } from "@/pages/Goals";
import { InsurancePage } from "@/pages/Insurance";
import { JournalPage } from "@/pages/Journal";
import { TickerBar } from "@/components/TickerBar";
import { UndoToaster } from "@/components/UndoToaster";
import { UpdateNotice } from "@/components/Updates";
import { CalendarPage } from "@/pages/Calendar";
import { Reports } from "@/pages/Reports";
import { PeoplePage } from "@/pages/People";
import { TasksPage } from "@/pages/Tasks";
import { DocumentsPage } from "@/pages/Documents";
import { InvestmentsPage } from "@/pages/Investments";
import { ProjectsPage } from "@/pages/Projects";
import { LockedReminderListener, ReminderWatcher } from "@/components/Reminders";
import { HotkeysProvider, useHotkey } from "@/hooks/hotkeys";
import { applySenior } from "@/lib/senior";
import { History } from "@/pages/History";
import { DATA_RESTORED } from "@/api";
import { BookProvider, PERSONAL_ONLY, useBook } from "@/hooks/book";
import { BusinessPage } from "@/pages/Business";
import { VaultSplash } from "@/components/UnlockSplash";

const LAST_USER_KEY = "vaulty.lastUser";
/** Pages that reload themselves after an undo (so changes animate in place instead of the page re-mounting). */
const SELF_RELOADING = new Set<PageId>(["history", "transactions", "dashboard", "cards", "calendar", "reports", "people", "tasks", "documents"]);

export function App() {
  const [user, setUser] = useState<User | null | undefined>(undefined);
  const [backendMissing, setBackendMissing] = useState(false);
  const [lastUserId, setLastUserId] = useState<string | undefined>(() => { try { return localStorage.getItem(LAST_USER_KEY) ?? undefined; } catch { return undefined; } });

  useEffect(() => { api.auth.current().then(setUser).catch(() => setBackendMissing(true)); }, []);
  useEffect(() => { if (user) applySenior(user.senior); }, [user]);

  const { locale, currency, setLocale, setCurrency, setDisplay } = useLocale();
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null);
  const [attempts, setAttempts] = useState<FailedAttempts | null>(null);
  const [helped, setHelped] = useState<HelpedNotice[]>([]);
  const [nomineeRequest, setNomineeRequest] = useState<NomineeRequest | null>(null);
  const [unlocking, setUnlocking] = useState(false);
  const [locking, setLocking] = useState(false);
  const { motion, setHideBalances } = useTheme();
  const signedIn = (u: SignedInUser, fresh?: boolean) => {
    setUser(u);
    setUnlocking(true);
    if (u.recovery_key) setRecoveryKey(u.recovery_key);
    setAttempts(u.failed_attempts ?? null);
    setHelped(u.helped ?? []);
    setNomineeRequest(u.nominee_request ?? null);
    // A new profile starts in the language and currency chosen on this device; an existing one
    // brings its own (which also becomes this device's default for the sign-in screens).
    if (fresh) { api.settings.update({ locale, currency }).catch(() => {}); setDisplay(DEFAULT_DISPLAY); }
    else api.settings.get().then((s) => { if (s.locale) setLocale(s.locale); if (s.currency) setCurrency(s.currency); setDisplay(displayFrom(s)); }).catch(() => {});
    setLastUserId(u.id);
    try { localStorage.setItem(LAST_USER_KEY, u.id); } catch { /* ignore */ }
  };
  // Locking closes the vault: the app blurs away and the logo's dial turns clockwise before the
  // sign-in screen appears. The data is closed straight away; only the picture lingers.
  // The panic shortcut: gone at once, no animation, balances hidden for whoever signs in next.
  const lockNow = useCallback(() => {
    setHideBalances(true);
    setLocking(false);
    setUser(null);
    void api.auth.signOut().catch(() => {});
  }, [setHideBalances]);
  const signOut = useCallback(() => {
    const done = api.auth.signOut();
    if (motion === "off") { void done.finally(() => setUser(null)); return; }
    setLocking(true);
    void done.catch(() => {});
  }, [motion]);
  const lockFinished = useCallback(() => { setUser(null); setLocking(false); }, []);
  const unlockFinished = useCallback(() => setUnlocking(false), []);

  // The tray's "Quick add" and the quick-add shortcut: open it now, or straight after signing in.
  const [quickPending, setQuickPending] = useState(false);
  useEffect(() => {
    const off = listen("vaulty://quick-add", () => setQuickPending(true)).catch(() => () => {});
    return () => { void off.then((f) => f()); };
  }, []);
  useEffect(() => {
    if (!quickPending || !user || unlocking || locking) return;
    // Let the app's screens mount before asking.
    const id = setTimeout(() => { window.dispatchEvent(new Event(QUICK_ADD_EVENT)); setQuickPending(false); }, 50);
    return () => clearTimeout(id);
  }, [quickPending, user, unlocking, locking]);
  // The tray menu speaks the window's language.
  useEffect(() => {
    const send = () => api.device.trayLabels(["quick", "open", "lock", "quit"].map((k) => i18n.t(`tray.${k}`))).catch(() => {});
    send();
    i18n.on("languageChanged", send);
    return () => { i18n.off("languageChanged", send); };
  }, []);

  if (backendMissing) return (
    <div className="text-muted-foreground grid min-h-screen place-items-center p-6 text-center text-sm">
      Vaulty's UI is running without its Rust backend.<br />Start it with <code className="text-foreground">cargo tauri dev</code> instead of <code className="text-foreground">npm run dev</code>.
    </div>
  );
  if (user === undefined) return <div className="grid min-h-screen place-items-center"><Loader2 className="text-muted-foreground size-6 animate-spin" /></div>;
  if (user === null) return <><AuthGate onSignedIn={signedIn} lastUserId={lastUserId} /><LockedReminderListener /></>;

  return (
    <SessionProvider value={{ user, setUser, signOut }}>
      <LockedReminderListener />
      <div className={locking ? "pointer-events-none opacity-0 blur-md transition-[opacity,filter] duration-300" : "transition-[opacity,filter] duration-300"}>
        <Shell key={user.id} signOut={signOut} lockNow={lockNow} />
      </div>
      {unlocking && !locking && <VaultSplash mode="open" onDone={unlockFinished} />}
      {locking && <VaultSplash mode="close" onDone={lockFinished} />}
      <RecoveryKeyDialog recoveryKey={recoveryKey} child={user.role === "child"} onDone={() => setRecoveryKey(null)} />
      {!recoveryKey && !unlocking && <NomineeRequestDialog request={nomineeRequest} onDone={() => setNomineeRequest(null)} />}
      {!recoveryKey && !unlocking && !nomineeRequest && <FailedAttemptsDialog attempts={attempts} onDone={() => setAttempts(null)} />}
      {!recoveryKey && !unlocking && !nomineeRequest && !attempts && <HelpedDialog helped={helped} onDone={() => setHelped([])} />}
    </SessionProvider>
  );
}

function Shell(props: { signOut: () => void; lockNow: () => void }) {
  return <HotkeysProvider><BookProvider><ShellBody {...props} /></BookProvider></HotkeysProvider>;
}

function ShellBody({ signOut, lockNow }: { signOut: () => void; lockNow: () => void }) {
  const { t } = useTranslation();
  const [page, setPage] = useState<PageId>("dashboard");
  const [collapsed, setCollapsed] = useState(false);
  const { hideBalances, setHideBalances } = useTheme();
  useIdleLock(signOut);
  const { book } = useBook();
  // Leaving a page a business book doesn't have.
  useEffect(() => { if (book && (PERSONAL_ONLY as readonly string[]).includes(page)) setPage("dashboard"); }, [book, page]);
  // After an undo or restore, re-mount the open page so it shows current data.
  const [version, setVersion] = useState(0);
  useEffect(() => {
    const bump = () => setVersion((v) => v + 1);
    window.addEventListener(DATA_RESTORED, bump);
    return () => window.removeEventListener(DATA_RESTORED, bump);
  }, []);

  // Keyboard shortcuts (each member can change them in Settings). Ctrl on Windows/Linux, Cmd on macOS.
  useHotkey("sidebar", () => setCollapsed((c) => !c));
  useHotkey("lock", () => signOut());
  useHotkey("panic", () => lockNow());
  useHotkey("hide_balances", () => setHideBalances(!hideBalances));
  useHotkey("settings", () => setPage("settings"));
  useHotkey("go_dashboard", () => setPage("dashboard"));
  useHotkey("go_transactions", () => setPage("transactions"));
  useHotkey("go_accounts", () => setPage("accounts"));
  useHotkey("go_reports", () => setPage("reports"));
  useHotkey("go_budgets", () => setPage("budgets"));
  useHotkey("go_recurring", () => setPage("recurring"));
  useHotkey("go_cards", () => setPage("cards"));
  useHotkey("go_import", () => setPage("import"));

  const item = ALL_ITEMS.find((i) => i.id === page)!;
  const body = { dashboard: <Dashboard />, accounts: <Accounts />, transactions: <Transactions />, settings: <Settings />, family: <Family onOpenSettings={() => { requestSettingsTab("family"); setPage("settings"); }} />, cards: <Cards />, budgets: <Budgets />, recurring: <RecurringPage />, calendar: <CalendarPage />, reports: <Reports />, people: <PeoplePage />, tasks: <TasksPage />, documents: <DocumentsPage />, investments: <InvestmentsPage />, projects: <ProjectsPage />, import: <ImportPage onOpenHistory={() => setPage("history")} />, tax: <TaxPlanner />, goals: <GoalsPage />, insurance: <InsurancePage />, journal: <JournalPage />, business: <BusinessPage />, history: <History /> }[page as string];

  return (
    <div className="flex h-screen overflow-hidden">
      {/* First thing Tab reaches: jumps past the menu straight to the page. */}
      <a href="#content" onClick={(e) => { e.preventDefault(); document.getElementById("content")?.focus(); }}
        className="bg-primary text-primary-foreground sr-only z-50 rounded-md text-sm shadow-lg focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:px-4 focus:py-2.5">
        {t("nav.skip")}
      </a>
      <Sidebar page={page} onNavigate={setPage} collapsed={collapsed} onToggle={() => setCollapsed((c) => !c)} />
      <div className="flex min-w-0 flex-1 flex-col">
        <Header title={t(`nav.${item.id}`)} />
        <TickerBar />
        <main id="content" tabIndex={-1} className="flex-1 overflow-y-auto p-6 outline-none">
          <div key={SELF_RELOADING.has(page) ? page : `${page}:${version}`} className="page-enter">{body ?? <ComingSoon item={item} />}</div>
        </main>
      </div>
      <UndoToaster />
      <UpdateNotice />
      <ReminderWatcher />
      <QuickAdd />
      <CommandPalette onNavigate={setPage} onLock={signOut} />
    </div>
  );
}
