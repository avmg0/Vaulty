import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "@/api";

/** Older versions kept the choice in this window's storage; it's moved into the profile once. */
const OLD_KEY = "vaulty.autoLockMinutes";
export const AUTO_LOCK_CHOICES = [1, 5, 15, 30, 0] as const; // 0 = never
export const AUTO_LOCK_CHANGED = "vaulty:autolock-changed";
/** How often activity is reported to the app at most. */
const TOUCH_EVERY_MS = 20_000;

let minutes = 5;

export function getAutoLockMinutes(): number { return minutes; }

/** Saves the choice in the profile; the app enforces it too, a minute after the window would. */
export async function setAutoLockMinutes(m: number) {
  minutes = m;
  window.dispatchEvent(new Event(AUTO_LOCK_CHANGED));
  await api.settings.update({ auto_lock_minutes: m });
}

async function loadAutoLock() {
  const s = await api.settings.get();
  minutes = s.auto_lock_minutes ?? 5;
  let old: string | null = null;
  try { old = localStorage.getItem(OLD_KEY); localStorage.removeItem(OLD_KEY); } catch { /* private mode */ }
  const m = Number(old);
  if (old !== null && (AUTO_LOCK_CHOICES as readonly number[]).includes(m) && m !== minutes) await setAutoLockMinutes(m);
  window.dispatchEvent(new Event(AUTO_LOCK_CHANGED));
}

/**
 * Locks the profile after a period without keyboard, mouse or touch input, and on return if the
 * app was hidden (minimised, other window, laptop asleep) for longer than that. Activity is also
 * reported to the app, which locks by itself if this window stops working.
 */
export function useIdleLock(lock: () => void) {
  useEffect(() => {
    let last = Date.now();
    let reported = 0;
    const report = () => {
      if (Date.now() - reported < TOUCH_EVERY_MS) return;
      reported = Date.now();
      api.auth.touch().catch(() => lock());
    };
    const touch = () => { last = Date.now(); report(); };
    const check = () => { if (minutes > 0 && Date.now() - last > minutes * 60_000) lock(); };
    const onVisible = () => { if (document.visibilityState === "visible") check(); };
    const onChange = () => touch();
    const events = ["pointerdown", "pointermove", "keydown", "wheel", "touchstart"] as const;
    events.forEach((e) => window.addEventListener(e, touch, { passive: true }));
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener(AUTO_LOCK_CHANGED, onChange);
    const id = setInterval(check, 10_000);
    loadAutoLock().catch(() => {});
    // The app locked an idle session on its own.
    const unlisten = listen("vaulty://locked", () => lock()).catch(() => () => {});
    return () => {
      events.forEach((e) => window.removeEventListener(e, touch));
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener(AUTO_LOCK_CHANGED, onChange);
      clearInterval(id);
      void unlisten.then((f) => f());
    };
  }, [lock]);
}
