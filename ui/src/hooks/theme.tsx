import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { MotionConfig } from "motion/react";
import { flushSync } from "react-dom";

export type Theme = "light" | "dark" | "system";
export type Accent = "neutral" | "black" | "blue" | "green" | "violet" | "orange" | "rose";
/** What the person picked. "full" still gives way to the system's reduce-motion setting. */
export type MotionPref = "full" | "reduced" | "off";
export type HiddenInMain = "masked" | "off";
/** What actually applies after taking the system setting into account. */
export type MotionLevel = MotionPref;

type Ctx = {
  theme: Theme; setTheme: (t: Theme) => void;
  accent: Accent; setAccent: (a: Accent) => void;
  hideBalances: boolean; setHideBalances: (v: boolean) => void;
  /** How hidden transactions appear outside the Hidden section on this device. */
  hiddenInMain: HiddenInMain; setHiddenInMain: (v: HiddenInMain) => void;
  motionPref: MotionPref; setMotionPref: (m: MotionPref) => void;
  motion: MotionLevel;
};
const ThemeCtx = createContext<Ctx | null>(null);

function read<T extends string>(key: string, fallback: T): T {
  try { return (localStorage.getItem(key) as T) || fallback; } catch { return fallback; }
}
function write(key: string, v: string) { try { localStorage.setItem(key, v); } catch { /* ignore */ } }

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(() => read("vaulty.theme", "system"));
  const [accent, setAccentState] = useState<Accent>(() => read("vaulty.accent", "neutral"));
  const [hideBalances, setHideBalances] = useState(() => read<string>("vaulty.hide", "0") === "1");
  const [hiddenInMain, setHiddenInMain] = useState<HiddenInMain>(() => read<HiddenInMain>("vaulty.hiddenMain", "masked"));
  const [motionPref, setMotionPref] = useState<MotionPref>(() => read("vaulty.motion", "full"));
  const [systemReduce, setSystemReduce] = useState(() => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  const motion: MotionLevel = motionPref === "off" ? "off" : motionPref === "reduced" || systemReduce ? "reduced" : "full";

  /** Theme and accent changes cross-fade the window where the browser supports view transitions. */
  const crossFade = (apply: () => void) => {
    const doc = document as Document & { startViewTransition?: (cb: () => void) => unknown };
    if (motion === "off" || !doc.startViewTransition) { apply(); return; }
    doc.startViewTransition(() => flushSync(apply));
  };
  const setTheme = (t: Theme) => crossFade(() => setThemeState(t));
  const setAccent = (a: Accent) => crossFade(() => setAccentState(a));

  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const on = () => setSystemReduce(mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);

  // CSS reads this to tone down or stop its own animations.
  useEffect(() => {
    document.documentElement.dataset.motion = motion;
    write("vaulty.motion", motionPref);
  }, [motion, motionPref]);

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
  useEffect(() => write("vaulty.hiddenMain", hiddenInMain), [hiddenInMain]);

  return (
    <ThemeCtx.Provider value={{ theme, setTheme, accent, setAccent, hideBalances, setHideBalances, hiddenInMain, setHiddenInMain, motionPref, setMotionPref, motion }}>
      {/* Motion components skip movement (keeping fades) unless motion is "full". */}
      <MotionConfig reducedMotion={motion === "full" ? "never" : "always"}>{children}</MotionConfig>
    </ThemeCtx.Provider>
  );
}

export function useTheme() {
  const c = useContext(ThemeCtx);
  if (!c) throw new Error("useTheme outside ThemeProvider");
  return c;
}
