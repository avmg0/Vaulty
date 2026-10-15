import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

/** 6-digit authenticator code field. */
export function CodeInput({ value, onChange, className, autoFocus = true }: { value: string; onChange: (v: string) => void; className?: string; autoFocus?: boolean }) {
  return (
    <Input
      inputMode="numeric" autoComplete="one-time-code" autoFocus={autoFocus} maxLength={6} placeholder="000000"
      value={value} onChange={(e) => onChange(e.target.value.replace(/\D/g, "").slice(0, 6))}
      className={cn("h-12 text-center font-mono text-2xl tracking-[0.5em]", className)}
    />
  );
}
