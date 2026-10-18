import { useEffect, useRef, useState, type FormEvent } from "react";
import { Banknote, Building2, CreditCard, Gift, ImageMinus, ImagePlus, Landmark, LineChart, Plus, Ticket, Trash2, Wallet } from "lucide-react";
import { api, errorText, type Account, type Institution } from "@/api";
import { InstitutionBadge } from "@/components/InstitutionBadge";
import { CurrencyPicker } from "@/components/CurrencyPicker";
import { useLocale } from "@/hooks/locale";
import { InstitutionPicker } from "@/components/InstitutionPicker";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { formatMoney } from "@/lib/utils";
import { useTheme } from "@/hooks/theme";
import { useTranslation } from "react-i18next";

/** Account types; labels are `accounts.type.<id>` in the translation files. */
export const ACCOUNT_TYPES = {
  checking: { icon: Landmark },
  cash: { icon: Banknote },
  credit: { icon: CreditCard },
  investment: { icon: LineChart },
  business: { icon: Building2 },
  prepaid: { icon: Ticket },
  gift_card: { icon: Gift },
  other: { icon: Wallet },
} as const;
const typeLabel = (t: (k: string) => string, type: string) => type in ACCOUNT_TYPES ? t(`accounts.type.${type}`) : type;

export function Accounts() {
  const { hideBalances } = useTheme();
  const { t } = useTranslation();
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [open, setOpen] = useState(false);
  const [newType, setNewType] = useState<keyof typeof ACCOUNT_TYPES>("checking");
  const [error, setError] = useState("");
  const [institution, setInstitution] = useState<Institution | null>(null);
  const { currency: mainCurrency } = useLocale();
  const [currency, setCurrency] = useState(mainCurrency);
  const [logoTarget, setLogoTarget] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const load = () => api.accounts.getAll().then(setAccounts);
  const isCard = newType === "credit";
  const showInstitution = newType !== "cash";

  const pickLogo = (accountId: string) => { setLogoTarget(accountId); fileInput.current?.click(); };
  const onLogoFile = async (file: File | undefined) => {
    if (!file || !logoTarget) return;
    try {
      await api.accounts.update(logoTarget, { logo: await toSquarePng(file, 128) });
      load();
    } catch (err) { setError(err instanceof Error && err.message === "not-an-image" ? t("accounts.notImage") : errorText(err)); }
    setLogoTarget(null);
  };
  useEffect(() => { load(); }, []);

  async function create(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const f = new FormData(e.currentTarget);
    try {
      await api.accounts.create({
        name: String(f.get("name")), type: String(f.get("type")),
        currency: currency.toUpperCase(), initial_balance: String(f.get("balance") || "0"),
        institution_id: showInstitution ? institution?.id ?? null : null,
      });
      setOpen(false); setError(""); setNewType("checking"); setInstitution(null);
      load();
    } catch (err) { setError(errorText(err)); }
  }

  return (
    <div className="space-y-4">
      <div className="flex justify-end">
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogTrigger asChild><Button><Plus />{t("accounts.add")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("accounts.newTitle")}</DialogTitle></DialogHeader>
            <form onSubmit={create} className="grid gap-4">
              <div className="grid gap-2"><Label htmlFor="name">{t("accounts.name")}</Label><Input id="name" name="name" placeholder={t("accounts.namePlaceholder")} required /></div>
              <div className="grid grid-cols-2 gap-4">
                <div className="grid gap-2"><Label htmlFor="type">{t("accounts.typeLabel")}</Label>
                  <NativeSelect id="type" name="type" value={newType} onChange={(e) => setNewType(e.target.value as keyof typeof ACCOUNT_TYPES)}>{Object.entries(ACCOUNT_TYPES).map(([k]) => <option key={k} value={k}>{typeLabel(t, k)}</option>)}</NativeSelect>
                </div>
                <div className="grid gap-2"><Label htmlFor="currency">{t("accounts.currency")}</Label><CurrencyPicker id="currency" value={currency} onChange={setCurrency} /></div>
              </div>
              {showInstitution && (
                <div className="grid gap-2">
                  <Label>{isCard ? t("accounts.issuer") : newType === "investment" ? t("accounts.broker") : t("accounts.bank")}</Label>
                  <InstitutionPicker value={institution} onChange={setInstitution} currency={currency} />
                </div>
              )}
              <div className="grid gap-2">
                <Label htmlFor="balance">{isCard ? t("accounts.owedNow") : t("accounts.opening")}</Label>
                <Input id="balance" name="balance" type="number" step="0.01" min={isCard ? "0" : undefined} defaultValue="0" />
                {isCard && <p className="text-muted-foreground text-xs">{t("accounts.cardNote")}</p>}
              </div>
              {error && <p className="text-destructive text-sm">{error}</p>}
              <DialogFooter><Button type="submit">{t("accounts.create")}</Button></DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </div>
      {accounts.length === 0 && (
        <Card><CardContent className="text-muted-foreground py-10 text-center text-sm">{t("accounts.empty")}</CardContent></Card>
      )}
      <input ref={fileInput} type="file" accept="image/png,image/jpeg,image/webp,image/svg+xml" className="hidden"
        onChange={(e) => { onLogoFile(e.target.files?.[0]); e.target.value = ""; }} />
      {error && !open && <p className="text-destructive text-sm">{error}</p>}
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {accounts.map((a) => {
          const Icon = ACCOUNT_TYPES[a.type as keyof typeof ACCOUNT_TYPES]?.icon ?? Wallet;
          return (
            <Card key={a.id} className="group">
              <CardHeader className="flex-row items-center gap-3">
                <InstitutionBadge logo={a.logo} institutionLogo={a.institution_logo} short={a.institution_short} name={a.institution ?? a.name} />
                {!a.logo && !a.institution_logo && !a.institution_short && (
                  <div className="bg-secondary grid size-9 place-items-center rounded-lg"><Icon className="size-4" /></div>
                )}
                <div className="min-w-0 flex-1">
                  <CardTitle className="truncate">{a.name}</CardTitle>
                  <CardDescription className="truncate">
                    {[a.institution, typeLabel(t, a.type)].filter(Boolean).join(" · ")}
                  </CardDescription>
                </div>
                <div className="flex opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                  {a.logo ? (
                    <Button variant="ghost" size="icon" title={t("accounts.removeLogo")} onClick={() => api.accounts.update(a.id, { logo: null }).then(load)}><ImageMinus /></Button>
                  ) : (
                    <Button variant="ghost" size="icon" title={t("accounts.ownLogo")} onClick={() => pickLogo(a.id)}><ImagePlus /></Button>
                  )}
                  <Button variant="ghost" size="icon" title={t("accounts.archive")} onClick={() => api.accounts.delete(a.id).then(load)}><Trash2 /></Button>
                </div>
              </CardHeader>
              <CardContent>
                {a.ledger_kind === "liability" ? (
                  <>
                    <div className="text-2xl font-semibold tabular-nums">{formatMoney(Math.max(0, -a.current_balance), a.currency, hideBalances)}</div>
                    <div className="text-muted-foreground text-xs">{a.current_balance > 0 ? t("accounts.inCredit", { amount: formatMoney(a.current_balance, a.currency, hideBalances) }) : t("accounts.owed")}</div>
                  </>
                ) : (
                  <div className="text-2xl font-semibold tabular-nums">{formatMoney(a.current_balance, a.currency, hideBalances)}</div>
                )}
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}

/** Fits an image into a transparent square PNG (keeps aspect ratio), small enough to store with the account. */
async function toSquarePng(file: File, size: number): Promise<string> {
  const url = URL.createObjectURL(file);
  try {
    const img = await new Promise<HTMLImageElement>((resolve, reject) => {
      const i = new Image();
      i.onload = () => resolve(i);
      i.onerror = () => reject(new Error("not-an-image"));
      i.src = url;
    });
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = size;
    const ctx = canvas.getContext("2d")!;
    const w = img.naturalWidth || size, h = img.naturalHeight || size;
    const scale = Math.min(size / w, size / h);
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(img, (size - w * scale) / 2, (size - h * scale) / 2, w * scale, h * scale);
    return canvas.toDataURL("image/png");
  } finally {
    URL.revokeObjectURL(url);
  }
}
