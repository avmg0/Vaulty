import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { Nfc } from "lucide-react";
import type { CardDesign, CardNetwork } from "@/api";
import { InstitutionBadge } from "@/components/InstitutionBadge";
import { NETWORK_NAME, isAmex, ordinal } from "@/lib/cards";
import { cn, formatMoney } from "@/lib/utils";

/** Card finishes. `ink` is the text colour; `line` tints the engraved pattern. */
export const DESIGNS: Record<CardDesign, { label: string; face: string; ink: string; line: string }> = {
  midnight: { label: "Midnight", face: "linear-gradient(135deg,#0b1736 0%,#1b2f6b 55%,#2a4aa8 100%)", ink: "#f4f6fb", line: "#9db4ff" },
  ocean: { label: "Ocean", face: "linear-gradient(140deg,#053b4a 0%,#0e6a8a 50%,#2bb3c9 100%)", ink: "#f2fbfd", line: "#bff3fb" },
  aurora: { label: "Aurora", face: "radial-gradient(120% 140% at 0% 100%,#0f766e 0%,transparent 55%),radial-gradient(120% 140% at 100% 0%,#7c3aed 0%,transparent 60%),#16123a", ink: "#f5f3ff", line: "#c4b5fd" },
  sunset: { label: "Sunset", face: "linear-gradient(135deg,#7c2d12 0%,#c2410c 45%,#db2777 100%)", ink: "#fff7f1", line: "#fed7aa" },
  graphite: { label: "Graphite", face: "repeating-linear-gradient(90deg,rgba(255,255,255,.035) 0 1px,transparent 1px 3px),linear-gradient(135deg,#2a2a2e 0%,#46464c 50%,#1f1f22 100%)", ink: "#ececee", line: "#a1a1aa" },
  emerald: { label: "Emerald", face: "linear-gradient(135deg,#022c22 0%,#065f46 55%,#10b981 100%)", ink: "#ecfdf5", line: "#a7f3d0" },
  rose: { label: "Rose", face: "linear-gradient(135deg,#4c0519 0%,#9f1239 50%,#fb7185 100%)", ink: "#fff1f2", line: "#fecdd3" },
  gold: { label: "Gold", face: "linear-gradient(120deg,#7a5410 0%,#c9a24b 30%,#f3dd9a 50%,#c9a24b 70%,#8a6115 100%)", ink: "#2b1d05", line: "#5c4210" },
};

export type CardVisualData = {
  name: string;
  institution?: string | null;
  institutionShort?: string | null;
  institutionLogo?: string | null;
  logo?: string | null;
  network: CardNetwork | null;
  last4: string | null;
  design: CardDesign;
  currency: string;
  owed?: number;
  creditLimit?: number | null;
  available?: number | null;
  statementDay?: number | null;
  dueDay?: number | null;
};

function usePrefersReducedMotion() {
  const [reduced, setReduced] = useState(() => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const on = () => setReduced(mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return reduced;
}

/** Engraved security-print lines (like banknote guilloche), generated once per design. */
function Guilloche({ colour, seed }: { colour: string; seed: number }) {
  const paths = useMemo(() => {
    const out: string[] = [];
    for (let k = 0; k < 16; k++) {
      const amp = 9 + (k % 5) * 3, freq = 1.6 + ((seed + k) % 4) * 0.35, phase = k * 0.42 + seed, y0 = 34 + k * 7.5;
      let d = "";
      for (let x = 0; x <= 320; x += 4) {
        const y = y0 + amp * Math.sin((x / 320) * Math.PI * 2 * freq + phase) * Math.cos((x / 320) * Math.PI + k * 0.2);
        d += `${x === 0 ? "M" : "L"}${x},${y.toFixed(1)}`;
      }
      out.push(d);
    }
    return out;
  }, [seed]);
  return (
    <svg viewBox="0 0 320 202" preserveAspectRatio="none" aria-hidden className="pointer-events-none absolute inset-0 size-full mix-blend-overlay">
      {paths.map((d, i) => <path key={i} d={d} fill="none" stroke={colour} strokeOpacity={0.22} strokeWidth={0.6} />)}
    </svg>
  );
}

/** EMV contact chip, drawn generically. */
function Chip() {
  return (
    <div aria-hidden className="relative h-[17%] w-[13%] overflow-hidden rounded-[18%] shadow-[inset_0_0_0_1px_rgba(0,0,0,.25)]"
      style={{ background: "linear-gradient(135deg,#d8c27a,#f6e7a8 40%,#b99a4a 70%,#e8d38c)" }}>
      <div className="absolute inset-y-0 left-1/3 w-px bg-black/25" />
      <div className="absolute inset-y-0 right-1/3 w-px bg-black/25" />
      <div className="absolute inset-x-0 top-1/2 h-px bg-black/25" />
      <div className="absolute top-1/4 bottom-1/4 left-1/3 right-1/3 rounded-[2px] border border-black/25" />
    </div>
  );
}

function maskedNumber(network: CardNetwork | null, last4: string | null) {
  const tail = last4 ?? "••••";
  return isAmex(network) ? `•••• •••••• •${tail}` : `•••• •••• •••• ${tail}`;
}

type Props = {
  card: CardVisualData;
  /** Tilt with the pointer and flip on click / Enter. */
  interactive?: boolean;
  hideAmounts?: boolean;
  className?: string;
  style?: CSSProperties;
};

/**
 * A credit card rendered as a physical object: tilts toward the pointer with a moving glare,
 * flips to show billing details. Respects prefers-reduced-motion. Never shows more than the last 4 digits.
 */
export function CardVisual({ card, interactive = false, hideAmounts = false, className, style }: Props) {
  const reduced = usePrefersReducedMotion();
  const [flipped, setFlipped] = useState(false);
  const shell = useRef<HTMLDivElement>(null);
  const d = DESIGNS[card.design] ?? DESIGNS.midnight;
  const seed = useMemo(() => [...card.design].reduce((s, c) => s + c.charCodeAt(0), 0) % 7, [card.design]);
  const tiltable = interactive && !reduced;

  const setVars = (rx: number, ry: number, gx: number, gy: number, go: number, fast: boolean) => {
    const el = shell.current;
    if (!el) return;
    el.style.setProperty("--rx", `${rx}deg`);
    el.style.setProperty("--ry", `${ry}deg`);
    el.style.setProperty("--gx", `${gx}%`);
    el.style.setProperty("--gy", `${gy}%`);
    el.style.setProperty("--go", `${go}`);
    el.style.setProperty("--tilt-speed", fast ? "120ms" : "600ms");
  };
  const onMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!tiltable) return;
    const r = e.currentTarget.getBoundingClientRect();
    const x = (e.clientX - r.left) / r.width, y = (e.clientY - r.top) / r.height;
    setVars((0.5 - y) * 14, (x - 0.5) * 18, x * 100, y * 100, 1, true);
  };
  const onLeave = () => setVars(0, 0, 50, 50, 0, false);
  const flip = () => interactive && setFlipped((f) => !f);
  const onKey = (e: KeyboardEvent) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); flip(); } };

  const money = (n: number | null | undefined) => (n == null ? "—" : formatMoney(n, card.currency, hideAmounts));
  const face = "absolute inset-0 overflow-hidden rounded-[4.5cqw] [backface-visibility:hidden] shadow-[0_18px_40px_-18px_rgba(0,0,0,.55),0_2px_6px_rgba(0,0,0,.18)]";
  const glare = (
    <div aria-hidden className="pointer-events-none absolute inset-0 transition-opacity duration-300"
      style={{ opacity: "var(--go,0)", background: "radial-gradient(circle at var(--gx,50%) var(--gy,50%), rgba(255,255,255,.38), rgba(255,255,255,0) 45%)" }} />
  );

  return (
    // Pointer is tracked on the outer, non-rotating box so the tilt doesn't feed back into itself.
    <div className={cn("@container w-full [perspective:1100px]", className)} style={style} onPointerMove={onMove} onPointerLeave={onLeave}>
      <div
        ref={shell}
        role={interactive ? "button" : undefined}
        tabIndex={interactive ? 0 : undefined}
        aria-pressed={interactive ? flipped : undefined}
        aria-label={interactive ? `${card.name} card ending ${card.last4 ?? "unknown"}. ${flipped ? "Showing billing details" : "Press to see billing details"}` : undefined}
        onClick={flip}
        onKeyDown={interactive ? onKey : undefined}
        className={cn(
          "relative aspect-[1.586] w-full [transform-style:preserve-3d] outline-none focus-visible:ring-4 focus-visible:ring-ring/50 rounded-[4.5cqw]",
          interactive && "cursor-pointer",
        )}
        style={{
          transform: `rotateX(var(--rx,0deg)) rotateY(calc(var(--ry,0deg) + ${flipped ? 180 : 0}deg))`,
          transition: reduced ? "none" : "transform var(--tilt-speed,600ms) cubic-bezier(.2,.7,.2,1)",
          color: d.ink,
        }}
      >
        {/* Front */}
        <div className={face} style={{ background: d.face }}>
          <Guilloche colour={d.line} seed={seed} />
          <div className="relative flex h-full flex-col justify-between p-[6.5cqw]">
            <div className="flex items-start justify-between gap-[3cqw]">
              <div className="flex min-w-0 items-center gap-[2.5cqw]">
                {(card.logo || card.institutionLogo || card.institutionShort) && (
                  <InstitutionBadge logo={card.logo} institutionLogo={card.institutionLogo} short={card.institutionShort}
                    name={card.institution ?? card.name} className="size-[9cqw] rounded-[2cqw] ring-1 ring-white/20" />
                )}
                {card.institution && <span className="truncate text-[4.4cqw] font-semibold tracking-tight">{card.institution}</span>}
              </div>
              <Nfc className="size-[7cqw] shrink-0 opacity-80" aria-hidden />
            </div>
            <Chip />
            <div className="space-y-[2.5cqw]">
              <div className="text-[6.2cqw] font-medium tracking-[0.12em] tabular-nums [text-shadow:0_1px_0_rgba(0,0,0,.35),0_-1px_0_rgba(255,255,255,.12)]">
                {maskedNumber(card.network, card.last4)}
              </div>
              <div className="flex items-end justify-between gap-[3cqw]">
                <span className="truncate text-[3.8cqw] opacity-85">{card.name}</span>
                <span className="shrink-0 text-[4.6cqw] font-semibold italic tracking-tight">{card.network ? NETWORK_NAME[card.network] : ""}</span>
              </div>
            </div>
          </div>
          {glare}
        </div>

        {/* Back */}
        <div className={face} style={{ background: d.face, transform: "rotateY(180deg)" }}>
          <Guilloche colour={d.line} seed={seed + 3} />
          <div className="absolute inset-x-0 top-[11%] h-[17%] bg-black/80" aria-hidden />
          <div className="relative flex h-full flex-col justify-end gap-[3cqw] p-[6.5cqw] pt-[33%]">
            <div className="grid grid-cols-3 gap-[3cqw] text-[3.4cqw]">
              <div><div className="opacity-70">Owed</div><div className="text-[4.6cqw] font-semibold tabular-nums">{money(card.owed)}</div></div>
              <div><div className="opacity-70">Available</div><div className="text-[4.6cqw] font-semibold tabular-nums">{money(card.available)}</div></div>
              <div><div className="opacity-70">Limit</div><div className="text-[4.6cqw] font-semibold tabular-nums">{money(card.creditLimit)}</div></div>
            </div>
            <div className="flex justify-between gap-[3cqw] text-[3.4cqw] opacity-85">
              <span>{card.statementDay ? `Statement on the ${ordinal(card.statementDay)}` : "Statement day not set"}</span>
              <span>{card.dueDay ? `Due on the ${ordinal(card.dueDay)}` : ""}</span>
            </div>
            <div className="text-[2.8cqw] opacity-60">Vaulty keeps only the last 4 digits of this card.</div>
          </div>
          {glare}
        </div>
      </div>
    </div>
  );
}
