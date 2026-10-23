import type { ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { Printer, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Estate, EstateHolding } from "@/api";
import { Button } from "@/components/ui/button";
import i18n from "@/i18n";
import { printNow } from "@/lib/print";
import { formatDate, formatMoney } from "@/lib/utils";

const kindLabel = (k: string) => {
  if (i18n.exists(`accounts.type.${k}`)) return i18n.t(`accounts.type.${k}`);
  const words = k.replace(/_/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
};
/** The same column widths in every table, so the amounts line up down the page. */
const WIDTHS: Record<number, string[]> = { 3: ["42%", "36%", "22%"], 4: ["22%", "26%", "26%", "26%"] };

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <section className="break-inside-avoid space-y-2">
      <div>
        <h2 className="text-base font-semibold">{title}</h2>
        {hint && <p className="text-muted-foreground text-xs print:text-black">{hint}</p>}
      </div>
      {children}
    </section>
  );
}

function Table({ head, rows }: { head: string[]; rows: ReactNode[][] }) {
  return (
    <table className="w-full table-fixed border-collapse text-sm">
      {WIDTHS[head.length] && <colgroup>{WIDTHS[head.length].map((w, i) => <col key={i} style={{ width: w }} />)}</colgroup>}
      <thead><tr className="border-b text-left">{head.map((h, i) => <th key={i} scope="col" className={i === head.length - 1 ? "py-1.5 text-end font-medium" : "py-1.5 pe-3 font-medium"}>{h}</th>)}</tr></thead>
      <tbody>{rows.map((r, i) => <tr key={i} className="border-b last:border-0 align-top">{r.map((c, j) => <td key={j} className={j === r.length - 1 ? "py-1.5 text-end tabular-nums" : "py-1.5 pe-3"}>{c}</td>)}</tr>)}</tbody>
    </table>
  );
}

/** The summary itself: what exists, where, and how much. */
export function EstateDoc({ estate: e, owner, letter }: { estate: Estate; owner: string; letter?: string | null }) {
  const { t } = useTranslation();
  const money = (n: number, cur: string) => formatMoney(n, cur);
  const holdings = (list: EstateHolding[]) => list.map((h) => [
    <><div className="font-medium">{h.name}</div>{h.notes && <div className="text-muted-foreground text-xs whitespace-pre-line print:text-black">{h.notes}</div>}</>,
    [h.institution, kindLabel(h.kind)].filter(Boolean).join(" · "), money(h.amount, h.currency),
  ]);
  const empty = !e.accounts.length && !e.debts.length && !e.investments.length && !e.cards.length && !e.people.length && !e.regular.length && !e.documents.length;
  return (
    <article className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">{t("estate.title", { name: owner })}</h1>
        <p className="text-muted-foreground text-sm print:text-black">{t("estate.asOf", { date: formatDate(e.as_of, { day: "numeric", month: "long", year: "numeric" }) })}</p>
      </header>
      {letter && (
        <Section title={t("estate.letter", { name: owner })}>
          <blockquote className="rounded-lg border-s-4 bg-muted/40 p-4 text-sm whitespace-pre-line print:bg-transparent">{letter}</blockquote>
        </Section>
      )}
      {e.totals.length > 0 && (
        <Section title={t("estate.totals")}>
          <Table head={[t("estate.currency"), t("estate.owned"), t("estate.owed"), t("estate.net")]}
            rows={e.totals.map((x) => [x.currency, money(x.owned, x.currency), money(x.owed, x.currency), <strong key="n">{money(x.net, x.currency)}</strong>])} />
        </Section>
      )}
      {e.accounts.length > 0 && <Section title={t("estate.accounts")} hint={t("estate.accountsHint")}><Table head={[t("estate.name"), t("estate.where"), t("estate.balance")]} rows={holdings(e.accounts)} /></Section>}
      {e.investments.length > 0 && <Section title={t("estate.investments")}><Table head={[t("estate.name"), t("estate.where"), t("estate.value")]} rows={holdings(e.investments)} /></Section>}
      {e.debts.length > 0 && <Section title={t("estate.debts")} hint={t("estate.debtsHint")}><Table head={[t("estate.name"), t("estate.where"), t("estate.owedAmount")]} rows={holdings(e.debts)} /></Section>}
      {e.cards.length > 0 && (
        <Section title={t("estate.cards")} hint={t("estate.cardsHint")}>
          <Table head={[t("estate.name"), t("estate.issuer"), t("estate.number")]}
            rows={e.cards.map((c) => [c.name, [c.issuer, c.network?.toUpperCase()].filter(Boolean).join(" · ") || "—", c.last4 ? `•••• ${c.last4}` : "—"])} />
        </Section>
      )}
      {e.people.length > 0 && (
        <Section title={t("estate.people")}>
          <Table head={[t("estate.name"), t("estate.phone"), t("estate.amount")]}
            rows={e.people.map((p) => [p.name, p.phone ?? "—", p.amount > 0 ? t("estate.theyOwe", { amount: money(p.amount, p.currency), name: owner }) : t("estate.ownerOwes", { amount: money(-p.amount, p.currency), name: owner })])} />
        </Section>
      )}
      {e.regular.length > 0 && (
        <Section title={t("estate.regular")} hint={t("estate.regularHint")}>
          <Table head={[t("estate.name"), t("estate.when"), t("estate.amount")]}
            rows={e.regular.map((r) => [
              <><div className="font-medium">{r.name}</div>{r.account && <div className="text-muted-foreground text-xs print:text-black">{r.account}</div>}</>,
              [t(`estate.freq.${r.frequency}`, { count: r.every, defaultValue: r.frequency }), r.next_date && t("estate.next", { date: formatDate(r.next_date, { day: "numeric", month: "short", year: "numeric" }) })].filter(Boolean).join(" · "),
              `${r.kind === "income" ? "+" : ""}${money(r.amount, r.currency)}`,
            ])} />
        </Section>
      )}
      {e.documents.length > 0 && (
        <Section title={t("estate.documents")} hint={t("estate.documentsHint")}>
          <Table head={[t("estate.name"), t("estate.number"), t("estate.keptAt")]}
            rows={e.documents.map((d) => [
              <><div className="font-medium">{d.title}</div><div className="text-muted-foreground text-xs print:text-black">{[d.holder, d.issuer, d.expires_on && t("estate.expires", { date: formatDate(d.expires_on) })].filter(Boolean).join(" · ")}</div></>,
              d.number ?? "—",
              <span key="k" className="whitespace-pre-line">{d.kept_at ?? "—"}{d.files > 0 && <span className="text-muted-foreground block text-xs print:text-black">{t("estate.scans", { count: d.files })}</span>}</span>,
            ])} />
        </Section>
      )}
      {empty && <p className="text-muted-foreground text-sm">{t("estate.empty")}</p>}
      <p className="text-muted-foreground border-t pt-3 text-xs print:text-black">{t("estate.footer")}</p>
    </article>
  );
}

/** Prints the summary on its own, black on white. It leaves the page once printing is done. */
export function printEstate(estate: Estate, owner: string, letter?: string | null) {
  const host = document.createElement("div");
  host.id = "print-root";
  host.className = "bg-white p-2 text-black";
  document.body.appendChild(host);
  document.body.classList.add("printing-sheet");
  const html = document.documentElement;
  const wasDark = html.classList.contains("dark");
  html.classList.remove("dark");
  const root = createRoot(host);
  root.render(<EstateDoc estate={estate} owner={owner} letter={letter} />);
  printNow(() => { root.unmount(); host.remove(); document.body.classList.remove("printing-sheet"); if (wasDark) html.classList.add("dark"); });
}

/** Full-screen, read-only: for the nominee on the sign-in screen, or the member's own preview. */
export function EstateView({ estate, owner, letter, onClose, preview }: { estate: Estate; owner: string; letter?: string | null; onClose: () => void; preview?: boolean }) {
  const { t } = useTranslation();
  return (
    <div role="dialog" aria-modal="true" aria-label={t("estate.title", { name: owner })} className="bg-background fixed inset-0 z-50 overflow-y-auto">
      <div className="bg-background/95 sticky top-0 z-10 border-b backdrop-blur">
        <div className="mx-auto flex max-w-3xl items-center gap-2 px-4 py-3">
          <p className="text-muted-foreground me-auto text-sm">{preview ? t("estate.previewNote") : t("estate.readOnly")}</p>
          <Button variant="outline" size="sm" onClick={() => printEstate(estate, owner, letter)}><Printer />{t("estate.print")}</Button>
          <Button size="sm" onClick={onClose}><X />{t("estate.close")}</Button>
        </div>
      </div>
      <div className="mx-auto max-w-3xl px-4 py-6"><EstateDoc estate={estate} owner={owner} letter={letter} /></div>
    </div>
  );
}
