import { useTheme } from "@/hooks/theme";
import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { Nfc } from "lucide-react";
import type { CardDesign, CardKind, CardLook, CardNetwork } from "@/api";
import { InstitutionBadge } from "@/components/InstitutionBadge";
import { KIND_LABEL, NETWORK_NAME, expiryText, maskedNumber, ordinal } from "@/lib/cards";
import { cn, formatMoney } from "@/lib/utils";

/** Preset finishes. `ink` is the text colour; `line` tints the engraved pattern. */
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

const INK = { light: "#f8fafc", dark: "#111827" };

/** Resolves any look (preset or custom) to CSS for the card face. */
export function resolveLook(look: CardLook | undefined): { background: string; ink: string; line: string; pattern: boolean; dim: number; darkInk: boolean } {
  if (!look || "preset" in look) {
    const key = look && "preset" in look ? look.preset : "midnight";
    const d = DESIGNS[key] ?? DESIGNS.midnight;
    return { background: d.face, ink: d.ink, line: d.line, pattern: true, dim: 0, darkInk: key === "gold" };
  }
  const darkInk = look.ink === "dark";
  const ink = darkInk ? INK.dark : INK.light;
  const pattern = look.pattern ?? look.fill !== "image";
  if (look.fill === "solid") return { background: look.colors[0], ink, line: ink, pattern, dim: 0, darkInk };
  if (look.fill === "gradient") return { background: `linear-gradient(${look.angle ?? 135}deg, ${look.colors.join(", ")})`, ink, line: ink, pattern, dim: 0, darkInk };
  return { background: `center / cover no-repeat url("${look.image}")`, ink, line: ink, pattern, dim: look.dim ?? 0.15, darkInk };
}

export type CardVisualData = {
  kind: CardKind;
  name: string;
  institution?: string | null;
  institutionShort?: string | null;
  institutionLogo?: string | null;
  issuerName?: string | null;
  logo?: string | null;
  network: CardNetwork | null;
  last4: string | null;
  expiryMonth?: number | null;
  expiryYear?: number | null;
  look: CardLook;
  currency: string;
  // credit
  owed?: number;
  creditLimit?: number | null;
  available?: number | null;
  statementDay?: number | null;
  dueDay?: number | null;
  // debit / prepaid / gift
  balance?: number;
  accountName?: string | null;
  notes?: string | null;
};

/** Tilt and flip only with full motion (the in-app setting, which also follows the system). */
function usePrefersReducedMotion() {
  return useTheme().motion !== "full";
}

/** Engraved security-print lines (like banknote guilloche). */
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

const HAS_CHIP: Record<CardKind, boolean> = { credit: true, debit: true, prepaid: true, gift: false, other: false };

type Props = {
  card: CardVisualData;
  /** Tilt with the pointer and flip on click / Enter. */
  interactive?: boolean;
  hideAmounts?: boolean;
  className?: string;
  style?: CSSProperties;
};

/**
 * Any card rendered as a physical object: tilts toward the pointer with a moving glare and flips
 * to show its details. Respects prefers-reduced-motion. Never shows more than the last 4 digits.
 */
export function CardVisual({ card, interactive = false, hideAmounts = false, className, style }: Props) {
  const reduced = usePrefersReducedMotion();
  const [flipped, setFlipped] = useState(false);
  const shell = useRef<HTMLDivElement>(null);
  const look = resolveLook(card.look);
  const seed = useMemo(() => [...card.name].reduce((s, c) => s + c.charCodeAt(0), 0) % 7, [card.name]);
  const tiltable = interactive && !reduced;
  const issuer = card.institution ?? card.issuerName;
  const expiry = expiryText(card.expiryMonth ?? null, card.expiryYear ?? null);
  const showNumber = card.kind !== "other" || !!card.last4;

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
  const faceClass = "absolute inset-0 overflow-hidden rounded-[4.5cqw] [backface-visibility:hidden] shadow-[0_18px_40px_-18px_rgba(0,0,0,.55),0_2px_6px_rgba(0,0,0,.18)]";
  const surface = (
    <>
      {look.dim > 0 && <div aria-hidden className="absolute inset-0 bg-black" style={{ opacity: look.dim }} />}
      {look.pattern && <Guilloche colour={look.line} seed={seed} />}
    </>
  );
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
        aria-label={interactive ? `${card.name}, ${KIND_LABEL[card.kind]} card${card.last4 ? ` ending ${card.last4}` : ""}. ${flipped ? "Showing details" : "Press to see details"}` : undefined}
        onClick={flip}
        onKeyDown={interactive ? onKey : undefined}
        className={cn("relative aspect-[1.586] w-full rounded-[4.5cqw] [transform-style:preserve-3d] outline-none focus-visible:ring-4 focus-visible:ring-ring/50", interactive && "cursor-pointer")}
        style={{
          transform: `rotateX(var(--rx,0deg)) rotateY(calc(var(--ry,0deg) + ${flipped ? 180 : 0}deg))`,
          transition: reduced ? "none" : "transform var(--tilt-speed,600ms) cubic-bezier(.2,.7,.2,1)",
          color: look.ink,
          textShadow: look.darkInk ? undefined : "0 1px 2px rgba(0,0,0,.35)",
        }}
      >
        {/* Front */}
        <div className={faceClass} style={{ background: look.background }}>
          {surface}
          <div className="relative flex h-full flex-col justify-between p-[6.5cqw]">
            <div className="flex items-start justify-between gap-[3cqw]">
              <div className="flex min-w-0 items-center gap-[2.5cqw]">
                {(card.logo || card.institutionLogo || card.institutionShort) && (
                  <InstitutionBadge logo={card.logo} institutionLogo={card.institutionLogo} short={card.institutionShort}
                    name={issuer ?? card.name} className="size-[9cqw] rounded-[2cqw] ring-1 ring-white/20" />
                )}
                {issuer && <span className="truncate text-[4.4cqw] font-semibold tracking-tight">{issuer}</span>}
              </div>
              <div className="flex shrink-0 items-center gap-[2cqw]">
                <span className="rounded-full border border-current/30 px-[2cqw] py-[0.4cqw] text-[2.8cqw] font-medium">{KIND_LABEL[card.kind]}</span>
                {HAS_CHIP[card.kind] && <Nfc className="size-[7cqw] opacity-80" aria-hidden />}
              </div>
            </div>
            {HAS_CHIP[card.kind] ? <Chip /> : <div />}
            <div className="space-y-[2cqw]">
              {showNumber && (
                <div className="text-[6cqw] font-medium tracking-[0.14em] tabular-nums">{maskedNumber(card.network, card.last4)}</div>
              )}
              <div className="flex items-end justify-between gap-[3cqw]">
                <div className="min-w-0">
                  <div className="truncate text-[3.8cqw] opacity-90">{card.name}</div>
                  {expiry && <div className="text-[2.9cqw] opacity-75">Valid thru <span className="tabular-nums">{expiry}</span></div>}
                </div>
                <span className="shrink-0 text-[4.6cqw] font-semibold italic tracking-tight">{card.network ? NETWORK_NAME[card.network] : ""}</span>
              </div>
            </div>
          </div>
          {glare}
        </div>

        {/* Back */}
        <div className={faceClass} style={{ background: look.background, transform: "rotateY(180deg)" }}>
          {surface}
          {card.kind !== "other" && <div className="absolute inset-x-0 top-[11%] h-[17%] bg-black/80" aria-hidden />}
          <div className={cn("relative flex h-full flex-col justify-end gap-[3cqw] p-[6.5cqw]", card.kind !== "other" && "pt-[33%]")}>
            <BackDetails card={card} money={money} />
            <div className="text-[2.8cqw] opacity-65">Vaulty keeps only the last 4 digits of this card.</div>
          </div>
          {glare}
        </div>
      </div>
    </div>
  );
}

function BackDetails({ card, money }: { card: CardVisualData; money: (n: number | null | undefined) => string }) {
  const stat = (label: string, value: string) => (
    <div className="min-w-0"><div className="text-[3.2cqw] opacity-70">{label}</div><div className="truncate text-[4.6cqw] font-semibold tabular-nums">{value}</div></div>
  );
  switch (card.kind) {
    case "credit":
      return (
        <>
          <div className="grid grid-cols-3 gap-[3cqw]">{stat("Owed", money(card.owed))}{stat("Available", money(card.available))}{stat("Limit", money(card.creditLimit))}</div>
          <div className="flex justify-between gap-[3cqw] text-[3.4cqw] opacity-85">
            <span>{card.statementDay ? `Statement on the ${ordinal(card.statementDay)}` : "Statement day not set"}</span>
            <span>{card.dueDay ? `Due on the ${ordinal(card.dueDay)}` : ""}</span>
          </div>
        </>
      );
    case "debit":
      return <div className="grid grid-cols-2 gap-[3cqw]">{stat("Linked account", card.accountName ?? "—")}{stat("Balance", money(card.balance))}</div>;
    case "prepaid":
    case "gift":
      return (
        <div className="grid grid-cols-2 gap-[3cqw]">
          {stat(card.kind === "gift" ? "Remaining" : "Balance", money(card.balance))}
          {stat("Expires", expiryText(card.expiryMonth ?? null, card.expiryYear ?? null) ?? "No expiry set")}
        </div>
      );
    default:
      return <div className="line-clamp-4 text-[3.6cqw] opacity-90">{card.notes || "No notes on this card."}</div>;
  }
}
