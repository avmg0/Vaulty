import { cn } from "@/lib/utils";

/** Placeholder shaped like the content it stands in for, with a soft shimmer. */
export function Skeleton({ className }: { className?: string }) {
  return <div aria-hidden className={cn("skeleton bg-muted rounded-md", className)} />;
}

/** A few transaction-like rows. */
export function SkeletonRows({ rows = 4 }: { rows?: number }) {
  return (
    <div className="divide-y" aria-busy="true" aria-label="Loading">
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="flex items-center gap-3 py-3">
          <Skeleton className="size-8 rounded-full" />
          <div className="flex-1 space-y-1.5"><Skeleton className="h-3.5 w-2/5" /><Skeleton className="h-3 w-1/4" /></div>
          <Skeleton className="h-3.5 w-16" />
        </div>
      ))}
    </div>
  );
}
