import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";
import { cn } from "@/lib/utils";

/** A tick that draws itself, for "saved" and "done" moments. */
export function SuccessTick({ className }: { className?: string }) {
  const { move, fade } = useMotion();
  return (
    <svg viewBox="0 0 24 24" className={cn("size-4", className)} fill="none" stroke="currentColor" strokeWidth={2.5} strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <motion.path d="M5 12.5l4.5 4.5L19 7.5" initial={{ pathLength: 0, opacity: 0 }} animate={{ pathLength: 1, opacity: 1 }}
        transition={{ pathLength: move({ duration: 0.35, ease: "easeOut" }), opacity: fade({ duration: 0.1 }) }} />
    </svg>
  );
}
