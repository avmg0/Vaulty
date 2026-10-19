import { useState, type ReactNode } from "react";
import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";
import { cn } from "@/lib/utils";

export type Slice = { key: string; label: string; value: number; color: string };

/**
 * Donut chart. Slices draw in around the ring one after another on first view and ease to new
 * sizes when the data changes. Hovering a slice or its legend row highlights it.
 */
export function Donut({ slices, format, center, label, size = 168 }: {
  slices: Slice[]; format: (n: number) => string; center?: ReactNode; label: string; size?: number;
}) {
  const { move, fade } = useMotion();
  const [active, setActive] = useState<string | null>(null);
  const total = slices.reduce((s, x) => s + x.value, 0);
  const r = 40, stroke = 12, gap = slices.length > 1 ? 0.006 : 0;
  let start = 0;
  const arcs = slices.map((s) => {
    const frac = total ? s.value / total : 0;
    const a = { ...s, frac, start };
    start += frac;
    return a;
  });
  return (
    <figure className="flex flex-wrap items-center gap-6" aria-label={label}>
      <div className="relative shrink-0" style={{ width: size, height: size }}>
        <svg viewBox="0 0 100 100" className="size-full -rotate-90" role="img" aria-label={`${label}: ${slices.map((s) => `${s.label} ${format(s.value)}`).join(", ")}`}>
          <circle cx="50" cy="50" r={r} fill="none" strokeWidth={stroke} className="stroke-muted" />
          {arcs.map((a, i) => (
            <motion.circle key={a.key} cx="50" cy="50" r={r} fill="none" strokeWidth={active === a.key ? stroke + 3 : stroke} stroke={a.color}
              pathLength={1} strokeLinecap="butt"
              className={cn("transition-[stroke-width,opacity] duration-200", active && active !== a.key && "opacity-40")}
              onMouseEnter={() => setActive(a.key)} onMouseLeave={() => setActive(null)}
              initial={{ strokeDasharray: "0 1", strokeDashoffset: -a.start }}
              animate={{ strokeDasharray: `${Math.max(0, a.frac - gap)} ${1 - Math.max(0, a.frac - gap)}`, strokeDashoffset: -a.start }}
              transition={move({ duration: 0.5, ease: [0.2, 0.7, 0.2, 1], delay: i * 0.12 })} />
          ))}
        </svg>
        {center && <div className="absolute inset-0 grid place-items-center text-center">{center}</div>}
      </div>
      <ul className="min-w-40 flex-1 space-y-1.5 text-sm">
        {arcs.map((a, i) => (
          <motion.li key={a.key} className={cn("flex items-center gap-2 rounded px-1 transition-opacity", active && active !== a.key && "opacity-50")}
            onMouseEnter={() => setActive(a.key)} onMouseLeave={() => setActive(null)}
            initial={{ opacity: 0, x: -6 }} animate={{ opacity: 1, x: 0 }} transition={{ ...fade(), delay: 0.1 + i * 0.06 }}>
            <span className="size-2.5 shrink-0 rounded-full" style={{ backgroundColor: a.color }} />
            <span className="min-w-0 flex-1 truncate">{a.label}</span>
            <span className="text-muted-foreground tabular-nums">{Math.round(a.frac * 100)}%</span>
            <span className="min-w-14 shrink-0 text-end tabular-nums">{format(a.value)}</span>
          </motion.li>
        ))}
      </ul>
    </figure>
  );
}
