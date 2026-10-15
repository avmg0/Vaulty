import { useEffect, useState, type FormEvent } from "react";
import { Copy, Check, Fingerprint, Loader2, MoreHorizontal, RotateCcw, ShieldCheck, Trash2, UserPlus } from "lucide-react";
import { api, errorText, type Invite, type Role, type User } from "@/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ColorPicker, PROFILE_COLORS, UserAvatar } from "@/components/UserAvatar";
import { useSession } from "@/hooks/session";
import { cn } from "@/lib/utils";

const ROLES: { id: Role; label: string; desc: string }[] = [
  { id: "member", label: "Member", desc: "Their own private finances." },
  { id: "admin", label: "Admin", desc: "Can also add, reset and remove family members." },
  { id: "child", label: "Child", desc: "For kids' allowance tracking (more controls coming)." },
];

export function Family() {
  const { user: me } = useSession();
  const [users, setUsers] = useState<User[]>([]);
  const [invite, setInvite] = useState<Invite | null>(null);
  const [confirm, setConfirm] = useState<{ kind: "reset" | "remove"; user: User } | null>(null);
  const [menu, setMenu] = useState<string | null>(null);
  const isAdmin = me.role === "admin";
  const load = () => api.users.list().then(setUsers);
  useEffect(() => { load(); }, []);

  return (
    <div className="max-w-3xl space-y-4">
      <div className="flex items-start justify-between gap-4">
        <p className="text-muted-foreground text-sm">Everyone gets a separate, private profile with their own password. Admins manage who's on this device but can't see other members' data.</p>
        {isAdmin && <AddMemberDialog onAdded={(inv) => { setInvite(inv); load(); }} />}
      </div>
      <Card className="py-2">
        <CardContent className="divide-y">
          {users.map((u) => (
            <div key={u.id} className="flex items-center gap-3 py-3">
              <UserAvatar name={u.name} color={u.color} />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 font-medium">{u.name}{u.id === me.id && <span className="text-muted-foreground text-xs font-normal">(you)</span>}</div>
                <div className="text-muted-foreground flex items-center gap-2 text-xs capitalize">
                  {u.role}
                  {u.totp_enabled && <span className="flex items-center gap-0.5 normal-case"><ShieldCheck className="size-3" />2FA</span>}
                  {u.biometric_enabled && <span className="flex items-center gap-0.5 normal-case"><Fingerprint className="size-3" />Biometric</span>}
                </div>
              </div>
              {!u.active && <Badge variant="secondary">Waiting for setup</Badge>}
              {isAdmin && u.id !== me.id && (
                <div className="relative">
                  <Button variant="ghost" size="icon" onClick={() => setMenu(menu === u.id ? null : u.id)} aria-label="Member actions"><MoreHorizontal /></Button>
                  {menu === u.id && (
                    <div className="bg-popover absolute right-0 z-10 mt-1 w-52 rounded-md border p-1 shadow-md" onMouseLeave={() => setMenu(null)}>
                      <MenuItem icon={RotateCcw} onClick={() => { setMenu(null); setConfirm({ kind: "reset", user: u }); }}>
                        {u.active ? "Reset password & 2FA" : "New setup code"}
                      </MenuItem>
                      <MenuItem icon={Trash2} destructive onClick={() => { setMenu(null); setConfirm({ kind: "remove", user: u }); }}>Remove from Vaulty</MenuItem>
                    </div>
                  )}
                </div>
              )}
            </div>
          ))}
        </CardContent>
      </Card>

      <InviteDialog invite={invite} onClose={() => setInvite(null)} />
      <ConfirmDialog target={confirm} onClose={() => setConfirm(null)}
        onDone={(inv) => { setConfirm(null); if (inv) setInvite(inv); load(); }} />
    </div>
  );
}

function MenuItem({ icon: Icon, children, onClick, destructive }: { icon: typeof Trash2; children: React.ReactNode; onClick: () => void; destructive?: boolean }) {
  return (
    <button onClick={onClick} className={cn("hover:bg-accent flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm", destructive && "text-destructive")}>
      <Icon className="size-4" />{children}
    </button>
  );
}

function AddMemberDialog({ onAdded }: { onAdded: (inv: Invite) => void }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [role, setRole] = useState<Role>("member");
  const [color, setColor] = useState(PROFILE_COLORS[1]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    try {
      const inv = await api.users.add(name, color, role);
      setOpen(false); setName(""); setRole("member");
      onAdded(inv);
    } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild><Button className="shrink-0"><UserPlus />Add member</Button></DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add a family member</DialogTitle>
          <DialogDescription>You'll get a one-time setup code to give them. They'll use it to choose their own password.</DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="space-y-4">
          <div className="flex items-end gap-3">
            <UserAvatar name={name || "?"} color={color} className="size-10" />
            <div className="flex-1 space-y-2"><Label htmlFor="mname">Name</Label><Input id="mname" autoFocus value={name} onChange={(e) => setName(e.target.value)} maxLength={40} required /></div>
          </div>
          <div className="space-y-2"><Label>Colour</Label><ColorPicker value={color} onChange={setColor} /></div>
          <div className="space-y-2"><Label>Role</Label>
            <div className="grid gap-2">
              {ROLES.map((r) => (
                <button type="button" key={r.id} onClick={() => setRole(r.id)}
                  className={cn("rounded-lg border p-3 text-left transition-colors hover:bg-accent", role === r.id && "border-primary ring-primary/30 ring-2")}>
                  <div className="text-sm font-medium">{r.label}</div><div className="text-muted-foreground text-xs">{r.desc}</div>
                </button>
              ))}
            </div>
          </div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          <DialogFooter><Button disabled={busy}>{busy && <Loader2 className="animate-spin" />}Add member</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function InviteDialog({ invite, onClose }: { invite: Invite | null; onClose: () => void }) {
  const [copied, setCopied] = useState(false);
  if (!invite) return null;
  const copy = async () => { await navigator.clipboard.writeText(invite.setup_code); setCopied(true); setTimeout(() => setCopied(false), 1500); };
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader className="items-center text-center">
          <UserAvatar name={invite.user.name} color={invite.user.color} className="mb-1 size-12" />
          <DialogTitle>Setup code for {invite.user.name}</DialogTitle>
          <DialogDescription>On the sign-in screen, {invite.user.name} picks their profile, enters this code and chooses a password. It works once and expires in 7 days.</DialogDescription>
        </DialogHeader>
        <button onClick={copy} className="bg-muted hover:bg-accent flex items-center justify-center gap-3 rounded-lg py-4 font-mono text-2xl tracking-widest transition-colors">
          {invite.setup_code}{copied ? <Check className="size-4" /> : <Copy className="text-muted-foreground size-4" />}
        </button>
        <DialogFooter><Button className="w-full" onClick={onClose}>Done</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ConfirmDialog({ target, onClose, onDone }: { target: { kind: "reset" | "remove"; user: User } | null; onClose: () => void; onDone: (inv?: Invite) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  if (!target) return null;
  const { kind, user } = target;
  const go = async () => {
    setBusy(true); setError("");
    try {
      if (kind === "reset") onDone(await api.users.reset(user.id));
      else { await api.users.remove(user.id); onDone(); }
    } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{kind === "reset" ? `Reset ${user.name}'s sign-in?` : `Remove ${user.name}?`}</DialogTitle>
          <DialogDescription>
            {kind === "reset"
              ? "Their password, two-step verification and biometric unlock will be cleared, and you'll get a new setup code for them. Their data is kept."
              : "This permanently deletes their profile and all of their financial data on this device. Make sure they've exported a backup first."}
          </DialogDescription>
        </DialogHeader>
        {error && <p className="text-destructive text-sm">{error}</p>}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancel</Button>
          <Button variant={kind === "remove" ? "destructive" : "default"} onClick={go} disabled={busy}>
            {busy && <Loader2 className="animate-spin" />}{kind === "reset" ? "Reset" : "Remove permanently"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
