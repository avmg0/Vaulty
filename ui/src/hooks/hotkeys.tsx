import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, type Hotkey } from "@/api";

export type HotkeyAction =
  | "quick_add" | "palette" | "undo" | "hide_balances" | "lock" | "panic" | "sidebar" | "settings"
  | "go_dashboard" | "go_transactions" | "go_accounts" | "go_reports" | "go_budgets" | "go_recurring" | "go_cards" | "go_import";

/** Used until the profile's own shortcuts have loaded (the same defaults as in Rust). */
const DEFAULTS: Record<HotkeyAction, string> = {
  quick_add: "Mod+J", palette: "Mod+K", undo: "Mod+Z", hide_balances: "Mod+H", lock: "Mod+L", panic: "Mod+Shift+L",
  sidebar: "Mod+B", settings: "Mod+,", go_dashboard: "Mod+1", go_transactions: "Mod+2", go_accounts: "Mod+3",
  go_reports: "Mod+4", go_budgets: "", go_recurring: "", go_cards: "", go_import: "",
};

export const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

const PUNCTUATION: Record<string, string> = {
  Comma: ",", Period: ".", Slash: "/", Semicolon: ";", Quote: "'", BracketLeft: "[", BracketRight: "]",
  Backslash: "\\", Minus: "-", Equal: "=", Backquote: "`",
};

/**
 * The combination a key press makes, in the same form Rust stores (`Mod+Shift+K`), or null for a
 * lone modifier. Uses the physical key, so Shift+1 is still "1" and other keyboard layouts work.
 */
export function comboOf(e: KeyboardEvent | React.KeyboardEvent): string | null {
  const code = e.code;
  let key: string | undefined;
  if (/^Key[A-Z]$/.test(code)) key = code.slice(3);
  else if (/^Digit\d$/.test(code)) key = code.slice(5);
  else if (/^Numpad\d$/.test(code)) key = code.slice(6);
  else if (/^F([1-9]|1[0-2])$/.test(code)) key = code;
  else key = PUNCTUATION[code];
  if (!key) return null;
  return `${e.ctrlKey || e.metaKey ? "Mod+" : ""}${e.shiftKey ? "Shift+" : ""}${e.altKey ? "Alt+" : ""}${key}`;
}

/** How a combination is written on this computer: ⌘⇧K on a Mac, Ctrl+Shift+K elsewhere. */
export function formatKeys(keys: string): string {
  if (!keys) return "";
  const parts = keys.split("+");
  const key = parts.pop()!;
  if (isMac) return parts.map((p) => ({ Mod: "⌘", Shift: "⇧", Alt: "⌥" })[p] ?? p).join("") + key;
  return [...parts.map((p) => (p === "Mod" ? "Ctrl" : p)), key].join("+");
}

/** A handler may return false to let the key through (e.g. Undo while typing in a field). */
type Handler = (e: KeyboardEvent) => boolean | void;

type Ctx = {
  keys: Record<string, string>;
  list: Hotkey[];
  set: (action: string, keys: string | null) => Promise<void>;
  resetAll: () => Promise<void>;
  register: (action: HotkeyAction, h: Handler) => () => void;
  /** While a new shortcut is being recorded, every shortcut is paused. */
  setPaused: (p: boolean) => void;
};

const HotkeysContext = createContext<Ctx | null>(null);

export function HotkeysProvider({ children }: { children: ReactNode }) {
  const [list, setList] = useState<Hotkey[]>([]);
  const handlers = useRef(new Map<string, Handler>());
  const paused = useRef(false);
  const keys = useMemo(() => ({ ...DEFAULTS, ...Object.fromEntries(list.map((h) => [h.action, h.keys])) }), [list]);
  const keysRef = useRef(keys);
  keysRef.current = keys;

  useEffect(() => { api.hotkeys.list().then(setList).catch(() => {}); }, []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (paused.current || e.repeat) return;
      const combo = comboOf(e);
      if (!combo) return;
      const action = Object.entries(keysRef.current).find(([, k]) => k && k === combo)?.[0];
      const h = action && handlers.current.get(action);
      if (h && h(e) !== false) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const register = useCallback((action: HotkeyAction, h: Handler) => {
    handlers.current.set(action, h);
    return () => { if (handlers.current.get(action) === h) handlers.current.delete(action); };
  }, []);
  const value = useMemo<Ctx>(() => ({
    keys, list, register,
    set: async (action, k) => setList(await api.hotkeys.set(action, k)),
    resetAll: async () => setList(await api.hotkeys.reset()),
    setPaused: (p) => { paused.current = p; },
  }), [keys, list, register]);
  return <HotkeysContext.Provider value={value}>{children}</HotkeysContext.Provider>;
}

export function useHotkeys() {
  const c = useContext(HotkeysContext);
  if (!c) throw new Error("useHotkeys outside HotkeysProvider");
  return c;
}

/** Runs `handler` when the action's shortcut is pressed. */
export function useHotkey(action: HotkeyAction, handler: Handler) {
  const { register } = useHotkeys();
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(() => register(action, (e) => latest.current(e)), [action, register]);
}

/** The action's shortcut as written on this computer ("" if it has none). */
export function useHotkeyLabel(action: HotkeyAction) {
  return formatKeys(useHotkeys().keys[action] ?? "");
}

/** Every action's shortcut as written on this computer. */
export function useHotkeyLabels(): Record<HotkeyAction, string> {
  const { keys } = useHotkeys();
  return Object.fromEntries(Object.entries(keys).map(([a, k]) => [a, formatKeys(k)])) as Record<HotkeyAction, string>;
}
