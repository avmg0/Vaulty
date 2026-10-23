import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Calculator, FileText, Info, Link2, Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type HraInput, type TaxComparison, type TaxEstimate, type TaxInput, type TaxPack, type TaxPlan } from "@/api";
import { Button } from "@/components/ui/button";
import { TaxPaidCard } from "@/components/tax/TaxPaid";
import { HraCard } from "@/components/tax/Hra";
import { LinksDialog } from "@/components/tax/LinksDialog";
import { Form16Dialog } from "@/components/tax/Form16Dialog";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { useDisplayNames, useLocale } from "@/hooks/locale";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate, formatMoney, formatNumber } from "@/lib/utils";
import { motion } from "motion/react";
import { AnimatedMoney } from "@/components/AnimatedMoney";
import { SPRING, useGrowIn, useMotion } from "@/lib/motion";

/** Tax is assessed in whole units; paise/cents only add noise here. */
const WHOLE: Intl.NumberFormatOptions = { maximumFractionDigits: 0, minimumFractionDigits: 0 };
const EMPTY: TaxInput = { age: 35, salary: "", other_income: "", deductions: {}, alt: [] };
const SELECT = "border-input dark:bg-input/30 h-9 rounded-md border bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]";

/** Plans a tax year: the figures (filled from linked categories if wanted), the regime, tax
 *  paid and advance-tax instalments, and HRA with rent receipts. Kept per country and year. */
export function TaxPlanner() {
  const { t } = useTranslation();
  const { country: myCountry } = useLocale();
  const names = useDisplayNames();
  const [countries, setCountries] = useState<string[] | null>(null);
  const [country, setCountry] = useState(myCountry);
  const [pack, setPack] = useState<TaxPack | null>(null);
  const [year, setYear] = useState<string>();
  const [input, setInputRaw] = useState<TaxInput>(EMPTY);
  const [regime, setRegimeRaw] = useState<string | null>(null);
  const [hra, setHraRaw] = useState<HraInput>({});
  const [plan, setPlan] = useState<TaxPlan | null>(null);
  const [linking, setLinking] = useState(false);
  const [reading, setReading] = useState(false);
  const [reload, setReload] = useState(0);
  const [error, setError] = useState("");
  // Only the person's own edits are saved, not what was just loaded.
  const dirty = useRef(false);
  const setInput = (i: TaxInput) => { dirty.current = true; setInputRaw(i); };
  const setRegime = (r: string | null) => { dirty.current = true; setRegimeRaw(r); };
  const setHra = (h: HraInput) => { dirty.current = true; setHraRaw(h); };

  useEffect(() => { api.tax.countries().then(setCountries).catch((e) => setError(errorText(e))); }, []);
  const supported = countries?.includes(country) ?? false;

  useEffect(() => {
    if (!supported) { setPack(null); return; }
    api.tax.describe(country).then((p) => { setPack(p); setYear(p.years.at(-1)?.id); }).catch((e) => setError(errorText(e)));
  }, [country, supported]);

  const refresh = useCallback(() => {
    if (!pack || !year) return Promise.resolve();
    return api.tax.plan(pack.country, year).then((p) => { setPlan(p); setError(""); }).catch((e) => setError(errorText(e)));
  }, [pack, year]);

  // Load the kept plan for the year.
  useEffect(() => {
    if (!pack || !year) return;
    let live = true;
    dirty.current = false;
    api.tax.plan(pack.country, year).then((p) => {
      if (!live) return;
      setPlan(p);
      setInputRaw({ ...EMPTY, ...p.input, deductions: { ...(p.input.deductions ?? {}) }, alt: p.input.alt ?? [] });
      setRegimeRaw(p.regime);
      setHraRaw(p.hra ?? {});
      setError("");
    }).catch((e) => setError(errorText(e)));
    return () => { live = false; };
  }, [pack, year, reload]);

  // Save and recalculate shortly after the person stops typing.
  useEffect(() => {
    if (!pack || !year || !dirty.current) return;
    const id = setTimeout(() => {
      api.tax.savePlan(pack.country, year, { input, regime, hra }).then(refresh).catch((e) => setError(errorText(e)));
    }, 300);
    return () => clearTimeout(id);
  }, [pack, year, input, regime, hra, refresh]);

  if (countries === null) return null;
  if (!supported) return (
    <Card className="mx-auto mt-6 max-w-lg">
      <CardContent className="space-y-4 py-8 text-center">
        <div className="bg-secondary mx-auto grid size-12 place-items-center rounded-xl"><Calculator className="size-5" /></div>
        <div className="font-semibold">{t("tax.unsupported", { country: names.region(country) })}</div>
        <p className="text-muted-foreground text-sm">{t("tax.unsupportedBody")}</p>
        <div className="flex flex-wrap justify-center gap-2">
          {countries.map((c) => <button key={c} onClick={() => setCountry(c)} className="hover:bg-accent rounded-md border px-3 py-1.5 text-sm">{t("tax.tryCountry", { country: names.region(c) })}</button>)}
        </div>
      </CardContent>
    </Card>
  );
  if (!pack) return null;

  const yr = pack.years.find((y) => y.id === year) ?? pack.years[0];
  const hasHra = yr.regimes.some((r) => r.deductions.some((d) => d.id === "hra"));
  return (
    <div className="mx-auto max-w-6xl space-y-6">
      <div className="flex flex-wrap items-end gap-4">
        <div className="space-y-1.5"><Label htmlFor="tax-country">{t("tax.country")}</Label>
          <select id="tax-country" className={SELECT} value={country} onChange={(e) => setCountry(e.target.value)}>
            {[...new Set([...countries, myCountry])].map((c) => <option key={c} value={c}>{names.region(c)}</option>)}
          </select>
        </div>
        <div className="space-y-1.5"><Label htmlFor="tax-year">{t("tax.year")}</Label>
          <select id="tax-year" className={SELECT} value={yr.id} onChange={(e) => setYear(e.target.value)}>
            {pack.years.map((y) => <option key={y.id} value={y.id}>{y.label}</option>)}
          </select>
        </div>
        {pack.country === "IN" && <Button variant="outline" onClick={() => setReading(true)}><FileText />{t("tax.form16.open")}</Button>}
        <p className="text-muted-foreground ms-auto max-w-sm text-xs">{t("tax.private")}</p>
      </div>

      <div className="grid grid-cols-1 items-start gap-6 lg:grid-cols-[minmax(0,22rem)_minmax(0,1fr)]">
        <InputsCard pack={pack} regimes={yr.regimes} input={input} onChange={setInput} plan={plan} onLink={() => setLinking(true)} />
        <div className="min-w-0 space-y-6">
          {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
          {plan && <Results result={plan.comparison} regime={regime} onRegime={setRegime} />}
          {plan && <TaxPaidCard plan={plan} onChange={refresh} />}
          {plan && hasHra && <HraCard plan={plan} hra={hra} onChange={setHra}
            onUse={(amount) => setInput({ ...input, deductions: { ...input.deductions, hra: amount } })} claimed={input.deductions.hra ?? ""} />}
          {plan && <Disclaimer result={plan.comparison} />}
        </div>
      </div>
      {reading && <Form16Dialog pack={pack} onClose={(y) => { setReading(false); if (y) { if (y !== year) setYear(y); else setReload((n) => n + 1); } }} />}
      {linking && plan && <LinksDialog plan={plan} regimes={yr.regimes} onClose={(changed) => { setLinking(false); if (changed) void refresh(); }} />}
    </div>
  );
}

/** "From your records: ₹X · Use", when linked categories add up to something else. */
function FromRecords({ amount, value, cur, onUse }: { amount: number | undefined; value: string; cur: string; onUse: (v: string) => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  if (!amount || amount <= 0) return null;
  const v = String(Math.round(amount));
  const same = Number(value || 0) === Math.round(amount);
  return (
    <div className="text-muted-foreground flex flex-wrap items-center gap-x-2 text-xs">
      <span>{t("tax.fromRecords", { amount: formatMoney(amount, cur, hideBalances, WHOLE) })}</span>
      {!same && <button type="button" className="text-primary font-medium underline-offset-2 hover:underline" onClick={() => onUse(v)}>{t("tax.use")}</button>}
    </div>
  );
}

function InputsCard({ pack, regimes, input, onChange, plan, onLink }: { pack: TaxPack; regimes: TaxPack["years"][number]["regimes"]; input: TaxInput; onChange: (i: TaxInput) => void; plan: TaxPlan | null; onLink: () => void }) {
  const { t } = useTranslation();
  const cur = pack.currency;
  const rec = plan?.records.by ?? {};
  const linked = (plan?.links.length ?? 0) > 0;
  const senior = input.age >= 60;
  // Each deduction once, noting which regimes allow it.
  const deductions = useMemo(() => {
    const seen = new Map<string, { d: (typeof regimes)[number]["deductions"][number]; in: string[] }>();
    for (const r of regimes) for (const d of r.deductions) {
      const e = seen.get(d.id);
      if (e) e.in.push(r.name); else seen.set(d.id, { d, in: [r.name] });
    }
    return [...seen.values()].sort((a, b) => b.in.length - a.in.length);
  }, [regimes]);
  const set = (patch: Partial<TaxInput>) => onChange({ ...input, ...patch });
  const amount = (v: string) => v.replace(/[^\d.]/g, "");

  return (
    <Card>
      <CardHeader><CardTitle>{t("tax.yourYear")}</CardTitle><CardDescription>{t("tax.yourYearDesc")}</CardDescription></CardHeader>
      <CardContent className="space-y-5">
        <div className="bg-muted/50 flex items-start gap-3 rounded-lg p-3 text-xs">
          <Link2 className="text-muted-foreground mt-0.5 size-4 shrink-0" />
          <div className="space-y-1.5">
            <p>{linked ? t("tax.linkedN", { count: plan!.links.length }) : t("tax.linkHint")}</p>
            <Button type="button" size="sm" variant="outline" onClick={onLink} disabled={!plan}>{linked ? t("tax.editLinks") : t("tax.linkCategories")}</Button>
          </div>
        </div>
        <div className="grid grid-cols-[5rem_1fr] gap-3">
          <div className="space-y-1.5"><Label htmlFor="age">{t("tax.age")}</Label>
            <Input id="age" type="number" min={18} max={120} value={input.age} onChange={(e) => set({ age: Math.max(0, Number(e.target.value) || 0) })} />
          </div>
          <div className="space-y-1.5"><Label htmlFor="salary">{t("tax.salary")}</Label>
            <Input id="salary" inputMode="decimal" placeholder="0" value={input.salary} onChange={(e) => set({ salary: amount(e.target.value) })} />
          </div>
        </div>
        <FromRecords amount={rec.salary} value={input.salary} cur={cur} onUse={(v) => set({ salary: v })} />
        <div className="space-y-1.5"><Label htmlFor="other">{t("tax.other")}</Label>
          <Input id="other" inputMode="decimal" placeholder="0" value={input.other_income} onChange={(e) => set({ other_income: amount(e.target.value) })} />
          <FromRecords amount={rec.other_income} value={input.other_income} cur={cur} onUse={(v) => set({ other_income: v })} />
          <p className="text-muted-foreground text-xs">{t("tax.otherHint")}</p>
        </div>
        <Separator />
        <div className="space-y-4">
          <div className="text-sm font-medium">{t("tax.deductions")}</div>
          {deductions.map(({ d, in: allowedIn }) => {
            const alt = input.alt.includes(d.id);
            const cap = alt ? d.cap_alt ?? d.cap : senior ? d.cap_senior ?? d.cap : d.cap;
            return (
              <div key={d.id} className="space-y-1.5">
                <div className="flex items-baseline justify-between gap-2">
                  <Label htmlFor={`d-${d.id}`} className="leading-snug">{d.label}</Label>
                  {allowedIn.length < regimes.length && <span className="text-muted-foreground shrink-0 text-[11px]">{t("tax.onlyIn", { regime: allowedIn.join(", ") })}</span>}
                </div>
                <Input id={`d-${d.id}`} inputMode="decimal" placeholder="0" value={input.deductions[d.id] ?? ""}
                  onChange={(e) => set({ deductions: { ...input.deductions, [d.id]: amount(e.target.value) } })} />
                <FromRecords amount={rec[d.id]} value={input.deductions[d.id] ?? ""} cur={cur} onUse={(v) => set({ deductions: { ...input.deductions, [d.id]: v } })} />
                <div className="text-muted-foreground text-xs">
                  {cap != null && <span>{t("tax.upTo", { amount: formatMoney(cap, cur, false, WHOLE) })}{d.hint ? ". " : ""}</span>}{d.hint}
                </div>
                {d.alt_label && (
                  <label className="flex items-center gap-2 text-xs">
                    <input type="checkbox" checked={alt} className="accent-[var(--primary)]"
                      onChange={(e) => set({ alt: e.target.checked ? [...input.alt, d.id] : input.alt.filter((x) => x !== d.id) })} />
                    {d.alt_label}
                  </label>
                )}
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}

function Results({ result, regime, onRegime }: { result: TaxComparison; regime: string | null; onRegime: (r: string | null) => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const cur = result.currency;
  const money = (n: number) => formatMoney(n, cur, hideBalances, WHOLE);
  const best = result.estimates.find((e) => e.regime === result.recommended);
  const anyIncome = result.estimates.some((e) => e.gross > 0);

  return (
    <>
      <div className={cn("rounded-xl border p-5", anyIncome && "border-primary/40 bg-primary/5")}>
        <div className="flex items-start gap-3">
          <Sparkles className="text-primary mt-0.5 size-5 shrink-0" />
          <div className="space-y-1">
            <div className="text-lg font-semibold tracking-tight">
              {!anyIncome ? t("tax.enterIncome")
                : result.saving > 0 && best ? t("tax.saves", { regime: best.name, amount: money(result.saving) })
                : t("tax.same")}
            </div>
            {anyIncome && best && <p className="text-muted-foreground text-sm">{t("tax.choiceNote")}</p>}
            {anyIncome && regime && regime !== result.recommended && result.saving > 0 && (
              <p className="text-sm">{t("tax.chosenCosts", { regime: result.estimates.find((e) => e.regime === regime)?.name, amount: money(result.saving) })}</p>
            )}
          </div>
        </div>
        <div className="mt-4 flex flex-wrap items-center gap-2 text-sm" role="radiogroup" aria-label={t("tax.planUnder")}>
          <span className="text-muted-foreground me-1">{t("tax.planUnder")}</span>
          {[null, ...result.estimates.map((e) => e.regime)].map((r) => {
            const on = (regime ?? null) === r;
            return (
              <button key={r ?? "auto"} type="button" role="radio" aria-checked={on} onClick={() => onRegime(r)}
                className={cn("rounded-full border px-3 py-1 text-xs transition-colors", on ? "bg-primary text-primary-foreground border-primary" : "hover:bg-accent")}>
                {r === null ? t("tax.whicheverLower") : result.estimates.find((e) => e.regime === r)?.name}
              </button>
            );
          })}
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        {result.estimates.map((e) => <RegimeCard key={e.regime} e={e} best={anyIncome && e.regime === result.recommended && result.saving > 0} money={money} cur={cur} />)}
      </div>
    </>
  );
}

function Disclaimer({ result }: { result: TaxComparison }) {
  const { t } = useTranslation();
  return (
    <>

      <div className="text-muted-foreground flex gap-2 text-xs leading-relaxed">
        <Info className="mt-0.5 size-3.5 shrink-0" />
        <div className="space-y-1">
          <p>{t("tax.disclaimer", { date: formatDate(result.last_checked, { dateStyle: "medium" }) })}</p>
          {result.notes && <p>{result.notes}</p>}
          <p>{t("tax.sources")} {result.sources.map((s, i) => <span key={s}>{i > 0 && ", "}<a href={s} target="_blank" rel="noreferrer" className="underline underline-offset-2">{new URL(s).hostname.replace(/^www\./, "")}</a></span>)}</p>
        </div>
      </div>
    </>
  );
}

function RegimeCard({ e, best, money, cur }: { e: TaxEstimate; best: boolean; money: (n: number) => string; cur: string }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const { move } = useMotion();
  const deducted = e.deductions.reduce((s, d) => s + d.allowed, 0);
  const capped = e.deductions.filter((d) => d.claimed > d.allowed);
  const rows: [string, number, boolean?][] = [
    [t("tax.gross"), e.gross],
    [t("tax.standardDeduction"), -e.standard_deduction],
    [t("tax.deductionsAllowed"), -deducted],
    [t("tax.taxable"), e.taxable, true],
    [t("tax.slabTax"), e.slab_tax],
    [t("tax.rebate"), -e.rebate],
    [t("tax.surcharge", { rate: formatNumber(e.surcharge_rate) }), e.surcharge],
    [t("tax.cess"), e.cess],
  ];
  return (
    <Card className={cn("gap-4 transition-shadow duration-300", best && "ring-primary ring-2")}>
      <CardHeader>
        <div className="flex items-center justify-between gap-2">
          <CardTitle>{e.name}</CardTitle>
          {/* One badge that hops to whichever regime is cheaper. */}
          {best && <motion.div layoutId="tax-lower" transition={move(SPRING)}><Badge>{t("tax.lower")}</Badge></motion.div>}
        </div>
        <div className="pt-2">
          <AnimatedMoney value={e.total} currency={cur} hidden={hideBalances} formatOptions={WHOLE} className="text-3xl font-semibold tracking-tight tabular-nums" />
          <div className="text-muted-foreground text-xs">{t("tax.effective", { rate: formatNumber(e.effective_rate, { maximumFractionDigits: 1 }) })}</div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <SlabBar e={e} />
        <dl className="space-y-1.5 text-sm">
          {rows.filter(([, v], i) => i === 0 || i === 3 || i === 4 || v !== 0).map(([label, v, strong]) => (
            <div key={label} className={cn("flex justify-between gap-3", strong && "border-t pt-1.5 font-medium")}>
              <dt className={cn(!strong && "text-muted-foreground")}>{label}</dt>
              <dd className="tabular-nums">{v < 0 ? `− ${money(-v)}` : money(v)}</dd>
            </div>
          ))}
        </dl>
        {capped.length > 0 && (
          <p className="text-muted-foreground text-xs">{t("tax.capped", { items: capped.map((d) => d.label).join(", ") })}</p>
        )}
      </CardContent>
    </Card>
  );
}

/** Taxable income split by band: width is the income in the band, shade is its rate. */
function SlabBar({ e }: { e: TaxEstimate }) {
  const { t } = useTranslation();
  const grown = useGrowIn();
  const used = e.bands.filter((b) => b.taxed > 0);
  const max = Math.max(...e.bands.map((b) => b.rate), 1);
  // Every band is always rendered (empty ones at zero width) so bands grow and shrink as you type.
  return (
    <div className="space-y-1.5">
      <div className="bg-muted flex h-2 overflow-hidden rounded-full" role="img" aria-label={t("tax.bandsLabel")}>
        {e.bands.map((b) => (
          <div key={b.from} style={{ flexGrow: grown ? b.taxed : 0, flexBasis: 0, opacity: b.rate === 0 ? 0.18 : 0.3 + (0.7 * b.rate) / max }}
            className={cn("bg-primary transition-[flex-grow,opacity] duration-500 ease-out", b.taxed > 0 && "border-background border-e last:border-e-0")}
            title={`${formatNumber(b.rate)}%`} />
        ))}
      </div>
      <div className="text-muted-foreground flex justify-between text-[11px] tabular-nums">
        <span>{used.length ? `${formatNumber(used[0].rate)}%` : ""}</span><span>{used.length ? `${formatNumber(used.at(-1)!.rate)}%` : ""}</span>
      </div>
    </div>
  );
}
