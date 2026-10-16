import { useEffect, useRef } from "react";
import { animate } from "motion/react";
import { useMotion } from "@/lib/motion";
import { cn } from "@/lib/utils";

/** Centre of the dial in the artwork's coordinates (the eight markers sit symmetrically around it). */
const DIAL_CX = 286.8, DIAL_CY = 297.6;

/** Turn the dial: counter-clockwise when the vault opens (unlocking), clockwise when it closes. */
export type DialTurn = "open" | "close";

/**
 * Vaulty logo, drawn inline so it follows the accent colour chosen in Settings.
 * Colours come from CSS variables (--logo, --logo-detail) defined in index.css;
 * on the neutral accent it keeps the brand blue #235BAA.
 * Master artwork: assets/brand/logo.svg.
 *
 * The dial (inner ring, spokes, centre and the eight markers) is its own group, centred on
 * (286.8, 297.6), so it can turn while the rest of the mark stays still.
 */
export function Logo({ className, title = "Vaulty", dial, duration = 0.9 }: { className?: string; title?: string; dial?: DialTurn; duration?: number }) {
  const { full } = useMotion();
  const turn = dial && full ? (dial === "open" ? -360 : 360) : 0;
  const dialRef = useRef<SVGGElement>(null);
  // Rotates with the SVG's own rotate(angle cx cy), which always turns around the dial's centre
  // (CSS transform origins on SVG groups vary between browsers).
  useEffect(() => {
    const g = dialRef.current;
    if (!g || !turn) return;
    const controls = animate(0, turn, {
      duration, ease: [0.65, 0, 0.35, 1],
      onUpdate: (a) => g.setAttribute("transform", `rotate(${a} ${DIAL_CX} ${DIAL_CY})`),
      onComplete: () => g.removeAttribute("transform"),
    });
    return () => controls.stop();
  }, [turn, duration]);
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 52 567 477" role="img" aria-label={title}
      className={cn("vaulty-logo h-auto select-none transition-colors duration-200", className)}>
      <g>
        <path fill="var(--logo)" d="M407.3,58.3h152.5l-18.6,7.7L364.7,522.8l-153.7,0L24.2,66L7.1,58.3l155.6,0.8l125.4,298L407.3,58.3z" />
        <circle fill="var(--logo)" cx="285.8" cy="289" r="134.7" />
        <path fill="var(--logo-detail)" d="M286,201.1c-53,0-96,43-96,96s43,96,96,96s96-43,96-96S339,201.1,286,201.1z M286,374.7 c-42.9,0-77.7-34.8-77.7-77.7c0-42.9,34.8-77.7,77.7-77.7s77.7,34.8,77.7,77.7C363.7,340,328.9,374.7,286,374.7z" />
        <path fill="var(--logo-detail)" d="M178.8,378.7L178.8,378.7c-3.1,0-5.7-2.4-5.7-5.5l-2.5-155.9c0-3.1,2.4-5.7,5.5-5.7h0 c3.1,0,5.7,2.4,5.7,5.5l2.5,155.9C184.4,376.1,181.9,378.7,178.8,378.7z" />
        <path fill="var(--logo-detail)" d="M206.7,263.3l-47.6,0.8c-1.8,0-3.2-1.4-3.3-3.1l-0.1-6.7c0-1.8,1.4-3.2,3.1-3.3l47.6-0.8 c1.8,0,3.2,1.4,3.3,3.1l0.1,6.7C209.9,261.8,208.5,263.3,206.7,263.3z" />
        <path fill="var(--logo-detail)" d="M206.7,346.3l-47.6,0.8c-1.8,0-3.2-1.4-3.3-3.1l-0.1-6.7c0-1.8,1.4-3.2,3.1-3.3l47.6-0.8 c1.8,0,3.2,1.4,3.3,3.1l0.1,6.7C209.9,344.8,208.5,346.3,206.7,346.3z" />
        <path fill="var(--logo-detail)" d="M393.5,378.7L393.5,378.7c-3.1,0-5.6-2.5-5.6-5.6V217.2c0-3.1,2.5-5.6,5.6-5.6l0,0c3.1,0,5.6,2.5,5.6,5.6 v155.9C399.1,376.2,396.6,378.7,393.5,378.7z" />
        <path fill="var(--logo-detail)" d="M415.7,263.3l-47.6,0.8c-1.8,0-3.2-1.4-3.3-3.1l-0.1-6.7c0-1.8,1.4-3.2,3.1-3.3l47.6-0.8 c1.8,0,3.2,1.4,3.3,3.1l0.1,6.7C418.9,261.8,417.5,263.3,415.7,263.3z" />
        <path fill="var(--logo-detail)" d="M415.7,346.3l-47.6,0.8c-1.8,0-3.2-1.4-3.3-3.1l-0.1-6.7c0-1.8,1.4-3.2,3.1-3.3l47.6-0.8 c1.8,0,3.2,1.4,3.3,3.1l0.1,6.7C418.9,344.8,417.5,346.3,415.7,346.3z" />
      </g>
      <g ref={dialRef} className="vaulty-dial">
          <path fill="var(--logo-detail)" d="M286.4,266.6c-17.1,0-31,13.9-31,31s13.9,31,31,31s31-13.9,31-31S303.5,266.6,286.4,266.6z M286.4,314.9 c-9.5,0-17.3-7.7-17.3-17.3c0-9.5,7.7-17.3,17.3-17.3s17.3,7.7,17.3,17.3C303.7,307.1,295.9,314.9,286.4,314.9z" />
          <path fill="var(--logo-detail)" d="M285.4,279.1l-1.8-37c0-2.1,1.8-3.8,4-3.8l0,0c2.2,0,4,1.7,4,3.8l-2.5,37l-1.8,17.2L285.4,279.1z" />
          <path fill="var(--logo-detail)" d="M272.8,285.5l-27.5-24.9c-1.5-1.5-1.4-4,0.1-5.5h0c1.6-1.6,4.1-1.6,5.5-0.1l24.4,27.9l10.8,13.4L272.8,285.5z " />
          <path fill="var(--logo-detail)" d="M268.4,299l-37,1.8c-2.1,0-3.8-1.8-3.8-4l0,0c0-2.2,1.7-4,3.8-4l37,2.5l17.2,1.8L268.4,299z" />
          <path fill="var(--logo-detail)" d="M274.8,311.6l-24.9,27.5c-1.5,1.5-4,1.4-5.5-0.1h0c-1.6-1.6-1.6-4.1-0.1-5.5l27.9-24.4l13.4-10.8L274.8,311.6 z" />
          <path fill="var(--logo-detail)" d="M288.2,316.1l1.8,37c0,2.1-1.8,3.8-4,3.8l0,0c-2.2,0-4-1.7-4-3.8l2.5-37l1.8-17.2L288.2,316.1z" />
          <path fill="var(--logo-detail)" d="M300.9,309.7l27.5,24.9c1.5,1.5,1.4,4-0.1,5.5h0c-1.6,1.6-4.1,1.6-5.5,0.1l-24.4-27.9l-10.8-13.4L300.9,309.7 z" />
          <path fill="var(--logo-detail)" d="M305.3,296.2l37-1.8c2.1,0,3.8,1.8,3.8,4v0c0,2.2-1.7,4-3.8,4l-37-2.5l-17.2-1.8L305.3,296.2z" />
          <path fill="var(--logo-detail)" d="M298.9,283.6l24.9-27.5c1.5-1.5,4-1.4,5.5,0.1h0c1.6,1.6,1.6,4.1,0.1,5.5l-27.9,24.4L288.1,297L298.9,283.6z" />
          <circle fill="var(--logo-detail)" cx="286.8" cy="297.6" r="3" />
          <circle fill="var(--logo)" cx="286.8" cy="209.6" r="3" />
          <circle fill="var(--logo-detail)" cx="286.8" cy="209.6" r="1" />
          <circle fill="var(--logo)" cx="224.6" cy="235.4" r="3" />
          <circle fill="var(--logo-detail)" cx="224.6" cy="235.4" r="1" />
          <circle fill="var(--logo)" cx="198.8" cy="297.6" r="3" />
          <circle fill="var(--logo-detail)" cx="198.8" cy="297.6" r="1" />
          <circle fill="var(--logo)" cx="224.6" cy="359.8" r="3" />
          <circle fill="var(--logo-detail)" cx="224.6" cy="359.8" r="1" />
          <circle fill="var(--logo)" cx="286.8" cy="385.6" r="3" />
          <circle fill="var(--logo-detail)" cx="286.8" cy="385.6" r="1" />
          <circle fill="var(--logo)" cx="349.1" cy="359.8" r="3" />
          <circle fill="var(--logo-detail)" cx="349.1" cy="359.8" r="1" />
          <circle fill="var(--logo)" cx="374.8" cy="297.6" r="3" />
          <circle fill="var(--logo-detail)" cx="374.8" cy="297.6" r="1" />
          <circle fill="var(--logo)" cx="349.1" cy="235.4" r="3" />
          <circle fill="var(--logo-detail)" cx="349.1" cy="235.4" r="1" />
      </g>
    </svg>
  );
}
