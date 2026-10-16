import { useState } from "react";
import { cn } from "@/lib/utils";

/** Neutral palette for monograms (deliberately not banks' brand colours). */
const PALETTE = ["#2563eb", "#0891b2", "#16a34a", "#ca8a04", "#ea580c", "#e11d48", "#9333ea", "#475569"];

function colourFor(key: string) {
  let h = 0;
  for (const ch of key) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return PALETTE[h % PALETTE.length];
}

type Props = {
  /** Account's own image (data URL) — wins over everything else. */
  logo?: string | null;
  /** Bundled logo file name in /institutions/, from the directory. */
  institutionLogo?: string | null;
  /** Monogram text, e.g. "SBI", "HDFC". */
  short?: string | null;
  /** Used for the monogram colour and alt text. */
  name: string;
  className?: string;
};

/** Acronyms up to 5 letters as-is (SBI, HDFC, ICICI, J&K); anything else by its first letter (Zerodha → Z). */
export function monogram(short: string) {
  const s = short.trim();
  return /^[A-Z&]{1,5}$/.test(s) ? s : s.charAt(0).toUpperCase();
}

/**
 * Badge for an account's bank or institution: the user's own image, else a bundled logo file,
 * else a monogram. Returns null when there's nothing to show (callers fall back to a type icon).
 */
export function InstitutionBadge({ logo, institutionLogo, short, name, className }: Props) {
  const [fileFailed, setFileFailed] = useState(false);
  const base = cn("grid size-9 shrink-0 place-items-center overflow-hidden rounded-lg", className);

  if (logo) return <img src={logo} alt={name} className={cn(base, "bg-white object-contain p-0.5")} />;
  if (institutionLogo && !fileFailed)
    return <img src={`/institutions/${institutionLogo}`} alt={name} onError={() => setFileFailed(true)} className={cn(base, "bg-white object-contain p-1")} />;
  if (!short) return null;

  const text = monogram(short);
  return (
    <div className={cn(base, "font-semibold tracking-tight text-white select-none")} style={{ backgroundColor: colourFor(name) }} aria-label={name} title={name}>
      <span className={text.length <= 2 ? "text-sm" : text.length === 3 ? "text-[11px]" : text.length === 4 ? "text-[9px]" : "text-[8px]"}>{text}</span>
    </div>
  );
}
