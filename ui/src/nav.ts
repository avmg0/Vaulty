import {
  LayoutDashboard, ArrowLeftRight, Wallet, CreditCard, Repeat, TrendingUp, Target, Shield,
  CalendarDays, ListTodo, NotebookPen, BarChart3, Upload, FileText, Briefcase, Users, Settings,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "dashboard" | "transactions" | "accounts" | "cards" | "recurring" | "investments" | "goals"
  | "insurance" | "calendar" | "tasks" | "journal" | "reports" | "import" | "documents"
  | "business" | "family" | "settings";

export type NavItem = { id: PageId; label: string; icon: LucideIcon; phase: number };

/** `phase` = roadmap phase in which the page gets built (see ROADMAP.md). */
export const NAV: { group: string; items: NavItem[] }[] = [
  { group: "Overview", items: [
    { id: "dashboard", label: "Dashboard", icon: LayoutDashboard, phase: 1 },
    { id: "reports", label: "Reports", icon: BarChart3, phase: 2 },
  ]},
  { group: "Money", items: [
    { id: "transactions", label: "Transactions", icon: ArrowLeftRight, phase: 1 },
    { id: "accounts", label: "Accounts", icon: Wallet, phase: 1 },
    { id: "cards", label: "Cards", icon: CreditCard, phase: 1 },
    { id: "recurring", label: "Recurring", icon: Repeat, phase: 2 },
  ]},
  { group: "Wealth", items: [
    { id: "investments", label: "Investments", icon: TrendingUp, phase: 3 },
    { id: "goals", label: "Goals & planning", icon: Target, phase: 3 },
    { id: "insurance", label: "Insurance", icon: Shield, phase: 3 },
  ]},
  { group: "Planner", items: [
    { id: "calendar", label: "Calendar", icon: CalendarDays, phase: 2 },
    { id: "tasks", label: "To-do", icon: ListTodo, phase: 2 },
    { id: "journal", label: "Journal", icon: NotebookPen, phase: 3 },
  ]},
  { group: "Data", items: [
    { id: "import", label: "Import statements", icon: Upload, phase: 2 },
    { id: "documents", label: "Documents", icon: FileText, phase: 3 },
  ]},
  { group: "More", items: [
    { id: "business", label: "Business", icon: Briefcase, phase: 4 },
    { id: "family", label: "Family", icon: Users, phase: 1 },
  ]},
];

export const SETTINGS_ITEM: NavItem = { id: "settings", label: "Settings", icon: Settings, phase: 1 };
export const ALL_ITEMS = [...NAV.flatMap((g) => g.items), SETTINGS_ITEM];
