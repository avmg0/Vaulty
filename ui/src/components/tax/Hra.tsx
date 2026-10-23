import { createRoot } from "react-dom/client";
import { useTranslation } from "react-i18next";
import { AlertTriangle, Printer } from "lucide-react";
import type { HraInput, TaxPlan } from "@/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTheme } from "@/hooks/theme";
import { formatDate, formatMoney, formatNumber } from "@/lib/utils";
import { printNow } from "@/lib/print";

const WHOLE: Intl.NumberFormatOptions = { maximumFractionDigits: 0, minimumFractionDigits: 0 };
const amount = (v: string) => v.replace(/[^\d.]/g, "");

/** The months rent was paid, "YYYY-MM", within the tax year. */
export function rentMonths(hra: HraInput, year: TaxPlan["year"]): string[] {
  const from = hra.from || year.start.slice(0, 7);
  const to = hra.to || year.end.slice(0, 7);
  const out: string[] = [];
  let [y, m] = from.split("-").map(Number);
  while (out.length < 12 && `${y}-${String(m).padStart(2, "0")}` <= to) {
    out.push(`${y}-${String(m).padStart(2, "0")}`);
    m += 1; if (m > 12) { m = 1; y += 1; }
  }
  return out;
}

/** House rent allowance: the exempt part, and rent receipts to hand to the employer. */
export function HraCard({ plan, hra, onChange, onUse, claimed }: { plan: TaxPlan; hra: HraInput; onChange: (h: HraInput) => void; onUse: (amount: string) => void; claimed: string }) {
  const { t } = useTranslation();
  const { hideBalances } = useTheme();
  const cur = plan.currency;
  const money = (n: number) => formatMoney(n, cur, hideBalances, WHOLE);
  const set = (patch: Partial<HraInput>) => onChange({ ...hra, ...patch });
  const r = plan.hra_result;
  const exempt = String(Math.round(r.exempt));
  const canPrint = Number(hra.monthly_rent || 0) > 0 && !!hra.landlord?.trim() && !!hra.tenant?.trim() && r.months > 0;
  const field = (id: keyof HraInput, label: string, props: React.ComponentProps<typeof Input> = {}) => (
    <div className="space-y-1.5"><Label htmlFor={`hra-${id}`}>{label}</Label>
      <Input id={`hra-${id}`} value={(hra[id] as string | undefined) ?? ""} {...props}
        onChange={(e) => set({ [id]: props.inputMode === "decimal" ? amount(e.target.value) : e.target.value })} />
    </div>
  );
  const money0 = { inputMode: "decimal" as const, placeholder: "0" };

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("tax.hraTitle")}</CardTitle>
        <CardDescription>{t("tax.hraDesc")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="grid gap-3 sm:grid-cols-3">
          {field("salary", t("tax.hraSalary"), money0)}
          {field("received", t("tax.hraReceived"), money0)}
          {field("monthly_rent", t("tax.hraRent"), money0)}
        </div>
        <div className="grid items-end gap-3 sm:grid-cols-3">
          {field("from", t("tax.hraFrom"), { type: "month", min: plan.year.start.slice(0, 7), max: plan.year.end.slice(0, 7) })}
          {field("to", t("tax.hraTo"), { type: "month", min: plan.year.start.slice(0, 7), max: plan.year.end.slice(0, 7) })}
          <label className="flex h-9 items-center gap-2 text-sm">
            <input type="checkbox" className="accent-[var(--primary)]" checked={!!hra.metro} onChange={(e) => set({ metro: e.target.checked })} />
            {t("tax.hraMetro")}
          </label>
        </div>
        <p className="text-muted-foreground text-xs">{t("tax.hraMetroHint")}</p>

        <div className="bg-muted/40 flex flex-wrap items-center justify-between gap-3 rounded-lg p-3">
          <div className="text-sm">
            <div className="font-medium">{t("tax.hraExempt", { amount: money(r.exempt) })}</div>
            <div className="text-muted-foreground text-xs">{t("tax.hraRentYear", { amount: money(r.rent), count: r.months, months: formatNumber(r.months) })}</div>
          </div>
          {r.exempt > 0 && Number(claimed || 0) !== Math.round(r.exempt) && (
            <Button size="sm" variant="outline" onClick={() => onUse(exempt)}>{t("tax.hraUse")}</Button>
          )}
        </div>

        <section className="space-y-3" aria-labelledby="receipts-h">
          <h2 id="receipts-h" className="text-sm font-medium">{t("tax.receiptsTitle")}</h2>
          <div className="grid gap-3 sm:grid-cols-2">
            {field("tenant", t("tax.tenant"), { maxLength: 200, autoComplete: "name" })}
            {field("landlord", t("tax.landlord"), { maxLength: 200 })}
            {field("landlord_pan", t("tax.landlordPan"), { maxLength: 10, className: "uppercase", placeholder: "ABCDE1234F" })}
            {field("address", t("tax.rentedAddress"), { maxLength: 200 })}
          </div>
          {r.pan_needed && !hra.landlord_pan?.trim() && (
            <p className="flex items-start gap-2 text-xs text-amber-700 dark:text-amber-400"><AlertTriangle className="mt-0.5 size-3.5 shrink-0" />{t("tax.panNeeded")}</p>
          )}
          <div className="flex flex-wrap items-center gap-3">
            <Button variant="outline" disabled={!canPrint} onClick={() => printReceipts(plan, hra)}><Printer />{t("tax.printReceipts", { count: r.months })}</Button>
            {!canPrint && <span className="text-muted-foreground text-xs">{t("tax.receiptsNeed")}</span>}
          </div>
        </section>
      </CardContent>
    </Card>
  );
}

function Receipts({ plan, hra }: { plan: TaxPlan; hra: HraInput }) {
  const { t } = useTranslation();
  const rent = Number(hra.monthly_rent || 0);
  const money = formatMoney(rent, plan.currency, false, WHOLE);
  const months = rentMonths(hra, plan.year);
  return (
    <div className="space-y-6 text-sm">
      {months.map((m, i) => {
        const [y, mm] = m.split("-").map(Number);
        const last = new Date(y, mm, 0);
        const month = formatDate(`${m}-01`, { month: "long", year: "numeric" });
        return (
          <article key={m} className="break-inside-avoid rounded border border-black p-5">
            <div className="flex items-baseline justify-between border-b border-black pb-2">
              <h1 className="text-lg font-semibold">{t("tax.receipt")}</h1>
              <span>{t("tax.receiptNo", { n: i + 1 })}</span>
            </div>
            <p className="mt-3 leading-relaxed">{t("tax.receiptBody", { tenant: hra.tenant, amount: money, address: hra.address || "—", month })}</p>
            <div className="mt-6 flex items-end justify-between gap-6">
              <div className="space-y-1">
                <div>{t("tax.receiptDate", { date: formatDate(last) })}</div>
                <div>{t("tax.landlord")}: {hra.landlord}</div>
                {hra.landlord_pan && <div>{t("tax.landlordPan")}: {hra.landlord_pan.toUpperCase()}</div>}
              </div>
              <div className="flex items-end gap-4">
                <div className="grid size-20 place-items-center border border-dashed border-black p-1 text-center text-[10px] leading-tight">{t("tax.revenueStamp")}</div>
                <div className="w-44 border-t border-black pt-1 text-center text-xs">{t("tax.landlordSignature")}</div>
              </div>
            </div>
          </article>
        );
      })}
    </div>
  );
}

/** Prints one receipt a month, black on white, on its own. */
export function printReceipts(plan: TaxPlan, hra: HraInput) {
  const host = document.createElement("div");
  host.id = "print-root";
  host.className = "bg-white p-2 text-black";
  document.body.appendChild(host);
  document.body.classList.add("printing-sheet");
  const html = document.documentElement;
  const wasDark = html.classList.contains("dark");
  html.classList.remove("dark");
  const root = createRoot(host);
  root.render(<Receipts plan={plan} hra={hra} />);
  printNow(() => { root.unmount(); host.remove(); document.body.classList.remove("printing-sheet"); if (wasDark) html.classList.add("dark"); });
}
