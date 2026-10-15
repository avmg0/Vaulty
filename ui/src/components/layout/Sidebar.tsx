import { LogOut, PanelLeftClose, PanelLeftOpen, Vault } from "lucide-react";
import { NAV, SETTINGS_ITEM, type NavItem, type PageId } from "@/nav";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { UserAvatar } from "@/components/UserAvatar";
import { useSession } from "@/hooks/session";

type Props = { page: PageId; onNavigate: (p: PageId) => void; collapsed: boolean; onToggle: () => void };

export function Sidebar({ page, onNavigate, collapsed, onToggle }: Props) {
  const { user, signOut } = useSession();
  const Item = ({ item }: { item: NavItem }) => (
    <button
      onClick={() => onNavigate(item.id)}
      title={collapsed ? item.label : undefined}
      className={cn(
        "flex w-full items-center gap-3 rounded-md px-2.5 py-1.5 text-sm transition-colors",
        page === item.id ? "bg-accent text-accent-foreground font-medium" : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
        collapsed && "justify-center px-0",
      )}>
      <item.icon className="size-4 shrink-0" />
      {!collapsed && <span className="truncate">{item.label}</span>}
    </button>
  );

  return (
    <aside className={cn("bg-sidebar text-sidebar-foreground flex h-full flex-col border-r border-sidebar-border transition-[width] duration-200", collapsed ? "w-14" : "w-60")}>
      <div className={cn("flex h-14 items-center gap-2 px-3", collapsed && "justify-center")}>
        <div className="bg-primary text-primary-foreground grid size-7 place-items-center rounded-md"><Vault className="size-4" /></div>
        {!collapsed && <span className="font-semibold tracking-tight">Vaulty</span>}
        {!collapsed && (
          <Button variant="ghost" size="icon" className="ml-auto size-7" onClick={onToggle} title="Collapse sidebar (Ctrl+B)"><PanelLeftClose /></Button>
        )}
      </div>
      <nav className="flex-1 space-y-4 overflow-y-auto px-2 py-2">
        {collapsed && <Button variant="ghost" size="icon" className="mx-auto flex" onClick={onToggle} title="Expand sidebar (Ctrl+B)"><PanelLeftOpen /></Button>}
        {NAV.map((g) => (
          <div key={g.group} className="space-y-0.5">
            {!collapsed && <div className="text-muted-foreground px-2.5 pb-1 text-xs font-medium">{g.group}</div>}
            {g.items.map((i) => <Item key={i.id} item={i} />)}
          </div>
        ))}
      </nav>
      <div className="space-y-1 border-t border-sidebar-border p-2">
        <Item item={SETTINGS_ITEM} />
        <div className={cn("flex items-center gap-2 rounded-md px-1.5 py-1", collapsed && "flex-col px-0")}>
          <UserAvatar name={user.name} color={user.color} className="size-7 text-xs" />
          {!collapsed && (
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium">{user.name}</div>
              <div className="text-muted-foreground text-xs capitalize">{user.role}</div>
            </div>
          )}
          <Button variant="ghost" size="icon" className="size-8" onClick={signOut} title="Lock / switch profile (Ctrl+L)"><LogOut /></Button>
        </div>
      </div>
    </aside>
  );
}
