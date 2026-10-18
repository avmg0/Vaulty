import { createRoot } from "react-dom/client";
import i18n from "@/i18n";
import { formatDate } from "@/lib/utils";

/** The sheet itself: plain, black on white, meant for paper. */
function Sheet({ name, recoveryKey, child }: { name: string; recoveryKey?: string; child?: boolean }) {
  const t = i18n.t.bind(i18n);
  const line = <div style={{ borderBottom: "1px solid #999", height: "2.2em" }} />;
  return (
    <div style={{ fontFamily: "system-ui, sans-serif", color: "#000", background: "#fff", padding: "12mm 14mm", fontSize: "11pt", lineHeight: 1.45 }}>
      <h1 style={{ fontSize: "20pt", fontWeight: 700, margin: 0 }}>{t("sheet.title")}</h1>
      <p style={{ margin: "2mm 0 8mm", color: "#444" }}>{t("sheet.for", { name, date: formatDate(new Date()) })}</p>
      <h2 style={{ fontSize: "12pt", fontWeight: 700, margin: "0 0 2mm" }}>{t("sheet.keyTitle")}</h2>
      {recoveryKey
        ? <div style={{ border: "2px solid #000", borderRadius: 6, padding: "5mm", fontFamily: "ui-monospace, monospace", fontSize: "18pt", letterSpacing: "0.12em", textAlign: "center" }}>{recoveryKey}</div>
        : <div style={{ border: "2px solid #000", borderRadius: 6, padding: "4mm 5mm" }}>{line}<p style={{ fontSize: "9pt", color: "#555", margin: "1mm 0 0" }}>{t("sheet.writeKey")}</p></div>}
      <p style={{ margin: "3mm 0 8mm" }}>{child ? t("sheet.useKeyChild") : t("sheet.useKey")}</p>
      <h2 style={{ fontSize: "12pt", fontWeight: 700, margin: "0 0 2mm" }}>{t("sheet.backupsTitle")}</h2>
      <p style={{ margin: "0 0 1mm" }}>{t("sheet.backupHint")}</p>{line}
      <p style={{ margin: "4mm 0 1mm" }}>{t("sheet.backupWhere")}</p>{line}
      <p style={{ margin: "4mm 0 8mm", fontSize: "10pt", color: "#333" }}>{t("sheet.backupNote")}</p>
      <div style={{ border: "1px solid #000", padding: "4mm 5mm", fontSize: "10pt" }}>
        <strong>{t("sheet.keepSafe")}</strong> {t("sheet.keepSafeBody")}
      </div>
    </div>
  );
}

/** Prints a recovery sheet (with the key if given, otherwise with space to write it). */
export function printRecoverySheet(opts: { name: string; recoveryKey?: string; child?: boolean }) {
  const host = document.createElement("div");
  host.id = "print-root";
  document.body.appendChild(host);
  document.body.classList.add("printing-sheet");
  const root = createRoot(host);
  root.render(<Sheet {...opts} />);
  const cleanup = () => {
    window.removeEventListener("afterprint", cleanup);
    // The key must not linger in the page after printing.
    root.unmount(); host.remove(); document.body.classList.remove("printing-sheet");
  };
  window.addEventListener("afterprint", cleanup);
  // Let React paint before the print dialog opens.
  setTimeout(() => window.print(), 50);
}
