import { useEffect, useRef, useState } from "react";
import { Check, ChevronsUpDown, Search, X } from "lucide-react";
import { api, type Institution } from "@/api";
import { InstitutionBadge } from "@/components/InstitutionBadge";
import { cn } from "@/lib/utils";

const KIND_LABEL: Record<Institution["kind"], string> = { bank: "Bank", card: "Card issuer", broker: "Broker", wallet: "Wallet" };

/** Searchable picker over the bundled bank / institution directory. */
export function InstitutionPicker({ value, onChange, currency }: { value: Institution | null; onChange: (i: Institution | null) => void; currency?: string }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [results, setResults] = useState<Institution[]>([]);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const t = setTimeout(() => { api.institutions.search(q, currency, 30).then(setResults).catch(() => setResults([])); }, 120);
    return () => clearTimeout(t);
  }, [q, currency, open]);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => { if (!box.current?.contains(e.target as Node)) setOpen(false); };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  return (
    <div ref={box} className="relative">
      <button type="button" onClick={() => setOpen((o) => !o)} aria-haspopup="listbox" aria-expanded={open}
        className="border-input flex h-9 w-full items-center gap-2 rounded-md border bg-transparent px-2 text-left text-sm shadow-xs outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50">
        {value ? (
          <>
            <InstitutionBadge short={value.short} institutionLogo={value.logo} name={value.name} className="size-6 rounded-md" />
            <span className="flex-1 truncate">{value.name}</span>
            <span role="button" tabIndex={0} aria-label="Clear" onClick={(e) => { e.stopPropagation(); onChange(null); }}
              className="text-muted-foreground hover:text-foreground"><X className="size-4" /></span>
          </>
        ) : (
          <>
            <span className="text-muted-foreground flex-1 px-1">Choose a bank (optional)</span>
            <ChevronsUpDown className="text-muted-foreground size-4" />
          </>
        )}
      </button>
      {open && (
        <div className="bg-popover absolute z-30 mt-1 w-full rounded-md border shadow-md">
          <div className="flex items-center gap-2 border-b px-2">
            <Search className="text-muted-foreground size-4" />
            <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search banks, cards, brokers…"
              className="h-9 flex-1 bg-transparent text-sm outline-none" />
          </div>
          <ul role="listbox" className="max-h-64 overflow-y-auto p-1">
            {results.length === 0 && <li className="text-muted-foreground px-2 py-3 text-center text-sm">No match. You can leave this empty.</li>}
            {results.map((i) => (
              <li key={i.id} role="option" aria-selected={value?.id === i.id}>
                <button type="button" onClick={() => { onChange(i); setOpen(false); setQ(""); }}
                  className={cn("hover:bg-accent flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left", value?.id === i.id && "bg-accent")}>
                  <InstitutionBadge short={i.short} institutionLogo={i.logo} name={i.name} className="size-7 rounded-md" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm">{i.name}</span>
                    <span className="text-muted-foreground block text-xs">{KIND_LABEL[i.kind]} · {i.country === "XX" ? "International" : i.country}</span>
                  </span>
                  {value?.id === i.id && <Check className="size-4" />}
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
