import { useState } from "react";
import { Check, Copy, Download } from "lucide-react";
import { Button } from "@/components/ui/button";

export function BackupCodes({ codes, userName }: { codes: string[]; userName: string }) {
  const [copied, setCopied] = useState(false);
  const text = `Vaulty backup codes for ${userName}\nEach code works once. Keep them somewhere safe.\n\n${codes.join("\n")}\n`;
  const copy = async () => { await navigator.clipboard.writeText(text); setCopied(true); setTimeout(() => setCopied(false), 1500); };
  const download = () => {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
    a.download = `vaulty-backup-codes-${userName.toLowerCase().replace(/\s+/g, "-")}.txt`;
    a.click();
    URL.revokeObjectURL(a.href);
  };
  return (
    <div className="space-y-3">
      <div className="bg-muted grid grid-cols-2 gap-x-6 gap-y-1.5 rounded-lg p-4 font-mono text-sm">
        {codes.map((c) => <div key={c}>{c}</div>)}
      </div>
      <div className="flex gap-2">
        <Button variant="outline" size="sm" onClick={copy}>{copied ? <Check /> : <Copy />}{copied ? "Copied" : "Copy"}</Button>
        <Button variant="outline" size="sm" onClick={download}><Download />Download</Button>
        <Button variant="outline" size="sm" onClick={() => window.print()}>Print</Button>
      </div>
    </div>
  );
}
