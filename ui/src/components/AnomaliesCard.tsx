import { useEffect, useState } from "react";
import { Copy, Sparkle, TrendingUp, Trash2, Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type Anomaly } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useTheme } from "@/hooks/theme";
import { formatDate, formatMoney, formatNumber } from "@/lib/utils";

const ICON = { double: Copy, amount: TrendingUp, new_place: Sparkle };

/** Spending worth a second look: double charges, unusual amounts, large first payments.
 *  Shown only when there's something to look at. */
export function AnomaliesCard() {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const [items, setItems] = useState<Anomaly[]>([]);
  const [error, setError] = useState("");
  const load = () => api.anomalies.list().then((d) => setItems(d ?? [])).catch(() => setItems([]));
  useEffect(() => { load(); window.addEventListener(DATA_RESTORED, load); return () => window.removeEventListener(DATA_RESTORED, load); }, []);
  if (items.length === 0) return null;
  const money = (a: Anomaly, n: number) => formatMoney(n, a.currency, hideBalances);
  const fine = (a: Anomaly) => api.anomalies.dismiss(a.key).then(load).catch((e) => setError(errorText(e)));
  const remove = (a: Anomaly) => api.transactions.delete(a.entry_id).then(load).catch((e) => setError(errorText(e)));
  return (
    <Card className="min-w-0 border-amber-500/40">
      <CardHeader><CardTitle>{t("anomalies.title")}</CardTitle><CardDescription>{t("anomalies.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-2">
        {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
        <ul className="divide-y rounded-lg border">
          {items.slice(0, 6).map((a) => {
            const Icon = ICON[a.kind];
            return (
              <li key={a.key} className="flex min-w-0 flex-wrap items-center gap-3 p-3">
                <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-amber-100 text-amber-900 dark:bg-amber-950 dark:text-amber-200"><Icon className="size-4" aria-hidden /></div>
                <div className="min-w-0 flex-1 basis-40">
                  <p className="text-sm font-medium">{t(`anomalies.kinds.${a.kind}`, { name: a.name, times: formatNumber(a.times ?? 0), usual: a.usual != null ? money(a, a.usual) : "" })}</p>
                  <p className="text-muted-foreground text-xs">{[formatDate(a.date), a.account].filter(Boolean).join(" · ")}</p>
                </div>
                <span className="font-semibold tabular-nums">{money(a, a.amount)}</span>
                <div className="flex flex-wrap gap-1">
                  {a.kind === "double" && <Button size="sm" variant="outline" onClick={() => remove(a)}><Trash2 />{t("anomalies.deleteSecond")}</Button>}
                  <Button size="sm" variant="ghost" onClick={() => fine(a)}><Check />{t("anomalies.fine")}</Button>
                </div>
              </li>
            );
          })}
        </ul>
        {items.length > 6 && <p className="text-muted-foreground text-xs">{t("anomalies.more", { count: items.length - 6 })}</p>}
      </CardContent>
    </Card>
  );
}
