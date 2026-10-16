import { motion } from "motion/react";
import { useMotion, SPRING } from "@/lib/motion";
import { cn } from "@/lib/utils";

export const PROFILE_COLORS = ["#2563eb", "#16a34a", "#9333ea", "#ea580c", "#e11d48", "#0891b2", "#ca8a04", "#52525b"];

export function initials(name: string) {
  const parts = name.trim().split(/\s+/);
  return ((parts[0]?.[0] ?? "") + (parts.length > 1 ? parts[parts.length - 1][0] : "")).toUpperCase() || "?";
}

/** `layoutId` lets the same person's avatar glide from one screen to the next (e.g. picker → password). */
export function UserAvatar({ name, color, className, layoutId }: { name: string; color: string; className?: string; layoutId?: string }) {
  const { move } = useMotion();
  return (
    <motion.div layoutId={layoutId} transition={move(SPRING)}
      className={cn("grid size-9 shrink-0 place-items-center rounded-full text-sm font-semibold text-white select-none", className)}
      style={{ backgroundColor: color }} aria-hidden>
      {initials(name)}
    </motion.div>
  );
}

export function ColorPicker({ value, onChange }: { value: string; onChange: (c: string) => void }) {
  return (
    <div className="flex flex-wrap gap-2">
      {PROFILE_COLORS.map((c) => (
        <button key={c} type="button" onClick={() => onChange(c)} aria-label={`Colour ${c}`}
          className={cn("size-7 rounded-full ring-offset-2 ring-offset-background transition-[box-shadow,transform] duration-200 hover:scale-110 active:scale-95", value === c && "ring-2 ring-ring")}
          style={{ backgroundColor: c }} />
      ))}
    </div>
  );
}
