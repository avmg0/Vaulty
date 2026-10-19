import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/** Fired after any change command succeeds, so the undo toast can offer to reverse it. */
export const DATA_CHANGED = "vaulty:data-changed";
/** Fired after an undo or restore, so open pages reload. */
export const DATA_RESTORED = "vaulty:data-restored";

/** Commands the app records in history (and so can undo). Keep in step with `s.act` in src-tauri/src/lib.rs. */
const RECORDED = new RegExp([
  "^(accounts|categories|transactions|cards)_(create|update|delete|set_visibility)$", "^transactions_(set_hidden|bulk)$",
  "^settings_update$", "^import_json$", "^backup_import$", "^statement_import$", "^budgets_set$",
  "^recurring_(create|update|delete|set_active|skip|pay)$", "^rules_(add|delete|apply)$",
].join("|"));

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const out = await tauriInvoke<T>(cmd, args);
  if (RECORDED.test(cmd)) window.dispatchEvent(new CustomEvent(DATA_CHANGED, { detail: { cmd } }));
  return out;
}

export type Role = "admin" | "member" | "child";
export type User = { id: string; name: string; color: string; role: Role; active: boolean; totp_enabled: boolean; biometric_enabled: boolean };
export type Invite = { user: User; setup_code: string };
/** A new recovery key appears only right after it was created and must be shown once. */
export type SignedInUser = User & { recovery_key?: string | null };
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
export type TaxComparison = {
  country: string; currency: string; year: { id: string; label: string }; estimates: TaxEstimate[];
  recommended: string | null; saving: number; sources: string[]; last_checked: string; notes?: string;
};
export type MarketItem = { symbol: string; label: string };
export type MarketPrefs = {
  enabled: boolean; show: { regional: boolean; crypto: boolean; watchlist: boolean }; region: string; crypto: string[];
  crypto_currency: string; watchlist: MarketItem[]; stock_provider: "none" | "twelvedata"; refresh_minutes: number; speed: "slow" | "normal" | "fast";
};
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
};
export type NewAccount = {
  name: string; type: string; currency: string; initial_balance?: string | number; color?: string; icon?: string;
  institution_id?: string | null; logo?: string | null;
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
}>;

/** Subset of user_settings the UI uses. `locale` is BCP 47, e.g. "hi-IN" (language + country). */
export type UserSettings = { locale: string; currency: string; date_format: string; start_of_week: string; fiscal_year_start: string; negative_format: string; decimals: string };

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
  /** True when the details were withheld: only amount, date, type and account are real. */
  masked?: boolean;
};
export type AppInfo = { version: string; demo: boolean; demo_password: string | null };
export type SplitPart = { category_id: string | null; category_name?: string | null; category_icon?: string | null; category_color?: string | null; amount: number; memo?: string | null };
/** What the app sends to save a transaction. Split amounts are decimal strings. */
export type TransactionInput = Omit<Partial<Transaction>, "amount" | "hidden" | "splits"> & {
  amount: string | number; splits?: { category_id?: string | null; amount: string; memo?: string }[];
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
  name: string; type: Recurring["type"]; amount: string; account_id: string; to_account_id?: string; category_id?: string; notes?: string;
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
export const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

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
  },
  auth: {
    current: () => invoke<User | null>("session_current"),
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
    export: (path: string, password: string, hint?: string) => invoke<void>("backup_export", { path, password, hint }),
    inspect: (path: string) => invoke<BackupInfo>("backup_inspect", { path }),
    import: (path: string, password?: string) => invoke<{ transactions: number; accounts: number; categories: number }>("backup_import", { path, password }),
    exportCsv: (path: string) => invoke<number>("export_csv", { path }),
  },
  /** Opens the system file dialog from the app itself; file commands only accept paths chosen here. */
  files: { pick: (kind: "statement" | "backup" | "save_backup" | "save_csv", label: string) => invoke<string | null>("pick_file", { kind, label }) },
  quick: { parse: (text: string) => invoke<QuickParsed>("quick_parse", { text }) },
  statements: {
    inspect: (path: string, accountId: string) => invoke<StatementInfo>("statement_inspect", { path, accountId }),
    preview: (path: string, accountId: string, mapping: ImportMapping) => invoke<ImportRow[]>("statement_preview", { path, accountId, mapping }),
    import: (path: string, accountId: string, mapping: ImportMapping, skip: number[], include: number[]) =>
      invoke<number>("statement_import", { path, accountId, mapping, skip, include }),
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
