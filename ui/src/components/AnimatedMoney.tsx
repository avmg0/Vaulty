import { useEffect, useRef, useState } from "react";
import { animate } from "motion/react";
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
};

/**
 * An amount that rolls to its new value when it changes. It never counts up on first view
 * (seeing your balance race from zero on every visit gets tiring) and never moves with
 * reduced motion. Hiding balances blurs the old figure away instead of snapping.
 */
export function AnimatedMoney({ value, currency, hidden, className, empty = "—", formatOptions }: Props) {
  const { full } = useMotion();
  const [shown, setShown] = useState(value ?? 0);
  const last = useRef(value ?? 0);

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
    <span className={cn("inline-block transition-[filter,opacity] duration-200", hidden && "opacity-80", className)}
      aria-label={hidden ? undefined : formatMoney(value, currency, false, formatOptions)}>
      {formatMoney(shown, currency, hidden, formatOptions)}
    </span>
  );
}
