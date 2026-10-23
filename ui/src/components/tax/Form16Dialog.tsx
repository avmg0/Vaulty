import { useEffect, useMemo, useState, type FormEvent } from "react";
import { FileUp, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type Form16Offer, type TaxInput, type TaxPack, type TaxPlan } from "@/api";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTheme } from "@/hooks/theme";
import { formatMoney } from "@/lib/utils";

const WHOLE: Intl.NumberFormatOptions = { maximumFractionDigits: 0, minimumFractionDigits: 0 };
const EMPTY: TaxInput = { age: 35, salary: "", other_income: "", deductions: {}, alt: [] };
type Row = { key: string; label: string; now: string; amount: number; apply: (i: TaxInput, v: string) => TaxInput };

/** Read a Form 16, see what it says next to what the plan has, and fill in the chosen figures,
 *  the regime and TDS by quarter. Opens the year the form is for. */
export function Form16Dialog({ pack, onClose }: { pack: TaxPack; onClose: (year: string | null) => void }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const money = (n: number) => formatMoney(n, "INR", hideBalances, WHOLE);
  const [path, setPath] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [offer, setOffer] = useState<Form16Offer | null>(null);
  const [plan, setPlan] = useState<TaxPlan | null>(null);
  const [off, setOff] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const name = path?.split(/[\\/]/).pop() ?? "";

  const choose = async () => {
    setError("");
    try { const p = await api.files.pick("tax_form", t("tax.form16.files")); if (p) { setPath(p); setOffer(null); } } catch (err) { setError(errorText(err)); }
  };
  const read = async (e: FormEvent) => {
    e.preventDefault();
    if (!path) return;
    setBusy(true); setError("");
    try {
      const o = await api.tax.readForm16(path, password);
      setPassword("");
      if (o.year) setPlan(await api.tax.plan(pack.country, o.year));
      setOffer(o);
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  const labels = useMemo(() => {
    const m = new Map<string, string>();
    for (const y of pack.years) for (const r of y.regimes) for (const d of r.deductions) if (!m.has(d.id)) m.set(d.id, d.label);
    return m;
  }, [pack]);
  const input: TaxInput = { ...EMPTY, ...plan?.input, deductions: { ...(plan?.input.deductions ?? {}) }, alt: plan?.input.alt ?? [] };
  const rows: Row[] = (offer?.fields ?? []).map((f) => f.field === "deduction"
    ? { key: `d:${f.id}`, label: labels.get(f.id!) ?? f.id!, now: input.deductions[f.id!] ?? "", amount: f.amount, apply: (i, v) => ({ ...i, deductions: { ...i.deductions, [f.id!]: v } }) }
    : f.field === "salary"
      ? { key: "salary", label: t("tax.salary"), now: input.salary, amount: f.amount, apply: (i, v) => ({ ...i, salary: v }) }
      : { key: "other", label: t("tax.other"), now: input.other_income, amount: f.amount, apply: (i, v) => ({ ...i, other_income: v }) });
  const regimeName = offer?.regime && pack.years.find((y) => y.id === offer.year)?.regimes.find((r) => r.id === offer.regime)?.name;
  const toggle = (k: string) => setOff((s) => { const n = new Set(s); if (n.has(k)) n.delete(k); else n.add(k); return n; });
  const on = (k: string) => !off.has(k);

  const fill = async () => {
    if (!offer?.year) return;
    setBusy(true); setError("");
    let next = input;
    for (const r of rows) if (on(r.key)) next = r.apply(next, String(Math.round(r.amount)));
    const note = [t("tax.form16.name"), offer.employer].filter(Boolean).join(" · ");
    try {
      await api.tax.applyForm16(pack.country, offer.year, {
        input: next,
        ...(regimeName && on("regime") ? { regime: offer.regime } : {}),
        payments: on("tds") ? offer.payments.map((p) => ({ date: p.date, amount: p.amount, note: `${note} · Q${p.quarter}` })) : [],
      });
      onClose(offer.year);
    } catch (err) { setError(errorText(err)); setBusy(false); }
  };

  useEffect(() => { setOff(new Set()); }, [offer]);
  const box = (k: string) => <input type="checkbox" className="accent-[var(--primary)] size-4" checked={on(k)} onChange={() => toggle(k)} aria-label={t("tax.form16.use")} />;

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(null); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("tax.form16.title")}</DialogTitle>
          <DialogDescription>{t("tax.form16.desc")}</DialogDescription>
        </DialogHeader>
        {!offer ? (
          <form onSubmit={read} className="grid gap-3">
            <div className="flex flex-wrap items-center gap-2">
              <Button type="button" variant="outline" onClick={choose} autoFocus><FileUp />{path ? t("tax.form16.chooseOther") : t("tax.form16.choose")}</Button>
              {path && <span className="min-w-0 flex-1 truncate text-sm" title={path}>{name}</span>}
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="f16-password">{t("tax.form16.password")}</Label>
              <Input id="f16-password" type="password" autoComplete="off" value={password} onChange={(e) => setPassword(e.target.value)} disabled={!path} />
              <p className="text-muted-foreground text-xs">{t("tax.form16.passwordHint")}</p>
            </div>
            <p className="text-muted-foreground text-xs">{t("tax.form16.stays")}</p>
            {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
            <DialogFooter className="gap-2">
              <Button type="button" variant="outline" onClick={() => onClose(null)}>{t("common.cancel")}</Button>
              <Button disabled={!path || busy}>{busy && <Loader2 className="animate-spin" />}{t("tax.form16.read")}</Button>
            </DialogFooter>
          </form>
        ) : (
          <div className="grid gap-4">
            <div className="text-sm">
              <p className="font-medium">{offer.employer ?? t("tax.form16.unknownEmployer")}</p>
              <p className="text-muted-foreground text-xs">{[offer.tan && `TAN ${offer.tan}`, offer.employee, offer.assessment_year && t("tax.form16.ay", { year: offer.assessment_year })].filter(Boolean).join(" · ")}</p>
            </div>
            {!offer.year ? <p className="text-destructive text-sm" role="alert">{t("tax.form16.noYear", { year: offer.assessment_year ?? "?" })}</p> : (
              <>
                {plan && <p className="text-sm">{t("tax.form16.forYear", { year: plan.year.label })}</p>}
                <table className="w-full text-sm">
                  <thead className="text-muted-foreground text-xs">
                    <tr className="border-b"><th className="w-8 py-1.5"><span className="sr-only">{t("tax.form16.use")}</span></th><th className="py-1.5 text-start font-normal">{t("tax.form16.line")}</th>
                      <th className="py-1.5 text-end font-normal">{t("tax.form16.now")}</th><th className="py-1.5 text-end font-normal">{t("tax.form16.onForm")}</th></tr>
                  </thead>
                  <tbody className="divide-y">
                    {rows.map((r) => (
                      <tr key={r.key} className={on(r.key) ? "" : "text-muted-foreground"}>
                        <td className="py-2">{box(r.key)}</td>
                        <td className="py-2 pe-2">{r.label}</td>
                        <td className="py-2 text-end tabular-nums">{r.now ? money(Number(r.now)) : "—"}</td>
                        <td className="py-2 text-end font-medium tabular-nums">{money(r.amount)}</td>
                      </tr>
                    ))}
                    {offer.payments.length > 0 && (
                      <tr className={on("tds") ? "" : "text-muted-foreground"}>
                        <td className="py-2">{box("tds")}</td>
                        <td className="py-2 pe-2" colSpan={2}>{t("tax.form16.tds", { count: offer.payments.length })}</td>
                        <td className="py-2 text-end font-medium tabular-nums">{money(offer.tds)}</td>
                      </tr>
                    )}
                    {regimeName && (
                      <tr className={on("regime") ? "" : "text-muted-foreground"}>
                        <td className="py-2">{box("regime")}</td>
                        <td className="py-2 pe-2" colSpan={3}>{t("tax.form16.regime", { regime: regimeName })}</td>
                      </tr>
                    )}
                  </tbody>
                </table>
                <ul className="text-muted-foreground list-disc space-y-1 ps-4 text-xs">
                  <li>{t("tax.form16.twoEmployers")}</li>
                  {offer.professional_tax != null && <li>{t("tax.form16.professionalTax", { amount: money(offer.professional_tax) })}</li>}
                  {offer.tax != null && <li>{t("tax.form16.employerTax", { amount: money(offer.tax) })}</li>}
                </ul>
              </>
            )}
            {error && <p className="text-destructive text-sm" role="alert">{error}</p>}
            <DialogFooter className="gap-2">
              <Button type="button" variant="outline" onClick={() => setOffer(null)}>{t("tax.form16.back")}</Button>
              <Button onClick={fill} disabled={busy || !offer.year} autoFocus>{busy && <Loader2 className="animate-spin" />}{t("tax.form16.fill")}</Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
