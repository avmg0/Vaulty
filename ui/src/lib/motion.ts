import type { Transition } from "motion/react";
import { useTheme } from "@/hooks/theme";

/**
 * Shared timing so the whole app moves the same way. Movement is short (150–250 ms) and eases
 * out; springs are kept for physical things like cards.
 */
export const EASE_OUT: Transition = { duration: 0.2, ease: [0.2, 0.7, 0.2, 1] };
export const SPRING: Transition = { type: "spring", stiffness: 420, damping: 36, mass: 0.9 };
export const SOFT_SPRING: Transition = { type: "spring", stiffness: 260, damping: 30 };

/** Transitions adjusted for the motion setting: unchanged, fades only, or instant. */
export function useMotion() {
  const { motion } = useTheme();
  const instant: Transition = { duration: 0 };
  return {
    level: motion,
    full: motion === "full",
    /** Use for movement (position, size, scale). */
    move: (t: Transition = EASE_OUT): Transition => (motion === "full" ? t : instant),
    /** Use for fades; kept in "reduced", dropped in "off". */
    fade: (t: Transition = EASE_OUT): Transition => (motion === "off" ? instant : t),
  };
}
