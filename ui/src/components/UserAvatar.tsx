import { cn } from "@/lib/utils";

export const PROFILE_COLORS = ["#2563eb", "#16a34a", "#9333ea", "#ea580c", "#e11d48", "#0891b2", "#ca8a04", "#52525b"];

export function initials(name: string) {
  const parts = name.trim().split(/\s+/);
  return ((parts[0]?.[0] ?? "") + (parts.length > 1 ? parts[parts.length - 1][0] : "")).toUpperCase() || "?";
}

export function UserAvatar({ name, color, className }: { name: string; color: string; className?: string }) {
  return (
    <div className={cn("grid size-9 shrink-0 place-items-center rounded-full text-sm font-semibold text-white select-none", className)}
      style={{ backgroundColor: color }} aria-hidden>
      {initials(name)}
    </div>
  );
}

export function ColorPicker({ value, onChange }: { value: string; onChange: (c: string) => void }) {
  return (
    <div className="flex flex-wrap gap-2">
      {PROFILE_COLORS.map((c) => (
        <button key={c} type="button" onClick={() => onChange(c)} aria-label={`Colour ${c}`}
          className={cn("size-7 rounded-full ring-offset-2 ring-offset-background transition", value === c && "ring-2 ring-ring")}
          style={{ backgroundColor: c }} />
      ))}
    </div>
  );
}
