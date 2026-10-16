import { useEffect, useState, type FormEvent } from "react";
import { Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type MarketPrefs, type MarketView } from "@/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { CurrencyPicker } from "@/components/CurrencyPicker";
import { MARKET_CHANGED } from "@/components/TickerBar";
import { useDisplayNames } from "@/hooks/locale";

const SELECT = "border-input dark:bg-input/30 h-8 rounded-md border bg-transparent px-2 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]";

/** Opt-in ticker: what to show, where prices come from, and the person's own provider keys. */
export function MarketCard() {
  const { t } = useTranslation();
  const names = useDisplayNames();
  const [view, setView] = useState<MarketView | null>(null);
  const [error, setError] = useState("");
  const [symbol, setSymbol] = useState("");
  const [coin, setCoin] = useState("");
  const [stockKey, setStockKey] = useState("");
  const [cryptoKey, setCryptoKey] = useState("");

  useEffect(() => { api.market.get().then(setView).catch((e) => setError(errorText(e))); }, []);
  const apply = (p: Promise<MarketView>) => p.then((v) => { setView(v); setError(""); window.dispatchEvent(new Event(MARKET_CHANGED)); }).catch((e) => setError(errorText(e)));
  const update = (patch: Parameters<typeof api.market.update>[0]) => apply(api.market.update(patch));
  if (!view) return null;
  const p: MarketPrefs = view.prefs;

  const addSymbol = (e: FormEvent) => {
    e.preventDefault();
    if (!symbol.trim()) return;
    update({ watchlist: [...p.watchlist, { symbol: symbol.trim(), label: "" }] }).then(() => setSymbol(""));
  };
  const addCoin = (id: string) => { if (id && !p.crypto.includes(id)) update({ crypto: [...p.crypto, id] }).then(() => setCoin("")); };
  const saveKey = (which: "stock" | "crypto", key: string) => apply(api.market.setKey(which, key)).then(() => (which === "stock" ? setStockKey("") : setCryptoKey("")));
  const coinLabel = (id: string) => view.known_crypto.find((c) => c.symbol === id)?.label ?? id;

  return (
    <Card>
      <CardHeader><CardTitle>{t("market.title")}</CardTitle><CardDescription>{t("market.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-5">
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="text-sm font-medium">{t("market.enable")}</div>
            <div className="text-muted-foreground text-xs">{t("market.privacy")}</div>
          </div>
          <Switch checked={p.enabled} onCheckedChange={(on) => update({ enabled: on })} aria-label={t("market.enable")} />
        </div>

        {p.enabled && (
          <>
            <Separator />
            <section className="space-y-3">
              <div className="text-sm font-medium">{t("market.show")}</div>
              <label className="flex flex-wrap items-center gap-2 text-sm">
                <input type="checkbox" className="accent-[var(--primary)]" checked={p.show.regional} onChange={(e) => update({ show: { regional: e.target.checked } })} />
                {t("market.regional")}
                <select className={SELECT} value={p.region} onChange={(e) => update({ region: e.target.value })} aria-label={t("market.region")}>
                  {view.regional_countries.map((c) => <option key={c} value={c}>{names.region(c)}</option>)}
                </select>
              </label>
              {p.show.regional && <p className="text-muted-foreground ps-6 text-xs">{view.regional.map((i) => i.label).join(", ")}</p>}
              <label className="flex items-center gap-2 text-sm">
                <input type="checkbox" className="accent-[var(--primary)]" checked={p.show.crypto} onChange={(e) => update({ show: { crypto: e.target.checked } })} />
                {t("market.crypto")}
              </label>
              {p.show.crypto && (
                <div className="space-y-2 ps-6">
                  <Chips items={p.crypto.map((id) => ({ key: id, label: coinLabel(id) }))} onRemove={(id) => update({ crypto: p.crypto.filter((c) => c !== id) })} removeLabel={t("market.remove")} />
                  <div className="flex flex-wrap items-center gap-2">
                    <select className={SELECT} value="" onChange={(e) => addCoin(e.target.value)} aria-label={t("market.addCoin")}>
                      <option value="">{t("market.addCoin")}</option>
                      {view.known_crypto.filter((c) => !p.crypto.includes(c.symbol)).map((c) => <option key={c.symbol} value={c.symbol}>{c.label}</option>)}
                    </select>
                    <form className="flex gap-1" onSubmit={(e) => { e.preventDefault(); addCoin(coin.trim().toLowerCase()); }}>
                      <Input className="h-8 w-40" placeholder={t("market.coinId")} value={coin} onChange={(e) => setCoin(e.target.value)} aria-label={t("market.coinId")} />
                      <Button size="icon" variant="outline" className="size-8" aria-label={t("market.add")}><Plus /></Button>
                    </form>
                  </div>
                  <div className="flex items-center gap-2 text-xs"><span className="text-muted-foreground">{t("market.cryptoCurrency")}</span>
                    <CurrencyPicker className="h-8 w-56" value={p.crypto_currency} onChange={(c) => update({ crypto_currency: c })} />
                  </div>
                </div>
              )}
              <label className="flex items-center gap-2 text-sm">
                <input type="checkbox" className="accent-[var(--primary)]" checked={p.show.watchlist} onChange={(e) => update({ show: { watchlist: e.target.checked } })} />
                {t("market.watchlist")}
              </label>
              {p.show.watchlist && (
                <div className="space-y-2 ps-6">
                  <Chips items={p.watchlist.map((w) => ({ key: w.symbol, label: w.label }))} onRemove={(s) => update({ watchlist: p.watchlist.filter((w) => w.symbol !== s) })} removeLabel={t("market.remove")} />
                  <form className="flex gap-1" onSubmit={addSymbol}>
                    <Input className="h-8 w-56" placeholder={t("market.symbolPlaceholder")} value={symbol} onChange={(e) => setSymbol(e.target.value)} aria-label={t("market.symbol")} />
                    <Button size="icon" variant="outline" className="size-8" aria-label={t("market.add")}><Plus /></Button>
                  </form>
                </div>
              )}
            </section>

            <Separator />
            <section className="space-y-3">
              <div className="text-sm font-medium">{t("market.stockSource")}</div>
              <div className="flex flex-wrap items-center gap-2">
                <select className={SELECT} value={p.stock_provider} onChange={(e) => update({ stock_provider: e.target.value as MarketPrefs["stock_provider"] })} aria-label={t("market.stockSource")}>
                  <option value="none">{t("market.providerNone")}</option>
                  <option value="twelvedata">Twelve Data</option>
                </select>
                {p.stock_provider === "twelvedata" && (
                  <KeyField has={view.has_stock_key} value={stockKey} onChange={setStockKey} onSave={() => saveKey("stock", stockKey)} onRemove={() => saveKey("stock", "")} />
                )}
              </div>
              <p className="text-muted-foreground text-xs">{p.stock_provider === "none" ? t("market.providerNoneHint") : t("market.twelveHint")}</p>
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm">{t("market.cryptoKey")}</span>
                <KeyField has={view.has_crypto_key} value={cryptoKey} onChange={setCryptoKey} onSave={() => saveKey("crypto", cryptoKey)} onRemove={() => saveKey("crypto", "")} />
              </div>
              <p className="text-muted-foreground text-xs">{t("market.cryptoKeyHint")}</p>
            </section>

            <Separator />
            <div className="flex flex-wrap gap-6">
              <label className="flex items-center gap-2 text-sm">{t("market.refresh")}
                <select className={SELECT} value={p.refresh_minutes} onChange={(e) => update({ refresh_minutes: Number(e.target.value) })}>
                  {[5, 15, 30, 60, 240].map((m) => <option key={m} value={m}>{t("market.minutes", { count: m })}</option>)}
                </select>
              </label>
              <label className="flex items-center gap-2 text-sm">{t("market.speed")}
                <select className={SELECT} value={p.speed} onChange={(e) => update({ speed: e.target.value as MarketPrefs["speed"] })}>
                  {(["slow", "normal", "fast"] as const).map((s) => <option key={s} value={s}>{t(`market.speeds.${s}`)}</option>)}
                </select>
              </label>
            </div>
          </>
        )}
        {error && <p className="text-destructive text-sm">{error}</p>}
      </CardContent>
    </Card>
  );
}

function Chips({ items, onRemove, removeLabel }: { items: { key: string; label: string }[]; onRemove: (key: string) => void; removeLabel: string }) {
  if (!items.length) return null;
  return (
    <ul className="flex flex-wrap gap-1.5">
      {items.map((i) => (
        <li key={i.key} className="bg-secondary flex items-center gap-1 rounded-full py-0.5 ps-2.5 pe-1 text-xs" title={i.key}>
          {i.label}
          <button type="button" onClick={() => onRemove(i.key)} className="hover:bg-background rounded-full p-0.5" aria-label={`${removeLabel} ${i.label}`}><X className="size-3" /></button>
        </li>
      ))}
    </ul>
  );
}

function KeyField({ has, value, onChange, onSave, onRemove }: { has: boolean; value: string; onChange: (v: string) => void; onSave: () => void; onRemove: () => void }) {
  const { t } = useTranslation();
  if (has) return (
    <span className="flex items-center gap-2 text-xs"><span className="text-muted-foreground">{t("market.keySaved")}</span>
      <Button size="sm" variant="ghost" className="h-7" onClick={onRemove}>{t("market.removeKey")}</Button></span>
  );
  return (
    <form className="flex gap-1" onSubmit={(e) => { e.preventDefault(); if (value.trim()) onSave(); }}>
      <Input type="password" autoComplete="off" className="h-8 w-56" placeholder={t("market.apiKey")} value={value} onChange={(e) => onChange(e.target.value)} aria-label={t("market.apiKey")} />
      <Button size="sm" variant="outline" className="h-8">{t("common.save")}</Button>
    </form>
  );
}
