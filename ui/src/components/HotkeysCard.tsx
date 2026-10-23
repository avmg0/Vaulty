import { useState, type KeyboardEvent } from "react";
import { Keyboard, RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { errorText } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { comboOf, formatKeys, useHotkeys, type HotkeyAction } from "@/hooks/hotkeys";
import { cn } from "@/lib/utils";

const GROUPS: { id: "general" | "go"; actions: HotkeyAction[] }[] = [
  { id: "general", actions: ["quick_add", "palette", "undo", "hide_balances", "lock", "panic", "sidebar"] },
  { id: "go", actions: ["settings", "go_dashboard", "go_transactions", "go_accounts", "go_reports", "go_budgets", "go_recurring", "go_cards", "go_import"] },
];

/** Settings: every shortcut, with Change (press the new keys), Turn off and Reset. */
export function HotkeysCard() {
  const { t } = useTranslation();
  const { list, keys, set, resetAll, setPaused } = useHotkeys();
  const [recording, setRecording] = useState<HotkeyAction | null>(null);
  const [error, setError] = useState<{ action: HotkeyAction; text: string } | null>(null);
  const custom = list.some((h) => h.custom);

  const start = (a: HotkeyAction) => { setError(null); setRecording(a); setPaused(true); };
  const stop = () => { setRecording(null); setPaused(false); };
  const save = async (a: HotkeyAction, k: string | null) => {
    stop();
    try { await set(a, k); } catch (e) { setError({ action: a, text: errorText(e) }); }
  };

  const onKey = (a: HotkeyAction, e: KeyboardEvent) => {
    e.preventDefault(); e.stopPropagation();
    if (e.key === "Escape") return stop();
    if (e.key === "Backspace" || e.key === "Delete") return void save(a, "");
    const combo = comboOf(e);
    if (!combo) return; // only a modifier so far; wait for the key
    const taken = (Object.entries(keys) as [HotkeyAction, string][]).find(([other, k]) => other !== a && k === combo);
    if (taken) {
      stop();
      return setError({ action: a, text: t("hotkeys.taken", { keys: formatKeys(combo), action: t(`hotkeys.actions.${taken[0]}`) }) });
    }
    void save(a, combo);
  };

  return (
    <Card>
      <CardHeader><CardTitle>{t("hotkeys.title")}</CardTitle><CardDescription>{t("hotkeys.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-5">
        {GROUPS.map((g) => (
          <div key={g.id}>
            <div className="text-muted-foreground mb-1 text-xs font-medium">{t(`hotkeys.groups.${g.id}`)}</div>
            <div className="divide-y">
              {g.actions.map((a) => {
                const h = list.find((x) => x.action === a);
                const now = keys[a] ?? "";
                return (
                  <div key={a} className="py-2">
                    <div className="flex items-center gap-3">
                      <div className="min-w-0 flex-1 text-sm">{t(`hotkeys.actions.${a}`)}</div>
                      {recording === a ? (
                        <button type="button" autoFocus onKeyDown={(e) => onKey(a, e)} onBlur={stop}
                          className="border-primary bg-primary/5 text-primary h-8 min-w-36 rounded-md border border-dashed px-3 text-xs">
                          {t("hotkeys.press")}
                        </button>
                      ) : (
                        <button type="button" onClick={() => start(a)} aria-label={`${now ? formatKeys(now) : t("hotkeys.none")}: ${t("hotkeys.changeFor", { action: t(`hotkeys.actions.${a}`) })}`}
                          className={cn("hover:bg-accent h-8 min-w-24 rounded-md border px-3 font-mono text-xs", !now && "text-muted-foreground font-sans")}>
                          {now ? formatKeys(now) : t("hotkeys.none")}
                        </button>
                      )}
                      <Button variant="ghost" size="icon" className="size-8" disabled={!h?.custom}
                        onClick={() => void save(a, null)} title={t("hotkeys.reset")} aria-label={t("hotkeys.resetFor", { action: t(`hotkeys.actions.${a}`) })}>
                        <RotateCcw className="size-3.5" />
                      </Button>
                    </div>
                    {error?.action === a && <p className="text-destructive mt-1 text-xs">{error.text}</p>}
                  </div>
                );
              })}
            </div>
          </div>
        ))}
        <div className="text-muted-foreground flex flex-wrap items-center justify-between gap-2 text-xs">
          <span className="flex items-center gap-1.5"><Keyboard className="size-3.5" />{t("hotkeys.hint")}</span>
          <Button variant="outline" size="sm" disabled={!custom} onClick={() => void resetAll()}>{t("hotkeys.resetAll")}</Button>
        </div>
      </CardContent>
    </Card>
  );
}
