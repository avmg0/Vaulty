import { useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useMotion } from "@/lib/motion";
import { cn } from "@/lib/utils";

/**
 * 6-digit authenticator code. One real input (so paste, autofill and screen readers work)
 * sits invisibly over six boxes; each digit pops into its box as it's typed.
 */
export function CodeInput({ value, onChange, className, autoFocus = true }: { value: string; onChange: (v: string) => void; className?: string; autoFocus?: boolean }) {
  const [focused, setFocused] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const { move, fade } = useMotion();
  return (
    <div className={cn("relative", className)} onClick={() => input.current?.focus()}>
      <div className="grid grid-cols-6 gap-2" aria-hidden>
        {Array.from({ length: 6 }, (_, i) => {
          const d = value[i];
          const active = focused && (i === value.length || (i === 5 && value.length === 6));
          return (
            <div key={i} className={cn("bg-background grid h-12 place-items-center rounded-md border font-mono text-2xl transition-[border-color,box-shadow] duration-150",
              active ? "border-ring ring-ring/40 ring-[3px]" : "border-input")}>
              <AnimatePresence mode="popLayout" initial={false}>
                {d && (
                  <motion.span key={`${i}-${d}`} initial={{ scale: 0.4, opacity: 0, y: 6 }} animate={{ scale: 1, opacity: 1, y: 0 }} exit={{ scale: 0.6, opacity: 0 }}
                    transition={{ scale: move({ type: "spring", stiffness: 600, damping: 26 }), y: move(), opacity: fade({ duration: 0.12 }) }}>
                    {d}
                  </motion.span>
                )}
              </AnimatePresence>
            </div>
          );
        })}
      </div>
      <input ref={input}
        inputMode="numeric" autoComplete="one-time-code" autoFocus={autoFocus} maxLength={6} aria-label="6-digit code"
        value={value} onChange={(e) => onChange(e.target.value.replace(/\D/g, "").slice(0, 6))}
        onFocus={() => setFocused(true)} onBlur={() => setFocused(false)}
        className="absolute inset-0 h-full w-full cursor-text opacity-0" />
    </div>
  );
}
