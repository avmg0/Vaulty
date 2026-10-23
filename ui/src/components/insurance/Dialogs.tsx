import { useEffect, useState, type FormEvent } from "react";
import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, CLAIM_STATUSES, errorText, POLICY_KINDS, PREMIUM_FREQUENCIES, type Account, type Category, type Doc, type Policy, type PolicyClaim, type PolicyInput } from "@/api";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input, NativeSelect } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useLocale } from "@/hooks/locale";
import { today } from "@/components/projects/ProjectDialogs";

const amount = (v: string) => v.replace(/[^\d.]/g, "");
const str = (n: number | null | undefined) => (n == null ? "" : String(n));

function Field({ id, label, children, className }: { id: string; label: string; children: React.ReactNode; className?: string }) {
  return <div className={`grid gap-1.5 ${className ?? ""}`}><Label htmlFor={id}>{label}</Label>{children}</div>;
}

/** Add or change a policy. */
export function PolicyDialog({ policy, onClose }: { policy: Policy | null; onClose: (changed: boolean) => void }) {
  const { t } = useTranslation();
  const { currency } = useLocale();
  const [v, setV] = useState<Record<string, string | boolean>>((): Record<string, string | boolean> => policy ? {
    kind: policy.kind, name: policy.name, insurer: policy.insurer ?? "", policy_no: policy.policy_no ?? "", holder: policy.holder ?? "", covers: policy.covers ?? "",
    for_parents: policy.for_parents, cover: str(policy.cover), premium: str(policy.premium), frequency: policy.frequency, next_due: policy.next_due ?? "",
    start_date: policy.start_date ?? "", end_date: policy.end_date ?? "", nominee: policy.nominee ?? "", contact: policy.contact ?? "",
    document_id: policy.document_id ?? "", status: policy.status, notes: policy.notes ?? "",
  } : { kind: "health", frequency: "yearly", status: "active", for_parents: false });
  const [docs, setDocs] = useState<Doc[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => { api.documents.list().then((d) => setDocs(d.filter((x) => x.kind === "insurance"))).catch(() => {}); }, []);
  const s = (k: string) => (v[k] as string | undefined) ?? "";
  const set = (k: string, val: string | boolean) => setV((o) => ({ ...o, [k]: val }));
  const text = (k: string, max = 100, extra: React.ComponentProps<typeof Input> = {}) => <Input id={`p-${k}`} maxLength={max} value={s(k)} onChange={(e) => set(k, e.target.value)} {...extra} />;
  const money = (k: string) => <Input id={`p-${k}`} inputMode="decimal" placeholder="0" value={s(k)} onChange={(e) => set(k, amount(e.target.value))} />;
  const dateIn = (k: string) => <Input id={`p-${k}`} type="date" value={s(k)} onChange={(e) => set(k, e.target.value)} />;

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    const input = { ...v, currency: policy?.currency ?? currency } as PolicyInput;
    try { if (policy) await api.insurance.update(policy.id, input); else await api.insurance.create(input); onClose(true); }
    catch (err) { setError(errorText(err)); setBusy(false); }
  };
  const life = s("kind") === "term_life" || s("kind") === "life";

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(false); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{policy ? t("insurance.editTitle") : t("insurance.newTitle")}</DialogTitle>
          <DialogDescription>{t("insurance.dialogDesc")}</DialogDescription>
        </DialogHeader>
        <form onSubmit={save} className="grid gap-4">
          <div className="grid gap-3 sm:grid-cols-[12rem_minmax(0,1fr)]">
            <Field id="p-kind" label={t("insurance.kind")}>
              <NativeSelect id="p-kind" value={s("kind")} onChange={(e) => set("kind", e.target.value)}>
                {POLICY_KINDS.map((k) => <option key={k} value={k}>{t(`insurance.kinds.${k}`)}</option>)}
              </NativeSelect>
            </Field>
            <Field id="p-name" label={t("insurance.name")}>{text("name", 100, { required: true, autoFocus: true, placeholder: t(`insurance.namePlaceholder.${s("kind")}`) })}</Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field id="p-insurer" label={t("insurance.insurer")}>{text("insurer")}</Field>
            <Field id="p-policy_no" label={t("insurance.policyNo")}>{text("policy_no", 60)}</Field>
            <Field id="p-holder" label={t("insurance.holder")}>{text("holder")}</Field>
            <Field id="p-covers" label={t(life ? "insurance.lifeInsured" : "insurance.covers")}>{text("covers", 200)}</Field>
          </div>
          {s("kind") === "health" && (
            <label className="flex items-center gap-2 text-sm"><input type="checkbox" className="accent-[var(--primary)]" checked={!!v.for_parents} onChange={(e) => set("for_parents", e.target.checked)} />{t("insurance.forParents")}</label>
          )}
          <div className="grid gap-3 sm:grid-cols-3">
            <Field id="p-cover" label={t(life ? "insurance.sumAssured" : "insurance.cover")}>{money("cover")}</Field>
            <Field id="p-premium" label={t("insurance.premium")}>{money("premium")}</Field>
            <Field id="p-frequency" label={t("insurance.frequency")}>
              <NativeSelect id="p-frequency" value={s("frequency")} onChange={(e) => set("frequency", e.target.value)}>
                {PREMIUM_FREQUENCIES.map((f) => <option key={f} value={f}>{t(`insurance.frequencies.${f}`)}</option>)}
              </NativeSelect>
            </Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-3">
            <Field id="p-next_due" label={t("insurance.nextDue")}>{dateIn("next_due")}</Field>
            <Field id="p-start_date" label={t("insurance.startDate")}>{dateIn("start_date")}</Field>
            <Field id="p-end_date" label={t("insurance.endDate")}>{dateIn("end_date")}</Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field id="p-nominee" label={t("insurance.nominee")}>{text("nominee", 200)}</Field>
            <Field id="p-contact" label={t("insurance.contact")}>{text("contact", 200, { placeholder: t("insurance.contactHint") })}</Field>
            <Field id="p-document_id" label={t("insurance.document")}>
              <NativeSelect id="p-document_id" value={s("document_id")} onChange={(e) => set("document_id", e.target.value)}>
                <option value="">{t("insurance.noDocument")}</option>
                {docs.map((d) => <option key={d.id} value={d.id}>{d.title}</option>)}
              </NativeSelect>
            </Field>
            <Field id="p-status" label={t("insurance.statusLabel")}>
              <NativeSelect id="p-status" value={s("status")} onChange={(e) => set("status", e.target.value)}>
                {["active", "lapsed", "ended"].map((x) => <option key={x} value={x}>{t(`insurance.statuses.${x}`)}</option>)}
              </NativeSelect>
            </Field>
          </div>
          <Field id="p-notes" label={t("insurance.notes")}>{text("notes", 2000)}</Field>
          {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            <Button type="button" variant="outline" onClick={() => onClose(false)}>{t("common.cancel")}</Button>
            <Button disabled={busy || !s("name").trim()}>{busy && <Loader2 className="animate-spin" />}{policy ? t("common.save") : t("insurance.add")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Record a premium paid; optionally add it as a transaction. */
export function PayDialog({ policy, onClose }: { policy: Policy; onClose: (changed: boolean) => void }) {
  const { t } = useTranslation();
  const [date, setDate] = useState(today());
  const [amt, setAmt] = useState(String(policy.premium || ""));
  const [record, setRecord] = useState(true);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [cats, setCats] = useState<Category[]>([]);
  const [account, setAccount] = useState("");
  const [category, setCategory] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    api.accounts.getAll().then((a) => { const ok = a.filter((x) => x.currency === policy.currency); setAccounts(ok); setAccount(ok[0]?.id ?? ""); }).catch(() => {});
    api.categories.getAll().then((c) => {
      const ex = c.filter((x) => x.type === "expense"); setCats(ex);
      setCategory(ex.find((x) => /insur|बीमा/i.test(x.name))?.id ?? "");
    }).catch(() => {});
  }, [policy.currency]);
  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    try {
      await api.insurance.pay(policy.id, { date, amount: amt, ...(record && account ? { record: { account_id: account, category_id: category || null } } : {}) });
      onClose(true);
    } catch (err) { setError(errorText(err)); setBusy(false); }
  };
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(false); }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("insurance.payTitle", { name: policy.name })}</DialogTitle>
          <DialogDescription>{t("insurance.payDesc")}</DialogDescription>
        </DialogHeader>
        <form onSubmit={save} className="grid gap-4">
          <div className="grid grid-cols-2 gap-3">
            <Field id="pay-date" label={t("insurance.paidOn")}><Input id="pay-date" type="date" required value={date} onChange={(e) => setDate(e.target.value)} /></Field>
            <Field id="pay-amount" label={t("insurance.amountPaid")}><Input id="pay-amount" inputMode="decimal" required value={amt} onChange={(e) => setAmt(amount(e.target.value))} /></Field>
          </div>
          <label className="flex items-center gap-2 text-sm"><input type="checkbox" className="accent-[var(--primary)]" checked={record} onChange={(e) => setRecord(e.target.checked)} disabled={accounts.length === 0} />{t("insurance.addTransaction")}</label>
          {record && accounts.length > 0 && (
            <div className="grid grid-cols-2 gap-3">
              <Field id="pay-account" label={t("insurance.fromAccount")}>
                <NativeSelect id="pay-account" value={account} onChange={(e) => setAccount(e.target.value)}>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect>
              </Field>
              <Field id="pay-category" label={t("insurance.category")}>
                <NativeSelect id="pay-category" value={category} onChange={(e) => setCategory(e.target.value)}>
                  <option value="">{t("insurance.uncategorised")}</option>
                  {cats.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
                </NativeSelect>
              </Field>
            </div>
          )}
          <p className="text-muted-foreground text-xs">{policy.frequency === "single" ? t("insurance.singleNote") : t("insurance.movesOn")}</p>
          {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            <Button type="button" variant="outline" onClick={() => onClose(false)}>{t("common.cancel")}</Button>
            <Button disabled={busy || !amt}>{busy && <Loader2 className="animate-spin" />}{t("insurance.recordPayment")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Add or change a claim. */
export function ClaimDialog({ policy, claim, onClose }: { policy: Policy; claim: PolicyClaim | null; onClose: (changed: boolean) => void }) {
  const { t } = useTranslation();
  const [v, setV] = useState<Record<string, string>>(() => ({
    date: claim?.date ?? today(), what: claim?.what ?? "", claimed: str(claim?.claimed), settled: str(claim?.settled), status: claim?.status ?? "filed",
    settled_on: claim?.settled_on ?? "", reference: claim?.reference ?? "", notes: claim?.notes ?? "",
  }));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (k: string, val: string) => setV((o) => ({ ...o, [k]: val }));
  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true); setError("");
    try { if (claim) await api.insurance.updateClaim(claim.id, v); else await api.insurance.addClaim(policy.id, v); onClose(true); }
    catch (err) { setError(errorText(err)); setBusy(false); }
  };
  const done = v.status === "settled" || v.status === "partly_settled";
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(false); }}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{claim ? t("insurance.editClaim") : t("insurance.newClaim")}</DialogTitle>
          <DialogDescription>{policy.name}</DialogDescription>
        </DialogHeader>
        <form onSubmit={save} className="grid gap-4">
          <Field id="cl-what" label={t("insurance.claimWhat")}><Input id="cl-what" required autoFocus maxLength={200} value={v.what} onChange={(e) => set("what", e.target.value)} /></Field>
          <div className="grid gap-3 sm:grid-cols-3">
            <Field id="cl-date" label={t("insurance.claimDate")}><Input id="cl-date" type="date" required value={v.date} onChange={(e) => set("date", e.target.value)} /></Field>
            <Field id="cl-claimed" label={t("insurance.claimed")}><Input id="cl-claimed" inputMode="decimal" required value={v.claimed} onChange={(e) => set("claimed", amount(e.target.value))} /></Field>
            <Field id="cl-status" label={t("insurance.claimStatus")}>
              <NativeSelect id="cl-status" value={v.status} onChange={(e) => set("status", e.target.value)}>
                {CLAIM_STATUSES.map((c) => <option key={c} value={c}>{t(`insurance.claimStatuses.${c}`)}</option>)}
              </NativeSelect>
            </Field>
          </div>
          {done && (
            <div className="grid gap-3 sm:grid-cols-2">
              <Field id="cl-settled" label={t("insurance.settled")}><Input id="cl-settled" inputMode="decimal" value={v.settled} onChange={(e) => set("settled", amount(e.target.value))} /></Field>
              <Field id="cl-settled_on" label={t("insurance.settledOn")}><Input id="cl-settled_on" type="date" value={v.settled_on} onChange={(e) => set("settled_on", e.target.value)} /></Field>
            </div>
          )}
          <div className="grid gap-3 sm:grid-cols-2">
            <Field id="cl-reference" label={t("insurance.claimRef")}><Input id="cl-reference" maxLength={60} value={v.reference} onChange={(e) => set("reference", e.target.value)} /></Field>
            <Field id="cl-notes" label={t("insurance.notes")}><Input id="cl-notes" maxLength={1000} value={v.notes} onChange={(e) => set("notes", e.target.value)} /></Field>
          </div>
          {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
          <DialogFooter className="gap-2">
            {claim && <Button type="button" variant="ghost" className="text-destructive me-auto" onClick={() => api.insurance.deleteClaim(claim.id).then(() => onClose(true))}>{t("common.delete")}</Button>}
            <Button type="button" variant="outline" onClick={() => onClose(false)}>{t("common.cancel")}</Button>
            <Button disabled={busy || !v.what.trim() || !v.claimed}>{busy && <Loader2 className="animate-spin" />}{t("common.save")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
