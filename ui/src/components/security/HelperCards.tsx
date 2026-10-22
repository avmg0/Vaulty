import { createRoot } from "react-dom/client";
import type { HelperCard } from "@/api";
import i18n from "@/i18n";
import { printNow } from "@/lib/print";
import { formatDate } from "@/lib/utils";

/** One helper card per sheet: plain, black on white, meant to be kept in an envelope. */
function Sheet({ card }: { card: HelperCard }) {
  const t = i18n.t.bind(i18n);
  const p = { margin: "0 0 3mm" };
  const groups = card.code.split("-");
  const half = Math.ceil(groups.length / 2);
  return (
    <div style={{ fontFamily: "system-ui, sans-serif", color: "#000", background: "#fff", padding: "2mm 4mm", fontSize: "11pt", lineHeight: 1.45, breakAfter: "page" }}>
      <h1 style={{ fontSize: "20pt", fontWeight: 700, margin: 0 }}>{t("helpers.card.title")}</h1>
      <p style={{ margin: "2mm 0 8mm", color: "#444" }}>{t("helpers.card.for", { owner: card.owner, holder: card.label, date: formatDate(new Date()) })}</p>
      <p style={p}>{t("helpers.card.what", { owner: card.owner })}</p>
      <div style={{ border: "2px solid #000", borderRadius: 6, padding: "5mm", margin: "5mm 0", fontFamily: "ui-monospace, monospace", fontSize: "17pt", letterSpacing: "0.08em", textAlign: "center", lineHeight: 1.6 }}>
        {/* Two even rows of groups, so the code never breaks inside a group. */}
        {[groups.slice(0, half), groups.slice(half)].map((row, i) => <div key={i} style={{ whiteSpace: "nowrap" }}>{row.join("-")}</div>)}
      </div>
      <h2 style={{ fontSize: "12pt", fontWeight: 700, margin: "6mm 0 2mm" }}>{t("helpers.card.whenTitle")}</h2>
      <p style={p}>{t("helpers.card.when", { owner: card.owner })}</p>
      <p style={p}>{t("helpers.card.check", { owner: card.owner })}</p>
      <div style={{ border: "1px solid #000", padding: "4mm 5mm", fontSize: "10pt", marginTop: "6mm" }}>
        <strong>{t("helpers.card.keepTitle")}</strong> {t("helpers.card.keep", { owner: card.owner })}
      </div>
    </div>
  );
}

/** Prints each card on its own page. The codes leave the page as soon as printing is done. */
export function printHelperCards(cards: HelperCard[]) {
  const host = document.createElement("div");
  host.id = "print-root";
  document.body.appendChild(host);
  document.body.classList.add("printing-sheet");
  const root = createRoot(host);
  root.render(<>{cards.map((c) => <Sheet key={c.slot} card={c} />)}</>);
  printNow(() => { root.unmount(); host.remove(); document.body.classList.remove("printing-sheet"); });
}
