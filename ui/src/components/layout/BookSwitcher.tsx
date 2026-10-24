import * as Menu from "@radix-ui/react-dropdown-menu";
import { Briefcase, Check, ChevronsUpDown, Plus, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useBook } from "@/hooks/book";
import { cn } from "@/lib/utils";

/** Which book the pages show: the personal one or one of the member's businesses. */
export function BookSwitcher({ collapsed, onNew }: { collapsed: boolean; onNew: () => void }) {
  const { t } = useTranslation();
  const { book, businesses, switchTo } = useBook();
  const open = businesses.filter((b) => !b.archived_at);
  const label = book ? book.name : t("book.personal");
  const Icon = book ? Briefcase : User;
  return (
    <Menu.Root modal={false}>
      <Menu.Trigger asChild>
        <button type="button" aria-label={collapsed ? t("book.switch", { name: label }) : undefined} title={collapsed ? label : undefined}
          className={cn("flex w-full items-center gap-2 rounded-md border px-2 py-1.5 text-start text-sm transition-colors hover:bg-accent/60",
            book ? "border-indigo-500/50 bg-indigo-500/10" : "border-sidebar-border", collapsed && "justify-center px-0")}>
          <Icon className={cn("size-4 shrink-0", book && "text-indigo-600 dark:text-indigo-400")} />
          {!collapsed && <>
            <span className="min-w-0 flex-1"><span className="text-muted-foreground block text-[10px] leading-tight">{book ? t("book.business") : t("book.book")}</span><span className="block truncate font-medium leading-tight">{label}</span></span>
            <span className="sr-only">{t("book.switchShort")}</span>
            <ChevronsUpDown className="text-muted-foreground size-3.5" aria-hidden />
          </>}
        </button>
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Content align="start" sideOffset={6} className="bg-popover text-popover-foreground z-50 min-w-56 rounded-lg border p-1 shadow-md">
          <Item active={!book} onSelect={() => void switchTo(null)} icon={<User className="size-4" />}>{t("book.personal")}</Item>
          {open.length > 0 && <Menu.Separator className="bg-border my-1 h-px" />}
          {open.map((b) => <Item key={b.id} active={book?.id === b.id} onSelect={() => void switchTo(b.id)} icon={<Briefcase className="size-4" />}>{b.name}</Item>)}
          <Menu.Separator className="bg-border my-1 h-px" />
          <Item onSelect={onNew} icon={<Plus className="size-4" />}>{t("book.new")}</Item>
        </Menu.Content>
      </Menu.Portal>
    </Menu.Root>
  );
}

function Item({ children, icon, active, onSelect }: { children: React.ReactNode; icon: React.ReactNode; active?: boolean; onSelect: () => void }) {
  return (
    <Menu.Item onSelect={onSelect} className="data-[highlighted]:bg-accent flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-sm outline-none">
      {icon}<span className="min-w-0 flex-1 truncate">{children}</span>{active && <Check className="size-4" />}
    </Menu.Item>
  );
}
