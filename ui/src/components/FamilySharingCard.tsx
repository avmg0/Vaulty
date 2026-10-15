import { useEffect, useState } from "react";
import { Check, Globe, Loader2, Lock, Users } from "lucide-react";
import { api, errorText, type FamilyMember, type SharingPrefs } from "@/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { Badge } from "@/components/ui/badge";
import { UserAvatar } from "@/components/UserAvatar";
import { VisibilityPicker } from "@/components/Visibility";
import { useSession } from "@/hooks/session";
import { cn } from "@/lib/utils";

/** Each member's own consent and audience for Family sharing. */
export function FamilySharingCard() {
  const { user } = useSession();
  const [prefs, setPrefs] = useState<SharingPrefs | null>(null);
  const [members, setMembers] = useState<FamilyMember[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    api.sharing.get().then((s) => { setPrefs(s.prefs); setMembers(s.members.filter((m) => m.id !== user.id)); });
  }, [user.id]);

  const save = async (next: SharingPrefs) => {
    setPrefs(next); setSaving(true); setError("");
    try { setPrefs(await api.sharing.update(next)); } catch (e) { setError(errorText(e)); } finally { setSaving(false); }
  };

  if (!prefs) return <Card><CardContent className="py-8"><Loader2 className="text-muted-foreground mx-auto animate-spin" /></CardContent></Card>;
  const toggleViewer = (id: string) =>
    save({ ...prefs, shared_with: prefs.shared_with.includes(id) ? prefs.shared_with.filter((x) => x !== id) : [...prefs.shared_with, id] });

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-4">
          <div className="space-y-1.5">
            <CardTitle className="flex items-center gap-2">Family sharing {saving && <Loader2 className="text-muted-foreground size-4 animate-spin" />}</CardTitle>
            <CardDescription>Share chosen expenses with your family, and see what they share with you. Nothing is shared until you turn this on.</CardDescription>
          </div>
          <Switch checked={prefs.enabled} onCheckedChange={(enabled) => save({ ...prefs, enabled })} aria-label="Family sharing" />
        </div>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="grid gap-3 text-sm sm:grid-cols-3">
          {[
            { icon: Lock, title: "Private", text: "Only you. The default for everything." },
            { icon: Users, title: "Shared", text: "You plus the people you pick below, e.g. parents." },
            { icon: Globe, title: "Public", text: "Everyone in the family who has sharing on." },
          ].map((x) => (
            <div key={x.title} className="bg-muted/50 rounded-lg p-3">
              <div className="mb-1 flex items-center gap-1.5 font-medium"><x.icon className="size-4" />{x.title}</div>
              <div className="text-muted-foreground text-xs">{x.text}</div>
            </div>
          ))}
        </div>

        <div className={cn("space-y-5", !prefs.enabled && "pointer-events-none opacity-50")} aria-disabled={!prefs.enabled}>
          <div className="space-y-2">
            <Label>Default for new transactions</Label>
            <VisibilityPicker value={prefs.default_visibility} onChange={(default_visibility) => save({ ...prefs, default_visibility })} disabled={!prefs.enabled} />
            <p className="text-muted-foreground text-xs">You can still change it for each transaction.</p>
          </div>
          <Separator />
          <div className="space-y-2">
            <Label>Who can see your Shared items</Label>
            {members.length === 0 && <p className="text-muted-foreground text-sm">No other family members on this device yet.</p>}
            <div className="grid gap-2 sm:grid-cols-2">
              {members.map((m) => {
                const on = prefs.shared_with.includes(m.id);
                return (
                  <button key={m.id} type="button" onClick={() => toggleViewer(m.id)} disabled={!prefs.enabled} aria-pressed={on}
                    className={cn("flex items-center gap-3 rounded-lg border p-2.5 text-left transition-colors hover:bg-accent", on && "border-primary ring-primary/30 ring-2")}>
                    <UserAvatar name={m.name} color={m.color} className="size-8 text-xs" />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-medium">{m.name}</div>
                      <div className="text-muted-foreground text-xs">
                        {!m.active ? "Not set up yet" : m.sharing_enabled ? "Sharing on" : "Sharing off · won't see it until they turn it on"}
                      </div>
                    </div>
                    <div className={cn("grid size-5 place-items-center rounded border", on && "bg-primary border-primary text-primary-foreground")}>{on && <Check className="size-3.5" />}</div>
                  </button>
                );
              })}
            </div>
          </div>
        </div>
        {error && <p className="text-destructive text-sm">{error}</p>}
        {prefs.enabled && <Badge variant="outline" className="text-muted-foreground">Turning this off withdraws everything you've shared, immediately.</Badge>}
      </CardContent>
    </Card>
  );
}
