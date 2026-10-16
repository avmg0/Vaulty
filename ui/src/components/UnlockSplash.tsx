import { useEffect } from "react";
import { motion } from "motion/react";
import { Logo } from "@/components/Logo";
import { useMotion } from "@/lib/motion";

/**
 * The brief "vault opening" moment after signing in: the logo settles, then the cover lifts
 * away and the dashboard (already loading underneath) shows through. Skipped with motion off.
 */
export function UnlockSplash({ onDone }: { onDone: () => void }) {
  const { level } = useMotion();
  useEffect(() => {
    if (level === "off") { onDone(); return; }
    const t = setTimeout(onDone, level === "full" ? 820 : 320);
    return () => clearTimeout(t);
  }, [level, onDone]);
  if (level === "off") return null;
  const full = level === "full";
  return (
    <motion.div aria-hidden className="bg-background pointer-events-none fixed inset-0 z-[60] grid place-items-center"
      initial={{ opacity: 1 }} animate={{ opacity: 0 }}
      transition={{ duration: full ? 0.35 : 0.25, delay: full ? 0.45 : 0.05, ease: "easeOut" }}>
      <motion.div
        initial={{ scale: 0.85, opacity: 0 }}
        animate={full ? { scale: [0.85, 1, 1.25], opacity: [0, 1, 0], filter: ["blur(0px)", "blur(0px)", "blur(6px)"] } : { opacity: [0, 1, 0] }}
        transition={{ duration: full ? 0.8 : 0.3, times: [0, 0.45, 1], ease: [0.2, 0.7, 0.2, 1] }}>
        <Logo className="w-20" />
      </motion.div>
    </motion.div>
  );
}
