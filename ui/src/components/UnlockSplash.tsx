import { useEffect } from "react";
import { motion } from "motion/react";
import { Logo } from "@/components/Logo";
import { useMotion } from "@/lib/motion";

/** How long each moment lasts, in seconds, with full motion. */
const FULL = { open: 1.25, close: 1.05 };
const DIAL = 0.9;

/**
 * The vault moment.
 * - "open" (after signing in): the cover shows the logo, its dial turns counter-clockwise like a
 *   vault being unlocked, then the cover lifts away and the dashboard (already loading
 *   underneath) shows through.
 * - "close" (locking or switching profile): the cover fades in over the app and the dial turns
 *   clockwise, closing the vault, before the sign-in screen appears.
 * Reduced motion keeps a short fade with a still dial; motion off skips it entirely.
 */
export function VaultSplash({ mode, onDone }: { mode: "open" | "close"; onDone: () => void }) {
  const { level } = useMotion();
  const full = level === "full";
  const total = full ? FULL[mode] : 0.3;
  useEffect(() => {
    if (level === "off") { onDone(); return; }
    const t = setTimeout(onDone, total * 1000);
    return () => clearTimeout(t);
  }, [level, total, onDone]);
  if (level === "off") return null;

  const open = mode === "open";
  return (
    <motion.div aria-hidden className="bg-background pointer-events-none fixed inset-0 z-[60] grid place-items-center"
      initial={{ opacity: open ? 1 : 0 }}
      animate={{ opacity: open ? 0 : 1 }}
      transition={open
        ? { duration: full ? 0.3 : 0.25, delay: full ? total - 0.3 : 0.05, ease: "easeOut" }
        : { duration: full ? 0.25 : 0.2, ease: "easeOut" }}>
      <motion.div
        initial={{ scale: open ? 0.92 : 1.08, opacity: 0 }}
        animate={open
          ? (full ? { scale: [0.92, 1, 1, 1.18], opacity: [0, 1, 1, 0] } : { opacity: [0, 1, 0] })
          : { scale: 1, opacity: 1 }}
        transition={open
          ? { duration: total, times: full ? [0, 0.12, 0.78, 1] : undefined, ease: [0.2, 0.7, 0.2, 1] }
          : { duration: 0.25, ease: "easeOut" }}>
        <Logo className="w-24" dial={mode} duration={DIAL} />
      </motion.div>
    </motion.div>
  );
}

/** Kept for existing imports: the opening moment. */
export function UnlockSplash({ onDone }: { onDone: () => void }) {
  return <VaultSplash mode="open" onDone={onDone} />;
}
