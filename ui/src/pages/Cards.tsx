import { useCallback, useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { CalendarClock, ChevronLeft, ChevronRight, CreditCard as CardIcon, Info, Landmark, Loader2, Pencil, Plus, Trash2 } from "lucide-react";
import { api, errorText, type Account, type CardInput, type CardKind, type CardLook, type CardNetwork, type Institution, type WalletCard } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { InstitutionPicker } from "@/components/InstitutionPicker";
import { CardVisual, type CardVisualData } from "@/components/cards/CardVisual";
import { LookEditor } from "@/components/cards/LookEditor";
import { KIND_LABEL, NETWORK_NAME, detectNetwork, expiryText, luhnOk, maskedNumber, monthsUntilExpiry, nextDay, ordinal } from "@/lib/cards";
import { useTheme } from "@/hooks/theme";
import { cn, formatMoney } from "@/lib/utils";

const KINDS: CardKind[] = ["credit", "debit", "prepaid", "gift", "other"];
const KIND_HINT: Record<CardKind, string> = {
  credit: "What you owe, your limit and when the bill is due",
  debit: "Linked to one of your bank accounts",
  prepaid: "Prepaid, forex or transit cards with their own balance",
  gift: "Store gift cards with the value left on them",
  other: "Membership, loyalty and store cards",
};

const toVisual = (c: WalletCard): CardVisualData => ({
  kind: c.kind, name: c.name, institution: c.institution, institutionShort: c.institution_short, institutionLogo: c.institution_logo,
  issuerName: c.issuer_name, logo: c.account_logo, network: c.network, last4: c.last4, expiryMonth: c.expiry_month, expiryYear: c.expiry_year,
  look: c.look, currency: c.currency, owed: c.owed, creditLimit: c.credit_limit, available: c.available, statementDay: c.statement_day,
  dueDay: c.due_day, balance: c.balance, accountName: c.account_name, notes: c.notes,
});

export function Cards() {
  const { hideBalances } = useTheme();
  const [cards, setCards] = useState<WalletCard[] | null>(null);
  const [filter, setFilter] = useState<CardKind | "all">("all");
  const [index, setIndex] = useState(0);
  const [dealt, setDealt] = useState(false);
  const [dialog, setDialog] = useState<{ mode: "add" } | { mode: "edit"; card: WalletCard } | null>(null);
  const [removing, setRemoving] = useState<WalletCard | null>(null);

  const load = useCallback(() => api.cards.list().then(setCards), []);
  useEffect(() => { load(); }, [load]);

  const shown = useMemo(() => (cards ?? []).filter((c) => filter === "all" || c.kind === filter), [cards, filter]);
  const counts = useMemo(() => Object.fromEntries(KINDS.map((k) => [k, (cards ?? []).filter((c) => c.kind === k).length])) as Record<CardKind, number>, [cards]);
  useEffect(() => { setIndex((i) => Math.min(i, Math.max(0, shown.length - 1))); }, [shown.length]);
  // One orchestrated entrance: cards are dealt into the stack after first paint.
  useEffect(() => { if (shown.length) { const t = requestAnimationFrame(() => setDealt(true)); return () => cancelAnimationFrame(t); } }, [shown.length]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (shown.length < 2 || (e.target as HTMLElement)?.closest("input,select,textarea,[role=dialog]")) return;
      if (e.key === "ArrowRight") setIndex((i) => (i + 1) % shown.length);
      if (e.key === "ArrowLeft") setIndex((i) => (i - 1 + shown.length) % shown.length);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown.length]);

  if (!cards) return <Loader2 className="text-muted-foreground mx-auto mt-16 animate-spin" />;
  const current = shown[Math.min(index, shown.length - 1)];

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="bg-muted inline-flex flex-wrap rounded-lg p-1" role="tablist" aria-label="Card type">
          {(["all", ...KINDS] as const).map((k) => (
            <button key={k} role="tab" aria-selected={filter === k} onClick={() => { setFilter(k); setIndex(0); }}
              className={cn("rounded-md px-3 py-1.5 text-sm transition-colors", filter === k ? "bg-background font-medium shadow-sm" : "text-muted-foreground hover:text-foreground")}>
              {k === "all" ? "All" : KIND_LABEL[k]}
              {k !== "all" && counts[k] > 0 && <span className="text-muted-foreground ml-1.5 tabular-nums">{counts[k]}</span>}
            </button>
          ))}
        </div>
        <Button onClick={() => setDialog({ mode: "add" })}><Plus />Add card</Button>
      </div>

      {shown.length === 0 ? (
        <EmptyState kind={filter} onAdd={() => setDialog({ mode: "add" })} />
      ) : (
        <div className="grid items-start gap-8 lg:grid-cols-[minmax(0,460px)_1fr]">
          <div className="space-y-4">
            <Stack cards={shown} index={index} dealt={dealt} hideBalances={hideBalances} onSelect={setIndex} />
            {shown.length > 1 && (
              <div className="flex items-center justify-center gap-3">
                <Button variant="ghost" size="icon" aria-label="Previous card" onClick={() => setIndex((i) => (i - 1 + shown.length) % shown.length)}><ChevronLeft /></Button>
                <div className="flex gap-1.5" role="tablist" aria-label="Cards">
                  {shown.map((c, i) => (
                    <button key={c.id} role="tab" aria-selected={i === index} aria-label={c.name} onClick={() => setIndex(i)}
                      className={cn("h-1.5 rounded-full transition-all duration-300", i === index ? "bg-primary w-6" : "bg-muted-foreground/30 w-1.5 hover:bg-muted-foreground/60")} />
                  ))}
                </div>
                <Button variant="ghost" size="icon" aria-label="Next card" onClick={() => setIndex((i) => (i + 1) % shown.length)}><ChevronRight /></Button>
              </div>
            )}
            <p className="text-muted-foreground text-center text-xs">Move over the card to tilt it. Click it to turn it over.</p>
          </div>
          {current && (
            <Details key={current.id} card={current} hideBalances={hideBalances}
              onEdit={() => setDialog({ mode: "edit", card: current })} onRemove={() => setRemoving(current)} />
          )}
        </div>
      )}

      {dialog && (
        <CardDialog key={dialog.mode === "edit" ? dialog.card.id : "add"} initial={dialog.mode === "edit" ? dialog.card : null}
          defaultKind={filter === "all" ? "credit" : filter}
          onClose={() => setDialog(null)}
          onSaved={(saved) => { setDialog(null); load().then(() => { setFilter((f) => (f === "all" || f === saved.kind ? f : "all")); }); }} />
      )}
      <RemoveDialog card={removing} onClose={() => setRemoving(null)} onRemoved={() => { setRemoving(null); load(); }} />
    </div>
  );
}

/** Selected card in front; up to two more peek out behind it. */
function Stack({ cards, index, dealt, hideBalances, onSelect }: { cards: WalletCard[]; index: number; dealt: boolean; hideBalances: boolean; onSelect: (i: number) => void }) {
  const order = useMemo(() => cards.map((_, i) => (i - index + cards.length) % cards.length), [cards, index]);
  const peek = Math.min(cards.length - 1, 2);
  return (
    <div className="relative mx-auto w-full max-w-[460px]" style={{ paddingTop: `${peek * 22}px` }}>
      {cards.map((c, i) => {
        const depth = order[i];
        if (depth > 2) return null;
        const front = depth === 0;
        return (
          <div key={c.id}
            className={cn("w-full transition-[transform,opacity,filter] duration-500 ease-[cubic-bezier(.2,.7,.2,1)] motion-reduce:transition-none", front ? "relative" : "absolute inset-x-0 cursor-pointer")}
            style={{
              top: front ? undefined : `${peek * 22}px`,
              transform: `translateY(${dealt ? -depth * 22 : 40}px) scale(${1 - depth * 0.06})`,
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

function Details({ card, hideBalances, onEdit, onRemove }: { card: WalletCard; hideBalances: boolean; onEdit: () => void; onRemove: () => void }) {
  const money = (n: number | null | undefined) => (n == null ? "—" : formatMoney(n, card.currency, hideBalances));
  const subtitle = [KIND_LABEL[card.kind], card.institution ?? card.issuer_name, card.network && NETWORK_NAME[card.network], card.last4 && `ending ${card.last4}`]
    .filter(Boolean).join(", ");
  return (
    <div className="page-enter space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate text-xl font-semibold tracking-tight">{card.name}</h2>
          <p className="text-muted-foreground text-sm">{subtitle}</p>
        </div>
        <div className="flex shrink-0 gap-1">
          <Button variant="outline" size="sm" onClick={onEdit}><Pencil />Edit</Button>
          <Button variant="ghost" size="icon" className="size-8" aria-label="Remove card" title="Remove card" onClick={onRemove}><Trash2 /></Button>
        </div>
      </div>
      {card.kind === "credit" && <CreditDetails card={card} money={money} />}
      {card.kind === "debit" && (
        <Card><CardContent className="flex items-center gap-4">
          <div className="bg-secondary grid size-10 place-items-center rounded-lg"><Landmark className="size-5" /></div>
          <div className="grid flex-1 grid-cols-2 gap-4 text-sm">
            <Stat label="Linked account" value={card.account_name ?? "—"} />
            <Stat label="Balance" value={money(card.balance)} strong />
          </div>
        </CardContent></Card>
      )}
      {(card.kind === "prepaid" || card.kind === "gift") && (
        <Card><CardContent className="grid grid-cols-2 gap-4 text-sm">
          <Stat label={card.kind === "gift" ? "Remaining value" : "Balance"} value={money(card.balance)} strong />
          <Stat label="Currency" value={card.currency} />
        </CardContent></Card>
      )}
      {card.kind === "other" && (
        <Card><CardContent className="text-sm">{card.notes ? <p className="whitespace-pre-wrap">{card.notes}</p> : <p className="text-muted-foreground">Add notes like a membership number or renewal date.</p>}</CardContent></Card>
      )}
      <ExpiryRow card={card} />
    </div>
  );
}

function CreditDetails({ card, money }: { card: WalletCard; money: (n: number | null | undefined) => string }) {
  const u = card.utilization ?? null;
  const tone = u == null ? "text-muted-foreground" : u < 0.3 ? "text-success" : u < 0.7 ? "text-orange-500" : "text-destructive";
  const due = card.due_day ? nextDay(card.due_day) : null;
  const dueText = !due ? "Set a due day to see the countdown" : due.days === 0 ? "Due today" : due.days === 1 ? "Due tomorrow" : `Due in ${due.days} days`;
  return (
    <>
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
          <div className={cn("grid size-10 place-items-center rounded-lg", due && due.days <= 3 ? "bg-destructive/10 text-destructive" : "bg-secondary")}><CalendarClock className="size-5" /></div>
          <div className="flex-1">
            <div className="font-medium">{dueText}</div>
            <div className="text-muted-foreground text-sm">
              {due ? due.date.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "long" }) : "Edit the card to add its billing cycle."}
              {card.statement_day ? `. Next statement on the ${ordinal(card.statement_day)}` : ""}
            </div>
          </div>
        </CardContent>
      </Card>
      {u != null && u >= 0.3 && (
        <p className="text-muted-foreground flex items-start gap-2 text-sm"><Info className="mt-0.5 size-4 shrink-0" />
          Keeping credit used under 30% of the limit is generally better for your credit score.</p>
      )}
    </>
  );
}

function ExpiryRow({ card }: { card: WalletCard }) {
  if (!card.expiry_month || !card.expiry_year) return null;
  const months = monthsUntilExpiry(card.expiry_month, card.expiry_year);
  const text = months < 0 ? "Expired" : months === 0 ? "Expires this month" : months === 1 ? "Expires next month" : months < 12 ? `Expires in ${months} months` : `Valid until ${expiryText(card.expiry_month, card.expiry_year)}`;
  return (
    <p className={cn("flex items-center gap-2 text-sm", months <= 1 ? "text-destructive" : "text-muted-foreground")}>
      <CalendarClock className="size-4" />{text}
    </p>
  );
}

function Stat({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div className="min-w-0">
      <div className="text-muted-foreground">{label}</div>
      <div className={cn("truncate tabular-nums", strong ? "text-xl font-semibold" : "font-medium")}>{value}</div>
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
          strokeDasharray={c} strokeDashoffset={c * (1 - shown)} className="transition-[stroke-dashoffset] duration-1000 ease-out motion-reduce:transition-none" />
      </svg>
      <div className="text-center">
        <div className="text-foreground text-lg font-semibold tabular-nums">{value == null ? "—" : `${Math.round(value * 100)}%`}</div>
        <div className="text-muted-foreground text-[10px]">used</div>
      </div>
    </div>
  );
}

function EmptyState({ kind, onAdd }: { kind: CardKind | "all"; onAdd: () => void }) {
  const k: CardKind = kind === "all" ? "credit" : kind;
  return (
    <Card className="mx-auto max-w-xl">
      <CardContent className="flex flex-col items-center gap-5 py-8 text-center">
        <div className="w-full max-w-[300px] opacity-90">
          <CardVisual card={{ kind: k, name: "Your card", network: null, last4: null, look: { preset: "graphite" }, currency: "INR" }} />
        </div>
        <div>
          <div className="font-semibold">{kind === "all" ? "No cards yet" : `No ${KIND_LABEL[kind].toLowerCase()} cards yet`}</div>
          <p className="text-muted-foreground mt-1 text-sm">{kind === "all" ? "Keep credit, debit, prepaid, gift and membership cards together, each with its own look." : KIND_HINT[kind]}</p>
        </div>
        <Button onClick={onAdd}><CardIcon />Add a card</Button>
      </CardContent>
    </Card>
  );
}

function RemoveDialog({ card, onClose, onRemoved }: { card: WalletCard | null; onClose: () => void; onRemoved: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  if (!card) return null;
  const ownsMoney = card.owns_account === 1;
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>Remove {card.name}?</DialogTitle>
          <DialogDescription>
            {ownsMoney
              ? "The card and its balance account are archived. Past transactions stay in your history and reports."
              : card.kind === "debit" ? `Only the card is removed. ${card.account_name ?? "The bank account"} and its transactions stay as they are.` : "The card and its notes are removed."}
          </DialogDescription>
        </DialogHeader>
        {error && <p className="text-destructive text-sm">{error}</p>}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancel</Button>
          <Button variant="destructive" disabled={busy} onClick={async () => {
            setBusy(true);
            try { await api.cards.delete(card.id); onRemoved(); } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
          }}>{busy && <Loader2 className="animate-spin" />}Remove card</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

const MONTHS = Array.from({ length: 12 }, (_, i) => i + 1);
const YEARS = Array.from({ length: 16 }, (_, i) => new Date().getFullYear() - 2 + i);
const DAYS = Array.from({ length: 31 }, (_, i) => i + 1);

/** Add or edit any card. In edit mode only changed fields are sent. */
function CardDialog({ initial, defaultKind, onClose, onSaved }: { initial: WalletCard | null; defaultKind: CardKind; onClose: () => void; onSaved: (c: WalletCard) => void }) {
  const editing = !!initial;
  const [kind, setKind] = useState<CardKind>(initial?.kind ?? defaultKind);
  const [name, setName] = useState(initial?.name ?? "");
  const [issuer, setIssuer] = useState<Institution | null>(null);
  const [issuerTouched, setIssuerTouched] = useState(false);
  const [issuerName, setIssuerName] = useState(initial?.issuer_name ?? "");
  const [number, setNumber] = useState("");
  const [networkOverride, setNetworkOverride] = useState<CardNetwork | "">(initial?.network ?? "");
  const [currency, setCurrency] = useState(initial?.currency ?? "INR");
  const [amount, setAmount] = useState("");
  const [limit, setLimit] = useState(initial?.credit_limit != null ? String(initial.credit_limit) : "");
  const [statementDay, setStatementDay] = useState(initial?.statement_day ? String(initial.statement_day) : "");
  const [dueDay, setDueDay] = useState(initial?.due_day ? String(initial.due_day) : "");
  const [expMonth, setExpMonth] = useState(initial?.expiry_month ? String(initial.expiry_month) : "");
  const [expYear, setExpYear] = useState(initial?.expiry_year ? String(initial.expiry_year) : "");
  const [accountId, setAccountId] = useState(initial?.account_id ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [look, setLook] = useState<CardLook>(initial?.look ?? { preset: "midnight" });
  const [bankAccounts, setBankAccounts] = useState<Account[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    api.accounts.getAll().then((all) => {
      const banks = all.filter((a) => a.ledger_kind === "asset" && !["prepaid", "gift_card"].includes(a.type));
      setBankAccounts(banks);
      if (!initial && banks[0]) setAccountId((v) => v || banks[0].id);
    });
  }, [initial]);

  const digits = number.replace(/\D/g, "");
  const detected = detectNetwork(digits);
  const network = (networkOverride || detected || (editing && !digits ? initial?.network : null) || null) as CardNetwork | null;
  const last4 = digits.length >= 4 ? digits.slice(-4) : editing ? initial?.last4 ?? null : null;
  const luhnWarning = digits.length >= 13 && !luhnOk(digits);
  const linked = bankAccounts.find((a) => a.id === accountId);
  const usesDirectory = kind !== "gift" && kind !== "other";

  const onNumber = (v: string) => {
    const d = v.replace(/\D/g, "").slice(0, 19);
    const groups = detectNetwork(d) === "amex" ? [4, 6, 5] : [4, 4, 4, 4, 3];
    const parts: string[] = [];
    let i = 0;
    for (const g of groups) { if (i >= d.length) break; parts.push(d.slice(i, i + g)); i += g; }
    setNumber(parts.join(" "));
    if (networkOverride && !editing) setNetworkOverride("");
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    const common: CardInput = {
      name: name.trim() || undefined, network, last4, look,
      expiry_month: expMonth ? Number(expMonth) : null, expiry_year: expYear ? Number(expYear) : null,
      issuer_name: kind === "gift" || kind === "other" ? issuerName.trim() || null : undefined,
      notes: kind === "other" ? notes.trim() || null : undefined,
      ...(kind === "credit" ? { credit_limit: limit || null, statement_day: statementDay ? Number(statementDay) : null, due_day: dueDay ? Number(dueDay) : null } : {}),
    };
    if (usesDirectory && (!editing || issuerTouched)) common.institution_id = issuer?.id ?? null;
    try {
      let saved: WalletCard;
      if (editing && initial) {
        const patch: CardInput = { ...common };
        if (!name.trim()) delete patch.name;
        if (kind === "debit" && accountId && accountId !== initial.account_id) patch.account_id = accountId;
        saved = await api.cards.update(initial.id, patch);
      } else {
        saved = await api.cards.create({
          kind, ...common, currency,
          ...(kind === "credit" ? { owed: amount || "0" } : {}),
          ...(kind === "prepaid" || kind === "gift" ? { balance: amount || "0" } : {}),
          ...(kind === "debit" ? { account_id: accountId } : {}),
        });
      }
      onSaved(saved);
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  const shownIssuer = issuer ?? (!issuerTouched && initial ? { name: initial.institution, short: initial.institution_short, logo: initial.institution_logo } : null);
  const preview: CardVisualData = {
    kind, name: name || (kind === "debit" ? "Debit card" : KIND_LABEL[kind] + " card"),
    institution: usesDirectory ? shownIssuer?.name ?? (kind === "debit" ? linked?.institution ?? null : null) : null,
    institutionShort: usesDirectory ? shownIssuer?.short ?? (kind === "debit" ? linked?.institution_short ?? null : null) : null,
    institutionLogo: usesDirectory ? shownIssuer?.logo ?? null : null,
    issuerName: issuerName || null, network, last4,
    expiryMonth: expMonth ? Number(expMonth) : null, expiryYear: expYear ? Number(expYear) : null, look, currency,
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[92vh] overflow-y-auto sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{editing ? `Edit ${initial!.name}` : "Add a card"}</DialogTitle>
          <DialogDescription>Only the card network and last 4 digits are saved. The full number never leaves this form.</DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-6 md:grid-cols-[1fr_1.1fr]">
          <div className="space-y-4">
            <CardVisual card={preview} className="motion-safe:animate-[float_6s_ease-in-out_infinite]" />
            <div className="space-y-2"><Label>Background</Label><LookEditor value={look} onChange={setLook} /></div>
          </div>
          <div className="grid content-start gap-4">
            {!editing && (
              <div className="grid gap-2">
                <Label>Type of card</Label>
                <div className="bg-muted grid grid-cols-5 gap-1 rounded-lg p-1" role="radiogroup">
                  {KINDS.map((k) => (
                    <button key={k} type="button" role="radio" aria-checked={kind === k} onClick={() => setKind(k)} title={KIND_HINT[k]}
                      className={cn("rounded-md py-1.5 text-xs transition-colors", kind === k ? "bg-background font-medium shadow-sm" : "text-muted-foreground hover:text-foreground")}>
                      {k === "gift" ? "Gift" : KIND_LABEL[k]}
                    </button>
                  ))}
                </div>
                <p className="text-muted-foreground text-xs">{KIND_HINT[kind]}</p>
              </div>
            )}
            <Field label="Card name"><Input value={name} onChange={(e) => setName(e.target.value)} placeholder={kind === "other" ? "e.g. Gym membership" : "e.g. Travel rewards"} maxLength={40} /></Field>

            {kind === "debit" && (
              <Field label="Bank account it draws from">
                {bankAccounts.length ? (
                  <NativeSelect value={accountId} onChange={(e) => setAccountId(e.target.value)}>
                    {bankAccounts.map((a) => <option key={a.id} value={a.id}>{a.name}{a.institution ? ` (${a.institution})` : ""}</option>)}
                  </NativeSelect>
                ) : <p className="text-muted-foreground text-sm">Add a bank account on the Accounts page first.</p>}
              </Field>
            )}
            {usesDirectory ? (
              <Field label={kind === "debit" ? "Issuing bank (optional, defaults to the account's bank)" : "Card issuer"}>
                <InstitutionPicker value={issuer} onChange={(i) => { setIssuer(i); setIssuerTouched(true); }} currency={currency} />
              </Field>
            ) : (
              <Field label={kind === "gift" ? "Store or brand" : "Issued by"}><Input value={issuerName} onChange={(e) => setIssuerName(e.target.value)} placeholder={kind === "gift" ? "e.g. Corner Books" : "e.g. City Gym"} maxLength={40} /></Field>
            )}

            <Field label={kind === "other" ? "Card number (optional)" : "Card number"}>
              <Input value={number} onChange={(e) => onNumber(e.target.value)} inputMode="numeric" autoComplete="off" spellCheck={false}
                placeholder={editing && initial?.last4 ? `${maskedNumber(initial.network, initial.last4)} (type to replace)` : "Type it, or just the last 4 digits"}
                className="font-medium tracking-wider tabular-nums" />
              <div className="text-muted-foreground flex items-center justify-between gap-2 text-xs">
                <span>{network ? `${NETWORK_NAME[network]}${!networkOverride && detected ? " (detected)" : ""}` : "Network is detected as you type"}</span>
                <NativeSelect aria-label="Card network" className="h-7 w-36 text-xs" value={networkOverride} onChange={(e) => setNetworkOverride(e.target.value as CardNetwork | "")}>
                  <option value="">{editing ? "Keep / detect" : "Auto-detect"}</option>
                  {(Object.keys(NETWORK_NAME) as CardNetwork[]).map((n) => <option key={n} value={n}>{NETWORK_NAME[n]}</option>)}
                </NativeSelect>
              </div>
              {luhnWarning && <p className="text-xs text-orange-600">That number doesn't look valid. Check for a typo; only the last 4 digits are saved either way.</p>}
            </Field>

            <div className="grid grid-cols-2 gap-3">
              <Field label="Expiry month"><NativeSelect value={expMonth} onChange={(e) => setExpMonth(e.target.value)}><option value="">—</option>{MONTHS.map((m) => <option key={m} value={m}>{String(m).padStart(2, "0")}</option>)}</NativeSelect></Field>
              <Field label="Expiry year"><NativeSelect value={expYear} onChange={(e) => setExpYear(e.target.value)}><option value="">—</option>{YEARS.map((y) => <option key={y} value={y}>{y}</option>)}</NativeSelect></Field>
            </div>

            {(kind === "credit" || (!editing && (kind === "prepaid" || kind === "gift"))) && (
              <div className="grid grid-cols-[1fr_5rem] gap-3">
                {kind === "credit"
                  ? <Field label="Credit limit"><Input type="number" min="0" step="0.01" value={limit} onChange={(e) => setLimit(e.target.value)} /></Field>
                  : !editing ? <Field label={kind === "gift" ? "Value on the card" : "Balance"}><Input type="number" min="0" step="0.01" value={amount} onChange={(e) => setAmount(e.target.value)} placeholder="0" /></Field> : <div />}
                <Field label="Currency"><Input value={currency} maxLength={3} disabled={editing} onChange={(e) => setCurrency(e.target.value.toUpperCase())} /></Field>
              </div>
            )}
            {kind === "credit" && !editing && (
              <Field label="Amount owed right now"><Input type="number" min="0" step="0.01" value={amount} onChange={(e) => setAmount(e.target.value)} placeholder="0" /></Field>
            )}
            {kind === "credit" && (
              <div className="grid grid-cols-2 gap-3">
                <Field label="Statement day"><NativeSelect value={statementDay} onChange={(e) => setStatementDay(e.target.value)}><option value="">Not set</option>{DAYS.map((d) => <option key={d} value={d}>{ordinal(d)}</option>)}</NativeSelect></Field>
                <Field label="Payment due day"><NativeSelect value={dueDay} onChange={(e) => setDueDay(e.target.value)}><option value="">Not set</option>{DAYS.map((d) => <option key={d} value={d}>{ordinal(d)}</option>)}</NativeSelect></Field>
              </div>
            )}
            {editing && (kind === "prepaid" || kind === "gift" || kind === "credit") && (
              <p className="text-muted-foreground text-xs">To change the {kind === "credit" ? "amount owed" : "balance"}, add a transaction to the card's account.</p>
            )}
            {kind === "other" && (
              <Field label="Notes"><textarea value={notes} onChange={(e) => setNotes(e.target.value)} rows={3} maxLength={500}
                className="border-input placeholder:text-muted-foreground w-full rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                placeholder="Membership number, renewal date, perks…" /></Field>
            )}
            {error && <p className="text-destructive text-sm">{error}</p>}
            <DialogFooter>
              <Button type="button" variant="ghost" onClick={onClose}>Cancel</Button>
              <Button disabled={busy || (kind === "debit" && !accountId)}>{busy && <Loader2 className="animate-spin" />}{editing ? "Save changes" : "Add card"}</Button>
            </DialogFooter>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <div className="grid gap-2"><Label>{label}</Label>{children}</div>;
}
