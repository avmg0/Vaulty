import {
  LayoutDashboard, ArrowLeftRight, Wallet, CreditCard, Repeat, TrendingUp, Target, Shield,
  CalendarDays, ListTodo, PiggyBank, NotebookPen, BarChart3, Calculator, History, Upload, FileText, Briefcase, Users, Settings,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "dashboard" | "transactions" | "accounts" | "cards" | "budgets" | "recurring" | "investments" | "goals"
  | "insurance" | "tax" | "calendar" | "tasks" | "journal" | "reports" | "import" | "documents"
  | "business" | "family" | "settings" | "history";

/** Labels come from the translation files: `nav.<id>` and `nav.group.<group>`. */
export type NavItem = { id: PageId; icon: LucideIcon; phase: number };

/** `phase` = roadmap phase in which the page gets built (see ROADMAP.md). */
export const NAV: { group: "overview" | "money" | "wealth" | "planner" | "data" | "more"; items: NavItem[] }[] = [
  { group: "overview", items: [
    { id: "dashboard", icon: LayoutDashboard, phase: 1 },
    { id: "reports", icon: BarChart3, phase: 2 },
  ]},
  { group: "money", items: [
    { id: "transactions", icon: ArrowLeftRight, phase: 1 },
    { id: "accounts", icon: Wallet, phase: 1 },
    { id: "cards", icon: CreditCard, phase: 1 },
    { id: "budgets", icon: PiggyBank, phase: 1 },
    { id: "recurring", icon: Repeat, phase: 2 },
  ]},
  { group: "wealth", items: [
    { id: "investments", icon: TrendingUp, phase: 3 },
    { id: "goals", icon: Target, phase: 3 },
    { id: "insurance", icon: Shield, phase: 3 },
    { id: "tax", icon: Calculator, phase: 2 },
  ]},
  { group: "planner", items: [
    { id: "calendar", icon: CalendarDays, phase: 2 },
    { id: "tasks", icon: ListTodo, phase: 2 },
    { id: "journal", icon: NotebookPen, phase: 3 },
  ]},
  { group: "data", items: [
    { id: "import", icon: Upload, phase: 2 },
    { id: "documents", icon: FileText, phase: 3 },
    { id: "history", icon: History, phase: 1 },
  ]},
  { group: "more", items: [
    { id: "business", icon: Briefcase, phase: 4 },
    { id: "family", icon: Users, phase: 1 },
  ]},
];

export const SETTINGS_ITEM: NavItem = { id: "settings", icon: Settings, phase: 1 };
export const ALL_ITEMS = [...NAV.flatMap((g) => g.items), SETTINGS_ITEM];
