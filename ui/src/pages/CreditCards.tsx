import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { CalendarClock, ChevronLeft, ChevronRight, CreditCard as CardIcon, Info, Loader2, Plus, RotateCcw } from "lucide-react";
import { api, errorText, type CardDesign, type CardNetwork, type CreditCard, type Institution } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { InstitutionPicker } from "@/components/InstitutionPicker";
import { CardVisual, DESIGNS, type CardVisualData } from "@/components/cards/CardVisual";
import { NETWORK_NAME, detectNetwork, luhnOk, nextDay, ordinal } from "@/lib/cards";
import { useTheme } from "@/hooks/theme";
import { cn, formatMoney } from "@/lib/utils";

const toVisual = (c: CreditCard): CardVisualData => ({
  name: c.name, institution: c.institution, institutionShort: c.institution_short, institutionLogo: c.institution_logo, logo: c.logo,
  network: c.network, last4: c.last4, design: c.design, currency: c.currency, owed: c.owed, creditLimit: c.credit_limit,
  available: c.available, statementDay: c.statement_day, dueDay: c.due_day,
});

export function CreditCards() {
  const { hideBalances } = useTheme();
  const [cards, setCards] = useState<CreditCard[] | null>(null);
  const [index, setIndex] = useState(0);
  const [dealt, setDealt] = useState(false);
  const [adding, setAdding] = useState(false);

  const load = useCallback(() => api.cards.list().then((c) => { setCards(c); setIndex((i) => Math.min(i, Math.max(0, c.length - 1))); }), []);
  useEffect(() => { load(); }, [load]);
  // One orchestrated entrance: cards are dealt into the stack after first paint.
  useEffect(() => { if (cards?.length) { const t = requestAnimationFrame(() => setDealt(true)); return () => cancelAnimationFrame(t); } }, [cards?.length]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!cards?.length || (e.target as HTMLElement)?.closest("input,select,textarea,[role=dialog]")) return;
      if (e.key === "ArrowRight") setIndex((i) => (i + 1) % cards.length);
      if (e.key === "ArrowLeft") setIndex((i) => (i - 1 + cards.length) % cards.length);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cards?.length]);

  if (!cards) return <Loader2 className="text-muted-foreground mx-auto mt-16 animate-spin" />;
  const current = cards[index];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-4">
        <p className="text-muted-foreground text-sm">
          {cards.length ? "Move over a card to tilt it. Click it to see the billing details on the back." : "Add a card to track what you owe, your limit and when the bill is due."}
        </p>
        <Button onClick={() => setAdding(true)}><Plus />Add card</Button>
      </div>

      {cards.length === 0 ? (
        <EmptyState onAdd={() => setAdding(true)} />
      ) : (
        <div className="grid items-start gap-8 lg:grid-cols-[minmax(0,460px)_1fr]">
          <div className="space-y-4">
            <Stack cards={cards} index={index} dealt={dealt} hideBalances={hideBalances} onSelect={setIndex} />
            {cards.length > 1 && (
              <div className="flex items-center justify-center gap-3">
                <Button variant="ghost" size="icon" aria-label="Previous card" onClick={() => setIndex((i) => (i - 1 + cards.length) % cards.length)}><ChevronLeft /></Button>
                <div className="flex gap-1.5" role="tablist" aria-label="Cards">
                  {cards.map((c, i) => (
                    <button key={c.id} role="tab" aria-selected={i === index} aria-label={c.name} onClick={() => setIndex(i)}
                      className={cn("h-1.5 rounded-full transition-all duration-300", i === index ? "bg-primary w-6" : "bg-muted-foreground/30 w-1.5 hover:bg-muted-foreground/60")} />
                  ))}
                </div>
                <Button variant="ghost" size="icon" aria-label="Next card" onClick={() => setIndex((i) => (i + 1) % cards.length)}><ChevronRight /></Button>
              </div>
            )}
          </div>
          <Details key={current.id} card={current} hideBalances={hideBalances} />
        </div>
      )}

      <AddCardDialog open={adding} onOpenChange={setAdding} onAdded={() => { setIndex(0); load(); }} />
    </div>
  );
}

/** Selected card in front; up to two more peek out behind it. */
function Stack({ cards, index, dealt, hideBalances, onSelect }: { cards: CreditCard[]; index: number; dealt: boolean; hideBalances: boolean; onSelect: (i: number) => void }) {
  const order = useMemo(() => cards.map((_, i) => (i - index + cards.length) % cards.length), [cards, index]);
  const peek = Math.min(cards.length - 1, 2);
  return (
    <div className="relative mx-auto w-full max-w-[460px]" style={{ paddingTop: `${peek * 22}px` }}>
      {cards.map((c, i) => {
        const depth = order[i];
        if (depth > 2) return null;
        const front = depth === 0;
        const y = dealt ? -depth * 22 : 40;
        const scale = 1 - depth * 0.06;
        return (
          <div key={c.id}
            className={cn("w-full transition-[transform,opacity,filter] duration-500 ease-[cubic-bezier(.2,.7,.2,1)] motion-reduce:transition-none", front ? "relative" : "absolute inset-x-0 cursor-pointer")}
            style={{
              top: front ? undefined : `${peek * 22}px`,
              transform: `translateY(${y}px) scale(${scale})`,
              transformOrigin: "50% 0%",
              zIndex: 10 - depth,
              opacity: dealt ? 1 : 0,
              filter: depth ? `brightness(${1 - depth * 0.14}) saturate(${1 - depth * 0.15})` : undefined,
              transitionDelay: dealt ? `${(2 - depth) * 70}ms` : "0ms",
            }}
            onClick={front ? undefined : () => onSelect(i)}
            aria-hidden={!front}
          >
            <CardVisual card={toVisual(c)} interactive={front} hideAmounts={hideBalances} />
          </div>
        );
      })}
    </div>
  );
}

function Details({ card, hideBalances }: { card: CreditCard; hideBalances: boolean }) {
  const money = (n: number | null | undefined) => (n == null ? "—" : formatMoney(n, card.currency, hideBalances));
  const due = card.due_day ? nextDay(card.due_day) : null;
  const statement = card.statement_day ? nextDay(card.statement_day) : null;
  const u = card.utilization;
  const tone = u == null ? "text-muted-foreground" : u < 0.3 ? "text-success" : u < 0.7 ? "text-orange-500" : "text-destructive";
  const dueText = !due ? "Set a due day to see the countdown" : due.days === 0 ? "Due today" : due.days === 1 ? "Due tomorrow" : `Due in ${due.days} days`;

  return (
    <div className="page-enter space-y-4">
      <div>
        <h2 className="text-xl font-semibold tracking-tight">{card.name}</h2>
        <p className="text-muted-foreground text-sm">
          {[card.institution, card.network && NETWORK_NAME[card.network], card.last4 && `ending ${card.last4}`].filter(Boolean).join(", ")}
        </p>
      </div>
      <Card>
        <CardContent className="flex flex-wrap items-center gap-6">
          <UtilizationRing value={u} className={tone} />
          <div className="grid flex-1 grid-cols-2 gap-x-6 gap-y-3 text-sm">
            <Stat label="You owe" value={money(card.owed)} strong />
            <Stat label="Available" value={money(card.available)} />
            <Stat label="Credit limit" value={money(card.credit_limit)} />
            <Stat label="Credit used" value={u == null ? "Add a limit" : `${Math.round(u * 100)}%`} />
          </div>
        </CardContent>
      </Card>
      <Card>
        <CardContent className="flex items-center gap-4">
          <div className={cn("grid size-10 place-items-center rounded-lg", due && due.days <= 3 ? "bg-destructive/10 text-destructive" : "bg-secondary")}>
            <CalendarClock className="size-5" />
          </div>
          <div className="flex-1">
            <div className="font-medium">{dueText}</div>
            <div className="text-muted-foreground text-sm">
              {due ? due.date.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "long" }) : "Edit the card to add its billing cycle."}
              {statement ? `. Next statement on the ${ordinal(card.statement_day!)}` : ""}
            </div>
          </div>
        </CardContent>
      </Card>
      {u != null && u >= 0.3 && (
        <p className="text-muted-foreground flex items-start gap-2 text-sm"><Info className="mt-0.5 size-4 shrink-0" />
          Keeping credit used under 30% of the limit is generally better for your credit score.</p>
      )}
    </div>
  );
}

function Stat({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div>
      <div className="text-muted-foreground">{label}</div>
      <div className={cn("tabular-nums", strong ? "text-xl font-semibold" : "font-medium")}>{value}</div>
    </div>
  );
}

/** Animated ring for credit used; fills from empty on mount. */
function UtilizationRing({ value, className }: { value: number | null; className?: string }) {
  const [shown, setShown] = useState(0);
  useEffect(() => { const t = requestAnimationFrame(() => setShown(Math.min(1, value ?? 0))); return () => cancelAnimationFrame(t); }, [value]);
  const r = 34, c = 2 * Math.PI * r;
  return (
    <div className={cn("relative grid size-24 place-items-center", className)} role="img"
      aria-label={value == null ? "Credit used unknown" : `${Math.round(value * 100)}% of credit limit used`}>
      <svg viewBox="0 0 80 80" className="absolute inset-0 -rotate-90">
        <circle cx="40" cy="40" r={r} fill="none" strokeWidth="7" className="stroke-muted" />
        <circle cx="40" cy="40" r={r} fill="none" strokeWidth="7" stroke="currentColor" strokeLinecap="round"
          strokeDasharray={c} strokeDashoffset={c * (1 - shown)}
          className="transition-[stroke-dashoffset] duration-1000 ease-out motion-reduce:transition-none" />
      </svg>
      <div className="text-center">
        <div className="text-lg font-semibold tabular-nums text-foreground">{value == null ? "—" : `${Math.round(value * 100)}%`}</div>
        <div className="text-muted-foreground text-[10px]">used</div>
      </div>
    </div>
  );
}

function EmptyState({ onAdd }: { onAdd: () => void }) {
  return (
    <Card className="mx-auto max-w-xl">
      <CardContent className="flex flex-col items-center gap-5 py-8 text-center">
        <div className="w-full max-w-[300px] opacity-90">
          <CardVisual card={{ name: "Your card", network: null, last4: null, design: "graphite", currency: "INR" }} />
        </div>
        <div>
          <div className="font-semibold">No cards yet</div>
          <p className="text-muted-foreground mt-1 text-sm">Track what you owe, how much of your limit you've used, and when each bill is due.</p>
        </div>
        <Button onClick={onAdd}><CardIcon />Add your first card</Button>
      </CardContent>
    </Card>
  );
}

function AddCardDialog({ open, onOpenChange, onAdded }: { open: boolean; onOpenChange: (o: boolean) => void; onAdded: () => void }) {
  const [name, setName] = useState("");
  const [issuer, setIssuer] = useState<Institution | null>(null);
  const [number, setNumber] = useState("");
  const [networkOverride, setNetworkOverride] = useState<CardNetwork | "">("");
  const [currency, setCurrency] = useState("INR");
  const [limit, setLimit] = useState("");
  const [owed, setOwed] = useState("");
  const [statementDay, setStatementDay] = useState("");
  const [dueDay, setDueDay] = useState("");
  const [design, setDesign] = useState<CardDesign>("midnight");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const digits = number.replace(/\D/g, "");
  const detected = detectNetwork(digits);
  const network = (networkOverride || detected) as CardNetwork | null;
  const last4 = digits.length >= 4 ? digits.slice(-4) : null;
  const fullLength = digits.length >= 13;
  const luhnWarning = fullLength && !luhnOk(digits);

  const reset = () => {
    setName(""); setIssuer(null); setNumber(""); setNetworkOverride(""); setLimit(""); setOwed("");
    setStatementDay(""); setDueDay(""); setDesign("midnight"); setError("");
  };
  const close = (o: boolean) => { if (!o) reset(); onOpenChange(o); };

  // Format as the user types (groups of 4; Amex 4-6-5). The full number stays in this field only.
  const onNumber = (v: string) => {
    const d = v.replace(/\D/g, "").slice(0, 19);
    const groups = detectNetwork(d) === "amex" ? [4, 6, 5] : [4, 4, 4, 4, 3];
    const parts: string[] = [];
    let i = 0;
    for (const g of groups) { if (i >= d.length) break; parts.push(d.slice(i, i + g)); i += g; }
    setNumber(parts.join(" "));
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    try {
      const account = await api.accounts.create({
        name: name.trim() || (issuer ? `${issuer.short} card` : "Credit card"), type: "credit", currency,
        institution_id: issuer?.id ?? null, initial_balance: owed || "0",
      });
      await api.cards.setDetails(account.id, {
        network, last4, credit_limit: limit || null,
        statement_day: statementDay ? Number(statementDay) : null, due_day: dueDay ? Number(dueDay) : null, design,
      });
      close(false);
      onAdded();
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  const preview: CardVisualData = {
    name: name || "Card name", institution: issuer?.name, institutionShort: issuer?.short, institutionLogo: issuer?.logo,
    network, last4, design, currency,
  };
  const days = Array.from({ length: 31 }, (_, i) => i + 1);

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="max-h-[92vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Add a credit card</DialogTitle>
          <DialogDescription>Only the card network and last 4 digits are saved. The full number never leaves this form.</DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-6 md:grid-cols-[1fr_1.1fr]">
          <div className="space-y-4">
            <CardVisual card={preview} className="motion-safe:animate-[float_6s_ease-in-out_infinite]" />
            <div className="space-y-2">
              <Label>Design</Label>
              <div className="grid grid-cols-4 gap-2">
                {(Object.keys(DESIGNS) as CardDesign[]).map((k) => (
                  <button key={k} type="button" onClick={() => setDesign(k)} aria-pressed={design === k} title={DESIGNS[k].label}
                    className={cn("aspect-[1.586] rounded-md ring-offset-2 ring-offset-background transition", design === k ? "ring-2 ring-ring" : "hover:scale-105")}
                    style={{ background: DESIGNS[k].face }}><span className="sr-only">{DESIGNS[k].label}</span></button>
                ))}
              </div>
            </div>
          </div>
          <div className="grid content-start gap-4">
            <div className="grid gap-2"><Label htmlFor="cname">Card name</Label><Input id="cname" value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Travel rewards" maxLength={40} /></div>
            <div className="grid gap-2"><Label>Card issuer</Label><InstitutionPicker value={issuer} onChange={setIssuer} currency={currency} /></div>
            <div className="grid gap-2">
              <Label htmlFor="cnum">Card number</Label>
              <Input id="cnum" value={number} onChange={(e) => onNumber(e.target.value)} inputMode="numeric" autoComplete="off" spellCheck={false}
                placeholder="Type it, or just the last 4 digits" className="font-medium tracking-wider tabular-nums" />
              <div className="text-muted-foreground flex items-center justify-between gap-2 text-xs">
                <span>{network ? `${NETWORK_NAME[network]}${networkOverride ? "" : " (detected)"}` : "Network is detected as you type"}</span>
                <NativeSelect aria-label="Card network" className="h-7 w-36 text-xs" value={networkOverride} onChange={(e) => setNetworkOverride(e.target.value as CardNetwork | "")}>
                  <option value="">Auto-detect</option>
                  {(Object.keys(NETWORK_NAME) as CardNetwork[]).map((n) => <option key={n} value={n}>{NETWORK_NAME[n]}</option>)}
                </NativeSelect>
              </div>
              {luhnWarning && <p className="text-xs text-orange-600">That number doesn't look valid. Check for a typo; only the last 4 digits are saved either way.</p>}
            </div>
            <div className="grid grid-cols-[1fr_5rem] gap-3">
              <div className="grid gap-2"><Label htmlFor="climit">Credit limit</Label><Input id="climit" type="number" min="0" step="0.01" value={limit} onChange={(e) => setLimit(e.target.value)} /></div>
              <div className="grid gap-2"><Label htmlFor="ccur">Currency</Label><Input id="ccur" value={currency} maxLength={3} onChange={(e) => setCurrency(e.target.value.toUpperCase())} /></div>
            </div>
            <div className="grid gap-2"><Label htmlFor="cowed">Amount owed right now</Label><Input id="cowed" type="number" min="0" step="0.01" value={owed} onChange={(e) => setOwed(e.target.value)} placeholder="0" /></div>
            <div className="grid grid-cols-2 gap-3">
              <div className="grid gap-2"><Label htmlFor="cstmt">Statement day</Label>
                <NativeSelect id="cstmt" value={statementDay} onChange={(e) => setStatementDay(e.target.value)}><option value="">Not set</option>{days.map((d) => <option key={d} value={d}>{ordinal(d)}</option>)}</NativeSelect></div>
              <div className="grid gap-2"><Label htmlFor="cdue">Payment due day</Label>
                <NativeSelect id="cdue" value={dueDay} onChange={(e) => setDueDay(e.target.value)}><option value="">Not set</option>{days.map((d) => <option key={d} value={d}>{ordinal(d)}</option>)}</NativeSelect></div>
            </div>
            {error && <p className="text-destructive text-sm">{error}</p>}
            <DialogFooter>
              <Button type="button" variant="ghost" onClick={() => close(false)}><RotateCcw />Cancel</Button>
              <Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}Add card</Button>
            </DialogFooter>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
