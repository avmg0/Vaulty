import { Eye, EyeOff, Monitor, Moon, Sun, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useTheme, type Theme } from "@/hooks/theme";

const NEXT: Record<Theme, Theme> = { light: "dark", dark: "system", system: "light" };
const ICON = { light: Sun, dark: Moon, system: Monitor };

export function Header({ title }: { title: string }) {
  const { theme, setTheme, hideBalances, setHideBalances } = useTheme();
  const ThemeIcon = ICON[theme];
  return (
    <header className="flex h-14 shrink-0 items-center gap-2 border-b px-6">
      <h1 className="text-lg font-semibold tracking-tight">{title}</h1>
      <div className="ml-auto flex items-center gap-1">
        <Badge variant="outline" className="text-muted-foreground mr-2 gap-1"><WifiOff className="size-3" />Offline</Badge>
        <Button variant="ghost" size="icon" onClick={() => setHideBalances(!hideBalances)} title="Hide balances (Ctrl+H)">
          {hideBalances ? <EyeOff /> : <Eye />}
        </Button>
        <Button variant="ghost" size="icon" onClick={() => setTheme(NEXT[theme])} title={`Theme: ${theme}`}>
          <ThemeIcon />
        </Button>
      </div>
    </header>
  );
}
