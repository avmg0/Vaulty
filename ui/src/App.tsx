import { useCallback, useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useLocale } from "@/hooks/locale";
import { api, type SignedInUser, type User } from "@/api";
import { RecoveryKeyDialog } from "@/components/security/RecoveryKeyDialog";
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
import { TaxPlanner } from "@/pages/TaxPlanner";
import { TickerBar } from "@/components/TickerBar";
import { UndoToaster } from "@/components/UndoToaster";
import { History } from "@/pages/History";
import { DATA_RESTORED } from "@/api";

const LAST_USER_KEY = "vaulty.lastUser";

export function App() {
  const [user, setUser] = useState<User | null | undefined>(undefined);
  const [backendMissing, setBackendMissing] = useState(false);
  const [lastUserId, setLastUserId] = useState<string | undefined>(() => { try { return localStorage.getItem(LAST_USER_KEY) ?? undefined; } catch { return undefined; } });

  useEffect(() => { api.auth.current().then(setUser).catch(() => setBackendMissing(true)); }, []);

  const { locale, currency, setLocale, setCurrency } = useLocale();
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null);
  const signedIn = (u: SignedInUser, fresh?: boolean) => {
    setUser(u);
    if (u.recovery_key) setRecoveryKey(u.recovery_key);
    // A new profile starts in the language and currency chosen on this device; an existing one
    // brings its own (which also becomes this device's default for the sign-in screens).
    if (fresh) api.settings.update({ locale, currency }).catch(() => {});
    else api.settings.get().then((s) => { if (s.locale) setLocale(s.locale); if (s.currency) setCurrency(s.currency); }).catch(() => {});
    setLastUserId(u.id);
    try { localStorage.setItem(LAST_USER_KEY, u.id); } catch { /* ignore */ }
  };
  const signOut = useCallback(() => { api.auth.signOut().finally(() => setUser(null)); }, []);

  if (backendMissing) return (
    <div className="text-muted-foreground grid min-h-screen place-items-center p-6 text-center text-sm">
      Vaulty's UI is running without its Rust backend.<br />Start it with <code className="text-foreground">cargo tauri dev</code> instead of <code className="text-foreground">npm run dev</code>.
    </div>
  );
  if (user === undefined) return <div className="grid min-h-screen place-items-center"><Loader2 className="text-muted-foreground size-6 animate-spin" /></div>;
  if (user === null) return <AuthGate onSignedIn={signedIn} lastUserId={lastUserId} />;

  return (
    <SessionProvider value={{ user, setUser, signOut }}>
      <Shell key={user.id} signOut={signOut} />
      <RecoveryKeyDialog recoveryKey={recoveryKey} child={user.role === "child"} onDone={() => setRecoveryKey(null)} />
    </SessionProvider>
  );
}

function Shell({ signOut }: { signOut: () => void }) {
  const { t } = useTranslation();
  const [page, setPage] = useState<PageId>("dashboard");
  const [collapsed, setCollapsed] = useState(false);
  const { hideBalances, setHideBalances } = useTheme();
  useIdleLock(signOut);
  // After an undo or restore, re-mount the open page so it shows current data.
  const [version, setVersion] = useState(0);
  useEffect(() => {
    const bump = () => setVersion((v) => v + 1);
    window.addEventListener(DATA_RESTORED, bump);
    return () => window.removeEventListener(DATA_RESTORED, bump);
  }, []);

  // Keyboard shortcuts. Ctrl on Windows/Linux, Cmd on macOS.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const k = e.key.toLowerCase();
      if (k === "b") { e.preventDefault(); setCollapsed((c) => !c); }
      else if (k === "l") { e.preventDefault(); signOut(); }
      else if (k === "h") { e.preventDefault(); setHideBalances(!hideBalances); }
      else if (k === ",") { e.preventDefault(); setPage("settings"); }
      else if (k >= "1" && k <= "4") { e.preventDefault(); setPage((["dashboard", "transactions", "accounts", "reports"] as PageId[])[+k - 1]); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [signOut, hideBalances, setHideBalances]);

  const item = ALL_ITEMS.find((i) => i.id === page)!;
  const body = { dashboard: <Dashboard />, accounts: <Accounts />, transactions: <Transactions />, settings: <Settings />, family: <Family onOpenSettings={() => setPage("settings")} />, cards: <Cards />, tax: <TaxPlanner />, history: <History /> }[page as string];

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar page={page} onNavigate={setPage} collapsed={collapsed} onToggle={() => setCollapsed((c) => !c)} />
      <div className="flex min-w-0 flex-1 flex-col">
        <Header title={t(`nav.${item.id}`)} />
        <TickerBar />
        <main className="flex-1 overflow-y-auto p-6">
          <div key={page === "history" ? page : `${page}:${version}`} className="page-enter">{body ?? <ComingSoon item={item} />}</div>
        </main>
      </div>
      <UndoToaster />
    </div>
  );
}
