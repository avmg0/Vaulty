import { useEffect, useId, useRef, useState } from "react";
import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";

/**
 * Small trend line. Draws itself left to right on first view; the shaded area fades in under it.
 * When the data changes the line eases into its new shape.
 */
export function Sparkline({ values, color = "var(--primary)", className, label }: { values: number[]; color?: string; className?: string; label: string }) {
  const { move, fade } = useMotion();
  const id = useId();
  // Drawn at its real pixel size so the line keeps an even width and draws in fully.
  const box = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => setSize({ w: Math.round(e.contentRect.width), h: Math.round(e.contentRect.height) }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const W = size.w || 100, H = size.h || 30, pad = 2;
  if (values.length < 2) return <div ref={box} className={className} />;
  const min = Math.min(...values), max = Math.max(...values), span = max - min || 1;
  const pts = values.map((v, i) => [pad + (i / (values.length - 1)) * (W - 2 * pad), H - pad - ((v - min) / span) * (H - 2 * pad)] as const);
  const line = pts.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(2)},${y.toFixed(2)}`).join(" ");
  const area = `${line} L${pts.at(-1)![0].toFixed(2)},${H} L${pts[0][0].toFixed(2)},${H} Z`;
  return (
    <div ref={box} className={className}>
    {size.w > 0 && (
    <svg viewBox={`0 0 ${W} ${H}`} width={W} height={H} className="block" role="img" aria-label={label}>
      <defs>
        <linearGradient id={id} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.25} /><stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      <motion.path d={area} fill={`url(#${id})`} initial={{ opacity: 0 }} animate={{ opacity: 1, d: area }} transition={{ opacity: { ...fade({ duration: 0.5 }), delay: 0.4 }, d: move() }} />
      <motion.path d={line} fill="none" stroke={color} strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round"
        initial={{ pathLength: 0 }} animate={{ pathLength: 1, d: line }} transition={{ pathLength: move({ duration: 0.9, ease: "easeOut" }), d: move() }} />
    </svg>
    )}
    </div>
  );
}
