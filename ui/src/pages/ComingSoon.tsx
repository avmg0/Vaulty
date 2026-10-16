import { Construction } from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useTranslation } from "react-i18next";
import type { NavItem } from "@/nav";

export function ComingSoon({ item }: { item: NavItem }) {
  const { t } = useTranslation();
  return (
    <Card className="mx-auto mt-10 max-w-md">
      <CardContent className="flex flex-col items-center gap-3 py-8 text-center">
        <div className="bg-secondary grid size-12 place-items-center rounded-xl"><item.icon className="size-5" /></div>
        <div className="font-semibold">{t(`nav.${item.id}`)}</div>
        <p className="text-muted-foreground text-sm">{t("comingSoon.body")}</p>
        <Badge variant="outline" className="gap-1"><Construction className="size-3" />{t("comingSoon.phase", { phase: item.phase })}</Badge>
      </CardContent>
    </Card>
  );
}
