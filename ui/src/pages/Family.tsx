import { useEffect, useMemo, useState } from "react";
import { Skeleton, SkeletonRows } from "@/components/Skeleton";
import { ChevronLeft, ChevronRight, Globe, Layers, Loader2, Lock, Settings2, Users, UsersRound } from "lucide-react";
import { api, errorText, type FamilyItem, type FamilyMember, type FeedView, type SharingPrefs, type Transaction } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { UserAvatar } from "@/components/UserAvatar";
import { useSession } from "@/hooks/session";
import { useTheme } from "@/hooks/theme";
import { Segmented } from "@/components/Segmented";
import { AnimatedMoney } from "@/components/AnimatedMoney";
import { AnimatedItem, AnimatedList } from "@/components/AnimatedList";
import { motion } from "motion/react";
import { useMotion } from "@/lib/motion";
import { cn, formatMoney, getNumberLocale } from "@/lib/utils";
import { TransactionRow } from "./Transactions";
import { FamilyMembers } from "./FamilyMembers";

type Tab = "private" | FeedView;
const TABS: { id: Tab; label: string; icon: typeof Lock; hint: string }[] = [
  { id: "all", label: "All", icon: Layers, hint: "Everything you can see: your shared and public items, plus what others share with you" },
  { id: "private", label: "Private", icon: Lock, hint: "Only you can see these" },
  { id: "shared", label: "Shared", icon: Users, hint: "Shared with specific people" },
  { id: "public", label: "Public", icon: Globe, hint: "Visible to the whole family" },
];

export function Family({ onOpenSettings }: { onOpenSettings: () => void }) {
  const [section, setSection] = useState<"view" | "members">("view");
  return (
    <div className="space-y-4">
      <Segmented role="tablist" value={section} onChange={setSection} itemClassName="py-1.5"
        options={[{ value: "view", label: <><Layers className="size-4" />Family view</> }, { value: "members", label: <><UsersRound className="size-4" />Members</> }]} />
      {section === "view" ? <FamilyView onOpenSettings={onOpenSettings} /> : <FamilyMembers />}
    </div>
  );
}

function monthBounds(offset: number) {
  const now = new Date();
  const first = new Date(now.getFullYear(), now.getMonth() + offset, 1);
  const last = new Date(first.getFullYear(), first.getMonth() + 1, 0);
  const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  return { startDate: iso(first), endDate: iso(last), label: first.toLocaleDateString(getNumberLocale(), { month: "long", year: "numeric" }) };
}

type Row = FamilyItem;

function FamilyView({ onOpenSettings }: { onOpenSettings: () => void }) {
  const { user } = useSession();
  const { hideBalances } = useTheme();
  const [prefs, setPrefs] = useState<SharingPrefs | null>(null);
  const [members, setMembers] = useState<FamilyMember[]>([]);
  const [tab, setTab] = useState<Tab>("all");
  const [month, setMonth] = useState(0);
  const [owner, setOwner] = useState<string | null>(null);
  const [rows, setRows] = useState<Row[] | null>(null);
  const [error, setError] = useState("");
  const range = monthBounds(month);

  useEffect(() => { api.sharing.get().then((s) => { setPrefs(s.prefs); setMembers(s.members); }); }, []);

  useEffect(() => {
    if (!prefs?.enabled) return;
    setRows(null); setError("");
    const { startDate, endDate } = range;
    const load: Promise<Row[]> = tab === "private"
      ? api.transactions.getAll({ visibility: "private", startDate, endDate }).then((txs: Transaction[]) =>
          txs.map((t) => ({ ...t, owner_id: user.id, is_mine: 1 })))
      : api.sharing.feed(tab, { startDate, endDate });
    load.then(setRows).catch((e) => { setRows([]); setError(errorText(e)); });
  }, [prefs?.enabled, tab, month, user.id]);

  const byId = useMemo(() => Object.fromEntries(members.map((m) => [m.id, m])), [members]);
  const totals = useMemo(() => {
    const t = new Map<string, number>();
    for (const r of rows ?? []) if (r.type === "expense") t.set(r.owner_id, (t.get(r.owner_id) ?? 0) + r.amount);
    return [...t.entries()].sort((a, b) => b[1] - a[1]);
  }, [rows]);
  const visible = (rows ?? []).filter((r) => !owner || r.owner_id === owner);
  const currency = rows?.find((r) => r.currency)?.currency ?? "INR";
  const grand = totals.reduce((s, [, v]) => s + v, 0);
  const fresh = useFreshItems(rows, user.id);

  if (!prefs) return <div className="space-y-4"><Skeleton className="h-9 w-72" /><Skeleton className="h-36 w-full rounded-xl" /></div>;
  if (!prefs.enabled) {
    return (
      <Card className="mx-auto mt-6 max-w-lg">
        <CardContent className="flex flex-col items-center gap-3 py-8 text-center">
          <div className="bg-secondary grid size-12 place-items-center rounded-xl"><Users className="size-5" /></div>
          <div className="font-semibold">Family sharing is off for you</div>
          <p className="text-muted-foreground max-w-sm text-sm">
            Turn it on to see the expenses your family chooses to share, and to share yours. Each person decides for themselves; nothing is shared until they do.
          </p>
          <Button onClick={onOpenSettings}><Settings2 />Open sharing settings</Button>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <Segmented role="tablist" value={tab} onChange={(v) => { setTab(v); setOwner(null); }} itemClassName="py-1.5"
          options={TABS.map((t) => ({ value: t.id, title: t.hint, label: <><t.icon className="size-3.5" />{t.label}</> }))} />
        <div className="ml-auto flex items-center gap-1">
          <Button variant="ghost" size="icon" onClick={() => setMonth((m) => m - 1)} aria-label="Previous month"><ChevronLeft /></Button>
          <div className="w-36 text-center text-sm font-medium">{range.label}</div>
          <Button variant="ghost" size="icon" onClick={() => setMonth((m) => Math.min(0, m + 1))} disabled={month === 0} aria-label="Next month"><ChevronRight /></Button>
        </div>
      </div>

      <Card className="gap-3">
        <CardHeader>
          <CardTitle>Spent in {range.label}</CardTitle>
          <CardDescription>{TABS.find((t) => t.id === tab)!.hint}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {rows === null ? <Skeleton className="h-9 w-40" /> : <AnimatedMoney value={grand} currency={currency} hidden={hideBalances} className="text-3xl font-semibold tabular-nums" />}
          {totals.length > 0 && (
            <>
              <div className="flex h-2 overflow-hidden rounded-full">
                {totals.map(([id, v]) => (
                  <div key={id} className="transition-[width] duration-500 ease-out" style={{ width: `${(v / grand) * 100}%`, backgroundColor: byId[id]?.color ?? "#71717a" }} title={byId[id]?.name} />
                ))}
              </div>
              <div className="flex flex-wrap gap-2">
                <Chip active={!owner} onClick={() => setOwner(null)}>Everyone</Chip>
                {totals.map(([id, v]) => {
                  const m = byId[id];
                  return (
                    <Chip key={id} active={owner === id} onClick={() => setOwner(owner === id ? null : id)}>
                      <UserAvatar name={m?.name ?? "?"} color={m?.color ?? "#71717a"} className="size-5 text-[10px]" />
                      {id === user.id ? "You" : m?.name ?? "Removed member"}
                      <span className="text-muted-foreground tabular-nums">{formatMoney(v, currency, hideBalances)}</span>
                    </Chip>
                  );
                })}
              </div>
            </>
          )}
        </CardContent>
      </Card>

      <Card className="py-2">
        <CardContent className="divide-y">
          {rows === null && <SkeletonRows />}
          {error && <p className="text-destructive py-6 text-center text-sm">{error}</p>}
          {rows !== null && !error && visible.length === 0 && (
            <p className="text-muted-foreground py-10 text-center text-sm">
              {tab === "private" ? "No private transactions this month." : "Nothing shared for this month yet."}
            </p>
          )}
          <AnimatedList>
          {visible.map((r) => {
            const m = byId[r.owner_id];
            const key = `${r.owner_id}:${r.id}`;
            return (
              <AnimatedItem key={key}>
                <TransactionRow t={r} hidden={hideBalances}
                  lead={<span className="flex shrink-0 items-center gap-1 font-medium text-foreground/80">
                    {fresh.has(key) && <NewDot />}
                    <UserAvatar name={m?.name ?? "?"} color={m?.color ?? "#71717a"} className="size-4 text-[8px]" />
                    {r.owner_id === user.id ? "You" : m?.name ?? "Removed member"} ·
                  </span>} />
              </AnimatedItem>
            );
          })}
          </AnimatedList>
        </CardContent>
      </Card>
    </div>
  );
}

function Chip({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button onClick={onClick}
      className={cn("flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-sm transition-colors hover:bg-accent", active && "border-primary bg-accent")}>
      {children}
    </button>
  );
}

const SEEN_KEY = "vaulty.family.seen";

/**
 * Family items this device hasn't shown before (other people's only). The first visit marks
 * everything as seen, so dots only ever mean "shared since you last looked".
 */
function useFreshItems(rows: Row[] | null, me: string) {
  const [fresh, setFresh] = useState<Set<string>>(new Set());
  useEffect(() => {
    if (!rows) return;
    let seen: string[] | null = null;
    try { seen = JSON.parse(localStorage.getItem(`${SEEN_KEY}.${me}`) ?? "null"); } catch { seen = null; }
    const keys = rows.filter((r) => r.owner_id !== me).map((r) => `${r.owner_id}:${r.id}`);
    const known = new Set(seen ?? keys);
    setFresh(new Set(keys.filter((k) => !known.has(k))));
    try { localStorage.setItem(`${SEEN_KEY}.${me}`, JSON.stringify([...new Set([...keys, ...(seen ?? [])])].slice(0, 3000))); } catch { /* private mode */ }
  }, [rows, me]);
  return fresh;
}

/** "New" marker that fades after a few seconds. */
function NewDot() {
  const { fade } = useMotion();
  return (
    <motion.span className="bg-primary size-1.5 shrink-0 rounded-full" title="New" aria-label="New"
      initial={{ opacity: 0, scale: 0 }} animate={{ opacity: [0, 1, 1, 0], scale: [0, 1.4, 1, 1] }}
      transition={{ ...fade({ duration: 6, times: [0, 0.05, 0.85, 1] }) }} />
  );
}
