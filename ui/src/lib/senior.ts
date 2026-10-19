import type { PageId } from "@/nav";

/** Pages in the shorter senior-mode menu. Everything else is still reachable with Ctrl/Cmd+K. */
export const SENIOR_PAGES: readonly PageId[] = ["dashboard", "transactions", "accounts", "recurring", "budgets"];

/** Larger text and controls for the whole window (see `html[data-senior]` in index.css). */
export function applySenior(on: boolean) {
  document.documentElement.toggleAttribute("data-senior", on);
}
