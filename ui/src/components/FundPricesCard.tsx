import { useEffect, useState } from "react";
import { Loader2, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type FundPriceStatus } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { formatDate } from "@/lib/utils";

/** Daily mutual fund prices from AMFI: on or off, when they were last fetched, and "Update now". */
export function FundPricesCard({ compact, onUpdated }: { compact?: boolean; onUpdated?: () => void }) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<FundPriceStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [note, setNote] = useState("");
  const load = () => api.investments.fundPricesStatus().then(setStatus).catch((e) => setError(errorText(e)));
  useEffect(() => { load(); }, []);
  const toggle = async (on: boolean) => {
    setError("");
    try { await api.market.update({ fund_prices: on }); await load(); if (on) await update(); } catch (e) { setError(errorText(e)); }
  };
  const update = async () => {
    setBusy(true); setError(""); setNote("");
    try {
      const r = await api.investments.fundPricesUpdate(true);
      if (r.skipped) setNote(t("investments.noFundsToPrice"));
      else { setNote(t("investments.pricesUpdated", { count: r.updated ?? 0, date: r.as_of ? formatDate(r.as_of) : "" })); onUpdated?.(); }
      await load();
    } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  if (!status) return null;
  const last = status.last;
  const body = (
    <div className="grid gap-3">
      <div className="flex items-center justify-between gap-3">
        <Label htmlFor="fund-prices" className="leading-snug">{t("investments.fundPrices")}</Label>
        <Switch id="fund-prices" checked={status.on} onCheckedChange={toggle} aria-describedby="fund-prices-hint" />
      </div>
      <p id="fund-prices-hint" className="text-muted-foreground text-xs">{t("investments.fundPricesHelp")}</p>
      {status.on && (
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <span className="text-muted-foreground flex-1">{last?.as_of ? t("investments.pricesAsOf", { date: formatDate(last.as_of), fetched: formatDate(last.fetched_on) }) : t("investments.pricesNotYet")}</span>
          <Button size="sm" variant="outline" onClick={update} disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <RefreshCw />}{t("investments.updateNow")}</Button>
        </div>
      )}
      {note && <p className="text-sm" role="status">{note}</p>}
      {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
    </div>
  );
  if (compact) return body;
  return (
    <Card>
      <CardHeader><CardTitle>{t("investments.fundPricesTitle")}</CardTitle><CardDescription>{t("investments.fundPricesDesc")}</CardDescription></CardHeader>
      <CardContent>{body}</CardContent>
    </Card>
  );
}
