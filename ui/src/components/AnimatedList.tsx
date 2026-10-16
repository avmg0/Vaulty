import type { ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useMotion } from "@/lib/motion";

/** Wraps a list so items open in and collapse out. Nothing animates on first render. */
export function AnimatedList({ children }: { children: ReactNode }) {
  return <AnimatePresence initial={false}>{children}</AnimatePresence>;
}

/** One list row. Give it the item's id as `key`, so an undone delete slides back in place. */
export function AnimatedItem({ children }: { children: ReactNode }) {
  const { move, fade } = useMotion();
  return (
    <motion.div
      layout="position"
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ height: move(), opacity: fade(), layout: move() }}
      style={{ overflow: "hidden" }}>
      {children}
    </motion.div>
  );
}
