import { useEffect, useMemo, useState } from "react";
import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type Category, type TaxPack, type TaxPlan } from "@/api";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

const SELECT = "border-input dark:bg-input/30 h-9 w-full rounded-md border bg-transparent px-2 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]";

/** Which categories count towards what: income categories towards salary or other income,
 *  spending categories towards a deduction. */
export function LinksDialog({ plan, regimes, onClose }: { plan: TaxPlan; regimes: TaxPack["years"][number]["regimes"]; onClose: (changed: boolean) => void }) {
  const { t } = useTranslation();
  const [cats, setCats] = useState<Category[] | null>(null);
  const [map, setMap] = useState<Record<string, string>>(() => Object.fromEntries(plan.links.map((l) => [l.category_id, l.target])));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => { api.categories.getAll().then(setCats).catch((e) => setError(errorText(e))); }, []);

  const deductions = useMemo(() => {
    const seen = new Map<string, string>();
    for (const r of regimes) for (const d of r.deductions) if (!seen.has(d.id)) seen.set(d.id, d.label);
    return [...seen];
  }, [regimes]);
  const income: [string, string][] = [["salary", t("tax.salary")], ["other_income", t("tax.other")]];
  const groups: [string, Category[]][] = cats ? [
    [t("tax.incomeCategories"), cats.filter((c) => c.type === "income")],
    [t("tax.spendingCategories"), cats.filter((c) => c.type === "expense")],
  ] : [];

  const save = () => {
    setBusy(true);
    const links = Object.entries(map).filter(([, v]) => v).map(([category_id, target]) => ({ category_id, target }));
    api.tax.setLinks(plan.country, plan.year.id, links).then(() => onClose(true)).catch((e) => { setError(errorText(e)); setBusy(false); });
  };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(false); }}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t("tax.linksTitle")}</DialogTitle>
          <DialogDescription>{t("tax.linksDesc", { year: plan.year.label })}</DialogDescription>
        </DialogHeader>
        <div className="-mx-1 max-h-[60vh] space-y-5 overflow-y-auto px-1">
          {!cats && !error && <Loader2 className="text-muted-foreground mx-auto size-5 animate-spin" />}
          {groups.map(([title, list]) => list.length > 0 && (
            <section key={title} className="space-y-2">
              <h3 className="text-muted-foreground text-xs font-medium">{title}</h3>
              <ul className="space-y-2">
                {list.map((c) => (
                  <li key={c.id} className="grid grid-cols-[minmax(0,1fr)_minmax(0,14rem)] items-center gap-3">
                    <label htmlFor={`link-${c.id}`} className="truncate text-sm">{c.icon && <span aria-hidden className="me-1.5">{c.icon}</span>}{c.name}</label>
                    <select id={`link-${c.id}`} className={SELECT} value={map[c.id] ?? ""} onChange={(e) => setMap({ ...map, [c.id]: e.target.value })}>
                      <option value="">{t("tax.notCounted")}</option>
                      {(c.type === "income" ? income : deductions).map(([id, label]) => <option key={id} value={id}>{label}</option>)}
                    </select>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
        {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
        <DialogFooter className="gap-2">
          <Button type="button" variant="outline" onClick={() => onClose(false)}>{t("common.cancel")}</Button>
          <Button onClick={save} disabled={busy || !cats}>{busy && <Loader2 className="animate-spin" />}{t("common.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
