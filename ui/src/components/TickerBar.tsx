import { useCallback, useEffect, useRef, useState } from "react";
import { AlertCircle, RefreshCw, TrendingDown, TrendingUp } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type MarketPrefs, type Quote } from "@/api";
import { cn, formatDate, formatMoney, formatNumber } from "@/lib/utils";
import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";

/** Fired by settings so the ticker reloads its preferences. */
export const MARKET_CHANGED = "vaulty:market-changed";

const SECONDS_PER_ITEM = { slow: 6, normal: 4, fast: 2.5 } as const;

function price(q: Quote) {
  // Big numbers (BTC in INR) don't need paise; small ones (a $0.12 coin) need more digits.
  const digits = q.price >= 1000 ? 0 : q.price >= 1 ? 2 : 4;
  return q.currency
    ? formatMoney(q.price, q.currency, false, { minimumFractionDigits: digits, maximumFractionDigits: digits })
    : formatNumber(q.price, { maximumFractionDigits: digits });
}

/** Scrolling strip of prices under the header. Only shown when the person turned it on. */
export function TickerBar() {
  const { t } = useTranslation();
  const [prefs, setPrefs] = useState<MarketPrefs | null>(null);
  const [quotes, setQuotes] = useState<Quote[]>([]);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState("");
  const busyRef = useRef(false);
  // Last price per symbol, to flash the ones that moved since the previous refresh.
  const prices = useRef<Record<string, number>>({});
  const [flashes, setFlashes] = useState<Record<string, { dir: "up" | "down"; n: number }>>({});
  const show = useCallback((qs: Quote[]) => {
    const next: Record<string, { dir: "up" | "down"; n: number }> = {};
    for (const q of qs) {
      const k = `${q.kind}:${q.symbol}`, before = prices.current[k];
      if (before !== undefined && before !== q.price) next[k] = { dir: q.price > before ? "up" : "down", n: Date.now() };
      prices.current[k] = q.price;
    }
    setFlashes(next);
    setQuotes(qs);
  }, []);

  const load = useCallback(() => {
    api.market.get().then((v) => setPrefs(v.prefs)).catch(() => setPrefs(null));
    api.market.quotes().then(show).catch(() => {});
  }, []);
  useEffect(() => {
    load();
    window.addEventListener(MARKET_CHANGED, load);
    return () => window.removeEventListener(MARKET_CHANGED, load);
  }, [load]);

  const refresh = useCallback(async () => {
    if (busyRef.current) return;
    busyRef.current = true; setBusy(true);
    try {
      const r = await api.market.refresh();
      show(r.quotes);
      setProblem(r.problems.map((p) => (p.symbol ? `${p.symbol}: ${p.message}` : `${p.source}: ${p.message}`)).join("\n"));
    } catch (e) { setProblem(errorText(e)); }
    finally { busyRef.current = false; setBusy(false); }
  }, []);

  // Refresh on open when the cache is older than the chosen interval, then on that interval.
  useEffect(() => {
    if (!prefs?.enabled) return;
    const ms = prefs.refresh_minutes * 60_000;
    const newest = Math.max(0, ...quotes.map((q) => Date.parse(q.as_of) || 0));
    if (Date.now() - newest > ms) void refresh();
    const id = setInterval(() => void refresh(), ms);
    return () => clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [prefs, refresh]);

  if (!prefs?.enabled) return null;
  const asOf = quotes.length ? new Date(Math.max(...quotes.map((q) => Date.parse(q.as_of) || 0))) : null;
  const duration = Math.max(20, quotes.length * SECONDS_PER_ITEM[prefs.speed]);

  return (
    <div className="bg-muted/40 flex h-9 shrink-0 items-center border-b text-xs">
      <div className="ticker-mask relative min-w-0 flex-1 overflow-hidden" aria-label={t("ticker.label")} role="region">
        {quotes.length === 0
          ? <div className="text-muted-foreground px-6">{busy ? t("ticker.loading") : t("ticker.empty")}</div>
          : (
            <div className="ticker-track flex w-max" style={{ animationDuration: `${duration}s` }}>
              {[0, 1].map((copy) => (
                <ul key={copy} className="flex shrink-0" aria-hidden={copy === 1}>
                  {quotes.map((q) => <TickerItem key={`${q.kind}:${q.symbol}`} q={q} flash={flashes[`${q.kind}:${q.symbol}`]} />)}
                </ul>
              ))}
            </div>
          )}
      </div>
      <div className="text-muted-foreground flex shrink-0 items-center gap-1.5 border-s px-3">
        {problem && <span title={problem} className="text-amber-600 dark:text-amber-400"><AlertCircle className="size-3.5" /><span className="sr-only">{problem}</span></span>}
        {asOf && <span className="tabular-nums">{t("ticker.asOf", { time: formatDate(asOf, { hour: "numeric", minute: "2-digit" }) })}</span>}
        <button onClick={() => void refresh()} disabled={busy} title={t("ticker.refresh")} className="hover:text-foreground rounded p-1 disabled:opacity-50">
          <RefreshCw className={cn("size-3.5", busy && "animate-spin")} /><span className="sr-only">{t("ticker.refresh")}</span>
        </button>
      </div>
    </div>
  );
}

/** A price that just moved flashes green or red for a moment, like a trading screen. */
function TickerItem({ q, flash }: { q: Quote; flash?: { dir: "up" | "down"; n: number } }) {
  const { fade } = useMotion();
  const up = (q.change_pct ?? 0) >= 0;
  const Arrow = up ? TrendingUp : TrendingDown;
  return (
    <li className="flex items-center gap-1.5 whitespace-nowrap px-4" title={`${q.symbol} · ${q.source}`}>
      <span className="font-medium">{q.label}</span>
      <motion.span key={flash?.n ?? 0} className="rounded px-1 tabular-nums"
        initial={{ backgroundColor: flash ? (flash.dir === "up" ? "rgba(16,185,129,.35)" : "rgba(244,63,94,.35)") : "rgba(0,0,0,0)" }}
        animate={{ backgroundColor: "rgba(0,0,0,0)" }} transition={fade({ duration: 1.4, ease: "easeOut" })}>
        {price(q)}
      </motion.span>
      {q.change_pct != null && (
        <span className={cn("flex items-center gap-0.5 tabular-nums", up ? "text-emerald-600 dark:text-emerald-400" : "text-rose-600 dark:text-rose-400")}>
          <Arrow className="size-3" />{formatNumber(Math.abs(q.change_pct), { maximumFractionDigits: 2, minimumFractionDigits: 2 })}%
        </span>
      )}
    </li>
  );
}

/** Whether the ticker is on, so the header can say Vaulty is online for prices. */
export function useMarketEnabled() {
  const [on, setOn] = useState(false);
  useEffect(() => {
    const load = () => { api.market.get().then((v) => setOn(v.prefs.enabled)).catch(() => setOn(false)); };
    load();
    window.addEventListener(MARKET_CHANGED, load);
    return () => window.removeEventListener(MARKET_CHANGED, load);
  }, []);
  return on;
}
