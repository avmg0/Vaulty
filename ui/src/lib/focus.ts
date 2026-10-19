/**
 * Dialogs opened by a keyboard shortcut have no button to return focus to, so Radix would drop it
 * on the page body. Remember what had focus when one opens and go back there when it closes.
 */
let last: HTMLElement | null = null;

export function rememberFocus() {
  const el = document.activeElement;
  last = el instanceof HTMLElement && el !== document.body ? el : null;
}

export function returnFocus() {
  const el = last;
  last = null;
  if (el && el.isConnected) el.focus();
  else document.getElementById("content")?.focus();
}
