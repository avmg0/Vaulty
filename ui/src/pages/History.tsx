import { useCallback, useEffect, useState } from "react";
import { SkeletonRows } from "@/components/Skeleton";
import { ArrowLeftRight, CreditCard, FolderTree, Info, RotateCcw, Settings2, Shapes, Undo2, Wallet } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type HistoryItem } from "@/api";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useActionLabel } from "@/components/UndoToaster";
import { HistoryCheck } from "@/components/HistoryCheck";
import { Segmented } from "@/components/Segmented";
import { AnimatedItem, AnimatedList } from "@/components/AnimatedList";
import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";
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
        <Segmented role="tablist" value={tab} onChange={(k) => { setItems(null); setTab(k); }}
          options={(["recent", "deleted"] as const).map((k) => ({ value: k, label: t(`history.${k}`) }))} />
        <span className="text-muted-foreground text-xs">{t("history.shortcut")}</span>
      </div>
      <p className="text-muted-foreground flex gap-2 text-xs"><Info className="mt-px size-3.5 shrink-0" />{t("history.keep")}</p>
      {tab === "recent" && <HistoryCheck />}
      {error && <p className="text-destructive text-sm">{error}</p>}
      {!items && !error && <Card className="py-1"><CardContent className="px-4"><SkeletonRows rows={5} /></CardContent></Card>}
      {items && (
        <Card className="py-1">
          <CardContent className="divide-y px-4">
            {items.length === 0 && <p className="text-muted-foreground py-8 text-center text-sm">{tab === "recent" ? t("history.empty") : t("history.emptyTrash")}</p>}
            <AnimatedList>
              {items.map((h) => (
                <AnimatedItem key={h.id} exitX={tab === "deleted" ? -48 : 0}>
                  <Row h={h} restore={tab === "deleted"} onAct={() => act(h)} />
                </AnimatedItem>
              ))}
            </AnimatedList>
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
  const { move } = useMotion();
  const isUndo = h.action === "undo";
  return (
    <div className={cn("flex items-center gap-3 py-3 transition-opacity duration-300", h.undone && "opacity-60")}>
      <div className="bg-secondary grid size-9 shrink-0 place-items-center rounded-lg"><Icon className="size-4" /></div>
      <div className="min-w-0 flex-1">
        <div className="relative w-fit max-w-full truncate text-sm font-medium">
          {label(h)}
          {/* Undoing draws a line through the change rather than just swapping styles. */}
          {h.undone && (
            <motion.span aria-hidden className="bg-foreground/70 absolute inset-x-0 top-1/2 h-px origin-left"
              initial={{ scaleX: 0 }} animate={{ scaleX: 1 }} transition={move({ duration: 0.35, ease: "easeOut" })} />
          )}
        </div>
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
