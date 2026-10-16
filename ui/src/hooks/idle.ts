import { useEffect } from "react";

const KEY = "vaulty.autoLockMinutes";
export const AUTO_LOCK_CHOICES = [1, 5, 15, 30, 0] as const; // 0 = never
export const AUTO_LOCK_CHANGED = "vaulty:autolock-changed";

export function getAutoLockMinutes(): number {
  try { const v = Number(localStorage.getItem(KEY)); return Number.isFinite(v) && localStorage.getItem(KEY) !== null ? v : 5; } catch { return 5; }
}
export function setAutoLockMinutes(m: number) {
  try { localStorage.setItem(KEY, String(m)); } catch { /* private mode */ }
  window.dispatchEvent(new Event(AUTO_LOCK_CHANGED));
}

/**
 * Locks the profile after a period without keyboard, mouse or touch input. Also locks on
 * return if the app was hidden (minimised, other window, laptop asleep) for longer than that.
 */
export function useIdleLock(lock: () => void) {
  useEffect(() => {
    let minutes = getAutoLockMinutes();
    let last = Date.now();
    const touch = () => { last = Date.now(); };
    const check = () => { if (minutes > 0 && Date.now() - last > minutes * 60_000) lock(); };
    const onVisible = () => { if (document.visibilityState === "visible") check(); };
    const onChange = () => { minutes = getAutoLockMinutes(); touch(); };
    const events = ["pointerdown", "pointermove", "keydown", "wheel", "touchstart"] as const;
    events.forEach((e) => window.addEventListener(e, touch, { passive: true }));
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener(AUTO_LOCK_CHANGED, onChange);
    const id = setInterval(check, 10_000);
    return () => {
      events.forEach((e) => window.removeEventListener(e, touch));
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener(AUTO_LOCK_CHANGED, onChange);
      clearInterval(id);
    };
  }, [lock]);
}
