import { useRef, useState } from "react";
import { ImagePlus, Minus, Plus } from "lucide-react";
import type { CardDesign, CardLook } from "@/api";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { DESIGNS } from "@/components/cards/CardVisual";
import { inkFor, luminance } from "@/lib/cards";
import { cn } from "@/lib/utils";
import { Segmented } from "@/components/Segmented";

type Mode = "preset" | "solid" | "gradient" | "image";
const MODES: { id: Mode; label: string }[] = [
  { id: "preset", label: "Finishes" }, { id: "solid", label: "Colour" }, { id: "gradient", label: "Gradient" }, { id: "image", label: "Image" },
];
const QUICK = ["#0f172a", "#1e3a8a", "#0e7490", "#047857", "#7c3aed", "#be123c", "#c2410c", "#f5f5f4"];

const modeOf = (l: CardLook): Mode => ("preset" in l ? "preset" : l.fill);

/** Card background editor: preset finishes, a solid colour, a 2–3 colour gradient, or the user's own image. */
export function LookEditor({ value, onChange }: { value: CardLook; onChange: (l: CardLook) => void }) {
  const [mode, setMode] = useState<Mode>(modeOf(value));
  // Remember each mode's last settings so switching tabs doesn't lose work.
  const [solid, setSolid] = useState<string>("fill" in value && value.fill === "solid" ? value.colors[0] : "#1e3a8a");
  const [grad, setGrad] = useState<{ colors: string[]; angle: number }>(
    "fill" in value && value.fill === "gradient" ? { colors: value.colors, angle: value.angle ?? 135 } : { colors: ["#0f766e", "#7c3aed"], angle: 135 },
  );
  const [image, setImage] = useState<{ src: string; lum: number } | null>("fill" in value && value.fill === "image" ? { src: value.image, lum: 0.5 } : null);
  const [dim, setDim] = useState("fill" in value && value.fill === "image" ? value.dim ?? 0.15 : 0.15);
  const [ink, setInk] = useState<"light" | "dark" | null>("fill" in value ? value.ink ?? null : null);
  const [pattern, setPattern] = useState("fill" in value ? value.pattern ?? value.fill !== "image" : true);
  const [error, setError] = useState("");
  const file = useRef<HTMLInputElement>(null);

  // Suggested text colour from the background; the user can override it.
  const autoInk = (m: Mode, s = solid, g = grad, img = image, d = dim): "light" | "dark" => {
    if (m === "solid") return inkFor(luminance(s));
    if (m === "gradient") return inkFor(g.colors.reduce((a, c) => a + luminance(c), 0) / g.colors.length);
    if (m === "image" && img) return inkFor(img.lum * (1 - d));
    return "light";
  };

  const emit = (next: Partial<{ mode: Mode; solid: string; grad: typeof grad; image: typeof image; dim: number; ink: typeof ink; pattern: boolean; preset: CardDesign }>) => {
    const m = next.mode ?? mode, s = next.solid ?? solid, g = next.grad ?? grad, img = next.image !== undefined ? next.image : image;
    const d = next.dim ?? dim, p = next.pattern ?? pattern, i = (next.ink !== undefined ? next.ink : ink) ?? autoInk(m, s, g, img, d);
    if (m === "preset") onChange({ preset: next.preset ?? ("preset" in value ? value.preset : "midnight") });
    else if (m === "solid") onChange({ fill: "solid", colors: [s], ink: i, pattern: p });
    else if (m === "gradient") onChange({ fill: "gradient", colors: g.colors, angle: g.angle, ink: i, pattern: p });
    else if (img) onChange({ fill: "image", image: img.src, dim: d, ink: i, pattern: p });
  };

  const switchMode = (m: Mode) => {
    setMode(m);
    const p = m === "image" ? false : true;
    setPattern(p);
    setInk(null);
    if (m === "image" && !image) { file.current?.click(); return; }
    emit({ mode: m, pattern: p, ink: null });
  };

  const onFile = async (f: File | undefined) => {
    if (!f) return;
    setError("");
    try {
      const img = await cropToCard(f);
      setImage(img);
      setMode("image");
      setPattern(false);
      setInk(null);
      emit({ mode: "image", image: img, pattern: false, ink: null });
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
  };

  const currentInk = ink ?? autoInk(mode);

  return (
    <div className="space-y-3">
      <Segmented role="tablist" label="Background type" value={mode} onChange={switchMode} className="flex w-full" itemClassName="text-xs"
        options={MODES.map((m) => ({ value: m.id, label: m.label }))} />

      {mode === "preset" && (
        <div className="grid grid-cols-4 gap-2">
          {(Object.keys(DESIGNS) as CardDesign[]).map((k) => {
            const active = "preset" in value && value.preset === k;
            return (
              <button key={k} type="button" onClick={() => emit({ mode: "preset", preset: k })} aria-pressed={active} title={DESIGNS[k].label}
                className={cn("aspect-[1.586] rounded-md ring-offset-2 ring-offset-background transition", active ? "ring-2 ring-ring" : "hover:scale-105")}
                style={{ background: DESIGNS[k].face }}><span className="sr-only">{DESIGNS[k].label}</span></button>
            );
          })}
        </div>
      )}

      {mode === "solid" && (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <ColourInput value={solid} onChange={(c) => { setSolid(c); emit({ solid: c }); }} label="Card colour" />
            <div className="flex flex-wrap gap-1.5">
              {QUICK.map((c) => (
                <button key={c} type="button" aria-label={c} onClick={() => { setSolid(c); emit({ solid: c }); }}
                  className={cn("size-6 rounded-full ring-1 ring-black/10", solid === c && "ring-2 ring-ring ring-offset-1 ring-offset-background")} style={{ background: c }} />
              ))}
            </div>
          </div>
        </div>
      )}

      {mode === "gradient" && (
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            {grad.colors.map((c, i) => (
              <ColourInput key={i} value={c} label={`Colour ${i + 1}`} onChange={(v) => {
                const g = { ...grad, colors: grad.colors.map((x, j) => (j === i ? v : x)) }; setGrad(g); emit({ grad: g });
              }} />
            ))}
            {grad.colors.length < 3 ? (
              <Button type="button" variant="outline" size="icon" className="size-8" aria-label="Add a colour"
                onClick={() => { const g = { ...grad, colors: [...grad.colors, "#db2777"] }; setGrad(g); emit({ grad: g }); }}><Plus /></Button>
            ) : (
              <Button type="button" variant="outline" size="icon" className="size-8" aria-label="Remove the last colour"
                onClick={() => { const g = { ...grad, colors: grad.colors.slice(0, 2) }; setGrad(g); emit({ grad: g }); }}><Minus /></Button>
            )}
          </div>
          <div className="space-y-1">
            <div className="flex justify-between text-xs"><Label htmlFor="angle">Direction</Label><span className="text-muted-foreground tabular-nums">{grad.angle}°</span></div>
            <input id="angle" type="range" min={0} max={360} step={5} value={grad.angle} className="w-full accent-[var(--primary)]"
              onChange={(e) => { const g = { ...grad, angle: Number(e.target.value) }; setGrad(g); emit({ grad: g }); }} />
          </div>
        </div>
      )}

      {mode === "image" && (
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <Button type="button" variant="outline" size="sm" onClick={() => file.current?.click()}><ImagePlus />{image ? "Choose another image" : "Choose an image"}</Button>
            <span className="text-muted-foreground text-xs">Cropped to card shape. Stays on this device.</span>
          </div>
          {image && (
            <div className="space-y-1">
              <div className="flex justify-between text-xs"><Label htmlFor="dim">Darken image</Label><span className="text-muted-foreground tabular-nums">{Math.round(dim * 100)}%</span></div>
              <input id="dim" type="range" min={0} max={0.8} step={0.05} value={dim} className="w-full accent-[var(--primary)]"
                onChange={(e) => { const d = Number(e.target.value); setDim(d); emit({ dim: d }); }} />
            </div>
          )}
        </div>
      )}
      <input ref={file} type="file" accept="image/png,image/jpeg,image/webp" className="hidden" onChange={(e) => { onFile(e.target.files?.[0]); e.target.value = ""; }} />

      {mode !== "preset" && (
        <div className="flex flex-wrap items-center justify-between gap-3 border-t pt-3">
          <div className="flex items-center gap-2 text-xs">
            <span className="text-muted-foreground">Text</span>
            <div className="bg-muted inline-flex rounded-md p-0.5">
              {(["light", "dark"] as const).map((v) => (
                <button key={v} type="button" onClick={() => { setInk(v); emit({ ink: v }); }} aria-pressed={currentInk === v}
                  className={cn("rounded px-2 py-0.5 capitalize", currentInk === v ? "bg-background font-medium shadow-sm" : "text-muted-foreground")}>{v}</button>
              ))}
            </div>
            {ink && <button type="button" className="text-muted-foreground underline-offset-2 hover:underline" onClick={() => { setInk(null); emit({ ink: null }); }}>Auto</button>}
          </div>
          <label className="flex items-center gap-2 text-xs">
            <Switch checked={pattern} onCheckedChange={(p) => { setPattern(p); emit({ pattern: p }); }} />Engraved lines
          </label>
        </div>
      )}
      {error && <p className="text-destructive text-xs">{error}</p>}
    </div>
  );
}

function ColourInput({ value, onChange, label }: { value: string; onChange: (v: string) => void; label: string }) {
  return (
    <label className="relative size-8 shrink-0 cursor-pointer overflow-hidden rounded-md ring-1 ring-black/10" style={{ background: value }} title={`${label}: ${value}`}>
      <input type="color" value={value} onChange={(e) => onChange(e.target.value)} aria-label={label} className="absolute inset-0 size-full cursor-pointer opacity-0" />
    </label>
  );
}

/**
 * Crops an image to card proportions (cover), downsizes to 856×540 and compresses as JPEG so it
 * stays well under the backend's size limit. Also returns its average brightness for text colour.
 */
async function cropToCard(file: File): Promise<{ src: string; lum: number }> {
  const url = URL.createObjectURL(file);
  try {
    const img = await new Promise<HTMLImageElement>((resolve, reject) => {
      const i = new Image();
      i.onload = () => resolve(i);
      i.onerror = () => reject(new Error("That file isn't an image Vaulty can read."));
      i.src = url;
    });
    const W = 856, H = 540;
    const canvas = document.createElement("canvas");
    canvas.width = W; canvas.height = H;
    const ctx = canvas.getContext("2d")!;
    const scale = Math.max(W / img.naturalWidth, H / img.naturalHeight);
    const w = img.naturalWidth * scale, h = img.naturalHeight * scale;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(img, (W - w) / 2, (H - h) / 2, w, h);
    // Average brightness from a tiny copy.
    const tiny = document.createElement("canvas");
    tiny.width = 16; tiny.height = 10;
    const t = tiny.getContext("2d")!;
    t.drawImage(canvas, 0, 0, 16, 10);
    const px = t.getImageData(0, 0, 16, 10).data;
    let sum = 0;
    for (let i = 0; i < px.length; i += 4) sum += (0.2126 * px[i] + 0.7152 * px[i + 1] + 0.0722 * px[i + 2]) / 255;
    let quality = 0.85, src = canvas.toDataURL("image/jpeg", quality);
    while (src.length > 850_000 && quality > 0.4) { quality -= 0.1; src = canvas.toDataURL("image/jpeg", quality); }
    return { src, lum: sum / (px.length / 4) };
  } finally {
    URL.revokeObjectURL(url);
  }
}
