import { useEffect, useState } from "react";
import { Check, Landmark, LineChart, Loader2, Shield, UserCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, DATA_RESTORED, errorText, type NomineeItem } from "@/api";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const ICON = { account: Landmark, holding: LineChart, policy: Shield };

/** Every account, investment and policy with its nominee; missing ones first, filled in place. */
export function NomineesPanel({ onCount }: { onCount?: (missing: number) => void }) {
  const { t } = useTranslation();
  const [data, setData] = useState<{ items: NomineeItem[]; missing: number } | null>(null);
  const [error, setError] = useState("");
  const load = () => api.nominees.list().then((d) => { setData(d); onCount?.(d.missing); setError(""); }).catch((e) => setError(errorText(e)));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { load(); window.addEventListener(DATA_RESTORED, load); return () => window.removeEventListener(DATA_RESTORED, load); }, []);
  if (!data) return error ? <p role="alert" className="text-destructive text-sm">{error}</p> : null;
  return (
    <div className="space-y-4">
      <div className={cn("flex items-start gap-3 rounded-xl border p-4 text-sm", data.missing === 0 && data.items.length > 0 && "border-emerald-600/40 bg-emerald-600/5")}>
        <UserCheck className="text-muted-foreground mt-0.5 size-5 shrink-0" />
        <div className="space-y-1">
          <p className="font-medium">{data.items.length === 0 ? t("nominees.none") : data.missing === 0 ? t("nominees.allSet") : t("nominees.missing", { count: data.missing })}</p>
          <p className="text-muted-foreground">{t("nominees.why")}</p>
        </div>
      </div>
      {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
      {data.items.length > 0 && (
        <ul className="divide-y rounded-xl border">
          {data.items.map((i) => <Row key={`${i.kind}:${i.id}`} item={i} onSaved={load} />)}
        </ul>
      )}
    </div>
  );
}

function Row({ item, onSaved }: { item: NomineeItem; onSaved: () => void }) {
  const { t } = useTranslation();
  const [value, setValue] = useState(item.nominee ?? "");
  const [state, setState] = useState<"idle" | "saving" | "saved">("idle");
  const [error, setError] = useState("");
  const Icon = ICON[item.kind];
  const changed = value.trim() !== (item.nominee ?? "");
  const save = async () => {
    if (!changed) return;
    setState("saving"); setError("");
    try { await api.nominees.set(item.kind, item.id, value.trim() || null); setState("saved"); onSaved(); }
    catch (err) { setError(errorText(err)); setState("idle"); }
  };
  const id = `nom-${item.kind}-${item.id}`;
  return (
    <li className="grid gap-2 p-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,18rem)_auto] sm:items-center">
      <div className="flex min-w-0 items-center gap-3">
        <div className="bg-secondary grid size-9 shrink-0 place-items-center rounded-lg"><Icon className="size-4" aria-hidden /></div>
        <div className="min-w-0">
          <label htmlFor={id} className="block truncate text-sm font-medium">{item.name}</label>
          <p className="text-muted-foreground truncate text-xs">{[t(`nominees.kinds.${item.kind}`), item.provider].filter(Boolean).join(" · ")}</p>
        </div>
      </div>
      <Input id={id} value={value} maxLength={200} placeholder={t("nominees.placeholder")} aria-invalid={!!error || undefined}
        className={cn(!item.nominee && "border-amber-500/60")}
        onChange={(e) => { setValue(e.target.value); setState("idle"); }}
        onKeyDown={(e) => { if (e.key === "Enter") void save(); }} onBlur={() => void save()} />
      <div className="flex h-9 min-w-20 items-center justify-end">
        {state === "saving" ? <Loader2 className="text-muted-foreground size-4 animate-spin" />
          : state === "saved" && !changed ? <span className="flex items-center gap-1 text-xs text-emerald-700 dark:text-emerald-400" role="status"><Check className="size-3.5" />{t("nominees.saved")}</span>
          : changed ? <Button size="sm" variant="outline" onClick={() => void save()}>{t("common.save")}</Button> : null}
      </div>
      {error && <p role="alert" className="text-destructive text-xs sm:col-span-3">{error}</p>}
    </li>
  );
}
