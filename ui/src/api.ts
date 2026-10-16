import { invoke } from "@tauri-apps/api/core";

export type Role = "admin" | "member" | "child";
export type User = { id: string; name: string; color: string; role: Role; active: boolean; totp_enabled: boolean; biometric_enabled: boolean };
export type Invite = { user: User; setup_code: string };
export type SignIn = { status: "signed_in"; user: User } | { status: "needs_second_factor" };
export type TotpEnrollment = { secret_base32: string; otpauth_url: string; qr_png_base64: string };
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
/** A bank, card issuer, broker or wallet from the bundled directory. */
export type Institution = { id: string; name: string; short: string; kind: "bank" | "card" | "broker" | "wallet"; country: string; aliases: string[]; logo?: string };
export type Category = { id: string; name: string; type: "income" | "expense"; icon?: string; color?: string; is_system: number };
export type Visibility = "private" | "shared" | "public";
export type Transaction = {
  id: string; account_id: string; to_account_id?: string; category_id?: string; type: "income" | "expense" | "transfer";
  /** Positive decimal for display; `amount_minor` is exact. */
  amount: number; amount_minor: number; date: string; to_account_name?: string;
  merchant?: string; description?: string; account_name?: string; category_name?: string; category_icon?: string; currency?: string;
  visibility: Visibility;
};

export type SharingPrefs = { enabled: boolean; default_visibility: Visibility; shared_with: string[] };
export type FamilyMember = User & { sharing_enabled: boolean };
/** A transaction someone published to the family view. */
export type FamilyItem = Omit<Transaction, "account_id" | "category_id" | "to_account_id" | "to_account_name"> & { owner_id: string; is_mine: number };
export type FeedView = "all" | "shared" | "public";
export type FeedFilter = { startDate?: string; endDate?: string; ownerId?: string; limit?: number };

/** Rust errors arrive as plain strings; normalise for display. */
export const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

export const api = {
  users: {
    list: () => invoke<User[]>("users_list"),
    add: (name: string, color: string, role: Role) => invoke<Invite>("users_add", { name, color, role }),
    reset: (userId: string) => invoke<Invite>("users_reset", { userId }),
    remove: (userId: string) => invoke<void>("users_remove", { userId }),
    updateProfile: (name: string, color: string) => invoke<User>("profile_update", { name, color }),
  },
  auth: {
    current: () => invoke<User | null>("session_current"),
    setupFirstAdmin: (name: string, color: string, password: string) => invoke<User>("setup_first_admin", { name, color, password }),
    claimProfile: (userId: string, setupCode: string, password: string) => invoke<User>("claim_profile", { userId, setupCode, password }),
    signIn: (userId: string, password: string) => invoke<SignIn>("sign_in", { userId, password }),
    signInTotp: (code: string) => invoke<User>("sign_in_totp", { code }),
    signInBackupCode: (code: string) => invoke<User>("sign_in_backup_code", { code }),
    signInBiometric: (userId: string) => invoke<User>("sign_in_biometric", { userId }),
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
  },
  accounts: {
    getAll: () => invoke<Account[]>("accounts_get_all"),
    create: (account: NewAccount) => invoke<Account>("accounts_create", { account }),
    /** Partial update: only the fields given change. */
    update: (id: string, account: Partial<NewAccount>) => invoke<Account>("accounts_update", { id, account }),
    delete: (id: string) => invoke<void>("accounts_delete", { id }),
  },
  categories: { getAll: () => invoke<Category[]>("categories_get_all") },
  institutions: {
    search: (query: string, currency?: string, limit = 20) => invoke<Institution[]>("institutions_search", { query, currency, limit }),
  },
  transactions: {
    getAll: (filters?: { accountId?: string; categoryId?: string; type?: string; visibility?: Visibility; startDate?: string; endDate?: string; limit?: number }) =>
      invoke<Transaction[]>("transactions_get_all", { filters }),
    create: (transaction: Omit<Partial<Transaction>, "amount"> & { amount: string | number }) =>
      invoke<Transaction>("transactions_create", { transaction }),
    setVisibility: (id: string, visibility: Visibility) => invoke<Transaction>("transactions_set_visibility", { id, visibility }),
    delete: (id: string) => invoke<void>("transactions_delete", { id }),
  },
  sharing: {
    get: () => invoke<{ prefs: SharingPrefs; members: FamilyMember[] }>("sharing_get"),
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
