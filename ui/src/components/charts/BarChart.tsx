import { useState } from "react";
import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";
import { cn } from "@/lib/utils";

export type BarSeries = { key: string; label: string; color: string };
export type BarGroup = { label: string; values: Record<string, number> };

/**
 * Grouped bar chart. Bars grow up from the baseline one group after another on first view,
 * and ease to new heights when the data changes. Hover or focus a group to read its values.
 */
export function BarChart({ series, groups, format, height = 180, label }: {
  series: BarSeries[]; groups: BarGroup[]; format: (n: number) => string; height?: number; label: string;
}) {
  const { move } = useMotion();
  const [active, setActive] = useState<number | null>(null);
  const max = Math.max(1, ...groups.flatMap((g) => series.map((s) => g.values[s.key] ?? 0)));
  const ticks = [1, 0.5, 0];
  return (
    <figure className="space-y-3" aria-label={label}>
      <div className="relative" style={{ height }}>
        {/* Gridlines with the top value labelled. */}
        {ticks.map((t) => (
          <div key={t} className="border-border/70 absolute inset-x-0 border-t border-dashed" style={{ top: `${(1 - t) * 100}%` }}>
            {t > 0 && <span className="text-muted-foreground bg-card absolute -top-2 left-0 pe-1 text-[10px] tabular-nums">{format(max * t)}</span>}
          </div>
        ))}
        <div className="absolute inset-0 flex items-end gap-2 ps-12">
          {groups.map((g, gi) => (
            <div key={g.label} tabIndex={0} role="group" aria-label={`${g.label}: ${series.map((s) => `${s.label} ${format(g.values[s.key] ?? 0)}`).join(", ")}`}
              className="relative flex h-full flex-1 items-end justify-center gap-1 rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onMouseEnter={() => setActive(gi)} onMouseLeave={() => setActive(null)} onFocus={() => setActive(gi)} onBlur={() => setActive(null)}>
              <div className={cn("bg-muted/60 absolute inset-0 rounded-md transition-opacity", active === gi ? "opacity-100" : "opacity-0")} />
              {series.map((s, si) => (
                <motion.div key={s.key} className="relative w-full max-w-5 rounded-t-[4px]" style={{ backgroundColor: s.color }}
                  initial={{ height: "0%" }} animate={{ height: `${((g.values[s.key] ?? 0) / max) * 100}%` }}
                  transition={move({ duration: 0.6, ease: [0.2, 0.7, 0.2, 1], delay: gi * 0.06 + si * 0.03 })} />
              ))}
              {active === gi && (
                <div className="bg-popover text-popover-foreground pointer-events-none absolute bottom-full z-10 mb-2 min-w-32 rounded-md border px-2.5 py-1.5 text-xs shadow-md">
                  <div className="mb-1 font-medium">{g.label}</div>
                  {series.map((s) => (
                    <div key={s.key} className="flex items-center justify-between gap-3">
                      <span className="flex items-center gap-1.5"><span className="size-2 rounded-full" style={{ backgroundColor: s.color }} />{s.label}</span>
                      <span className="tabular-nums">{format(g.values[s.key] ?? 0)}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
      <div className="flex gap-2 ps-12">
        {groups.map((g) => <div key={g.label} className="text-muted-foreground flex-1 text-center text-xs">{g.label}</div>)}
      </div>
      <figcaption className="flex flex-wrap gap-4 ps-12 text-xs">
        {series.map((s) => <span key={s.key} className="flex items-center gap-1.5"><span className="size-2 rounded-full" style={{ backgroundColor: s.color }} />{s.label}</span>)}
      </figcaption>
    </figure>
  );
}
