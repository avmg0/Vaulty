import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";
import { cn } from "@/lib/utils";

/** A thin bar that fills from empty on first view and eases to new values after. 0–1. */
export function ProgressBar({ value, className, barClassName, label }: { value: number; className?: string; barClassName?: string; label?: string }) {
  const { move } = useMotion();
  const v = Math.max(0, Math.min(1, value));
  return (
    <div className={cn("bg-muted h-1.5 overflow-hidden rounded-full", className)} role="progressbar" aria-valuemin={0} aria-valuemax={100}
      aria-valuenow={Math.round(v * 100)} aria-label={label}>
      <motion.div className={cn("bg-primary h-full origin-left rounded-full", barClassName)}
        initial={{ scaleX: 0 }} animate={{ scaleX: v }} transition={move({ duration: 0.8, ease: [0.2, 0.7, 0.2, 1] })} />
    </div>
  );
}
