import { useEffect, useId, useRef } from "react";

import type { StepId } from "@/kiekje/content";
import { float, ink } from "@/shared/helpers/line-art";

export type Source = "tray" | "key" | "cli";
export type Flow = "next" | "jump";

type Props = {
  step: StepId | undefined;
  source: Source;
  flow: Flow;
};

type LabelProps = { x: number; y: number; text: string; end?: boolean; middle?: boolean };

const TERM_LINES: [number, number][] = [
  [108, 222],
  [114, 238],
  [120, 214],
  [126, 232],
];
const SIDE_LINES: [number, number][] = [
  [124, 360],
  [130, 372],
  [136, 352],
];
const FILE_LINES: [number, number][] = [
  [130, 450],
  [136, 450],
  [142, 446],
];
const TILES: [number, number][] = [
  [431, 184],
  [442, 184],
  [453, 184],
  [431, 195],
  [442, 195],
  [453, 195],
];
const HANDLES: [number, number][] = [
  [200, 104],
  [235, 104],
  [270, 104],
  [270, 127],
  [270, 150],
  [235, 150],
  [200, 150],
  [200, 127],
];

function Label({ x, y, text, end, middle }: LabelProps) {
  const anchor = end ? "end" : middle ? "middle" : "start";
  return (
    <text className="kk-art-label" x={x} y={y} textAnchor={anchor}>
      {text}
    </text>
  );
}

function Aperture({ x, y, r }: { x: number; y: number; r: number }) {
  return (
    <>
      <circle cx={x} cy={y} r={r} />
      <circle className="kk-art-fine" cx={x} cy={y} r={r * 0.38} />
    </>
  );
}

function Backdrop({ id }: { id: string }) {
  return (
    <>
      <defs>
        <pattern id={`${id}-dots`} width="12" height="12" patternUnits="userSpaceOnUse">
          <circle cx="6" cy="6" r="0.6" style={{ fill: "var(--faint)", stroke: "none" }} />
        </pattern>
        <radialGradient id={`${id}-fade`}>
          <stop offset="0" stopColor="#fff" stopOpacity="0.5" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </radialGradient>
        <radialGradient id={`${id}-glow`}>
          <stop offset="0" style={{ stopColor: "var(--accent)", stopOpacity: 0.1 }} />
          <stop offset="0.55" style={{ stopColor: "var(--accent)", stopOpacity: 0.03 }} />
          <stop offset="1" style={{ stopColor: "var(--accent)", stopOpacity: 0 }} />
        </radialGradient>
        <radialGradient id={`${id}-shadow`}>
          <stop offset="0" stopColor="#000" stopOpacity="0.6" />
          <stop offset="1" stopColor="#000" stopOpacity="0" />
        </radialGradient>
        <mask id={`${id}-mask`}>
          <rect width="480" height="300" style={{ fill: `url(#${id}-fade)`, stroke: "none" }} />
        </mask>
      </defs>
      <rect
        className="kk-art-dash"
        width="480"
        height="300"
        mask={`url(#${id}-mask)`}
        style={{ fill: `url(#${id}-dots)`, stroke: "none" }}
      />
      <path
        className="kk-art-fine"
        d="M8 18 V8 H18 M462 8 H472 V18 M472 282 V292 H462 M18 292 H8 V282"
      />
      <g className="kk-art-layer" data-depth="0.15">
        <ellipse
          className="kk-art-dash"
          cx="290"
          cy="130"
          rx="170"
          ry="96"
          style={{ fill: `url(#${id}-glow)`, stroke: "none" }}
        />
      </g>
      <g className="kk-art-layer" data-depth="0.5">
        <ellipse
          className="kk-art-dash"
          cx="290"
          cy="200"
          rx="140"
          ry="10"
          style={{ fill: `url(#${id}-shadow)`, stroke: "none" }}
        />
      </g>
    </>
  );
}

function Sources() {
  return (
    <g className="kk-art-layer" data-depth="0.7">
      <path className="kk-art-dash kk-art-fine" d="M80 99 C104 99 108 166 132 166" />
      <path className="kk-art-dash kk-art-fine" d="M112 147 C122 147 122 169 132 169" />
      <path className="kk-art-dash kk-art-fine" d="M112 187 C122 187 122 172 132 172" />
      <path className="kk-art-dash kk-art-fine" d="M160 169 C168 169 168 140 176 140" />
      <line className="kk-art-dash kk-art-fine" x1="124" y1="163" x2="124" y2="175" />
      <g className="kk-x-src kk-x-src-key">
        <rect className="kk-art-face" x="36" y="86" width="44" height="26" rx="3" />
        <rect className="kk-art-fine" x="39" y="89" width="38" height="18" rx="2" />
      </g>
      <g className="kk-x-src kk-x-src-tray">
        <rect className="kk-art-face" x="28" y="140" width="84" height="14" rx="2" />
        <circle className="kk-art-fine" cx="40" cy="147" r="1.4" />
        <circle className="kk-art-fine" cx="48" cy="147" r="1.4" />
        <Aperture x={96} y={147} r={4} />
      </g>
      <g className="kk-x-src kk-x-src-cli">
        <rect className="kk-art-face" x="28" y="176" width="84" height="22" rx="2" />
      </g>
      <rect className="kk-art-face" x="132" y="158" width="28" height="22" rx="3" />
      <Aperture x={146} y={169} r={5} />
      <path className="kk-art-fine" d="M124 175 V222 H30" />
      <g className="kk-art-late">
        <Label x={58} y={101.5} text="Print" middle />
        <Label x={34} y={190.5} text="$ kiekje capture" />
        <Label x={146} y={192} text="one action" middle />
        <Label x={30} y={219} text="second press ignored" />
        <rect
          className="kk-art-accent kk-x-act kk-x-flash kk-x-flash-key"
          x="36"
          y="86"
          width="44"
          height="26"
          rx="3"
        />
        <rect
          className="kk-art-accent kk-x-act kk-x-flash kk-x-flash-tray"
          x="28"
          y="140"
          width="84"
          height="14"
          rx="2"
        />
        <rect
          className="kk-art-accent kk-x-act kk-x-flash kk-x-flash-cli"
          x="28"
          y="176"
          width="84"
          height="22"
          rx="2"
        />
        <circle className="kk-x-dot kk-x-act kk-x-bead kk-x-bead-key" cx="80" cy="99" r="2" />
        <circle className="kk-x-dot kk-x-act kk-x-bead kk-x-bead-tray" cx="112" cy="147" r="2" />
        <circle className="kk-x-dot kk-x-act kk-x-bead kk-x-bead-cli" cx="112" cy="187" r="2" />
        <circle className="kk-x-dot kk-x-act kk-x-again kk-x-again-key" cx="80" cy="99" r="2" />
        <circle className="kk-x-dot kk-x-act kk-x-again kk-x-again-tray" cx="112" cy="147" r="2" />
        <circle className="kk-x-dot kk-x-act kk-x-again kk-x-again-cli" cx="112" cy="187" r="2" />
        <line className="kk-art-accent kk-x-act kk-x-stop" x1="124" y1="163" x2="124" y2="175" />
        <rect
          className="kk-art-accent kk-x-act kk-x-inst"
          x="132"
          y="158"
          width="28"
          height="22"
          rx="3"
        />
        <circle className="kk-x-dot kk-x-act kk-x-grab" cx="160" cy="169" r="2" />
      </g>
    </g>
  );
}

function Monitors() {
  return (
    <>
      <g className="kk-art-layer" data-depth="0.9">
        <line className="kk-art-fine" x1="360" y1="170" x2="360" y2="186" />
        <ellipse className="kk-art-face" cx="360" cy="188" rx="15" ry="3" />
        <rect className="kk-art-face" x="316" y="98" width="88" height="72" rx="3" />
        <rect className="kk-art-fine" x="320" y="102" width="80" height="60" rx="1" />
        <rect className="kk-art-face" x="326" y="110" width="54" height="36" rx="2" />
        <line x1="326" y1="117" x2="380" y2="117" />
        {SIDE_LINES.map(([y, end]) => (
          <line key={y} className="kk-art-fine" x1="330" y1={y} x2={end} y2={y} />
        ))}
        <rect className="kk-art-face" x="368" y="128" width="28" height="28" rx="2" />
        <line className="kk-art-fine" x1="368" y1="134" x2="396" y2="134" />
      </g>
      <g className="kk-art-layer" data-depth="0.6">
        <line x1="168" y1="199" x2="430" y2="199" />
        <line className="kk-art-fine" x1="176" y1="203" x2="422" y2="203" />
      </g>
      <g className="kk-art-layer" data-depth="1.1">
        <line className="kk-art-fine" x1="241" y1="176" x2="241" y2="192" />
        <ellipse className="kk-art-face" cx="241" cy="194" rx="20" ry="3.5" />
        <rect className="kk-art-face" x="176" y="84" width="130" height="92" rx="3" />
        <rect className="kk-art-fine" x="180" y="88" width="122" height="80" rx="1" />
        <rect className="kk-art-face" x="186" y="94" width="66" height="46" rx="2" />
        <line x1="186" y1="101" x2="252" y2="101" />
        {TERM_LINES.map(([y, end]) => (
          <line key={y} className="kk-art-fine" x1="190" y1={y} x2={end} y2={y} />
        ))}
        <line className="kk-art-lit" x1="190" y1="132" x2="226" y2="132" />
        <rect className="kk-art-face" x="236" y="112" width="62" height="52" rx="2" />
        <line x1="236" y1="119" x2="298" y2="119" />
        <circle className="kk-art-fine" cx="267" cy="136" r="8" />
        <circle className="kk-art-fine" cx="267" cy="136" r="3" />
        <line className="kk-art-dash kk-art-fine" x1="240" y1="158" x2="294" y2="158" />
        <line className="kk-art-lit" x1="240" y1="158" x2="262" y2="158" />
      </g>
    </>
  );
}

function Frame() {
  return (
    <g className="kk-art-layer" data-depth="1">
      <g className="kk-art-late">
        <rect
          className="kk-x-act kk-x-shutter"
          x="170"
          y="78"
          width="240"
          height="100"
          style={{ fill: "var(--fg)", stroke: "none" }}
        />
        <g className="kk-x-keep kk-x-frame">
          <rect className="kk-art-dash" x="170" y="78" width="240" height="100" />
          <path
            className="kk-art-accent"
            d="M170 90 V78 H182 M398 78 H410 V90 M410 166 V178 H398 M182 178 H170 V166"
          />
        </g>
        <g className="kk-x-keep kk-x-lbl">
          <Label x={172} y={71} text="raw frame, every monitor" />
          <path className="kk-art-fine" d="M172 78 V73.5 H264" />
          <Label x={410} y={71} text="14:32:18.004" end />
          <path className="kk-art-fine" d="M410 78 V73.5 H364" />
        </g>
      </g>
    </g>
  );
}

function Destinations() {
  return (
    <g className="kk-art-layer" data-depth="1.2">
      <line className="kk-art-dash kk-art-fine" x1="410" y1="133" x2="421" y2="133" />
      <path className="kk-art-dash kk-art-fine" d="M421 133 C425 110 427 80 434 73" />
      <line className="kk-art-dash kk-art-fine" x1="421" y1="133" x2="434" y2="133" />
      <path className="kk-art-dash kk-art-fine" d="M421 133 C425 160 427 185 431 193" />
      <rect className="kk-art-face" x="434" y="60" width="20" height="26" rx="2" />
      <rect className="kk-art-face" x="440" y="56" width="8" height="5" rx="1.5" />
      <line className="kk-art-fine" x1="438" y1="70" x2="450" y2="70" />
      <line className="kk-art-fine" x1="438" y1="76" x2="446" y2="76" />
      <path className="kk-art-face" d="M434 118 H448 L454 124 V148 H434 Z" />
      <path d="M448 118 V124 H454" />
      {FILE_LINES.map(([y, end]) => (
        <line key={y} className="kk-art-fine" x1="438" y1={y} x2={end} y2={y} />
      ))}
      {TILES.map(([x, y]) => (
        <rect key={`${x}-${y}`} className="kk-art-fine" x={x} y={y} width="8" height="8" />
      ))}
      <g className="kk-art-late">
        <Label x={444} y={96} text="clipboard" middle />
        <Label x={444} y={158} text="encoded once" middle />
        <Label x={444} y={214} text="gallery" middle />
        <circle className="kk-x-dot kk-x-act kk-x-to kk-x-to-clip" cx="421" cy="133" r="2" />
        <circle className="kk-x-dot kk-x-act kk-x-to kk-x-to-file" cx="421" cy="133" r="2" />
        <circle className="kk-x-dot kk-x-act kk-x-to kk-x-to-gal" cx="421" cy="133" r="2" />
        <rect
          className="kk-art-accent kk-x-keep kk-x-done kk-x-lit-clip"
          x="434"
          y="60"
          width="20"
          height="26"
          rx="2"
        />
        <path
          className="kk-art-accent kk-x-keep kk-x-done kk-x-lit-file"
          d="M434 118 H448 L454 124 V148 H434 Z"
        />
        <rect
          className="kk-x-tile kk-x-keep kk-x-done kk-x-lit-gal"
          x="431"
          y="184"
          width="8"
          height="8"
        />
      </g>
    </g>
  );
}

function Selection() {
  return (
    <g className="kk-art-layer" data-depth="1.4">
      <g className="kk-art-late">
        <g className="kk-x-keep kk-x-sel kk-x-region">
          <rect className="kk-art-accent" x="200" y="104" width="70" height="46" />
        </g>
        <g className="kk-x-keep kk-x-sel kk-x-arrow">
          <line className="kk-art-accent kk-x-arrow-line" x1="212" y1="142" x2="256" y2="114" />
          <path className="kk-art-accent kk-x-arrow-head" d="M249.5 113 L256 114 L252 119.5" />
        </g>
        <g className="kk-x-keep kk-x-sel kk-x-handles">
          {HANDLES.map(([x, y]) => (
            <rect key={`${x}-${y}`} x={x - 2} y={y - 2} width="4" height="4" />
          ))}
        </g>
        <g className="kk-x-keep kk-x-sel kk-x-size">
          <Label x={235} y={100} text="640 × 300" middle />
        </g>
        <g className="kk-x-act kk-x-cursor">
          <g className="kk-x-drag">
            <path d="M194 104 H198 M202 104 H206 M200 98 V102 M200 106 V110" />
          </g>
        </g>
        <g className="kk-x-act kk-x-loupe">
          <g className="kk-x-drag">
            <circle className="kk-art-face" cx="218" cy="120" r="11" />
            <path className="kk-art-fine" d="M207 120 H229 M218 109 V131" />
            <path
              className="kk-art-fine"
              d="M211 115 H225 M211 125 H225 M213 109.5 V130.5 M223 109.5 V130.5"
            />
            <rect className="kk-art-accent" x="216" y="118" width="4" height="4" />
            <Label x={218} y={137} text="#2a1b3d" middle />
          </g>
        </g>
        <g className="kk-x-act kk-x-fly">
          <rect className="kk-art-dash kk-art-accent" x="200" y="104" width="70" height="46" />
          <line className="kk-art-accent" x1="212" y1="142" x2="256" y2="114" />
        </g>
        <g className="kk-x-keep kk-x-done kk-x-toast">
          <rect className="kk-art-face" x="204" y="238" width="176" height="30" rx="3" />
          <circle className="kk-art-fine" cx="218" cy="253" r="6" />
          <path className="kk-art-accent kk-x-check" d="M215 253 L217.5 255.5 L222 250.5" />
          <Label x={230} y={250} text="Kiekje" />
          <Label x={230} y={260} text="~/Pictures/Kiekje/14-32-18.png" />
        </g>
      </g>
    </g>
  );
}

export function StepArt({ step, source, flow }: Props) {
  const id = `kk-how-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const stage = useRef<HTMLDivElement>(null);
  const art = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const node = art.current;
    const surface = stage.current;
    if (!node || !surface) return;
    const stopFloat = float(node, surface);
    const stopInk = ink(node, surface);
    return () => {
      stopFloat?.();
      stopInk?.();
    };
  }, []);

  return (
    <div
      ref={stage}
      className="kk-how-stage"
      data-step={step}
      data-source={source}
      data-flow={flow}
      aria-hidden="true"
    >
      <svg ref={art} className="kk-art" viewBox="0 0 480 300" focusable="false">
        <Backdrop id={id} />
        <Sources />
        <Monitors />
        <Frame />
        <Destinations />
        <Selection />
      </svg>
    </div>
  );
}
