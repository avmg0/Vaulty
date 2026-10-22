import { invoke } from "@tauri-apps/api/core";

/**
 * Opens the system print dialog ("Save as PDF" is offered there). In the app this goes through
 * the window itself, which works on every system; in a plain browser, window.print().
 * `cleanup` runs once printing is done (or the person is back in the app).
 */
export function printNow(cleanup?: () => void) {
  let done = false;
  const finish = () => {
    if (done) return;
    done = true;
    window.removeEventListener("afterprint", finish);
    window.removeEventListener("pointerdown", finish, true);
    window.removeEventListener("keydown", finish, true);
    cleanup?.();
  };
  window.addEventListener("afterprint", finish);
  // Some systems don't report the end of printing: the next click or key press ends it.
  setTimeout(() => {
    window.addEventListener("pointerdown", finish, true);
    window.addEventListener("keydown", finish, true);
  }, 500);
  // Let the page repaint in its print layout before the dialog takes its picture.
  setTimeout(() => {
    invoke("print_window").catch(() => window.print());
  }, 80);
}

/** Prints the current page alone: no menu or toolbars, light colours, every page of it. */
export function printPage() {
  const html = document.documentElement;
  const wasDark = html.classList.contains("dark");
  html.classList.remove("dark");
  document.body.classList.add("printing-page");
  printNow(() => {
    document.body.classList.remove("printing-page");
    if (wasDark) html.classList.add("dark");
  });
}
