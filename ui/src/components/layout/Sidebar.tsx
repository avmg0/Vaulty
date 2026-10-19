import { useTranslation } from "react-i18next";
import { SENIOR_PAGES } from "@/lib/senior";
import { motion } from "motion/react";
import { useMotion, SPRING } from "@/lib/motion";
import { LogOut, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { Logo } from "@/components/Logo";
import { NAV, SETTINGS_ITEM, type NavItem, type PageId } from "@/nav";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { UserAvatar } from "@/components/UserAvatar";
import { useSession } from "@/hooks/session";

type Props = { page: PageId; onNavigate: (p: PageId) => void; collapsed: boolean; onToggle: () => void };

export function Sidebar({ page, onNavigate, collapsed, onToggle }: Props) {
  const { user, signOut } = useSession();
  const { t } = useTranslation();
  const item = (i: NavItem) => (
    <NavButton key={i.id} item={i} active={page === i.id} collapsed={collapsed} label={t(`nav.${i.id}`)} onClick={() => onNavigate(i.id)} />
  );

  return (
    <aside className={cn("bg-sidebar text-sidebar-foreground flex h-full flex-col border-r border-sidebar-border transition-[width] duration-200", collapsed ? "w-14" : "w-60")}>
      <div className={cn("flex h-14 items-center gap-2 px-3", collapsed && "justify-center")}>
        <Logo className="w-7 shrink-0" />
        {!collapsed && <span className="font-semibold tracking-tight">Vaulty</span>}
        {!collapsed && (
          <Button variant="ghost" size="icon" className="ml-auto size-7" onClick={onToggle} title={t("nav.collapse")} aria-label={t("nav.collapse")}><PanelLeftClose /></Button>
        )}
      </div>
      <nav className="isolate flex-1 space-y-4 overflow-y-auto px-2 py-2">
        {collapsed && <Button variant="ghost" size="icon" className="mx-auto flex" onClick={onToggle} title={t("nav.expand")} aria-label={t("nav.expand")}><PanelLeftOpen /></Button>}
        {/* Senior mode: one short list of everyday pages, no group headings. */}
        {user.senior ? <div className="space-y-0.5">{NAV.flatMap((g) => g.items).filter((i) => SENIOR_PAGES.includes(i.id)).map(item)}</div>
          : NAV.map((g) => (
          <div key={g.group} className="space-y-0.5">
            {!collapsed && <div className="text-muted-foreground px-2.5 pb-1 text-xs font-medium">{t(`nav.group.${g.group}`)}</div>}
            {g.items.map(item)}
          </div>
        ))}
      </nav>
      <div className="isolate space-y-1 border-t border-sidebar-border p-2">
        {item(SETTINGS_ITEM)}
        <div className={cn("flex items-center gap-2 rounded-md px-1.5 py-1", collapsed && "flex-col px-0")}>
          <UserAvatar name={user.name} color={user.color} className="size-7 text-xs" />
          {!collapsed && (
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium">{user.name}</div>
              <div className="text-muted-foreground text-xs">{t(`role.${user.role}`)}</div>
            </div>
          )}
          <Button variant="ghost" size="icon" className="size-8" onClick={signOut} title={t("nav.lock")} aria-label={t("nav.lock")}><LogOut /></Button>
        </div>
      </div>
    </aside>
  );
}

/** Nav item. The active highlight is one shared element that springs from item to item. */
function NavButton({ item, active, collapsed, label, onClick }: { item: NavItem; active: boolean; collapsed: boolean; label: string; onClick: () => void }) {
  const { move } = useMotion();
  return (
    <button onClick={onClick} title={collapsed ? label : undefined} aria-current={active ? "page" : undefined}
      className={cn(
        "relative flex w-full items-center gap-3 rounded-md px-2.5 py-1.5 text-sm transition-colors",
        active ? "text-accent-foreground font-medium" : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
        collapsed && "justify-center px-0",
      )}>
      {active && <motion.span layoutId="nav-active" transition={move(SPRING)} className="bg-accent absolute inset-0 -z-10 rounded-md" aria-hidden />}
      <item.icon className="relative size-4 shrink-0" />
      {!collapsed && <span className="relative truncate">{label}</span>}
    </button>
  );
}
