import { useId, type ReactNode } from "react";
import { motion } from "motion/react";
import { SPRING, useMotion } from "@/lib/motion";
import { cn } from "@/lib/utils";

type Option<T extends string> = { value: T; label: ReactNode; title?: string };

/**
 * Segmented control whose selected pill slides between options.
 * `role` is "tablist" for tabs (Transactions/History) or "radiogroup" for form choices.
 */
export function Segmented<T extends string>({ value, onChange, options, role = "radiogroup", label, className, itemClassName, disabled }: {
  value: T; onChange: (v: T) => void; options: Option<T>[]; role?: "radiogroup" | "tablist"; label?: string;
  className?: string; itemClassName?: string; disabled?: boolean;
}) {
  const id = useId();
  const { move } = useMotion();
  const item = role === "tablist" ? "tab" : "radio";
  return (
    <div role={role} aria-label={label} className={cn("bg-muted isolate inline-flex rounded-lg p-1", disabled && "pointer-events-none opacity-50", className)}>
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button key={o.value} type="button" role={item} title={o.title} disabled={disabled}
            {...(item === "tab" ? { "aria-selected": on } : { "aria-checked": on })}
            onClick={() => onChange(o.value)}
            className={cn("relative flex flex-1 items-center justify-center gap-1.5 whitespace-nowrap rounded-md px-3 py-1 text-sm transition-colors",
              on ? "text-foreground font-medium" : "text-muted-foreground hover:text-foreground", itemClassName)}>
            {on && <motion.span layoutId={`seg-${id}`} transition={move(SPRING)} className="bg-background absolute inset-0 -z-10 rounded-md shadow-sm" aria-hidden />}
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
