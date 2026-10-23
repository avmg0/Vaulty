import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { translateError } from "@/i18n/errors";

/** Fired after any change command succeeds, so the undo toast can offer to reverse it. */
export const DATA_CHANGED = "vaulty:data-changed";
/** Fired after an undo or restore, so open pages reload. */
export const DATA_RESTORED = "vaulty:data-restored";

/** Commands the app records in history (and so can undo). Keep in step with `s.act` in src-tauri/src/lib.rs. */
const RECORDED = new RegExp([
  "^(accounts|categories|transactions|cards)_(create|update|delete|set_visibility)$", "^transactions_(set_hidden|bulk)$",
  "^settings_update$", "^hotkeys_(set|reset)$", "^calendar_event_(create|update|delete)$", "^calendar_import$", "^people_(create|update|delete)$", "^tasks_(create|update|set_done|delete)$", "^documents_(create|update|delete|add_file|remove_file)$", "^investments_(create|update|set_price|delete)$", "^fund_prices_update$", "^projects_(create|update|delete|section_(add|update|delete|order)|item_(add|update|delete)|payment_(add|update|delete)|file_(add|link|remove)|items_add|members_set|share_(export|import)|settle_record)$", "^cas_import$", "^card_statement_(enter|forget)$", "^card_benefits_(create|update|delete|use)$", "^import_json$", "^backup_import$", "^statement_import$", "^budgets_set$", "^tax_(payment_(add|delete)|links_set)$", "^form16_apply$", "^goals_(create|update|set_done|delete)$", "^insurance_(create|update|delete|pay|payment_delete|claim_(add|update|delete))$",
  "^recurring_(create|update|delete|set_active|skip|pay)$", "^rules_(add|delete|apply)$",
].join("|"));

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const out = await tauriInvoke<T>(cmd, args);
  if (RECORDED.test(cmd)) window.dispatchEvent(new CustomEvent(DATA_CHANGED, { detail: { cmd } }));
  return out;
}

export type Role = "admin" | "member" | "child";
export type User = { id: string; name: string; color: string; role: Role; active: boolean; totp_enabled: boolean; biometric_enabled: boolean; senior: boolean };
export type Invite = { user: User; setup_code: string };
/** A new recovery key appears only right after it was created and must be shown once. */
/** Wrong passwords or codes since the profile was last signed in to (`last_at` is Unix seconds). */
export type FailedAttempts = { count: number; last_at: number };
/** Someone this member helped back into their profile, since they last signed in (`at` is Unix seconds). */
export type HelpedNotice = { owner_name: string; at: number };
export type SignedInUser = User & { recovery_key?: string | null; failed_attempts?: FailedAttempts | null; helped?: HelpedNotice[]; nominee_request?: NomineeRequest | null };
export const DOC_KINDS = ["id", "passport", "licence", "insurance", "property", "vehicle", "will", "certificate", "warranty", "medical", "tax", "other"] as const;
export type DocKind = (typeof DOC_KINDS)[number];
export type DocFile = { id: string; name: string; mime: string; size: number; created_at?: string };
export const PROJECT_KINDS = ["personal", "family", "group", "business"] as const;
export const PROJECT_STATUSES = ["planning", "ongoing", "on_hold", "done"] as const;
export const ITEM_KINDS = ["material", "labour", "fee", "transport", "tax", "other"] as const;
export const PAY_MODES = ["upi", "cash", "bank", "card", "cheque", "other"] as const;
export const COLUMN_TYPES = ["text", "number", "date", "choice", "yes_no"] as const;
export const HIDEABLE_COLUMNS = ["section", "kind", "qty", "rate", "tax", "vendor", "bill_no", "notes"] as const;
/** Standard item columns, in table order; each can be given the project's own name. */
export const NAMEABLE_COLUMNS = ["date", "item", "section", "kind", "qty", "unit", "rate", "amount", "tax", "total", "vendor", "bill_no", "notes"] as const;
export type NameableColumn = (typeof NAMEABLE_COLUMNS)[number];
export type ProjectKind = (typeof PROJECT_KINDS)[number];
export type ProjectStatus = (typeof PROJECT_STATUSES)[number];
export type ItemKind = (typeof ITEM_KINDS)[number];
export type PayMode = (typeof PAY_MODES)[number];
export type ColumnType = (typeof COLUMN_TYPES)[number];
export type HideableColumn = (typeof HIDEABLE_COLUMNS)[number];
export type ProjectColumn = { id?: string; name: string; type: ColumnType; options: string[] };
export type ProjectInput = { name: string; kind: ProjectKind; status: ProjectStatus; currency?: string; budget?: string; start_date?: string; end_date?: string; notes?: string; columns: ProjectColumn[]; hidden: HideableColumn[]; labels: Partial<Record<NameableColumn, string>>; improvement?: boolean; sections?: string[] };
export type ProjectSection = { id: string; name: string; position: number; budget: number | null; billed: number; items: number; paid: number };
export type ProjectPerson = { name: string; billed: number; paid: number; balance: number; items: number; payments: number };
export type Project = {
  id: string; name: string; kind: ProjectKind; status: ProjectStatus; currency: string; budget: number | null; start_date: string | null; end_date: string | null; notes: string | null;
  columns: (ProjectColumn & { id: string })[]; hidden: HideableColumn[]; labels: Partial<Record<NameableColumn, string>>; improvement: number; amount: number; tax: number; billed: number; paid: number; owed: number; spent: number; budget_left: number | null;
  items: number; payments: number; files: number; last_date: string | null; created_at: string;
};
export type ProjectDetail = Project & {
  improvement_by_year: { fy: string; amount: number }[];
  sections: ProjectSection[]; unsectioned: { billed: number; items: number }; by_kind: Partial<Record<ItemKind, number>>; by_mode: Partial<Record<PayMode, number>>; vendors: ProjectPerson[];
};
export type CustomValue = string | number | boolean;
export type ProjectItem = {
  id: string; section_id: string | null; section_name: string | null; date: string; item: string; kind: ItemKind; qty: number | null; unit: string | null; rate: number | null;
  amount: number; tax: number; total: number; vendor: string | null; bill_no: string | null; notes: string | null; custom: Record<string, CustomValue>; files: number; warranty_until: string | null; document_id: string | null;
};
export type ItemInput = { section_id?: string; date: string; item: string; kind: ItemKind; qty?: string; unit?: string; rate?: string; amount?: string; tax?: string; tax_percent?: string; vendor?: string; bill_no?: string; notes?: string; warranty_until?: string; custom: Record<string, CustomValue> };
export type ProjectPayment = {
  id: string; section_id: string | null; section_name: string | null; date: string; amount: number; vendor: string | null; mode: PayMode; account_name: string | null;
  reference: string | null; entry_id: string | null; from_account: string | null; notes: string | null; files: number; paid_by: string | null; paid_by_name: string | null;
};
export type PaymentInput = { paid_by?: string; section_id?: string; date: string; amount: string; vendor?: string; mode: PayMode; account_name?: string; reference?: string; notes?: string; entry_id?: string; record?: { account_id: string; category_id?: string } };
export type ProjectFile = {
  id: string; item_id: string | null; payment_id: string | null; name: string; mime: string; size: number; created_at: string;
  item: string | null; item_date: string | null; payment_vendor: string | null; payment_date: string | null; payment_amount: number | null;
};
export type BillDraft = {
  vendor: string | null; gstin: string | null; bill_no: string | null; date: string | null; taxable: string | null; tax: string | null; tax_rate: string | null; total: string | null;
  items_total: string; matches: boolean | null;
  items: { item: string; qty: string | null; unit: string | null; rate: string | null; amount: string; tax_percent: string | null; hsn: string | null }[];
};
export type ProjectMember = { id?: string; name: string; share: number; is_me: boolean | number };
export type ProjectSplit = {
  total: number;
  members: { id: string; name: string; is_me: boolean; share: number; paid: number; owes: number; balance: number }[];
  settle: { from: string; from_me: boolean; to: string; to_me: boolean; amount: number }[];
};
export type SettleLine = { member_id: string; name: string; person_id: string | null; person_name: string | null; owed: number; recorded: number; change: number };
export type SharePeek = {
  name: string; currency: string; sent_by: string | null; exported_at: string | null; members: { id: string; name: string }[];
  items: number; payments: number; files: number; files_left_out: number | null; billed: number; paid: number; updates: { id: string; name: string } | null;
};
export type FileLink = { item_id?: string | null; payment_id?: string | null };
export type LinkSuggestion = { payment_id: string; candidates: { id: string; date: string; merchant: string | null; description: string | null; account: string | null; named: boolean }[] };
export const HOLDING_KINDS = ["mutual_fund", "stock", "etf", "bond", "deposit", "gold", "other"] as const;
export type HoldingKind = (typeof HOLDING_KINDS)[number];
export type HoldingInput = { kind: HoldingKind; name: string; isin?: string; symbol?: string; folio?: string; provider?: string; currency?: string; units?: string; price?: string; price_date?: string; cost?: string; notes?: string };
export type Holding = {
  id: string; kind: HoldingKind; name: string; isin: string | null; symbol: string | null; folio: string | null; provider: string | null; registrar: string | null;
  currency: string; units: number; price: number; price_date: string | null; cost: number; value: number; gain: number; gain_percent: number | null;
  dividends: number; xirr: number | null; transactions: number; closed: boolean; source: "manual" | "cas"; statement_to: string | null; notes: string | null;
};
export type HoldingTxn = { id: string; date: string; kind: string; description: string; amount: number | null; units: number | null; price: number | null; balance: number | null };
export type InvestmentTotal = { currency: string; holdings: number; value: number; cost: number; gain: number; gain_percent: number | null; xirr: number | null };
export type CasPreview = {
  from: string | null; to: string | null; investor: string | null; new_transactions: number; value: number;
  schemes: { name: string; amc: string; folio: string; isin: string | null; registrar: string | null; status: "new" | "update" | "older"; units: number | null; nav: number | null; nav_date: string | null; cost: number | null; value: number | null; transactions: number; new_transactions: number; balanced: boolean }[];
};
export type DocInput = { title: string; kind: DocKind; holder?: string; number?: string; issuer?: string; issued_on?: string; expires_on?: string; remind_days: number; kept_at?: string; notes?: string };
/** A document's details and attached files. `days_left` until it expires (negative once expired). */
export type Doc = DocInput & { id: string; created_at: string; updated_at: string; files: DocFile[]; days_left: number | null; status: "expired" | "expiring" | "valid" | "no_expiry" };
/** A nominee's request for access (times are Unix seconds). */
export type NomineeRequest = { started_at: number; ready_at: number; allowed: boolean; ready: boolean };
export type NomineeStatus = { set_up: boolean; name: string | null; wait_days: number; has_letter: boolean; request: NomineeRequest | null };
export type EstateHolding = { name: string; kind: string; institution: string | null; currency: string; amount: number; notes: string | null };
/** "In case of emergency": what a nominee sees. */
export type Estate = {
  as_of: string; accounts: EstateHolding[]; debts: EstateHolding[]; investments: EstateHolding[];
  cards: { name: string; kind: string; issuer: string | null; network: string | null; last4: string | null }[];
  people: { name: string; phone: string | null; currency: string; amount: number }[];
  regular: { name: string; kind: string; amount: number; currency: string; frequency: string; every: number; next_date: string | null; account: string | null }[];
  documents: { title: string; kind: string; holder: string | null; number: string | null; issuer: string | null; expires_on: string | null; kept_at: string | null; files: number }[];
  totals: { currency: string; owned: number; owed: number; net: number }[];
};
export type NomineeOpened = { owner_name: string; letter: string | null; estate: Estate };
/** Shared recovery: any 2 of 3 helpers. */
export type HelperSlot = { slot: number; kind: "member" | "card"; helper_id: string | null; name: string; usable: boolean };
export type Helpers = { set_up: boolean; created_at: string | null; slots: HelperSlot[]; usable: number };
export type HelperChoice = { kind: "member"; id: string } | { kind: "card"; label: string };
/** A card to print, shown only right after the helpers were chosen. */
export type HelperCard = { slot: number; label: string; owner: string; code: string; nominee: boolean };
export type SignIn = { status: "signed_in"; user: SignedInUser } | { status: "needs_second_factor" };
export type TotpEnrollment = { secret_base32: string; otpauth_url: string; qr_png_base64: string };
export type TaxDeduction = { id: string; label: string; hint?: string; cap?: number; cap_senior?: number; cap_alt?: number; alt_label?: string };
export type TaxRegime = { id: string; name: string; default?: boolean; description?: string; deductions: TaxDeduction[]; cess?: { name: string; rate: number } };
export type TaxYear = { id: string; label: string; start: string; end: string; regimes: TaxRegime[] };
export type TaxPack = { country: string; currency: string; last_checked: string; notes?: string; sources: string[]; years: TaxYear[] };
/** Amounts are decimal strings in the pack's currency. */
export type TaxInput = { age: number; salary: string; other_income: string; deductions: Record<string, string>; alt: string[] };
export type TaxEstimate = {
  regime: string; name: string; gross: number; standard_deduction: number; taxable: number; slab_tax: number; rebate: number;
  surcharge: number; surcharge_rate: number; cess: number; total: number; effective_rate: number;
  deductions: { id: string; label: string; claimed: number; allowed: number; cap: number | null }[];
  bands: { from: number; upto: number | null; rate: number; taxed: number; tax: number }[];
};
export type HraInput = { salary?: string; received?: string; monthly_rent?: string; metro?: boolean; from?: string; to?: string; landlord?: string; landlord_pan?: string; address?: string; tenant?: string };
export type TaxPayment = { id: string; kind: "tds" | "advance" | "self_assessment"; date: string; amount: number; note: string | null };
export type TaxLink = { id: string; category_id: string; target: string; category_name: string; category_icon: string | null; category_type: "income" | "expense" };
export type TaxPlan = {
  id: string; country: string; currency: string; year: { id: string; label: string; start: string; end: string };
  input: Partial<TaxInput>; regime: string | null; hra: HraInput; hra_result: { months: number; rent: number; exempt: number; pan_needed: boolean };
  records: { by: Record<string, number>; elss: number }; links: TaxLink[]; payments: TaxPayment[]; comparison: TaxComparison;
  used: string; total: number; tds: number; paid: number; balance: number; advance_note: string | null;
  advance: { date: string; percent: number; needed: number; paid: number; short: number; past: boolean }[];
};
export const POLICY_KINDS = ["term_life", "life", "health", "motor", "home", "travel", "accident", "other"] as const;
export type PolicyKind = (typeof POLICY_KINDS)[number];
export const PREMIUM_FREQUENCIES = ["monthly", "quarterly", "half_yearly", "yearly", "single"] as const;
export const CLAIM_STATUSES = ["filed", "in_progress", "settled", "partly_settled", "rejected"] as const;
export type PolicyClaim = { id: string; policy_id: string; date: string; what: string; claimed: number; settled: number | null; status: (typeof CLAIM_STATUSES)[number]; settled_on: string | null; reference: string | null; notes: string | null };
export type Policy = {
  id: string; kind: PolicyKind; name: string; insurer: string | null; policy_no: string | null; holder: string | null; covers: string | null; for_parents: boolean;
  currency: string; cover: number | null; premium: number; premium_yearly: number; frequency: (typeof PREMIUM_FREQUENCIES)[number]; next_due: string | null;
  start_date: string | null; end_date: string | null; nominee: string | null; contact: string | null; document_id: string | null; document_title: string | null;
  status: "active" | "lapsed" | "ended"; notes: string | null; days_to_due: number | null; due: "soon" | "late" | "lapsing" | "none"; paid_this_year: number; open_claims: number;
  payments: { id: string; date: string; amount: number; entry_id: string | null }[]; claims: PolicyClaim[];
};
export type PolicyInput = Partial<Omit<Policy, "cover" | "premium" | "payments" | "claims">> & { cover?: string; premium?: string };
export type InsuranceTotal = { currency: string; life_cover: number; health_cover: number; premiums_yearly: number; income_year: number; life_multiple: number | null };
export type NetWorthPoint = { date: string; cash: number; investments: number; owed_to_you: number; debts: number; you_owe: number; net: number };
export type NetWorth = { currency: string; points: NetWorthPoint[]; net: number; change_month: number | null; change_period: number; since: string | null };
export type GoalStatus = "done" | "reached" | "on_track" | "behind" | "overdue" | "no_date";
export type GoalLink = { account_id: string | null; holding_id: string | null; share: number; name: string; value: number; over: boolean };
export type Goal = {
  id: string; name: string; icon: string | null; currency: string; target_date: string | null; return_rate: number; inflation: number; priority: number;
  done_on: string | null; notes: string | null; target: number; target_then: number; saved_by_hand: number; saved: number; monthly: number;
  progress: number; months_left: number | null; needed_monthly: number | null; projected: number | null; status: GoalStatus; links: GoalLink[];
};
export type GoalInput = {
  name: string; icon?: string | null; currency: string; target: string; target_date?: string | null; saved?: string; monthly?: string;
  return_rate?: number | string; inflation?: number | string; priority?: number; notes?: string | null;
  links?: { account_id?: string; holding_id?: string; share: number }[];
};
export type GoalTotal = { currency: string; target: number; saved: number; monthly: number; goals: number };
export type Form16Offer = {
  year: string | null; assessment_year: string | null; employer: string | null; tan: string | null; employee: string | null; pan: string | null;
  tds: number; paid: number; regime: "old" | "new" | null; standard_deduction: number | null; professional_tax: number | null; taxable: number | null; tax: number | null;
  fields: { field: "salary" | "other_income" | "deduction"; id?: string; amount: number }[];
  payments: { quarter: number; date: string; amount: number }[];
};
export type TaxComparison = {
  country: string; currency: string; year: { id: string; label: string }; estimates: TaxEstimate[];
  recommended: string | null; saving: number; sources: string[]; last_checked: string; notes?: string;
};
export type MarketItem = { symbol: string; label: string };
export type MarketPrefs = {
  enabled: boolean; show: { regional: boolean; crypto: boolean; watchlist: boolean }; region: string; crypto: string[];
  crypto_currency: string; watchlist: MarketItem[]; stock_provider: "none" | "twelvedata"; refresh_minutes: number; speed: "slow" | "normal" | "fast"; fund_prices: boolean;
};
export type FundPriceStatus = { on: boolean; last: { fetched_on: string; as_of: string | null; updated: number } | null };
export type MarketView = {
  prefs: MarketPrefs; has_stock_key: boolean; has_crypto_key: boolean; regional: MarketItem[]; regional_countries: string[]; known_crypto: MarketItem[];
};
/** Display-only prices; never part of the ledger. */
export type Quote = { kind: "stock" | "crypto"; symbol: string; label: string; price: number; change_pct: number | null; currency: string | null; source: string; as_of: string };
/** Result of checking the tamper-evident history. */
export type HistoryCheck = {
  ok: boolean; checked: number; since: string | null; head: string; broken_at: number | null; reason: string | null;
  changed_outside: { table: string; id: string }[]; unchained: number;
};
export type Snapshot = { name: string; at: string; reason: "daily" | "manual" | "before-import" | "before-update" | "before-restore"; size: number };
/** One undoable action from the member's change history. */
export type HistoryItem = {
  id: number; at: string; action: string; kind: string; op: "insert" | "update" | "delete" | ""; title: string;
  amount: { amount: number; currency: string } | null; changes: number; undo_of: number | null; undone: boolean;
  can_undo: boolean; blocked_reason: string | null;
};
export type SecurityStatus = { totp_enabled: boolean; backup_codes_remaining: number; biometric_enabled: boolean; biometric_available: boolean };

/**
 * Amounts: `*_minor` fields are exact integers (paise, cents…); the plain fields are decimals for display.
 * `current_balance` is what the account adds to net worth, so cards and loans are negative.
 * `initial_balance` is the opening balance as entered (for cards and loans: the amount owed).
 * When sending amounts, prefer the text the user typed (e.g. "1234.50") so nothing passes through floating point.
 */
export type Account = {
  id: string; name: string; type: string; currency: string; color?: string; icon?: string;
  ledger_kind: "asset" | "liability";
  /** Directory link (see Institution) and the user's own logo image (data URL), if any. */
  institution_id?: string | null; institution?: string | null; institution_short?: string | null;
  institution_logo?: string | null; logo?: string | null;
  current_balance: number; current_balance_minor: number;
  initial_balance: number; initial_balance_minor: number;
  /** Remind when the balance is (or is heading) below this. Bank and cash accounts only. */
  low_balance: number | null;
};
export type NewAccount = {
  name: string; type: string; currency: string; initial_balance?: string | number; color?: string; icon?: string;
  institution_id?: string | null; logo?: string | null;
  /** Amount as text; "" removes the level. */
  low_balance?: string | number | null;
};
export type CardNetwork = "visa" | "mastercard" | "amex" | "discover" | "rupay" | "diners" | "jcb" | "unionpay" | "maestro";
export type CardDesign = "midnight" | "ocean" | "aurora" | "sunset" | "graphite" | "emerald" | "rose" | "gold";
export type CardKind = "credit" | "debit" | "prepaid" | "gift" | "other";
type LookCommon = { ink?: "light" | "dark"; pattern?: boolean };
/** Card background: a preset finish, or the user's own colour, gradient or image. */
export type CardLook =
  | { preset: CardDesign }
  | ({ fill: "solid"; colors: [string] } & LookCommon)
  | ({ fill: "gradient"; colors: string[]; angle?: number } & LookCommon)
  | ({ fill: "image"; image: string; dim?: number } & LookCommon);

/** Any card. Only the network and last 4 digits are ever stored. Money fields depend on `kind`. */
export type WalletCard = {
  id: string; kind: CardKind; name: string; account_id: string | null; owns_account: number;
  institution_id: string | null; institution: string | null; institution_short: string | null; institution_logo: string | null;
  issuer_name: string | null; network: CardNetwork | null; last4: string | null;
  expiry_month: number | null; expiry_year: number | null; look: CardLook; notes: string | null;
  currency: string; account_name: string | null; account_logo: string | null;
  // credit
  statement_day: number | null; due_day: number | null; credit_limit_minor: number | null;
  min_due_percent?: number | null; min_due_floor?: number | null; annual_fee?: number | null; card_year_start?: string | null;
  owed?: number; credit_limit?: number | null; available?: number | null; utilization?: number | null;
  // debit / prepaid / gift
  balance?: number;
};
/** Create needs `kind`; edits send only what changed. Amounts as typed text. */
export type CardInput = Partial<{
  kind: CardKind; name: string; currency: string; owed: string; balance: string; account_id: string;
  institution_id: string | null; issuer_name: string | null; network: CardNetwork | null; last4: string | null;
  expiry_month: number | null; expiry_year: number | null; credit_limit: string | null;
  statement_day: number | null; due_day: number | null; look: CardLook; notes: string | null;
  min_due_percent: string | number | null; min_due_floor: string | null; annual_fee: string | null; card_year_start: string | null;
}>;
export type DeviceSettings = { tray: boolean; shortcut: string; keep_running: boolean; shortcut_choices: string[]; mac: boolean };
/** One credit card statement, worked out from the card's transactions (or as entered). */
export type CardCycle = {
  statement_date: string; due_date: string; total: number; min: number; paid: number; left: number; min_left: number;
  status: "nothing_due" | "paid" | "min_paid" | "due" | "overdue" | "carried" | "missed"; entered: boolean;
};
export type CardCycles = {
  card_id: string; name: string; currency: string; statement_day: number; due_day: number; current: CardCycle | null; unbilled: number;
  next_statement: string; buy_today_due: string; interest_free_days: number; history: CardCycle[];
};
export type BenefitKind = "reward" | "milestone" | "perk";
export type CardBenefit = {
  id: string; card_id: string; kind: BenefitKind; title: string; category_id: string | null; category_name: string | null; merchant: string | null;
  rate: number | null; cap: number | null; target: number | null; allowance: number | null; notes: string | null;
  progress: { spent?: number; earned?: number; cap?: number | null; capped?: boolean; target?: number; left?: number | null; reached?: boolean; used?: number; allowance?: number | null; since: string };
};
export type BenefitInput = { kind: BenefitKind; title: string; category_id?: string | null; merchant?: string | null; rate?: string | number | null; cap?: string | null; target?: string | null; allowance?: string | number | null; notes?: string | null };
/** A credit card's fit for a purchase today. */
export type BestPick = { card_id: string; name: string; currency: string; reward: string | null; rate: number; earns: number | null; interest_free_days: number | null; pay_by: string | null; available: number | null; fits: boolean };

/** Subset of user_settings the UI uses. `locale` is BCP 47, e.g. "hi-IN" (language + country). */
export type UserSettings = { locale: string; currency: string; date_format: string; start_of_week: string; fiscal_year_start: string; negative_format: string; decimals: string; update_check?: "off" | "weekly"; auto_lock_minutes?: number; notifications?: "on" | "off"; notify_amounts?: number | boolean; notify_locked?: number | boolean };

/** A keyboard shortcut: `keys` like "Mod+Shift+K" ("" = off). */
export type Hotkey = { action: string; keys: string; default: string; custom: boolean };

export type CalendarKind = "bill" | "income" | "transfer" | "paid" | "card_due" | "statement" | "event" | "reminder" | "holiday" | "task" | "expiry" | "tax" | "premium";
/** One thing on the calendar. `pay_by` is set when banks are closed on the due date. */
export type CalendarItem = { kind: CalendarKind; date: string; title: string; time?: string; amount?: number; currency?: string; icon?: string; source_id?: string; overdue: boolean; pay_by?: string };
export type CalendarEventInput = { title: string; date: string; time: string; type: "custom" | "reminder" | "holiday"; repeat: "none" | "monthly" | "yearly"; amount: string; icon: string; notes: string };
export type CalendarEvent = { id: string; title: string; date: string; time: string | null; type: CalendarEventInput["type"]; amount: number | null; icon: string | null; notes: string | null; repeat: "monthly" | "yearly" | null };
export type IcsEvent = { uid: string | null; title: string; date: string; time: string | null; repeat: string | null; notes: string | null };

/** A reminder due now (see crates/vaulty-core/src/reminders.rs). `days` < 0 means overdue. */
export type Reminder = { key: string; kind: "bill" | "income" | "transfer" | "card" | "budget" | "price" | "owed" | "task" | "low" | "expiry" | "tax" | "premium"; name: string; date: string; days: number; amount: number | null; currency: string | null; percent: number | null; previous: number | null; detail: string | null };
/** An account below its low-balance level, or a scheduled payment in the next 14 days that would take it there. */
export type LowBalance = { account_id: string; name: string; currency: string; balance: number; level: number; now: boolean; date: string; because: string | null; lowest: number };

export type ReportSlice = { id: string | null; name: string; icon: string | null; color: string | null; amount: number; count: number };
/** A period's income and spending (main currency). Names "\u0000uncategorised" and "\u0000hidden" are placeholders. */
export type ReportOverview = {
  currency: string; months: { month: string; income: number; spending: number; net: number }[]; income: number; spending: number;
  savings_rate: number | null; categories: ReportSlice[]; income_sources: ReportSlice[]; places: ReportSlice[]; other_currencies: number;
};
export type Forecast = {
  currency: string; balance: number; typical_day: number; points: { date: string; scheduled: number; expected: number }[];
  changes: { date: string; title: string; amount: number; kind: string }[]; lowest: { date: string; scheduled: number; expected: number } | null; other_currencies: number;
};
export type SafeToSpend = { currency: string; balance: number; committed: number; amount: number; per_day: number; until: string; days: number; income_name: string | null; short: boolean };

/** Someone you lend to, borrow from or split bills with. `balances` > 0: they owe you. */
export type Person = { id: string; name: string; phone: string | null; notes: string | null; balances: { currency: string; amount: number; amount_minor: number }[]; last_date: string | null };
/** One event with a person, newest first; `amount` > 0 means they came to owe you more. */
export type PersonEvent = { entry_id: string; kind: string; what: "lent" | "received" | "split" | "they_paid"; date: string; merchant: string | null; description: string | null; memo: string | null; amount: number; balance: number; currency: string };

export type PriceChange = { from: number; to: number; percent: number; date: string };
/** A regular charge that looks like a subscription and isn't tracked yet. */
export type SubscriptionCandidate = {
  key: string; name: string; rhythm: "weekly" | "monthly" | "quarterly" | "yearly"; frequency: Frequency; every: number; amount: number; currency: string;
  count: number; last_date: string; next_date: string; account_id: string | null; category_id: string | null; price_change: PriceChange | null;
};
export type RecurringRise = PriceChange & { recurring_id: string; name: string; currency: string };

export type TaskLink = "recurring" | "transaction" | "person" | "event";
/** A to-do. `linked_name` is the linked item's current name (or the saved one if it's gone: `link_missing`). */
export type Task = {
  id: string; title: string; notes: string | null; due_date: string | null; priority: "low" | "medium" | "high"; status: "pending" | "done";
  linked_type: TaskLink | null; linked_id: string | null; linked_name: string | null; link_missing: boolean; overdue: boolean; completed_at: string | null;
};
export type TaskInput = { title: string; notes?: string; due_date?: string; priority: Task["priority"]; linked_type?: TaskLink | ""; linked_id?: string };

/** A newer version found by an update check. */
export type UpdateInfo = { version: string; current: string; notes: string | null; date: string | null };

/** A bank, card issuer, broker or wallet from the bundled directory. */
export type Institution = { id: string; name: string; short: string; kind: "bank" | "card" | "broker" | "wallet"; country: string; aliases: string[]; logo?: string };
export type Category = { id: string; name: string; type: "income" | "expense"; icon?: string; color?: string; is_system: number; parent_id?: string | null };
export type CategoryInput = { name: string; type: "income" | "expense"; icon?: string; color?: string; parent_id?: string | null };
export type BudgetLine = {
  id: string; name: string; type: "income" | "expense"; icon: string | null; color: string | null; parent_id: string | null; is_system: boolean;
  budget: number | null; spent: number; other_currencies: number;
};
export type BudgetMonth = { month: string; currency: string; categories: BudgetLine[]; uncategorised_expense: number; uncategorised_income: number };
export type Visibility = "private" | "shared" | "public";
export type Transaction = {
  id: string; account_id: string; to_account_id?: string; category_id?: string; type: "income" | "expense" | "transfer";
  /** Positive decimal for display; `amount_minor` is exact. */
  amount: number; amount_minor: number; date: string; to_account_name?: string;
  merchant?: string; description?: string; account_name?: string; category_name?: string; category_icon?: string; category_color?: string | null; currency?: string;
  visibility: Visibility;
  notes?: string | null;
  /** Comma-separated, e.g. "trip, goa". */
  tags?: string | null;
  /** Present when the amount is split across several categories. */
  splits?: SplitPart[] | null;
  /** 1 when the transaction lives in the Hidden section. */
  hidden?: number;
  /** Money lent to or received from a person (a transfer): who, and which way. */
  person_id?: string | null; person_name?: string | null; flow?: "out" | "in";
  /** True when the details were withheld: only amount, date, type and account are real. */
  masked?: boolean;
};
export type AppInfo = { version: string; demo: boolean; demo_password: string | null };
/** One part of a split: a category, or a person who owes you that part. */
export type SplitPart = { category_id: string | null; category_name?: string | null; category_icon?: string | null; category_color?: string | null; amount: number; memo?: string | null; person_id?: string | null; person_name?: string | null };
/** What the app sends to save a transaction. Split amounts are decimal strings. */
export type TransactionInput = Omit<Partial<Transaction>, "amount" | "hidden" | "splits"> & {
  amount: string | number; splits?: { category_id?: string | null; person_id?: string; amount: string; memo?: string }[];
};
export type Frequency = "daily" | "weekly" | "monthly" | "yearly";
export type Recurring = {
  id: string; name: string; type: "expense" | "income" | "transfer"; amount: number; amount_minor: number; currency: string;
  account_id: string | null; to_account_id: string | null; category_id: string | null; merchant: string | null; notes: string | null; tags: string | null;
  frequency: Frequency; every: number; start_date: string; end_date: string | null; next_due_date: string | null; last_paid_date: string | null;
  reminder_days: number; auto_pay: number; is_active: number; visibility: Visibility;
  account_name: string | null; to_account_name: string | null; category_name: string | null; category_icon: string | null;
  days_until: number | null; status: "overdue" | "due" | "soon" | "upcoming" | "paused" | "ended";
};
export type RecurringInput = {
  name: string; type: Recurring["type"]; amount: string; account_id: string; to_account_id?: string; category_id?: string; notes?: string; merchant?: string;
  frequency: Frequency; every: number; start_date: string; end_date?: string; reminder_days: number; auto_pay?: boolean;
};
export type Occurrence = { recurring_id: string; name: string; type: Recurring["type"]; date: string; amount: number; currency: string; category_icon: string | null; account_name: string | null };
export type ImportMapping = {
  header_row: number; date: number; description: number; amount?: number | null; debit?: number | null; credit?: number | null;
  side?: number | null; reference?: number | null; date_order: "dmy" | "mdy" | "ymd"; negative_is_expense: boolean;
};
export type StatementInfo = { headers: string[]; header_row: number; row_count: number; sample: string[][]; mapping: ImportMapping; saved: boolean };
export type ImportRow = {
  index: number; date: string | null; description: string; merchant: string; amount: string | null; kind: "income" | "expense" | null;
  reference: string | null; status: "new" | "duplicate" | "error" | "skipped"; message: string | null; matched_date: string | null;
};
export type Rule = { id: string; pattern: string; category_id: string; source: "learned" | "manual"; category_name: string; category_icon: string | null; category_type: "income" | "expense" };
export type QuickParsed = { kind: "income" | "expense"; amount: string | null; date: string; merchant: string; account_id: string | null; category_id: string | null; understood: [string, string][] };
export type BackupInfo = { encrypted: boolean; hint: string | null; created_at: string | null };
export type BulkAction = { category_id: string | null } | { add_tag: string } | { remove_tag: string } | { visibility: Visibility } | { hidden: boolean } | { delete: true };
export type HiddenStatus = { unlocked: boolean; seconds_left: number; count: number; biometric: boolean };

export type SharingPrefs = { enabled: boolean; default_visibility: Visibility; shared_with: string[] };
export type FamilyMember = User & { sharing_enabled: boolean };
/** A transaction someone published to the family view. */
export type FamilyItem = Omit<Transaction, "account_id" | "category_id" | "to_account_id" | "to_account_name"> & { owner_id: string; is_mine: number };
export type FeedView = "all" | "shared" | "public";
export type FeedFilter = { startDate?: string; endDate?: string; ownerId?: string; limit?: number };

/** Rust errors arrive as plain strings; normalise for display. */
/** A message for people, in their language (errors from the app are translated; see i18n/errors.ts). */
export const errorText = (e: unknown) => translateError(typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

export const api = {
  app: {
    info: () => invoke<AppInfo>("app_info"),
  },
  users: {
    list: () => invoke<User[]>("users_list"),
    add: (name: string, color: string, role: Role) => invoke<Invite>("users_add", { name, color, role }),
    /** Child profiles only: a parent's recovery copy keeps their data. */
    reset: (userId: string) => invoke<Invite>("users_reset", { userId }),
    /** Deletes the member's data for good and issues a new setup code. */
    erase: (userId: string, confirmName: string) => invoke<Invite>("users_erase", { userId, confirmName }),
    remove: (userId: string, confirmName: string) => invoke<void>("users_remove", { userId, confirmName }),
    updateProfile: (name: string, color: string) => invoke<User>("profile_update", { name, color }),
    /** Own profile, or anyone's for the admin. */
    setSenior: (userId: string, on: boolean) => invoke<User>("users_set_senior", { userId, on }),
  },
  auth: {
    current: () => invoke<User | null>("session_current"),
    /** Tells the app the person is still using it (it locks on its own otherwise). */
    touch: () => invoke<void>("session_touch"),
    setupFirstAdmin: (name: string, color: string, password: string) => invoke<SignedInUser>("setup_first_admin", { name, color, password }),
    claimProfile: (userId: string, setupCode: string, password: string) => invoke<SignedInUser>("claim_profile", { userId, setupCode, password }),
    signIn: (userId: string, password: string) => invoke<SignIn>("sign_in", { userId, password }),
    signInTotp: (code: string) => invoke<SignedInUser>("sign_in_totp", { code }),
    signInBackupCode: (code: string) => invoke<SignedInUser>("sign_in_backup_code", { code }),
    signInBiometric: (userId: string) => invoke<SignedInUser>("sign_in_biometric", { userId }),
    signInRecovery: (userId: string, recoveryKey: string, newPassword: string) =>
      invoke<SignedInUser>("sign_in_recovery", { userId, recoveryKey, newPassword }),
    signOut: () => invoke<void>("sign_out"),
  },
  helpers: {
    /** On the sign-in screen: who can help this member back in. */
    forUser: (userId: string) => invoke<Helpers>("helpers_for", { userId }),
    approveMember: (userId: string, helperId: string, password: string) => invoke<number[]>("helpers_approve_member", { userId, helperId, password }),
    approveCard: (userId: string, code: string) => invoke<number[]>("helpers_approve_card", { userId, code }),
    cancel: () => invoke<void>("helpers_cancel"),
    signIn: (userId: string, newPassword: string, code?: string) => invoke<SignedInUser>("sign_in_helpers", { userId, newPassword, code: code || null }),
    mine: () => invoke<{ helpers: Helpers; candidates: User[] }>("helpers_mine"),
    set: (password: string, choices: HelperChoice[]) => invoke<HelperCard[]>("helpers_set", { password, choices }),
    remove: (password: string) => invoke<void>("helpers_remove", { password }),
  },
  nominee: {
    forUser: (userId: string) => invoke<NomineeStatus>("nominee_for", { userId }),
    start: (userId: string, code: string) => invoke<NomineeStatus>("nominee_start", { userId, code }),
    open: (userId: string, code: string) => invoke<NomineeOpened>("sign_in_nominee", { userId, code }),
    mine: () => invoke<{ nominee: NomineeStatus; letter: string | null }>("nominee_mine"),
    set: (password: string, name: string, waitDays: number, letter: string) => invoke<HelperCard>("nominee_set", { password, name, waitDays, letter: letter.trim() || null }),
    remove: (password: string) => invoke<void>("nominee_remove", { password }),
    answer: (allow: boolean) => invoke<void>("nominee_answer", { allow }),
    preview: () => invoke<Estate>("estate_preview"),
  },
  security: {
    status: () => invoke<SecurityStatus>("security_status"),
    changePassword: (current: string, next: string) => invoke<void>("password_change", { current, new: next }),
    totpBegin: (password: string) => invoke<TotpEnrollment>("totp_begin", { password }),
    totpConfirm: (password: string, secret: string, code: string) => invoke<string[]>("totp_confirm", { password, secret, code }),
    totpDisable: (password: string) => invoke<void>("totp_disable", { password }),
    regenerateBackupCodes: (password: string) => invoke<string[]>("backup_codes_regenerate", { password }),
    biometricEnable: (password: string) => invoke<void>("biometric_enable", { password }),
    biometricDisable: () => invoke<void>("biometric_disable"),
    newRecoveryKey: (password: string) => invoke<string>("recovery_key_new", { password }),
  },
  accounts: {
    getAll: () => invoke<Account[]>("accounts_get_all"),
    create: (account: NewAccount) => invoke<Account>("accounts_create", { account }),
    /** Partial update: only the fields given change. */
    update: (id: string, account: Partial<NewAccount>) => invoke<Account>("accounts_update", { id, account }),
    delete: (id: string) => invoke<void>("accounts_delete", { id }),
  },
  categories: {
    getAll: () => invoke<Category[]>("categories_get_all"),
    create: (category: CategoryInput) => invoke<Category>("categories_create", { category }),
    update: (id: string, category: CategoryInput) => invoke<Category>("categories_update", { id, category }),
    delete: (id: string) => invoke<void>("categories_delete", { id }),
  },
  rules: {
    suggest: (merchant: string, kind: string) => invoke<string | null>("rules_suggest", { merchant, kind }),
    list: () => invoke<Rule[]>("rules_list"),
    add: (pattern: string, categoryId: string) => invoke<Rule>("rules_add", { pattern, categoryId }),
    delete: (id: string) => invoke<void>("rules_delete", { id }),
    pending: () => invoke<number>("rules_pending"),
    apply: () => invoke<number>("rules_apply"),
  },
  backup: {
    /** `document_files_left_out`: there were too many document files to carry; the backup has their details. */
    export: (path: string, password: string, hint?: string) => invoke<{ document_files_left_out: boolean }>("backup_export", { path, password, hint }),
    inspect: (path: string) => invoke<BackupInfo>("backup_inspect", { path }),
    import: (path: string, password?: string) => invoke<{ transactions: number; accounts: number; categories: number }>("backup_import", { path, password }),
    exportCsv: (path: string) => invoke<number>("export_csv", { path }),
  },
  /** Opens the system file dialog from the app itself; file commands only accept paths chosen here. */
  files: { pick: (kind: "statement" | "backup" | "save_backup" | "save_csv" | "calendar" | "save_calendar" | "save_report" | "document" | "save_document" | "fund_statement" | "save_project" | "project_share" | "save_project_share" | "tax_form", label: string, name?: string) => invoke<string | null>("pick_file", { kind, label, name: name ?? null }) },
  /** This computer: the tray icon, the quick-add shortcut, keeping Vaulty running when closed. */
  device: {
    get: () => invoke<DeviceSettings>("app_settings_get"),
    set: (key: "tray" | "keep_running" | "shortcut", value: boolean | string) => invoke<void>("app_settings_set", { key, value }),
    trayLabels: (labels: string[]) => invoke<void>("tray_labels", { labels }),
  },
  cardExtras: {
    cycles: (cardId: string) => invoke<CardCycles | null>("card_cycles_get", { cardId }),
    enterStatement: (cardId: string, statement: { statement_date: string; total: string; min?: string }) => invoke<void>("card_statement_enter", { cardId, statement }),
    forgetStatement: (cardId: string, statementDate: string) => invoke<void>("card_statement_forget", { cardId, statementDate }),
    benefits: (cardId: string) => invoke<CardBenefit[]>("card_benefits_list", { cardId }),
    addBenefit: (cardId: string, benefit: BenefitInput) => invoke<{ id: string }>("card_benefits_create", { cardId, benefit }),
    updateBenefit: (id: string, benefit: BenefitInput) => invoke<void>("card_benefits_update", { id, benefit }),
    deleteBenefit: (id: string) => invoke<void>("card_benefits_delete", { id }),
    usePerk: (id: string, by: 1 | -1) => invoke<number>("card_benefits_use", { id, by }),
    best: (amount?: string, categoryId?: string, place?: string) => invoke<BestPick[]>("cards_best", { amount: amount || null, categoryId: categoryId || null, place: place || null }),
  },
  projects: {
    list: () => invoke<Project[]>("projects_list"),
    get: (id: string) => invoke<ProjectDetail>("projects_get", { id }),
    items: (id: string) => invoke<ProjectItem[]>("projects_items", { id }),
    payments: (id: string) => invoke<ProjectPayment[]>("projects_payments", { id }),
    linkSuggestions: (id: string) => invoke<LinkSuggestion[]>("projects_link_suggestions", { id }),
    names: () => invoke<{ vendors: string[]; account_names: string[]; units: string[] }>("projects_names"),
    create: (project: ProjectInput) => invoke<ProjectDetail>("projects_create", { project }),
    update: (id: string, project: ProjectInput) => invoke<ProjectDetail>("projects_update", { id, project }),
    delete: (id: string) => invoke<void>("projects_delete", { id }),
    addSection: (projectId: string, section: { name: string; budget?: string }) => invoke<{ id: string }>("projects_section_add", { projectId, section }),
    updateSection: (id: string, section: { name: string; budget?: string }) => invoke<void>("projects_section_update", { id, section }),
    deleteSection: (id: string) => invoke<void>("projects_section_delete", { id }),
    orderSections: (projectId: string, ids: string[]) => invoke<void>("projects_section_order", { projectId, ids }),
    addItem: (projectId: string, item: ItemInput) => invoke<ProjectItem>("projects_item_add", { projectId, item }),
    updateItem: (id: string, item: ItemInput) => invoke<ProjectItem>("projects_item_update", { id, item }),
    deleteItem: (id: string) => invoke<void>("projects_item_delete", { id }),
    addPayment: (projectId: string, payment: PaymentInput) => invoke<ProjectPayment>("projects_payment_add", { projectId, payment }),
    updatePayment: (id: string, payment: PaymentInput) => invoke<ProjectPayment>("projects_payment_update", { id, payment }),
    deletePayment: (id: string) => invoke<void>("projects_payment_delete", { id }),
    exportXlsx: (id: string, path: string, labels: Record<string, string>) => invoke<void>("projects_export_xlsx", { id, path, labels }),
    files: (id: string) => invoke<ProjectFile[]>("projects_files", { id }),
    addFile: (projectId: string, path: string, link: FileLink) => invoke<ProjectFile>("projects_file_add", { projectId, path, link }),
    linkFile: (id: string, link: FileLink) => invoke<ProjectFile>("projects_file_link", { id, link }),
    removeFile: (id: string) => invoke<void>("projects_file_remove", { id }),
    previewFile: (id: string) => invoke<{ name: string; mime: string; data: string }>("projects_file_preview", { id }),
    saveFile: (id: string, path: string) => invoke<void>("projects_file_save", { id, path }),
    members: (id: string) => invoke<ProjectMember[]>("projects_members", { id }),
    setMembers: (id: string, members: ProjectMember[]) => invoke<ProjectMember[]>("projects_members_set", { id, members }),
    split: (id: string) => invoke<ProjectSplit>("projects_split", { id }),
    settlePreview: (id: string) => invoke<SettleLine[]>("projects_settle_preview", { id }),
    settleRecord: (id: string, settle: { date?: string; category_id?: string; people: Record<string, string> }) => invoke<number>("projects_settle_record", { id, settle }),
    shareExport: (id: string, path: string, password: string, withFiles: boolean) => invoke<{ files: number; files_left_out: number }>("projects_share_export", { id, path, password, withFiles }),
    sharePeek: (path: string, password: string) => invoke<SharePeek>("projects_share_peek", { path, password }),
    shareImport: (path: string, password: string, me: string | null) => invoke<{ id: string; updated: boolean; files: number }>("projects_share_import", { path, password, me }),
    readBill: (id: string) => invoke<BillDraft>("projects_read_bill", { id }),
    addItems: (projectId: string, batch: { items: ItemInput[]; file_id?: string }) => invoke<ProjectItem[]>("projects_items_add", { projectId, batch }),
  },
  goals: {
    list: () => invoke<Goal[]>("goals_list"),
    summary: () => invoke<GoalTotal[]>("goals_summary"),
    create: (goal: GoalInput) => invoke<{ id: string }>("goals_create", { goal }),
    update: (id: string, goal: GoalInput) => invoke<void>("goals_update", { id, goal }),
    setDone: (id: string, done: boolean) => invoke<void>("goals_set_done", { id, done }),
    delete: (id: string) => invoke<void>("goals_delete", { id }),
  },
  insurance: {
    list: () => invoke<Policy[]>("insurance_list"),
    summary: () => invoke<InsuranceTotal[]>("insurance_summary"),
    create: (policy: PolicyInput) => invoke<{ id: string }>("insurance_create", { policy }),
    update: (id: string, policy: PolicyInput) => invoke<void>("insurance_update", { id, policy }),
    delete: (id: string) => invoke<void>("insurance_delete", { id }),
    pay: (id: string, payment: { date: string; amount?: string; record?: { account_id: string; category_id: string | null } }) => invoke<{ id: string }>("insurance_pay", { id, payment }),
    deletePayment: (id: string) => invoke<void>("insurance_payment_delete", { id }),
    addClaim: (policyId: string, claim: Record<string, unknown>) => invoke<{ id: string }>("insurance_claim_add", { policyId, claim }),
    updateClaim: (id: string, claim: Record<string, unknown>) => invoke<void>("insurance_claim_update", { id, claim }),
    deleteClaim: (id: string) => invoke<void>("insurance_claim_delete", { id }),
  },
  calc: { run: <T = Record<string, unknown>>(kind: string, input: Record<string, unknown>) => invoke<T>("calc_run", { kind, input }) },
  investments: {
    list: () => invoke<Holding[]>("investments_list"),
    summary: () => invoke<InvestmentTotal[]>("investments_summary"),
    transactions: (id: string) => invoke<HoldingTxn[]>("investments_transactions", { id }),
    create: (holding: HoldingInput) => invoke<Holding>("investments_create", { holding }),
    update: (id: string, holding: HoldingInput) => invoke<Holding>("investments_update", { id, holding }),
    setPrice: (id: string, price: string) => invoke<Holding>("investments_set_price", { id, price }),
    delete: (id: string) => invoke<void>("investments_delete", { id }),
    fundPricesStatus: () => invoke<FundPriceStatus>("fund_prices_status"),
    fundPricesUpdate: (now: boolean) => invoke<{ skipped?: boolean; updated?: number; found?: number; as_of?: string | null }>("fund_prices_update", { now }),
    previewStatement: (path: string, password: string) => invoke<CasPreview>("cas_preview", { path, password: password || null }),
    importStatement: (path: string, password: string) => invoke<{ added: number; updated: number; transactions: number }>("cas_import", { path, password: password || null }),
  },
  documents: {
    list: () => invoke<Doc[]>("documents_list"),
    create: (document: DocInput) => invoke<Doc>("documents_create", { document }),
    update: (id: string, document: DocInput) => invoke<Doc>("documents_update", { id, document }),
    delete: (id: string) => invoke<void>("documents_delete", { id }),
    addFile: (documentId: string, path: string) => invoke<DocFile>("documents_add_file", { documentId, path }),
    removeFile: (fileId: string) => invoke<void>("documents_remove_file", { fileId }),
    /** Photos only, as base64. */
    preview: (fileId: string) => invoke<{ name: string; mime: string; data: string }>("documents_preview", { fileId }),
    saveFile: (fileId: string, path: string) => invoke<void>("documents_save_file", { fileId, path }),
  },
  updates: {
    check: () => invoke<UpdateInfo | null>("update_check"),
    auto: () => invoke<UpdateInfo | null>("update_auto"),
    install: () => invoke<void>("update_install"),
  },
  hotkeys: {
    list: () => invoke<Hotkey[]>("hotkeys_list"),
    /** `keys`: the new combination, "" to turn it off, or null for the default. */
    set: (action: string, keys: string | null) => invoke<Hotkey[]>("hotkeys_set", { action, keys }),
    reset: () => invoke<Hotkey[]>("hotkeys_reset"),
  },
  calendar: {
    range: (from: string, to: string) => invoke<CalendarItem[]>("calendar_range", { from, to }),
    events: () => invoke<CalendarEvent[]>("calendar_events"),
    create: (event: Omit<CalendarEventInput, never>) => invoke<CalendarEvent>("calendar_event_create", { event }),
    update: (id: string, event: CalendarEventInput) => invoke<CalendarEvent>("calendar_event_update", { id, event }),
    delete: (id: string) => invoke<void>("calendar_event_delete", { id }),
    export: (path: string, from: string, to: string, includeAmounts: boolean) => invoke<number>("calendar_export", { path, from, to, includeAmounts }),
    importPreview: (path: string) => invoke<IcsEvent[]>("calendar_import_preview", { path }),
    import: (path: string, asHolidays: boolean) => invoke<number>("calendar_import", { path, asHolidays }),
  },
  reminders: {
    take: () => invoke<Reminder[]>("reminders_take"),
    /** `ask` shows the system's permission prompt if it hasn't been answered yet. */
    permission: (ask = false) => invoke<"granted" | "denied" | "prompt">("notify_permission", { ask }),
    show: (title: string, body: string) => invoke<void>("notify_show", { title, body }),
  },
  reports: {
    overview: (from: string, to: string) => invoke<ReportOverview>("reports_overview", { from, to }),
    forecast: (days: number) => invoke<Forecast>("reports_forecast", { days }),
    safeToSpend: () => invoke<SafeToSpend>("reports_safe_to_spend"),
    lowBalances: () => invoke<LowBalance[]>("reports_low_balances"),
    netWorth: (months: number) => invoke<NetWorth[]>("networth_timeline", { months }),
    exportXlsx: (path: string, from: string, to: string, labels: Record<string, string>) => invoke<void>("reports_export_xlsx", { path, from, to, labels }),
  },
  people: {
    list: () => invoke<Person[]>("people_list"),
    totals: () => invoke<{ currency: string; owed_to_me: number; i_owe: number }[]>("people_totals"),
    history: (id: string) => invoke<PersonEvent[]>("people_history", { id }),
    create: (person: { name: string; phone?: string; notes?: string }) => invoke<Person>("people_create", { person }),
    update: (id: string, person: { name: string; phone?: string; notes?: string }) => invoke<Person>("people_update", { id, person }),
    delete: (id: string) => invoke<void>("people_delete", { id }),
  },
  subscriptions: {
    detect: () => invoke<SubscriptionCandidate[]>("subscriptions_detect"),
    dismiss: (key: string) => invoke<void>("subscriptions_dismiss", { key }),
    rises: () => invoke<RecurringRise[]>("subscriptions_rises"),
  },
  tasks: {
    list: (status: "open" | "done" | "all") => invoke<Task[]>("tasks_list", { status }),
    create: (task: TaskInput) => invoke<Task>("tasks_create", { task }),
    update: (id: string, task: TaskInput) => invoke<Task>("tasks_update", { id, task }),
    setDone: (id: string, done: boolean) => invoke<Task>("tasks_set_done", { id, done }),
    delete: (id: string) => invoke<void>("tasks_delete", { id }),
  },
  quick: { parse: (text: string) => invoke<QuickParsed>("quick_parse", { text }) },
  statements: {
    inspect: (path: string, accountId: string, password?: string) => invoke<StatementInfo>("statement_inspect", { path, accountId, password: password || null }),
    preview: (path: string, accountId: string, mapping: ImportMapping, password?: string) => invoke<ImportRow[]>("statement_preview", { path, accountId, mapping, password: password || null }),
    import: (path: string, accountId: string, mapping: ImportMapping, skip: number[], include: number[], password?: string) =>
      invoke<number>("statement_import", { path, accountId, mapping, skip, include, password: password || null }),
  },
  recurring: {
    list: () => invoke<Recurring[]>("recurring_list"),
    create: (item: RecurringInput) => invoke<Recurring>("recurring_create", { item }),
    update: (id: string, item: RecurringInput) => invoke<Recurring>("recurring_update", { id, item }),
    setActive: (id: string, active: boolean) => invoke<Recurring>("recurring_set_active", { id, active }),
    delete: (id: string) => invoke<void>("recurring_delete", { id }),
    skip: (id: string) => invoke<Recurring>("recurring_skip", { id }),
    pay: (id: string, date?: string, amount?: string) => invoke<Transaction>("recurring_pay", { id, date, amount }),
    upcoming: (from: string, to: string) => invoke<Occurrence[]>("recurring_upcoming", { from, to }),
  },
  budgets: {
    month: (month: string) => invoke<BudgetMonth>("budgets_month", { month }),
    /** `amount` "" or null removes the budget. */
    set: (categoryId: string, amount: string | null) => invoke<unknown>("budgets_set", { categoryId, amount }),
  },
  /** Per-user preferences stored in the user's own database. */
  settings: {
    get: () => invoke<UserSettings>("settings_get"),
    update: (settings: Partial<UserSettings>) => invoke<UserSettings>("settings_update", { settings }),
  },
  cards: {
    list: () => invoke<WalletCard[]>("cards_list"),
    create: (card: CardInput) => invoke<WalletCard>("cards_create", { card }),
    update: (id: string, card: CardInput) => invoke<WalletCard>("cards_update", { id, card }),
    delete: (id: string) => invoke<void>("cards_delete", { id }),
  },
  market: {
    get: () => invoke<MarketView>("market_get"),
    update: (prefs: Partial<Omit<MarketPrefs, "show">> & { show?: Partial<MarketPrefs["show"]> }) => invoke<MarketView>("market_update", { prefs }),
    setKey: (which: "stock" | "crypto", key: string) => invoke<MarketView>("market_set_key", { which, key }),
    quotes: () => invoke<Quote[]>("market_quotes"),
    refresh: () => invoke<{ quotes: Quote[]; problems: { source: string; symbol?: string; message: string }[] }>("market_refresh"),
  },
  snapshots: {
    list: () => invoke<Snapshot[]>("snapshots_list"),
    create: () => invoke<Snapshot>("snapshots_create"),
    restore: async (name: string) => { await invoke<void>("snapshots_restore", { name }); window.dispatchEvent(new Event(DATA_RESTORED)); },
  },
  history: {
    recent: (limit = 100) => invoke<HistoryItem[]>("history_recent", { limit }),
    trash: () => invoke<HistoryItem[]>("history_trash"),
    item: (table: string, id: string) => invoke<HistoryItem[]>("history_item", { table, id }),
    verify: () => invoke<HistoryCheck>("history_verify"),
    undo: async (batchId: number) => { await invoke<void>("history_undo", { batchId }); window.dispatchEvent(new Event(DATA_RESTORED)); },
    undoLast: async () => {
      const done = await invoke<HistoryItem | null>("history_undo_last");
      if (done) window.dispatchEvent(new Event(DATA_RESTORED));
      return done;
    },
  },
  tax: {
    countries: () => invoke<string[]>("tax_countries"),
    describe: (country: string) => invoke<TaxPack>("tax_describe", { country }),
    compare: (country: string, input: TaxInput, year?: string) => invoke<TaxComparison>("tax_compare", { country, year, input }),
    plan: (country: string, year: string) => invoke<TaxPlan>("tax_plan_get", { country, year }),
    savePlan: (country: string, year: string, plan: { input: TaxInput; regime: string | null; hra: HraInput }) => invoke<void>("tax_plan_save", { country, year, plan }),
    addPayment: (country: string, year: string, payment: { kind: TaxPayment["kind"]; date: string; amount: string; note?: string }) => invoke<{ id: string }>("tax_payment_add", { country, year, payment }),
    deletePayment: (id: string) => invoke<void>("tax_payment_delete", { id }),
    readForm16: (path: string, password?: string) => invoke<Form16Offer>("form16_read", { path, password: password || null }),
    applyForm16: (country: string, year: string, data: { input: TaxInput; regime?: string | null; payments: { date: string; amount: number; note: string }[] }) => invoke<{ payments: number }>("form16_apply", { country, year, data }),
    setLinks: (country: string, year: string, links: { category_id: string; target: string }[]) => invoke<TaxLink[]>("tax_links_set", { country, year, links }),
  },
  institutions: {
    search: (query: string, currency?: string, limit = 20) => invoke<Institution[]>("institutions_search", { query, currency, limit }),
  },
  transactions: {
    getAll: (filters?: { accountId?: string; categoryId?: string; type?: string; visibility?: Visibility; startDate?: string; endDate?: string; limit?: number;
      hidden?: "only" | "exclude"; reveal_hidden?: boolean }) =>
      invoke<Transaction[]>("transactions_get_all", { filters }),
    create: (transaction: TransactionInput & { hidden?: boolean }) =>
      invoke<Transaction>("transactions_create", { transaction }),
    update: (id: string, transaction: TransactionInput) =>
      invoke<Transaction>("transactions_update", { id, transaction }),
    setVisibility: (id: string, visibility: Visibility) => invoke<Transaction>("transactions_set_visibility", { id, visibility }),
    delete: (id: string) => invoke<void>("transactions_delete", { id }),
    bulk: (ids: string[], action: BulkAction) => invoke<number>("transactions_bulk", { ids, action }),
    setHidden: (id: string, hidden: boolean) => invoke<Transaction>("transactions_set_hidden", { id, hidden }),
  },
  /** The Hidden section: details of hidden transactions are only sent while it is unlocked. */
  hidden: {
    status: () => invoke<HiddenStatus>("hidden_status"),
    unlock: (password: string) => invoke<HiddenStatus>("hidden_unlock", { password }),
    unlockBiometric: () => invoke<HiddenStatus>("hidden_unlock_biometric"),
    lock: () => invoke<void>("hidden_lock"),
  },
  sharing: {
    /** `available` is false until another member has signed in once and shared the family key. */
    get: () => invoke<{ prefs: SharingPrefs; members: FamilyMember[]; available: boolean }>("sharing_get"),
    update: (prefs: SharingPrefs) => invoke<SharingPrefs>("sharing_update", { prefs }),
    feed: (view: FeedView, filter?: FeedFilter) => invoke<FamilyItem[]>("family_feed", { view, filter }),
  },
};

/** What the OS calls its biometric/device unlock, for button labels. */
export function biometricLabel() {
  const ua = navigator.userAgent;
  if (/Mac/.test(ua)) return "Touch ID";
  if (/Windows/.test(ua)) return "Windows Hello";
  return "system unlock";
}
