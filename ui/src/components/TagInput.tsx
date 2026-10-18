import { useState, type KeyboardEvent } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";

/** Tags as chips. Enter or comma adds the typed tag; Backspace in an empty box removes the last. */
export function TagInput({ id, value, onChange, suggestions = [] }: { id?: string; value: string[]; onChange: (v: string[]) => void; suggestions?: string[] }) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState("");
  const add = (raw: string) => {
    const tag = raw.trim().replace(/^#/, "").trim();
    if (tag && !value.some((v) => v.toLowerCase() === tag.toLowerCase())) onChange([...value, tag.slice(0, 40)]);
    setDraft("");
  };
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || e.key === ",") { if (draft.trim()) { e.preventDefault(); add(draft); } }
    else if (e.key === "Backspace" && !draft && value.length) onChange(value.slice(0, -1));
  };
  const matches = draft ? suggestions.filter((s) => s.toLowerCase().startsWith(draft.toLowerCase()) && !value.includes(s)).slice(0, 5) : [];
  return (
    <div className="relative">
      <div className={cn("border-input dark:bg-input/30 flex min-h-9 flex-wrap items-center gap-1 rounded-md border bg-transparent px-2 py-1 shadow-xs",
        "focus-within:border-ring focus-within:ring-ring/50 focus-within:ring-[3px]")}>
        {value.map((tag) => (
          <span key={tag} className="bg-secondary flex items-center gap-0.5 rounded px-1.5 py-0.5 text-xs">
            #{tag}
            <button type="button" className="text-muted-foreground hover:text-foreground" onClick={() => onChange(value.filter((v) => v !== tag))} aria-label={t("tags.remove", { tag })}>
              <X className="size-3" />
            </button>
          </span>
        ))}
        <input id={id} value={draft} onChange={(e) => setDraft(e.target.value.replace(",", ""))} onKeyDown={onKey} onBlur={() => draft && add(draft)}
          placeholder={value.length ? "" : t("tags.placeholder")} className="min-w-24 flex-1 bg-transparent py-0.5 text-sm outline-none" />
      </div>
      {matches.length > 0 && (
        <div className="bg-popover absolute z-30 mt-1 w-48 rounded-md border p-1 shadow-md">
          {matches.map((m) => (
            <button key={m} type="button" onMouseDown={(e) => { e.preventDefault(); add(m); }} className="hover:bg-accent block w-full rounded-sm px-2 py-1 text-left text-sm">#{m}</button>
          ))}
        </div>
      )}
    </div>
  );
}

export const splitTags = (s?: string | null) => (s ? s.split(",").map((x) => x.trim()).filter(Boolean) : []);
