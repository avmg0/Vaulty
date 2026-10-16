import { useEffect, useRef, useState } from "react";
import { AnimatePresence, animate, motion } from "motion/react";
import { ArrowDown, ArrowUp } from "lucide-react";
import { useMotion } from "@/lib/motion";
import { cn, formatMoney } from "@/lib/utils";

type Props = {
  value: number | null | undefined;
  currency: string;
  hidden?: boolean;
  className?: string;
  /** Shown when there's no value. */
  empty?: string;
  formatOptions?: Intl.NumberFormatOptions;
  /** Briefly shows an up/down arrow after the value changes. */
  showChange?: boolean;
};

/**
 * An amount that rolls to its new value when it changes. It never counts up on first view
 * (seeing your balance race from zero on every visit gets tiring) and never moves with
 * reduced motion. Hiding balances blurs the old figure away instead of snapping.
 */
export function AnimatedMoney({ value, currency, hidden, className, empty = "—", formatOptions, showChange }: Props) {
  const { full, level } = useMotion();
  const [shown, setShown] = useState(value ?? 0);
  const last = useRef(value ?? 0);
  const [change, setChange] = useState<{ dir: "up" | "down"; n: number } | null>(null);
  useEffect(() => {
    if (!showChange || value == null || level === "off") return;
    const from = last.current;
    if (from === value) return;
    setChange((c) => ({ dir: value > from ? "up" : "down", n: (c?.n ?? 0) + 1 }));
    const t = setTimeout(() => setChange(null), 1600);
    return () => clearTimeout(t);
    // Runs before the rolling effect below updates `last`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);

  useEffect(() => {
    if (value == null) return;
    const from = last.current;
    last.current = value;
    if (!full || from === value) { setShown(value); return; }
    const controls = animate(from, value, {
      duration: Math.min(0.9, 0.35 + Math.log10(1 + Math.abs(value - from)) * 0.08),
      ease: [0.2, 0.7, 0.2, 1],
      onUpdate: setShown,
    });
    return () => controls.stop();
  }, [value, full]);

  if (value == null) return <span className={className}>{empty}</span>;
  return (
    <span className={cn("inline-flex items-center gap-1 transition-[filter,opacity] duration-200", hidden && "opacity-80", className)}
      aria-label={hidden ? undefined : formatMoney(value, currency, false, formatOptions)}>
      {formatMoney(shown, currency, hidden, formatOptions)}
      <AnimatePresence>
        {change && !hidden && (
          <motion.span key={change.n} aria-hidden className={change.dir === "up" ? "text-success" : "text-destructive"}
            initial={{ opacity: 0, y: change.dir === "up" ? 6 : -6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}
            transition={{ duration: 0.25 }}>
            {change.dir === "up" ? <ArrowUp className="size-3.5" /> : <ArrowDown className="size-3.5" />}
          </motion.span>
        )}
      </AnimatePresence>
    </span>
  );
}
