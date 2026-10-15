import { useEffect, useRef, useState } from "react";
import { Check, Globe, Lock, Users, type LucideIcon } from "lucide-react";
import type { Visibility } from "@/api";
import { cn } from "@/lib/utils";

export const VISIBILITY: Record<Visibility, { label: string; icon: LucideIcon; desc: string }> = {
  private: { label: "Private", icon: Lock, desc: "Only you" },
  shared: { label: "Shared", icon: Users, desc: "You and the people you chose" },
  public: { label: "Public", icon: Globe, desc: "Everyone in the family with sharing on" },
};
const ORDER: Visibility[] = ["private", "shared", "public"];

/** Segmented control used in forms and settings. */
export function VisibilityPicker({ value, onChange, disabled }: { value: Visibility; onChange: (v: Visibility) => void; disabled?: boolean }) {
  return (
    <div role="radiogroup" className={cn("bg-muted grid grid-cols-3 gap-1 rounded-lg p-1", disabled && "opacity-50")}>
      {ORDER.map((v) => {
        const { label, icon: Icon } = VISIBILITY[v];
        return (
          <button key={v} type="button" role="radio" aria-checked={value === v} disabled={disabled} onClick={() => onChange(v)}
            title={VISIBILITY[v].desc}
            className={cn("flex items-center justify-center gap-1.5 rounded-md py-1.5 text-sm transition-colors",
              value === v ? "bg-background font-medium shadow-sm" : "text-muted-foreground hover:text-foreground")}>
            <Icon className="size-3.5" />{label}
          </button>
        );
      })}
    </div>
  );
}

/** Small icon showing an item's visibility; when `onChange` is given it opens a menu to change it. */
export function VisibilityBadge({ value, onChange }: { value: Visibility; onChange?: (v: Visibility) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);
  const { icon: Icon, label } = VISIBILITY[value];
  const tone = value === "private" ? "text-muted-foreground" : value === "shared" ? "text-primary" : "text-success";

  return (
    <div ref={ref} className="relative">
      <button type="button" disabled={!onChange} onClick={() => setOpen((o) => !o)} title={`${label}: ${VISIBILITY[value].desc}`}
        className={cn("grid size-7 place-items-center rounded-md", tone, onChange && "hover:bg-accent")}>
        <Icon className="size-3.5" /><span className="sr-only">{label}</span>
      </button>
      {open && onChange && (
        <div className="bg-popover absolute right-0 z-20 mt-1 w-60 rounded-md border p-1 shadow-md">
          {ORDER.map((v) => {
            const o = VISIBILITY[v];
            return (
              <button key={v} type="button" onClick={() => { setOpen(false); if (v !== value) onChange(v); }}
                className="hover:bg-accent flex w-full items-start gap-2 rounded-sm px-2 py-1.5 text-left">
                <o.icon className="mt-0.5 size-4 shrink-0" />
                <span className="flex-1"><span className="block text-sm">{o.label}</span><span className="text-muted-foreground block text-xs">{o.desc}</span></span>
                {v === value && <Check className="mt-0.5 size-4" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
