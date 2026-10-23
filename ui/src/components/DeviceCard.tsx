import { useEffect, useState } from "react";
import { Keyboard, PanelTop, Power } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, errorText, type DeviceSettings } from "@/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { NativeSelect } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";

/** "CommandOrControl+Shift+J" as the keys are printed on this computer. */
export function shortcutText(keys: string, mac: boolean) {
  const names: Record<string, string> = mac
    ? { CommandOrControl: "⌘", Shift: "⇧", Alt: "⌥", Space: "Space" }
    : { CommandOrControl: "Ctrl", Shift: "Shift", Alt: "Alt", Space: "Space" };
  // Macs list modifiers as ⌃⌥⇧⌘, then the key.
  const order = ["Alt", "Shift", "CommandOrControl"];
  const parts = keys.split("+");
  if (mac) parts.sort((a, b) => (order.includes(a) ? order.indexOf(a) : 9) - (order.includes(b) ? order.indexOf(b) : 9));
  return parts.map((k) => names[k] ?? k).join(mac ? "" : "+");
}

function Row({ icon: Icon, label, desc, children }: { icon: typeof Power; label: string; desc: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex items-start gap-3"><Icon className="text-muted-foreground mt-0.5 size-4 shrink-0" aria-hidden />
        <div><div className="text-sm font-medium">{label}</div><div className="text-muted-foreground text-xs">{desc}</div></div></div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

/** This computer: the menu bar (tray) icon, quick add from anywhere, keep running when closed. */
export function DeviceCard() {
  const { t } = useTranslation();
  const [s, setS] = useState<DeviceSettings | null>(null);
  const [error, setError] = useState("");
  useEffect(() => { api.device.get().then(setS).catch(() => {}); }, []);
  if (!s || !s.shortcut_choices.length) return null;
  const change = async (key: "tray" | "keep_running" | "shortcut", value: boolean | string) => {
    setError("");
    try { await api.device.set(key, value); setS(await api.device.get()); } catch (e) { setError(errorText(e)); }
  };
  return (
    <Card>
      <CardHeader><CardTitle>{t("device.title")}</CardTitle><CardDescription>{t("device.desc")}</CardDescription></CardHeader>
      <CardContent className="space-y-4">
        <Row icon={PanelTop} label={s.mac ? t("device.trayMac") : t("device.tray")} desc={t("device.trayDesc")}>
          <Switch aria-label={s.mac ? t("device.trayMac") : t("device.tray")} checked={s.tray} onCheckedChange={(on) => change("tray", on)} />
        </Row>
        <Separator />
        <Row icon={Keyboard} label={t("device.shortcut")} desc={t("device.shortcutDesc")}>
          <NativeSelect aria-label={t("device.shortcut")} className="w-auto" value={s.shortcut} onChange={(e) => change("shortcut", e.target.value)}>
            <option value="">{t("device.off")}</option>
            {s.shortcut_choices.map((k) => <option key={k} value={k}>{shortcutText(k, s.mac)}</option>)}
          </NativeSelect>
        </Row>
        <Separator />
        <Row icon={Power} label={t("device.keepRunning")} desc={s.tray ? t("device.keepRunningDesc") : t("device.keepRunningNeedsTray")}>
          <Switch aria-label={t("device.keepRunning")} disabled={!s.tray} checked={s.keep_running && s.tray} onCheckedChange={(on) => change("keep_running", on)} />
        </Row>
        {error && <p className="text-destructive text-sm">{error}</p>}
      </CardContent>
    </Card>
  );
}
