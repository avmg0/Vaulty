import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { ArrowRight, Eye, Lock, Moon, Search, Zap, type LucideIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, type Transaction } from "@/api";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ALL_ITEMS, type PageId } from "@/nav";
import { requestSettingsTab, SETTINGS_TAB_ICON, SETTINGS_TABS } from "@/lib/settingsTabs";
import { QUICK_ADD_EVENT } from "@/components/QuickAdd";
import { useTheme } from "@/hooks/theme";
import { rememberFocus, returnFocus } from "@/lib/focus";
import { formatKeys, useHotkey, useHotkeys } from "@/hooks/hotkeys";
import { cn, formatDate, formatMoney } from "@/lib/utils";

/** A search the Transactions page picks up when it opens (set by the palette). */
let pendingSearch: string | null = null;
export const takePendingSearch = () => { const s = pendingSearch; pendingSearch = null; return s; };

type Item = { id: string; label: string; hint?: string; icon: LucideIcon; group: string; run: () => void; keywords?: string };

/** Loose match: every word of the query appears somewhere in the text, in any order. */
const matches = (q: string, text: string) => q.toLowerCase().split(/\s+/).filter(Boolean).every((w) => text.toLowerCase().includes(w));

/** Ctrl/Cmd+K: go anywhere, run an action, or jump to a transaction by name. */
export function CommandPalette({ onNavigate, onLock }: { onNavigate: (p: PageId) => void; onLock: () => void }) {
  const { t } = useTranslation();
  const { theme, setTheme, hideBalances, setHideBalances } = useTheme();
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);
  const [txs, setTxs] = useState<Transaction[]>([]);
  const list = useRef<HTMLDivElement>(null);

  useHotkey("palette", () => setOpen((o) => { if (!o) rememberFocus(); return !o; }));
  const keys = useHotkeys().keys;
  useEffect(() => {
    if (!open) return;
    setQ(""); setActive(0);
    // Recent transactions to search by name (hidden ones come back masked, so they're left out).
    api.transactions.getAll({ limit: 1000, hidden: "exclude" }).then(setTxs).catch(() => setTxs([]));
  }, [open]);

  const close = () => setOpen(false);
  const items = useMemo<Item[]>(() => {
    const go = (p: PageId) => () => { close(); onNavigate(p); };
    const pages: Item[] = ALL_ITEMS.map((i) => ({ id: `page-${i.id}`, label: t(`nav.${i.id}`), icon: i.icon, group: t("palette.go"), run: go(i.id), keywords: i.id }));
    const actions: Item[] = [
      { id: "quick", label: t("quick.button"), hint: formatKeys(keys.quick_add), icon: Zap, group: t("palette.do"), run: () => { close(); window.dispatchEvent(new Event(QUICK_ADD_EVENT)); }, keywords: "add new transaction expense" },
      { id: "balances", label: hideBalances ? t("palette.showBalances") : t("palette.hideBalances"), hint: formatKeys(keys.hide_balances), icon: Eye, group: t("palette.do"), run: () => { close(); setHideBalances(!hideBalances); } },
      { id: "theme", label: t("palette.theme"), icon: Moon, group: t("palette.do"), run: () => { close(); setTheme(theme === "dark" ? "light" : "dark"); }, keywords: "dark light mode" },
      { id: "backup", label: t("backups.make"), icon: ArrowRight, group: t("palette.do"), run: () => { requestSettingsTab("backups"); go("settings")(); }, keywords: "export backup encrypted" },
      { id: "import", label: t("import.step1"), icon: ArrowRight, group: t("palette.do"), run: go("import"), keywords: "csv statement bank excel" },
      { id: "lock", label: t("palette.lock"), hint: formatKeys(keys.lock), icon: Lock, group: t("palette.do"), run: () => { close(); onLock(); }, keywords: "sign out switch profile" },
    ];
    // Each section of Settings, so "security" or "backups" finds it straight away.
    const sections: Item[] = SETTINGS_TABS.map((id) => ({
      id: `settings-${id}`, label: `${t("nav.settings")}: ${t(`settings.tabs.${id}`)}`, icon: SETTINGS_TAB_ICON[id], group: t("palette.go"),
      run: () => { requestSettingsTab(id); go("settings")(); }, keywords: `settings ${id}`,
    }));
    return [...actions, ...pages, ...sections];
  }, [t, hideBalances, theme, keys]);

  const query = q.trim();
  const shown = useMemo(() => {
    const base = query ? items.filter((i) => matches(query, `${i.label} ${i.keywords ?? ""}`)) : items;
    // Transactions only once something is typed; the newest few that match.
    const found: Item[] = query.length >= 2 ? txs.filter((x) => matches(query, [x.merchant, x.description, x.category_name, x.notes, x.tags].filter(Boolean).join(" ")))
      .slice(0, 6).map((x) => ({
        id: `tx-${x.id}`, label: x.merchant || x.description || t("tx.untitled"), icon: Search, group: t("palette.transactions"),
        hint: `${formatDate(x.date)} · ${formatMoney(x.amount, x.currency ?? "INR", hideBalances)}`,
        run: () => { close(); pendingSearch = x.merchant || x.description || ""; onNavigate("transactions"); },
      })) : [];
    return [...base, ...found];
  }, [items, txs, query]);
  useEffect(() => { setActive(0); }, [q]);
  useEffect(() => { list.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" }); }, [active]);

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "ArrowDown") { e.preventDefault(); setActive((a) => Math.min(shown.length - 1, a + 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setActive((a) => Math.max(0, a - 1)); }
    else if (e.key === "Enter") { e.preventDefault(); shown[active]?.run(); }
  };

  let lastGroup = "";
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="top-[18%] translate-y-0 gap-0 p-0 sm:max-w-lg" showCloseButton={false} onCloseAutoFocus={(e) => { e.preventDefault(); returnFocus(); }}>
        <DialogHeader className="sr-only"><DialogTitle>{t("palette.title")}</DialogTitle><DialogDescription>{t("palette.hint")}</DialogDescription></DialogHeader>
        <div className="flex items-center gap-2 border-b px-3">
          <Search className="text-muted-foreground size-4" />
          <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} onKeyDown={onKey} placeholder={t("palette.placeholder")} aria-label={t("palette.title")}
            role="combobox" aria-expanded aria-controls="palette-list" aria-activedescendant={shown[active] ? `palette-${shown[active].id}` : undefined}
            className="placeholder:text-muted-foreground h-12 flex-1 bg-transparent text-sm outline-none" />
          <kbd className="text-muted-foreground text-[10px]">Esc</kbd>
        </div>
        <div ref={list} id="palette-list" role="listbox" className="max-h-80 overflow-y-auto p-1.5">
          {shown.length === 0 && <p className="text-muted-foreground py-8 text-center text-sm">{t("palette.nothing")}</p>}
          {shown.map((i, n) => {
            const header = i.group !== lastGroup ? (lastGroup = i.group) : null;
            return (
              <div key={i.id}>
                {header && <div className="text-muted-foreground px-2 pt-2 pb-1 text-[11px] font-medium">{header}</div>}
                <button type="button" id={`palette-${i.id}`} role="option" aria-selected={n === active} data-index={n}
                  onMouseMove={() => setActive(n)} onClick={i.run}
                  className={cn("flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-sm", n === active && "bg-accent")}>
                  <i.icon className="text-muted-foreground size-4 shrink-0" />
                  <span className="flex-1 truncate">{i.label}</span>
                  {i.hint && <span className="text-muted-foreground shrink-0 text-xs tabular-nums">{i.hint}</span>}
                </button>
              </div>
            );
          })}
        </div>
      </DialogContent>
    </Dialog>
  );
}
