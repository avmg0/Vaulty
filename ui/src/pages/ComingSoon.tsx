import { Construction } from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import type { NavItem } from "@/nav";

export function ComingSoon({ item }: { item: NavItem }) {
  return (
    <Card className="mx-auto mt-10 max-w-md">
      <CardContent className="flex flex-col items-center gap-3 py-8 text-center">
        <div className="bg-secondary grid size-12 place-items-center rounded-xl"><item.icon className="size-5" /></div>
        <div className="font-semibold">{item.label}</div>
        <p className="text-muted-foreground text-sm">This section is planned but not built yet.</p>
        <Badge variant="outline" className="gap-1"><Construction className="size-3" />Roadmap phase {item.phase}</Badge>
      </CardContent>
    </Card>
  );
}
