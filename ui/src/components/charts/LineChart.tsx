import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { cn } from "@/lib/utils";

export type LinePoint = { label: string; value: number; tick?: string };

/**
 * One series over time. A 2px line on a recessive grid, a dashed zero line when the values cross
 * it, and one marked point (`mark`) with a direct label. Hover, or focus and use the arrow keys,
 * to read any point.
 */
export function LineChart({ points, format, color, label, mark, markLabel, height = 200 }: {
  points: LinePoint[]; format: (n: number) => string; color: string; label: string;
  mark?: number; markLabel?: string; height?: number;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(600);
  const [active, setActive] = useState<number | null>(null);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => setWidth(Math.max(240, e.contentRect.width)));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  if (points.length < 2) return null;

  const left = 56, right = 12, top = 12, bottom = 24;
  const values = points.map((p) => p.value);
  let lo = Math.min(...values), hi = Math.max(...values);
  if (lo > 0 && lo < hi * 0.6) lo = 0; // a baseline at zero when it doesn't flatten the line
  const pad = (hi - lo || Math.abs(hi) || 1) * 0.08;
  lo -= lo === 0 ? 0 : pad; hi += pad;
  const x = (i: number) => left + (i / (points.length - 1)) * (width - left - right);
  const y = (v: number) => top + (1 - (v - lo) / (hi - lo)) * (height - top - bottom);
  const path = points.map((p, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(p.value).toFixed(1)}`).join("");
  const ticks = [hi - pad, (hi + lo) / 2, lo + (lo === 0 ? 0 : pad)];
  const crossesZero = lo < 0 && hi > 0;

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "ArrowRight") { e.preventDefault(); setActive((a) => Math.min(points.length - 1, (a ?? -1) + 1)); }
    else if (e.key === "ArrowLeft") { e.preventDefault(); setActive((a) => Math.max(0, (a ?? points.length) - 1)); }
    else if (e.key === "Home") setActive(0);
    else if (e.key === "End") setActive(points.length - 1);
  };
  const pick = (clientX: number) => {
    const r = box.current!.getBoundingClientRect();
    const i = Math.round(((clientX - r.left - left) / (width - left - right)) * (points.length - 1));
    setActive(Math.max(0, Math.min(points.length - 1, i)));
  };
  const a = active != null ? points[active] : null;

  return (
    <figure aria-label={label} className="relative">
      <div ref={box} tabIndex={0} role="img" aria-label={`${label}. ${a ? `${a.label}: ${format(a.value)}` : ""}`} onKeyDown={onKey}
        onBlur={() => setActive(null)} onMouseMove={(e) => pick(e.clientX)} onMouseLeave={() => setActive(null)}
        className="rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring">
        <svg width={width} height={height} className="block overflow-visible" aria-hidden>
          {ticks.map((t, i) => (
            <g key={i}>
              <line x1={left} x2={width - right} y1={y(t)} y2={y(t)} className="stroke-border" strokeDasharray="3 3" />
              <text x={left - 6} y={y(t)} dy="0.32em" textAnchor="end" className="fill-muted-foreground text-[10px] tabular-nums">{format(t)}</text>
            </g>
          ))}
          {crossesZero && <line x1={left} x2={width - right} y1={y(0)} y2={y(0)} className="stroke-foreground/50" strokeDasharray="5 3" />}
          {points.map((p, i) => p.tick && (
            <text key={i} x={x(i)} y={height - 6} textAnchor="middle" className="fill-muted-foreground text-[10px]">{p.tick}</text>
          ))}
          <path d={path} fill="none" stroke={color} strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
          {mark != null && points[mark] && (
            <g>
              <circle cx={x(mark)} cy={y(points[mark].value)} r={5} fill={color} className="stroke-card" strokeWidth={2} />
              {markLabel && (
                <text x={x(mark)} y={y(points[mark].value) + (y(points[mark].value) > height / 2 ? -12 : 18)}
                  textAnchor={mark > points.length * 0.75 ? "end" : mark < points.length * 0.25 ? "start" : "middle"}
                  className="fill-foreground text-[11px] font-medium">{markLabel}</text>
              )}
            </g>
          )}
          {a && active != null && (
            <g>
              <line x1={x(active)} x2={x(active)} y1={top} y2={height - bottom} className="stroke-foreground/40" />
              <circle cx={x(active)} cy={y(a.value)} r={4} fill={color} className="stroke-card" strokeWidth={2} />
            </g>
          )}
        </svg>
      </div>
      {a && active != null && (
        <div className={cn("bg-popover text-popover-foreground pointer-events-none absolute top-0 z-10 rounded-md border px-2.5 py-1.5 text-xs shadow-md",
          x(active) > width / 2 ? "-translate-x-full" : "")} style={{ left: x(active) + (x(active) > width / 2 ? -8 : 8) }}>
          <div className="font-medium">{a.label}</div>
          <div className="tabular-nums">{format(a.value)}</div>
        </div>
      )}
    </figure>
  );
}
