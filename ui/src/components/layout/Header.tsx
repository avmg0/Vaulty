import { Eye, EyeOff, FlaskConical, LineChart, Monitor, Moon, Sun, WifiOff, Zap } from "lucide-react";
import { QUICK_ADD_EVENT } from "@/components/QuickAdd";
import { useAppInfo } from "@/hooks/appInfo";
import { useMarketEnabled } from "@/components/TickerBar";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useTheme, type Theme } from "@/hooks/theme";

const NEXT: Record<Theme, Theme> = { light: "dark", dark: "system", system: "light" };
const ICON = { light: Sun, dark: Moon, system: Monitor };

export function Header({ title }: { title: string }) {
  const { theme, setTheme, hideBalances, setHideBalances } = useTheme();
  const ThemeIcon = ICON[theme];
  const { t } = useTranslation();
  const online = useMarketEnabled();
  const info = useAppInfo();
  return (
    <header className="flex h-14 shrink-0 items-center gap-2 border-b px-6">
      <h1 className="text-lg font-semibold tracking-tight">{title}</h1>
      <div className="ml-auto flex items-center gap-1">
        {info?.demo && <Badge className="me-1 gap-1 bg-amber-500/15 text-amber-700 dark:text-amber-300" title={t("demo.badgeTitle")}><FlaskConical className="size-3" />{t("demo.badge")}</Badge>}
        <Badge variant="outline" className="text-muted-foreground me-2 gap-1">{online ? <><LineChart className="size-3" />{t("header.pricesOnline")}</> : <><WifiOff className="size-3" />{t("header.offline")}</>}</Badge>
        <Button variant="outline" size="sm" className="me-1 gap-1.5" onClick={() => window.dispatchEvent(new Event(QUICK_ADD_EVENT))} title={t("quick.shortcut")}>
          <Zap />{t("quick.button")}<kbd className="text-muted-foreground hidden font-sans text-[10px] sm:inline">{navigator.platform.startsWith("Mac") ? "⌘J" : "Ctrl+J"}</kbd>
        </Button>
        <Button variant="ghost" size="icon" onClick={() => setHideBalances(!hideBalances)} title={t("header.hideBalances")}>
          {hideBalances ? <EyeOff /> : <Eye />}
        </Button>
        <Button variant="ghost" size="icon" onClick={() => setTheme(NEXT[theme])} title={t("header.theme", { theme: t(`settings.appearance.${theme}`) })}>
          <ThemeIcon />
        </Button>
      </div>
    </header>
  );
}
