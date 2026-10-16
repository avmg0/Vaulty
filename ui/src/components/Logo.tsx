import { cn } from "@/lib/utils";

/** Vaulty logo (source: assets/brand/logo.svg, served from ui/public/logo.svg). */
export function Logo({ className }: { className?: string }) {
  return <img src="/logo.svg" alt="Vaulty" draggable={false} className={cn("h-auto select-none", className)} />;
}
