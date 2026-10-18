import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent, type ReactNode } from "react";
import { Skeleton } from "@/components/Skeleton";
import { CalendarClock, ChevronLeft, ChevronRight, CreditCard as CardIcon, Info, Landmark, Loader2, Pencil, Plus, Trash2 } from "lucide-react";
import { api, DATA_RESTORED, errorText, type Account, type CardInput, type CardKind, type CardLook, type CardNetwork, type Institution, type WalletCard } from "@/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { CurrencyPicker } from "@/components/CurrencyPicker";
import { useLocale } from "@/hooks/locale";
import { InstitutionPicker } from "@/components/InstitutionPicker";
import { CardVisual, type CardVisualData } from "@/components/cards/CardVisual";
import { LookEditor } from "@/components/cards/LookEditor";
import { KIND_LABEL, NETWORK_NAME, detectNetwork, expiryText, luhnOk, maskedNumber, monthsUntilExpiry, nextDay, ordinal } from "@/lib/cards";
import { useTheme } from "@/hooks/theme";
import { cn, formatDate } from "@/lib/utils";
import { AnimatePresence, motion } from "motion/react";
import { AnimatedMoney } from "@/components/AnimatedMoney";
import { Segmented } from "@/components/Segmented";
import { EASE_OUT, SOFT_SPRING, useMotion } from "@/lib/motion";
import { useTranslation } from "react-i18next";
import i18n from "@/i18n";

const KINDS: CardKind[] = ["credit", "debit", "prepaid", "gift", "other"];
const KIND_HINT: Record<CardKind, string> = new Proxy({} as Record<CardKind, string>, { get: (_, k: string) => i18n.t(`cards.hint.${k}`) });

const toVisual = (c: WalletCard): CardVisualData => ({
  kind: c.kind, name: c.name, institution: c.institution, institutionShort: c.institution_short, institutionLogo: c.institution_logo,
  issuerName: c.issuer_name, logo: c.account_logo, network: c.network, last4: c.last4, expiryMonth: c.expiry_month, expiryYear: c.expiry_year,
  look: c.look, currency: c.currency, owed: c.owed, creditLimit: c.credit_limit, available: c.available, statementDay: c.statement_day,
  dueDay: c.due_day, balance: c.balance, accountName: c.account_name, notes: c.notes,
});

export function Cards() {
  const { hideBalances } = useTheme();
  const { t } = useTranslation();
  const [cards, setCards] = useState<WalletCard[] | null>(null);
  const [filter, setFilter] = useState<CardKind | "all">("all");
  const [index, setIndex] = useState(0);
  const [dealt, setDealt] = useState(false);
  const [dialog, setDialog] = useState<{ mode: "add" } | { mode: "edit"; card: WalletCard } | null>(null);
  const [removing, setRemoving] = useState<WalletCard | null>(null);

  const load = useCallback(() => api.cards.list().then((list) => { setCards(list); return list; }), []);
  useEffect(() => {
    load();
    window.addEventListener(DATA_RESTORED, load);
    return () => window.removeEventListener(DATA_RESTORED, load);
  }, [load]);

  const shown = useMemo(() => (cards ?? []).filter((c) => filter === "all" || c.kind === filter), [cards, filter]);
  const counts = useMemo(() => Object.fromEntries(KINDS.map((k) => [k, (cards ?? []).filter((c) => c.kind === k).length])) as Record<CardKind, number>, [cards]);
  useEffect(() => { setIndex((i) => Math.min(i, Math.max(0, shown.length - 1))); }, [shown.length]);
  // One orchestrated entrance: on first view the cards are dealt into the stack one by one.
  useEffect(() => { const t = setTimeout(() => setDealt(true), 500); return () => clearTimeout(t); }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (shown.length < 2 || (e.target as HTMLElement)?.closest("input,select,textarea,[role=dialog]")) return;
      if (e.key === "ArrowRight") setIndex((i) => (i + 1) % shown.length);
      if (e.key === "ArrowLeft") setIndex((i) => (i - 1 + shown.length) % shown.length);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown.length]);

  if (!cards) return (
    <div className="grid items-start gap-8 lg:grid-cols-[minmax(0,460px)_1fr]" aria-busy="true">
      <Skeleton className="mx-auto mt-14 aspect-[1.586] w-full max-w-[460px] rounded-2xl" />
      <div className="mt-14 space-y-3"><Skeleton className="h-6 w-40" /><Skeleton className="h-4 w-64" /><Skeleton className="h-32 w-full rounded-xl" /></div>
    </div>
  );
  const current = shown[Math.min(index, shown.length - 1)];

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Segmented role="tablist" label={t("cards.typeTab")} value={filter} onChange={(k) => { setFilter(k); setIndex(0); }} className="flex-wrap" itemClassName="py-1.5"
          options={(["all", ...KINDS] as const).map((k) => ({ value: k, label: <>
            {k === "all" ? t("cards.all") : KIND_LABEL[k]}
            {k !== "all" && counts[k] > 0 && <span className="text-muted-foreground tabular-nums">{counts[k]}</span>}
          </> }))} />
        <Button onClick={() => setDialog({ mode: "add" })}><Plus />{t("cards.add")}</Button>
      </div>

      {shown.length === 0 ? (
        <EmptyState kind={filter} onAdd={() => setDialog({ mode: "add" })} />
      ) : (
        <div className="grid items-start gap-8 lg:grid-cols-[minmax(0,460px)_1fr]">
          <div className="space-y-4">
            <Stack cards={shown} index={index} dealt={dealt} hideBalances={hideBalances} onSelect={setIndex} />
            {shown.length > 1 && (
              <div className="flex items-center justify-center gap-3">
                <Button variant="ghost" size="icon" aria-label={t("cards.prev")} onClick={() => setIndex((i) => (i - 1 + shown.length) % shown.length)}><ChevronLeft /></Button>
                <div className="flex gap-1.5" role="tablist" aria-label={t("nav.cards")}>
                  {shown.map((c, i) => (
                    <button key={c.id} role="tab" aria-selected={i === index} aria-label={c.name} onClick={() => setIndex(i)}
                      className={cn("h-1.5 rounded-full transition-all duration-300", i === index ? "bg-primary w-6" : "bg-muted-foreground/30 w-1.5 hover:bg-muted-foreground/60")} />
                  ))}
                </div>
                <Button variant="ghost" size="icon" aria-label={t("cards.next")} onClick={() => setIndex((i) => (i + 1) % shown.length)}><ChevronRight /></Button>
              </div>
            )}
            <p className="text-muted-foreground text-center text-xs">{t("cards.tiltHint")}</p>
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
          onSaved={(saved) => {
            const isNew = dialog.mode === "add";
            setDialog(null);
            load().then((list) => {
              const f2 = filter === "all" || filter === saved.kind ? filter : "all";
              setFilter(f2);
              // A new card rises into the wallet and comes to the front.
              if (isNew) setIndex(Math.max(0, list.filter((c) => f2 === "all" || c.kind === f2).findIndex((c) => c.id === saved.id)));
            });
          }} />
      )}
      <RemoveDialog card={removing} onClose={() => setRemoving(null)} onRemoved={() => { setRemoving(null); load(); }} />
    </div>
  );
}

/**
 * Selected card in front; up to two more peek out behind it. Picking a card behind lifts it up
 * out of the stack and drops it in front; a card that wasn't showing rises in from below.
 */
function Stack({ cards, index, dealt, hideBalances, onSelect }: { cards: WalletCard[]; index: number; dealt: boolean; hideBalances: boolean; onSelect: (i: number) => void }) {
  const order = useMemo(() => cards.map((_, i) => (i - index + cards.length) % cards.length), [cards, index]);
  const peek = Math.min(cards.length - 1, 2);
  const { full, move, fade } = useMotion();
  // Depth of each card in the previous render, to tell a card being lifted to the front.
  const prev = useRef<Record<string, number>>({});
  useEffect(() => { prev.current = Object.fromEntries(cards.map((c, i) => [c.id, order[i]])); });
  const ids = cards.map((c) => c.id);
  // A card leaving the stack drops out if it was removed, or just fades if it moved deeper than 3.
  return (
    <div className="relative mx-auto w-full max-w-[460px]" style={{ paddingTop: `${peek * 22}px` }}>
      <AnimatePresence custom={ids}>
      {cards.map((c, i) => {
        const depth = order[i];
        if (depth > 2) return null;
        const front = depth === 0;
        const was = prev.current[c.id];
        const lifting = full && front && was !== undefined && was > 0;
        const rest = { y: -depth * 22, scale: 1 - depth * 0.06, opacity: 1, rotate: 0 };
        return (
          <motion.div key={c.id}
            className={cn("w-full transition-[filter] duration-500", front ? "relative" : "absolute inset-x-0 cursor-pointer")}
            style={{
              top: front ? undefined : `${peek * 22}px`,
              transformOrigin: "50% 0%",
              zIndex: 10 - depth,
              filter: depth ? `brightness(${1 - depth * 0.14}) saturate(${1 - depth * 0.15})` : undefined,
            }}
            initial={{ y: 70, scale: 0.97, opacity: 0, rotate: 0 }}
            animate={lifting
              ? { y: [-was * 22, -was * 22 - 96, 0], scale: [1 - was * 0.06, 1.03, 1], rotate: [0, -2.5, 0], opacity: 1 }
              : rest}
            transition={lifting
              ? move({ duration: 0.6, times: [0, 0.42, 1], ease: [0.2, 0.7, 0.2, 1] })
              : { ...move(SOFT_SPRING), delay: dealt ? 0 : (2 - depth) * 0.08 }}
            custom={ids}
            variants={{ gone: (still: string[]) => still.includes(c.id)
              ? { opacity: 0, transition: fade({ duration: 0.15 }) }
              : { opacity: 0, y: 160, rotate: 6, scale: 0.92, transition: move({ duration: 0.45, ease: [0.4, 0, 0.8, 0.2] }) } }}
            exit="gone"
            onClick={front ? undefined : () => onSelect(i)}
            aria-hidden={!front}
          >
            <CardVisual card={toVisual(c)} interactive={front} hideAmounts={hideBalances} />
          </motion.div>
        );
      })}
      </AnimatePresence>
    </div>
  );
}

function Details({ card, hideBalances, onEdit, onRemove }: { card: WalletCard; hideBalances: boolean; onEdit: () => void; onRemove: () => void }) {
  const { t } = useTranslation();
  const money = (n: number | null | undefined) => <AnimatedMoney value={n} currency={card.currency} hidden={hideBalances} />;
  const { move, fade } = useMotion();
  // The panel settles in row by row, just after the card lands in front.
  const list = { hidden: {}, show: { transition: { staggerChildren: 0.05, delayChildren: 0.08 } } };
  const row = { hidden: { opacity: 0, y: 10 }, show: { opacity: 1, y: 0, transition: { y: move(EASE_OUT), opacity: fade(EASE_OUT) } } };
  const subtitle = [KIND_LABEL[card.kind], card.institution ?? card.issuer_name, card.network && NETWORK_NAME[card.network], card.last4 && t("cards.ending", { last4: card.last4 })]
    .filter(Boolean).join(", ");
  return (
    <motion.div className="space-y-4" variants={list} initial="hidden" animate="show">
      <motion.div variants={row} className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate text-xl font-semibold tracking-tight">{card.name}</h2>
          <p className="text-muted-foreground text-sm">{subtitle}</p>
        </div>
        <div className="flex shrink-0 gap-1">
          <Button variant="outline" size="sm" onClick={onEdit}><Pencil />{t("cards.edit")}</Button>
          <Button variant="ghost" size="icon" className="size-8" aria-label={t("cards.remove")} title={t("cards.remove")} onClick={onRemove}><Trash2 /></Button>
        </div>
      </motion.div>
      <motion.div variants={row} className="space-y-4">
      {card.kind === "credit" && <CreditDetails card={card} money={money} />}
      {card.kind === "debit" && (
        <Card><CardContent className="flex items-center gap-4">
          <div className="bg-secondary grid size-10 place-items-center rounded-lg"><Landmark className="size-5" /></div>
          <div className="grid flex-1 grid-cols-2 gap-4 text-sm">
            <Stat label={t("cards.linked")} value={card.account_name ?? "—"} />
            <Stat label={t("cards.balance")} value={money(card.balance)} strong />
          </div>
        </CardContent></Card>
      )}
      {(card.kind === "prepaid" || card.kind === "gift") && (
        <Card><CardContent className="grid grid-cols-2 gap-4 text-sm">
          <Stat label={card.kind === "gift" ? t("cards.remaining") : t("cards.balance")} value={money(card.balance)} strong />
          <Stat label={t("accounts.currency")} value={card.currency} />
        </CardContent></Card>
      )}
      {card.kind === "other" && (
        <Card><CardContent className="text-sm">{card.notes ? <p className="whitespace-pre-wrap">{card.notes}</p> : <p className="text-muted-foreground">{t("cards.notesEmpty")}</p>}</CardContent></Card>
      )}
      </motion.div>
      <motion.div variants={row}><ExpiryRow card={card} /></motion.div>
    </motion.div>
  );
}

function CreditDetails({ card, money }: { card: WalletCard; money: (n: number | null | undefined) => ReactNode }) {
  const u = card.utilization ?? null;
  const tone = u == null ? "text-muted-foreground" : u < 0.3 ? "text-success" : u < 0.7 ? "text-orange-500" : "text-destructive";
  const due = card.due_day ? nextDay(card.due_day) : null;
  const { full } = useMotion();
  const { t } = useTranslation();
  const dueText = !due ? t("cards.setDue") : due.days === 0 ? t("cards.dueToday") : due.days === 1 ? t("cards.dueTomorrow") : t("cards.dueIn", { count: due.days });
  return (
    <>
      <Card>
        <CardContent className="flex flex-wrap items-center gap-6">
          <UtilizationRing value={u} className={tone} />
          <div className="grid flex-1 grid-cols-2 gap-x-6 gap-y-3 text-sm">
            <Stat label={t("cards.youOwe")} value={money(card.owed)} strong />
            <Stat label={t("cards.available")} value={money(card.available)} />
            <Stat label={t("cards.limit")} value={money(card.credit_limit)} />
            <Stat label={t("cards.used")} value={u == null ? t("cards.addLimit") : `${Math.round(u * 100)}%`} />
          </div>
        </CardContent>
      </Card>
      <Card>
        <CardContent className="flex items-center gap-4">
          {/* Only "due today/tomorrow" moves, a few times, then settles. */}
          <motion.div className={cn("grid size-10 place-items-center rounded-lg", due && due.days <= 3 ? "bg-destructive/10 text-destructive" : "bg-secondary")}
            animate={due && due.days <= 1 && full ? { scale: [1, 1.12, 1] } : { scale: 1 }}
            transition={{ duration: 0.9, repeat: 3, repeatDelay: 0.6, ease: "easeInOut" }}>
            <CalendarClock className="size-5" />
          </motion.div>
          <div className="flex-1">
            <div className="font-medium">{dueText}</div>
            <div className="text-muted-foreground text-sm">
              {due ? formatDate(due.date, { weekday: "short", day: "numeric", month: "long" }) : t("cards.addCycle")}
              {card.statement_day ? `. ${t("cards.nextStatement", { day: ordinal(card.statement_day) })}` : ""}
            </div>
          </div>
        </CardContent>
      </Card>
      {u != null && u >= 0.3 && (
        <p className="text-muted-foreground flex items-start gap-2 text-sm"><Info className="mt-0.5 size-4 shrink-0" />
          {t("cards.under30")}</p>
      )}
    </>
  );
}

function ExpiryRow({ card }: { card: WalletCard }) {
  if (!card.expiry_month || !card.expiry_year) return null;
  const months = monthsUntilExpiry(card.expiry_month, card.expiry_year);
  const t = i18n.t.bind(i18n);
  const text = months < 0 ? t("cards.expired") : months === 0 ? t("cards.expiresThisMonth") : months === 1 ? t("cards.expiresNextMonth") : months < 12 ? t("cards.expiresIn", { count: months }) : t("cards.validUntil", { date: expiryText(card.expiry_month, card.expiry_year) });
  return (
    <p className={cn("flex items-center gap-2 text-sm", months <= 1 ? "text-destructive" : "text-muted-foreground")}>
      <CalendarClock className="size-4" />{text}
    </p>
  );
}

function Stat({ label, value, strong }: { label: string; value: ReactNode; strong?: boolean }) {
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
      aria-label={value == null ? i18n.t("cards.usedUnknown") : i18n.t("cards.usedAria", { pct: Math.round(value * 100) })}>
      <svg viewBox="0 0 80 80" className="absolute inset-0 -rotate-90">
        <circle cx="40" cy="40" r={r} fill="none" strokeWidth="7" className="stroke-muted" />
        <circle cx="40" cy="40" r={r} fill="none" strokeWidth="7" stroke="currentColor" strokeLinecap="round"
          strokeDasharray={c} strokeDashoffset={c * (1 - shown)} className="transition-[stroke-dashoffset] duration-1000 ease-out" />
      </svg>
      <div className="text-center">
        <div className="text-foreground text-lg font-semibold tabular-nums">{value == null ? "—" : `${Math.round(value * 100)}%`}</div>
        <div className="text-muted-foreground text-[10px]">{i18n.t("cards.usedShort")}</div>
      </div>
    </div>
  );
}

function EmptyState({ kind, onAdd }: { kind: CardKind | "all"; onAdd: () => void }) {
  const k: CardKind = kind === "all" ? "credit" : kind;
  const { t } = useTranslation();
  return (
    <Card className="mx-auto max-w-xl">
      <CardContent className="flex flex-col items-center gap-5 py-8 text-center">
        <div className="w-full max-w-[300px] opacity-90">
          <CardVisual card={{ kind: k, name: t("cards.yourCard"), network: null, last4: null, look: { preset: "graphite" }, currency: "INR" }} />
        </div>
        <div>
          <div className="font-semibold">{kind === "all" ? t("cards.noneYet") : t("cards.noneOfKind", { kind: KIND_LABEL[kind] })}</div>
          <p className="text-muted-foreground mt-1 text-sm">{kind === "all" ? t("cards.emptyHint") : KIND_HINT[kind]}</p>
        </div>
        <Button onClick={onAdd}><CardIcon />{t("cards.addA")}</Button>
      </CardContent>
    </Card>
  );
}

function RemoveDialog({ card, onClose, onRemoved }: { card: WalletCard | null; onClose: () => void; onRemoved: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const { t } = useTranslation();
  if (!card) return null;
  const ownsMoney = card.owns_account === 1;
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{t("cards.removeTitle", { name: card.name })}</DialogTitle>
          <DialogDescription>
            {ownsMoney
              ? t("cards.removeOwns")
              : card.kind === "debit" ? t("cards.removeDebit", { account: card.account_name ?? t("cards.theBankAccount") }) : t("cards.removeOther")}
          </DialogDescription>
        </DialogHeader>
        {error && <p className="text-destructive text-sm">{error}</p>}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="destructive" disabled={busy} onClick={async () => {
            setBusy(true);
            try { await api.cards.delete(card.id); onRemoved(); } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
          }}>{busy && <Loader2 className="animate-spin" />}{t("cards.remove")}</Button>
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
  const { t } = useTranslation();
  const [kind, setKind] = useState<CardKind>(initial?.kind ?? defaultKind);
  const [name, setName] = useState(initial?.name ?? "");
  const [issuer, setIssuer] = useState<Institution | null>(null);
  const [issuerTouched, setIssuerTouched] = useState(false);
  const [issuerName, setIssuerName] = useState(initial?.issuer_name ?? "");
  const [number, setNumber] = useState("");
  const [networkOverride, setNetworkOverride] = useState<CardNetwork | "">(initial?.network ?? "");
  const { currency: mainCurrency } = useLocale();
  const [currency, setCurrency] = useState(initial?.currency ?? mainCurrency);
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
    kind, name: name || t(`cards.defaultName.${kind}`),
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
          <DialogTitle>{editing ? t("cards.editTitle", { name: initial!.name }) : t("cards.addA")}</DialogTitle>
          <DialogDescription>{t("cards.privacy")}</DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-6 md:grid-cols-[1fr_1.1fr]">
          <div className="space-y-4">
            <CardVisual card={preview} className="motion-safe:animate-[float_6s_ease-in-out_infinite]" />
            <div className="space-y-2"><Label>{t("cards.background")}</Label><LookEditor value={look} onChange={setLook} /></div>
          </div>
          <div className="grid content-start gap-4">
            {!editing && (
              <div className="grid gap-2">
                <Label>{t("cards.typeOf")}</Label>
                <div className="bg-muted grid grid-cols-5 gap-1 rounded-lg p-1" role="radiogroup">
                  {KINDS.map((k) => (
                    <button key={k} type="button" role="radio" aria-checked={kind === k} onClick={() => setKind(k)} title={KIND_HINT[k]}
                      className={cn("rounded-md py-1.5 text-xs transition-colors", kind === k ? "bg-background font-medium shadow-sm" : "text-muted-foreground hover:text-foreground")}>
                      {t(`cards.kindShort.${k}`)}
                    </button>
                  ))}
                </div>
                <p className="text-muted-foreground text-xs">{KIND_HINT[kind]}</p>
              </div>
            )}
            <Field label={t("cards.name")}><Input value={name} onChange={(e) => setName(e.target.value)} placeholder={kind === "other" ? t("cards.namePhOther") : t("cards.namePh")} maxLength={40} /></Field>

            {kind === "debit" && (
              <Field label={t("cards.drawsFrom")}>
                {bankAccounts.length ? (
                  <NativeSelect value={accountId} onChange={(e) => setAccountId(e.target.value)}>
                    {bankAccounts.map((a) => <option key={a.id} value={a.id}>{a.name}{a.institution ? ` (${a.institution})` : ""}</option>)}
                  </NativeSelect>
                ) : <p className="text-muted-foreground text-sm">{t("cards.needBank")}</p>}
              </Field>
            )}
            {usesDirectory ? (
              <Field label={kind === "debit" ? t("cards.issuingBank") : t("accounts.issuer")}>
                <InstitutionPicker value={issuer} onChange={(i) => { setIssuer(i); setIssuerTouched(true); }} currency={currency} />
              </Field>
            ) : (
              <Field label={kind === "gift" ? t("cards.store") : t("cards.issuedBy")}><Input value={issuerName} onChange={(e) => setIssuerName(e.target.value)} placeholder={kind === "gift" ? t("cards.storePh") : t("cards.issuedByPh")} maxLength={40} /></Field>
            )}

            <Field label={kind === "other" ? t("cards.numberOptional") : t("cards.number")}>
              <Input value={number} onChange={(e) => onNumber(e.target.value)} inputMode="numeric" autoComplete="off" spellCheck={false}
                placeholder={editing && initial?.last4 ? t("cards.typeToReplace", { masked: maskedNumber(initial.network, initial.last4) }) : t("cards.numberPh")}
                className="font-medium tracking-wider tabular-nums" />
              <div className="text-muted-foreground flex items-center justify-between gap-2 text-xs">
                <span>{network ? (!networkOverride && detected ? t("cards.detected", { network: NETWORK_NAME[network] }) : NETWORK_NAME[network]) : t("cards.detectHint")}</span>
                <NativeSelect aria-label={t("cards.network")} className="h-7 w-36 text-xs" value={networkOverride} onChange={(e) => setNetworkOverride(e.target.value as CardNetwork | "")}>
                  <option value="">{editing ? t("cards.keepDetect") : t("cards.autoDetect")}</option>
                  {(Object.keys(NETWORK_NAME) as CardNetwork[]).map((n) => <option key={n} value={n}>{NETWORK_NAME[n]}</option>)}
                </NativeSelect>
              </div>
              {luhnWarning && <p className="text-xs text-orange-600">{t("cards.luhn")}</p>}
            </Field>

            <div className="grid grid-cols-2 gap-3">
              <Field label={t("cards.expMonth")}><NativeSelect value={expMonth} onChange={(e) => setExpMonth(e.target.value)}><option value="">—</option>{MONTHS.map((m) => <option key={m} value={m}>{String(m).padStart(2, "0")}</option>)}</NativeSelect></Field>
              <Field label={t("cards.expYear")}><NativeSelect value={expYear} onChange={(e) => setExpYear(e.target.value)}><option value="">—</option>{YEARS.map((y) => <option key={y} value={y}>{y}</option>)}</NativeSelect></Field>
            </div>

            {(kind === "credit" || (!editing && (kind === "prepaid" || kind === "gift"))) && (
              <div className="grid grid-cols-[1fr_5rem] gap-3">
                {kind === "credit"
                  ? <Field label={t("cards.limit")}><Input type="number" min="0" step="0.01" value={limit} onChange={(e) => setLimit(e.target.value)} /></Field>
                  : !editing ? <Field label={kind === "gift" ? t("cards.valueOn") : t("cards.balance")}><Input type="number" min="0" step="0.01" value={amount} onChange={(e) => setAmount(e.target.value)} placeholder="0" /></Field> : <div />}
                <Field label={t("accounts.currency")}><CurrencyPicker value={currency} disabled={editing} onChange={setCurrency} /></Field>
              </div>
            )}
            {kind === "credit" && !editing && (
              <Field label={t("cards.owedNow")}><Input type="number" min="0" step="0.01" value={amount} onChange={(e) => setAmount(e.target.value)} placeholder="0" /></Field>
            )}
            {kind === "credit" && (
              <div className="grid grid-cols-2 gap-3">
                <Field label={t("cards.statementDay")}><NativeSelect value={statementDay} onChange={(e) => setStatementDay(e.target.value)}><option value="">{t("cards.notSet")}</option>{DAYS.map((d) => <option key={d} value={d}>{ordinal(d)}</option>)}</NativeSelect></Field>
                <Field label={t("cards.dueDay")}><NativeSelect value={dueDay} onChange={(e) => setDueDay(e.target.value)}><option value="">{t("cards.notSet")}</option>{DAYS.map((d) => <option key={d} value={d}>{ordinal(d)}</option>)}</NativeSelect></Field>
              </div>
            )}
            {editing && (kind === "prepaid" || kind === "gift" || kind === "credit") && (
              <p className="text-muted-foreground text-xs">{kind === "credit" ? t("cards.changeOwed") : t("cards.changeBalance")}</p>
            )}
            {kind === "other" && (
              <Field label={t("cards.notes")}><textarea value={notes} onChange={(e) => setNotes(e.target.value)} rows={3} maxLength={500}
                className="border-input placeholder:text-muted-foreground w-full rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                placeholder={t("cards.notesPh")} /></Field>
            )}
            {error && <p className="text-destructive text-sm">{error}</p>}
            <DialogFooter>
              <Button type="button" variant="ghost" onClick={onClose}>{t("common.cancel")}</Button>
              <Button disabled={busy || (kind === "debit" && !accountId)}>{busy && <Loader2 className="animate-spin" />}{editing ? t("cards.saveChanges") : t("cards.add")}</Button>
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
