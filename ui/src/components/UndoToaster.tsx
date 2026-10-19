import { useCallback, useEffect, useRef, useState } from "react";
import { RotateCcw, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_CHANGED, errorText, type HistoryItem } from "@/api";
import { Button } from "@/components/ui/button";
import { useOptionalSession } from "@/hooks/session";
import { useHotkey } from "@/hooks/hotkeys";

type Toast = { id: number; text: string; batch?: number };

const OFFER_UNDO = /_(delete|update|set_visibility)$|^settings_update$|^import_json$/;

/** Describes an action for people: "Deleted transaction: Big Bazaar". */
export function useActionLabel() {
  const { t } = useTranslation();
  return useCallback((h: HistoryItem) => {
    const verb = t(`history.actions.${h.action}`, { defaultValue: h.action });
    return h.title ? `${verb}: ${h.title}` : verb;
  }, [t]);
}

/**
 * After a change, offers Undo for a few seconds. Also handles Ctrl/Cmd+Z outside text fields,
 * which undoes the last change.
 */
export function UndoToaster() {
  const { t } = useTranslation();
  const label = useActionLabel();
  const [toast, setToast] = useState<Toast | null>(null);
  const [busy, setBusy] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const hovered = useRef(false);

  const senior = !!useOptionalSession()?.user.senior;
  const show = useCallback((tst: Toast, ms = 8000) => {
    if (senior) ms *= 2;
    setToast(tst);
    clearTimeout(timer.current);
    const close = () => { if (hovered.current) timer.current = setTimeout(close, 1500); else setToast(null); };
    timer.current = setTimeout(close, ms);
  }, [senior]);

  useEffect(() => {
    const onChange = (e: Event) => {
      const cmd = (e as CustomEvent<{ cmd: string }>).detail?.cmd ?? "";
      if (!OFFER_UNDO.test(cmd)) return;
      api.history.recent(1).then(([last]) => {
        if (last && !last.undone) show({ id: Date.now(), text: t("history.toastDone", { what: label(last) }), batch: last.id });
      }).catch(() => {});
    };

    window.addEventListener(DATA_CHANGED, onChange);
    return () => { window.removeEventListener(DATA_CHANGED, onChange); clearTimeout(timer.current); };
  }, [label, show, t]);

  // Outside text fields only: there, the shortcut undoes typing as usual.
  useHotkey("undo", (e) => {
    const el = e.target as HTMLElement | null;
    if (el && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName))) return false;
    if (document.querySelector("[role=dialog]")) return false;
    api.history.undoLast()
      .then((done) => show({ id: Date.now(), text: done ? t("history.toastUndone", { what: label(done) }) : t("history.nothing") }, 4000))
      .catch((err) => show({ id: Date.now(), text: errorText(err) }, 6000));
  });

  const undo = async () => {
    if (!toast?.batch) return;
    setBusy(true);
    try {
      await api.history.undo(toast.batch);
      show({ id: Date.now(), text: t("history.undone") }, 2500);
    } catch (e) { show({ id: Date.now(), text: errorText(e) }, 6000); }
    finally { setBusy(false); }
  };

  return (
    <div aria-live="polite" className="pointer-events-none fixed inset-x-0 bottom-5 z-50 flex justify-center px-4">
      {toast && (
        <div key={toast.id} className="toast-in bg-foreground text-background pointer-events-auto flex max-w-lg items-center gap-3 rounded-lg py-2 ps-4 pe-2 text-sm shadow-lg"
          onMouseEnter={() => { hovered.current = true; }} onMouseLeave={() => { hovered.current = false; }}>
          <span className="min-w-0 truncate">{toast.text}</span>
          {toast.batch && (
            <Button size="sm" variant="secondary" className="h-7 shrink-0" onClick={undo} disabled={busy}><RotateCcw />{t("history.undo")}</Button>
          )}
          <button onClick={() => setToast(null)} className="shrink-0 rounded p-1 opacity-70 hover:opacity-100" aria-label={t("common.cancel")}><X className="size-4" /></button>
        </div>
      )}
    </div>
  );
}
