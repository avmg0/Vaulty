import { useCallback, useEffect, useState } from "react";
import { ArrowLeftRight, CreditCard, FolderTree, Info, RotateCcw, Settings2, Shapes, Undo2, Wallet } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type HistoryItem } from "@/api";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useActionLabel } from "@/components/UndoToaster";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate, formatMoney } from "@/lib/utils";

const ICONS: Record<string, typeof Wallet> = {
  entries: ArrowLeftRight, accounts: Wallet, cards: CreditCard, categories: FolderTree, user_settings: Settings2,
};

/** Recent changes with undo, and deleted items with restore. */
export function History() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<"recent" | "deleted">("recent");
  const [items, setItems] = useState<HistoryItem[] | null>(null);
  const [error, setError] = useState("");

  const load = useCallback(() => {
    setError("");
    (tab === "recent" ? api.history.recent(200) : api.history.trash()).then(setItems).catch((e) => setError(errorText(e)));
  }, [tab]);
  useEffect(() => {
    load();
    window.addEventListener(DATA_RESTORED, load);
    return () => window.removeEventListener(DATA_RESTORED, load);
  }, [load]);

  const act = async (h: HistoryItem) => {
    setError("");
    try { await api.history.undo(h.id); } catch (e) { setError(errorText(e)); }
  };

  return (
    <div className="max-w-3xl space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="bg-muted inline-flex rounded-lg p-1" role="tablist">
          {(["recent", "deleted"] as const).map((k) => (
            <button key={k} role="tab" aria-selected={tab === k} onClick={() => { setItems(null); setTab(k); }}
              className={cn("rounded-md px-3 py-1 text-sm transition-colors", tab === k ? "bg-background font-medium shadow-sm" : "text-muted-foreground hover:text-foreground")}>
              {t(`history.${k}`)}
            </button>
          ))}
        </div>
        <span className="text-muted-foreground text-xs">{t("history.shortcut")}</span>
      </div>
      <p className="text-muted-foreground flex gap-2 text-xs"><Info className="mt-px size-3.5 shrink-0" />{t("history.keep")}</p>
      {error && <p className="text-destructive text-sm">{error}</p>}
      {items && (
        <Card className="py-1">
          <CardContent className="divide-y px-4">
            {items.length === 0 && <p className="text-muted-foreground py-8 text-center text-sm">{tab === "recent" ? t("history.empty") : t("history.emptyTrash")}</p>}
            {items.map((h) => <Row key={h.id} h={h} restore={tab === "deleted"} onAct={() => act(h)} />)}
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function Row({ h, restore, onAct }: { h: HistoryItem; restore: boolean; onAct: () => void }) {
  const { t } = useTranslation();
  const label = useActionLabel();
  const { hideBalances } = useTheme();
  const Icon = h.action === "undo" ? Undo2 : ICONS[h.kind] ?? Shapes;
  const isUndo = h.action === "undo";
  return (
    <div className={cn("flex items-center gap-3 py-3", h.undone && "opacity-60")}>
      <div className="bg-secondary grid size-9 shrink-0 place-items-center rounded-lg"><Icon className="size-4" /></div>
      <div className="min-w-0 flex-1">
        <div className={cn("truncate text-sm font-medium", h.undone && "line-through")}>{label(h)}</div>
        <div className="text-muted-foreground text-xs tabular-nums">
          {formatDate(h.at, { day: "numeric", month: "short", hour: "numeric", minute: "2-digit" })}
          {h.amount && <> · {formatMoney(h.amount.amount, h.amount.currency, hideBalances)}</>}
        </div>
      </div>
      {h.undone ? <span className="text-muted-foreground text-xs">{t("history.undone")}</span>
        : !isUndo || restore ? (
          <Button size="sm" variant="outline" onClick={onAct} disabled={!h.can_undo} title={h.can_undo ? undefined : t("history.blocked")}>
            <RotateCcw />{restore ? t("history.restore") : t("history.undo")}
          </Button>
        ) : (
          <Button size="sm" variant="ghost" onClick={onAct} disabled={!h.can_undo} title={h.can_undo ? undefined : t("history.blocked")}>
            <RotateCcw />{t("history.undo")}
          </Button>
        )}
    </div>
  );
}
