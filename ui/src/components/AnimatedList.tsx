import type { ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useMotion } from "@/lib/motion";

/** Wraps a list so items open in and collapse out. Nothing animates on first render. */
export function AnimatedList({ children }: { children: ReactNode }) {
  return <AnimatePresence initial={false}>{children}</AnimatePresence>;
}

/** One list row. Give it the item's id as `key`, so an undone delete slides back in place. */
export function AnimatedItem({ children, exitX = 0, as = "div", className }: {
  children: ReactNode; /** Slide sideways while leaving, e.g. restored items. */ exitX?: number;
  /** "li" inside lists. */ as?: "div" | "li"; className?: string;
}) {
  const { move, fade } = useMotion();
  const Comp = as === "li" ? motion.li : motion.div;
  return (
    <Comp className={className}
      layout="position"
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0, x: exitX }}
      transition={{ height: move(exitX ? { duration: 0.25, delay: 0.15 } : undefined), x: move(), opacity: fade(), layout: move() }}
      style={{ overflow: "hidden" }}>
      {children}
    </Comp>
  );
}
