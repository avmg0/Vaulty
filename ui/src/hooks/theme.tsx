import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

export type Theme = "light" | "dark" | "system";
export type Accent = "neutral" | "blue" | "green" | "violet" | "orange" | "rose";

type Ctx = {
  theme: Theme; setTheme: (t: Theme) => void;
  accent: Accent; setAccent: (a: Accent) => void;
  hideBalances: boolean; setHideBalances: (v: boolean) => void;
};
const ThemeCtx = createContext<Ctx | null>(null);

function read<T extends string>(key: string, fallback: T): T {
  try { return (localStorage.getItem(key) as T) || fallback; } catch { return fallback; }
}
function write(key: string, v: string) { try { localStorage.setItem(key, v); } catch { /* ignore */ } }

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<Theme>(() => read("vaulty.theme", "system"));
  const [accent, setAccent] = useState<Accent>(() => read("vaulty.accent", "neutral"));
  const [hideBalances, setHideBalances] = useState(() => read<string>("vaulty.hide", "0") === "1");

  useEffect(() => {
    const root = document.documentElement;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => root.classList.toggle("dark", theme === "dark" || (theme === "system" && mq.matches));
    apply();
    write("vaulty.theme", theme);
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);

  useEffect(() => {
    if (accent === "neutral") delete document.documentElement.dataset.accent;
    else document.documentElement.dataset.accent = accent;
    write("vaulty.accent", accent);
  }, [accent]);

  useEffect(() => write("vaulty.hide", hideBalances ? "1" : "0"), [hideBalances]);

  return <ThemeCtx.Provider value={{ theme, setTheme, accent, setAccent, hideBalances, setHideBalances }}>{children}</ThemeCtx.Provider>;
}

export function useTheme() {
  const c = useContext(ThemeCtx);
  if (!c) throw new Error("useTheme outside ThemeProvider");
  return c;
}
