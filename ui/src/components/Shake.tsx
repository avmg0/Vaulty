import { useEffect, type ReactNode } from "react";
import { useAnimate } from "motion/react";
import { useMotion } from "@/lib/motion";

/**
 * Gives its contents a short sideways shake whenever `trigger` changes to a truthy value,
 * e.g. an error message. A shake says "try again" without another line of text.
 */
export function Shake({ trigger, children, className }: { trigger: unknown; children: ReactNode; className?: string }) {
  const [scope, animate] = useAnimate();
  const { full } = useMotion();
  useEffect(() => {
    if (!trigger || !full || !scope.current) return;
    animate(scope.current, { x: [0, -9, 8, -6, 4, -2, 0] }, { duration: 0.42, ease: "easeOut" });
  }, [trigger, full, animate, scope]);
  return <div ref={scope} className={className}>{children}</div>;
}
