import { useEffect, useId, useRef, type CSSProperties } from "react";

import type { PrivacyArt as Art } from "@/kiekje/content";
import { float, ink } from "@/shared/helpers/line-art";

const HOURS = Array.from({ length: 24 }, (_, index) => index * 15);
const RIM = Array.from({ length: 24 }, (_, index) => (index * Math.PI) / 12);
const PACKETS: [number, number, number][] = [
  [-34, -16, 0],
  [0, -26, 800],
  [34, -16, 1600],
];
const SELECTED: [number, number][] = [
  [62, 60],
  [136, 60],
  [62, 96],
  [136, 96],
];
const WORDS: [number, number, number][] = [
  [54, 52, 76],
  [82, 52, 120],
  [54, 64, 90],
  [96, 64, 132],
  [54, 76, 68],
  [74, 76, 112],
  [118, 76, 140],
  [54, 88, 104],
  [110, 88, 128],
  [54, 100, 84],
  [90, 100, 142],
  [54, 112, 98],
  [104, 112, 122],
  [54, 124, 80],
];
const COPIED: [number, number][] = [
  [72, 190],
  [82, 186],
  [92, 178],
];
const DOME = Array.from({ length: 13 }, (_, index) => 30 + index * 10);
const DETECTED: [number, number, number][] = [
  [64, 64, 90],
  [96, 64, 132],
  [74, 76, 112],
  [118, 76, 134],
  [64, 88, 104],
  [110, 88, 128],
];
const WEEK = Array.from({ length: 7 }, (_, index) => 176 + index * 6);
const RULER = Array.from({ length: 12 }, (_, index) => 46 + index * 8);

function vars(values: Record<string, string>) {
  return values as CSSProperties;
}

function rim(angle: number, grow: number) {
  return [120 + (78 + grow) * Math.cos(angle), 124 + (16 + grow / 4) * Math.sin(angle)];
}

const GLOW: Record<Art, [number, number, number, number]> = {
  telemetry: [120, 96, 110, 76],
  update: [96, 84, 92, 80],
  upload: [120, 84, 116, 64],
  text: [118, 82, 104, 74],
};
const SHADOW: Record<Art, [number, number, number, number]> = {
  telemetry: [120, 136, 92, 14],
  update: [92, 146, 58, 7],
  upload: [52, 138, 36, 6],
  text: [120, 154, 92, 6],
};

function Backdrop({ id, name }: { id: string; name: Art }) {
  const [gx, gy, grx, gry] = GLOW[name];
  const [sx, sy, srx, sry] = SHADOW[name];
  return (
    <>
      <defs>
        <pattern id={`${id}-dots`} width="12" height="12" patternUnits="userSpaceOnUse">
          <circle cx="6" cy="6" r="0.6" style={{ fill: "var(--faint)", stroke: "none" }} />
        </pattern>
        <radialGradient id={`${id}-fade`}>
          <stop offset="0" stopColor="#fff" stopOpacity="0.55" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </radialGradient>
        <radialGradient id={`${id}-glow`}>
          <stop offset="0" style={{ stopColor: "var(--accent)", stopOpacity: 0.11 }} />
          <stop offset="0.55" style={{ stopColor: "var(--accent)", stopOpacity: 0.035 }} />
          <stop offset="1" style={{ stopColor: "var(--accent)", stopOpacity: 0 }} />
        </radialGradient>
        <radialGradient id={`${id}-shadow`}>
          <stop offset="0" stopColor="#000" stopOpacity="0.65" />
          <stop offset="1" stopColor="#000" stopOpacity="0" />
        </radialGradient>
        <mask id={`${id}-mask`}>
          <rect width="240" height="160" style={{ fill: `url(#${id}-fade)`, stroke: "none" }} />
        </mask>
      </defs>
      <rect
        className="kk-art-dash"
        width="240"
        height="160"
        mask={`url(#${id}-mask)`}
        style={{ fill: `url(#${id}-dots)`, stroke: "none" }}
      />
      <path
        className="kk-art-fine"
        d="M6 14 V6 H14 M226 6 H234 V14 M234 146 V154 H226 M14 154 H6 V146"
      />
      <g className="kk-art-layer" data-depth="0.15">
        <ellipse
          className="kk-art-dash"
          cx={gx}
          cy={gy}
          rx={grx}
          ry={gry}
          style={{ fill: `url(#${id}-glow)`, stroke: "none" }}
        />
      </g>
      <g className="kk-art-layer" data-depth="0.6">
        <ellipse
          className="kk-art-dash"
          cx={sx}
          cy={sy}
          rx={srx}
          ry={sry}
          style={{ fill: `url(#${id}-shadow)`, stroke: "none" }}
        />
      </g>
    </>
  );
}

type LabelProps = { x: number; y: number; text: string; end?: boolean; middle?: boolean };

function Label({ x, y, text, end, middle }: LabelProps) {
  const anchor = end ? "end" : middle ? "middle" : "start";
  return (
    <text className="kk-art-label" x={x} y={y} textAnchor={anchor}>
      {text}
    </text>
  );
}

function Telemetry() {
  return (
    <>
      <g className="kk-art-layer" data-depth="0.3">
        <path className="kk-art-dash kk-art-fine" d="M28 124 A92 92 0 0 1 212 124" />
        {DOME.map((angle) => {
          const turn = (angle * Math.PI) / 180;
          const reach = angle % 30 === 0 ? 86 : 82;
          return (
            <line
              key={angle}
              className="kk-art-fine"
              x1={120 - 78 * Math.cos(turn)}
              y1={124 - 78 * Math.sin(turn)}
              x2={120 - reach * Math.cos(turn)}
              y2={124 - reach * Math.sin(turn)}
            />
          );
        })}
        <g className="kk-art-late">
          <ellipse
            className="kk-art-dash kk-art-flow"
            cx="120"
            cy="106"
            rx="110"
            ry="22"
            transform="rotate(-8 120 106)"
          />
          <ellipse
            className="kk-art-beads"
            cx="120"
            cy="106"
            rx="110"
            ry="22"
            transform="rotate(-8 120 106)"
          />
        </g>
      </g>
      <g className="kk-art-layer" data-depth="0.6">
        <ellipse className="kk-art-face" cx="120" cy="124" rx="78" ry="16" />
        {RIM.map((angle) => {
          const [x1, y1] = rim(angle, 0);
          const [x2, y2] = rim(angle, 6);
          return <line key={angle} className="kk-art-fine" x1={x1} y1={y1} x2={x2} y2={y2} />;
        })}
        <ellipse className="kk-art-dash" cx="120" cy="124" rx="62" ry="11" />
        <ellipse className="kk-art-dash kk-art-fine" cx="120" cy="124" rx="44" ry="7" />
        <g className="kk-art-late">
          <ellipse className="kk-art-pulse" cx="120" cy="124" rx="78" ry="16" />
          <ellipse
            className="kk-art-pulse"
            cx="120"
            cy="124"
            rx="78"
            ry="16"
            style={{ animationDelay: "1500ms" }}
          />
        </g>
      </g>
      <g className="kk-art-layer" data-depth="1">
        <path className="kk-art-face" d="M42 124 A78 78 0 0 1 198 124 A78 16 0 0 1 42 124 Z" />
        <path d="M42 124 A78 78 0 0 1 198 124" />
        <path className="kk-art-dash" d="M86 124 A34 78 0 0 1 154 124" />
        <path className="kk-art-dash kk-art-fine" d="M60 124 A60 78 0 0 1 180 124" />
        <path className="kk-art-dash" d="M49 92 A71 12 0 0 0 191 92" />
        <path className="kk-art-dash kk-art-fine" d="M66 68 A54 9 0 0 0 174 68" />
      </g>
      <g className="kk-art-layer" data-depth="1.7">
        <g className="kk-art-late">
          <path
            className="kk-art-dash kk-art-flow"
            d="M10 106 A110 22 0 0 0 230 106"
            transform="rotate(-8 120 106)"
          />
          <path
            className="kk-art-beads"
            d="M10 106 A110 22 0 0 0 230 106"
            transform="rotate(-8 120 106)"
          />
        </g>
      </g>
      <g className="kk-art-layer" data-depth="1.2">
        <rect x="96" y="76" width="48" height="31" rx="3" />
        <rect className="kk-art-fine" x="100" y="80" width="40" height="23" rx="1" />
        <line className="kk-art-fine" x1="104" y1="86" x2="124" y2="86" />
        <line className="kk-art-fine" x1="104" y1="91" x2="132" y2="91" />
        <line className="kk-art-fine" x1="104" y1="96" x2="118" y2="96" />
        <rect className="kk-art-accent" x="122" y="88" width="12" height="10" />
        <path d="M88 108 H152 L158 116 H82 Z" />
        <line className="kk-art-dash" x1="92" y1="112" x2="148" y2="112" />
        <path className="kk-art-face" d="M54 114 H64 L66 116 H78 V128 H54 Z" />
        <line className="kk-art-fine" x1="54" y1="118" x2="78" y2="118" />
        <g className="kk-art-late">
          <rect className="kk-art-thumb" x="122" y="88" width="12" height="10" rx="1" />
          {PACKETS.map(([dx, dy, delay]) => (
            <g key={dx}>
              <circle
                className="kk-art-packet"
                cx="120"
                cy="74"
                r="1.8"
                style={vars({
                  "--dx": `${dx}px`,
                  "--dy": `${dy}px`,
                  animationDelay: `${delay}ms`,
                })}
              />
              <path
                className="kk-art-ripple"
                d={`M${114 + dx} ${76 + dy} A8 8 0 0 1 ${126 + dx} ${76 + dy}`}
                style={{
                  transformOrigin: `${120 + dx}px ${74 + dy}px`,
                  animationDelay: `${delay}ms`,
                }}
              />
            </g>
          ))}
        </g>
      </g>
      <g className="kk-art-layer" data-depth="1">
        <path className="kk-art-fine" d="M152 52 L166 34 H222" />
        <path className="kk-art-fine" d="M62 128 L80 148 H18" />
        <g className="kk-art-late">
          <Label x={168} y={31} text="0 requests sent" />
          <Label x={20} y={145} text="~/Pictures/Kiekje" />
        </g>
      </g>
    </>
  );
}

function Update() {
  return (
    <>
      <g className="kk-art-layer" data-depth="0.5">
        <path className="kk-art-dash" d="M92 32 C112 2 160 6 182 44" />
      </g>
      <g className="kk-art-layer" data-depth="0.9">
        <rect className="kk-art-face" x="178" y="40" width="34" height="11" rx="2" />
        <rect className="kk-art-face" x="178" y="54" width="34" height="11" rx="2" />
        <rect className="kk-art-face" x="178" y="68" width="34" height="11" rx="2" />
        {[45.5, 59.5, 73.5].map((y) => (
          <line key={y} className="kk-art-fine" x1="184" y1={y} x2="196" y2={y} />
        ))}
        <line className="kk-art-fine" x1="195" y1="79" x2="195" y2="92" />
        <line className="kk-art-fine" x1="184" y1="92" x2="206" y2="92" />
      </g>
      <g className="kk-art-layer" data-depth="0.3">
        <circle className="kk-art-dash kk-art-fine" cx="92" cy="84" r="64" />
        <g className="kk-art-late">
          <circle className="kk-art-beads" cx="92" cy="84" r="64" />
        </g>
      </g>

      <g className="kk-art-layer" data-depth="0.9">
        {WEEK.map((x) => (
          <rect key={x} className="kk-art-fine" x={x} y="106" width="6" height="8" />
        ))}
        <rect className="kk-art-fine" x="176" y="133" width="16" height="8" rx="4" />
        <g className="kk-art-late">
          <rect className="kk-art-switch" x="176" y="133" width="16" height="8" rx="4" />
          <circle className="kk-art-knob" cx="180" cy="137" r="2.4" />
          <Label x={197} y={139.5} text="opt-in" />
        </g>
      </g>
      <g className="kk-art-layer" data-depth="1">
        <circle className="kk-art-face" cx="92" cy="84" r="54" />
        <circle cx="92" cy="84" r="50" />
        <path className="kk-art-dash kk-art-hatch" d="M45 84 A47 47 0 0 0 139 84" />
        {HOURS.map((angle) => (
          <line
            key={angle}
            className={angle % 90 === 0 ? undefined : "kk-art-fine"}
            x1="92"
            y1="40"
            x2="92"
            y2={angle % 90 === 0 ? 48 : 44}
            transform={`rotate(${angle} 92 84)`}
          />
        ))}
        <circle className="kk-art-dash" cx="92" cy="84" r="30" />
        <circle className="kk-art-fine" cx="92" cy="84" r="18" />
        <circle className="kk-art-fine" cx="92" cy="62" r="4" />
        <path className="kk-art-fine" d="M88 106 A5 5 0 1 0 96 102 A4 4 0 0 1 88 106 Z" />
        <path className="kk-art-accent" d="M86 34.4 A50 50 0 0 1 98 34.4" />
        <g className="kk-art-hand">
          <line x1="92" y1="96" x2="92" y2="46" />
          <circle cx="92" cy="46" r="2" />
        </g>
        <circle className="kk-art-face" cx="92" cy="84" r="4" />
        <g className="kk-art-late">
          <circle className="kk-art-request" cx="92" cy="32" r="2.2" />
          <circle className="kk-art-response" cx="92" cy="32" r="2.2" />
          <circle className="kk-art-blink" cx="206" cy="45.5" r="1.8" />
          <rect className="kk-art-blink" x="200.5" y="106.5" width="5" height="7" />
          <Label x={176} y={101} text="api.github.com" />
          <Label x={176} y={125} text="once a day, max" />
          <Label x={20} y={145} text="00:00" />
        </g>
        <path className="kk-art-fine" d="M80 133 L66 148 H18" />
      </g>
    </>
  );
}

function Upload() {
  return (
    <>
      <g className="kk-art-layer" data-depth="0.8">
        <line className="kk-art-dash" x1="154" y1="40" x2="154" y2="142" />
        {RULER.map((y) => (
          <line
            key={y}
            className="kk-art-fine"
            x1={y % 16 === 14 ? 146 : 149}
            y1={y}
            x2="152"
            y2={y}
          />
        ))}
      </g>
      <g className="kk-art-layer" data-depth="0.4">
        <path className="kk-art-dash kk-art-fine" d="M36 50 V40 H74 L84 50 V104 H78" />
      </g>
      <g className="kk-art-layer" data-depth="1">
        <path className="kk-art-fine" d="M26 50 L30 46 H68 L78 56 V110 L74 114" />
        <path className="kk-art-face" d="M26 50 H64 L74 60 V114 H26 Z" />
        <path d="M64 50 V60 H74" />
        <rect x="32" y="66" width="36" height="26" rx="2" />
        <line className="kk-art-fine" x1="32" y1="71" x2="68" y2="71" />
        <path d="M32 92 L43 81 L50 87 L56 82 L68 92" />
        <circle cx="61" cy="77" r="2.2" />
        <line className="kk-art-dash" x1="32" y1="102" x2="56" y2="102" />
        <rect x="26" y="122" width="48" height="10" rx="2" />
      </g>
      <g className="kk-art-layer" data-depth="1">
        <line className="kk-art-dash kk-art-fine" x1="106" y1="98" x2="106" y2="108" />
        <rect className="kk-art-face" x="82" y="108" width="48" height="10" rx="2" />
        <g className="kk-art-late">
          <Label x={50} y={129.5} text="IMG_0412.png" middle />
          <g className="kk-art-name-old">
            <Label x={106} y={115.5} text="IMG_0412.png" middle />
          </g>
          <g className="kk-art-name-new">
            <Label x={106} y={115.5} text="a7f3.png" middle />
          </g>
          <Label x={204} y={110} text="litterbox, 1 to 72 h" middle />
          <Label x={150} y={150} text="this machine" end />
        </g>
      </g>
      <g className="kk-art-layer" data-depth="1">
        <path d="M74 82 C90 82 92 98 106 98 C118 98 120 82 136 82" />
        <g className="kk-art-late">
          <path className="kk-art-beads" d="M74 82 C90 82 92 98 106 98 C118 98 120 82 128 82" />
        </g>
      </g>
      <g className="kk-art-layer" data-depth="0.4">
        <ellipse className="kk-art-dash kk-art-fine" cx="204" cy="81" rx="30" ry="20" />
      </g>
      <g className="kk-art-layer" data-depth="1.2">
        <g className="kk-art-plug">
          <rect className="kk-art-face" x="126" y="74" width="16" height="16" rx="2.5" />
          {[131, 134, 137].map((x) => (
            <line key={x} className="kk-art-fine" x1={x} y1="78" x2={x} y2="86" />
          ))}
          <line x1="142" y1="79" x2="148" y2="79" />
          <line x1="142" y1="85" x2="148" y2="85" />
        </g>
      </g>
      <g className="kk-art-layer" data-depth="0.7">
        <rect className="kk-art-face" x="160" y="70" width="10" height="24" rx="2.5" />
        <line className="kk-art-fine" x1="160" y1="79" x2="165" y2="79" />
        <line className="kk-art-fine" x1="160" y1="85" x2="165" y2="85" />
        <line className="kk-art-dash" x1="170" y1="82" x2="184" y2="82" />
        <path d="M186 92 H222 A8 8 0 0 0 222 76 A12 12 0 0 0 201 69 A10 10 0 0 0 186 77 A7.5 7.5 0 0 0 186 92 Z" />
        <circle className="kk-art-fine" cx="205" cy="83" r="4" />
        <g className="kk-art-late">
          <circle
            className="kk-art-expire"
            cx="205"
            cy="83"
            r="6.5"
            transform="rotate(-90 205 83)"
          />
          <line className="kk-art-tick" x1="205" y1="83" x2="205" y2="80.4" />
        </g>
      </g>
      <g className="kk-art-layer" data-depth="1.6">
        <circle className="kk-art-face" cx="154" cy="30" r="9" />
        <rect x="149.5" y="29" width="9" height="7" rx="1.5" />
        <path d="M151.5 29 V26.5 A2.5 2.5 0 0 1 156.5 26.5 V29" />
        <g className="kk-art-late">
          <line className="kk-art-block kk-art-barrier" x1="154" y1="64" x2="154" y2="100" />
          <rect
            className="kk-art-block kk-art-lock"
            x="149.5"
            y="29"
            width="9"
            height="7"
            rx="1.5"
          />
        </g>
      </g>
    </>
  );
}

function Text() {
  return (
    <>
      <g className="kk-art-layer" data-depth="0.3">
        <rect className="kk-art-dash" x="34" y="14" width="172" height="134" rx="12" />
      </g>
      <g className="kk-art-layer" data-depth="0.8">
        <rect x="44" y="26" width="108" height="110" rx="6" />
        <line x1="44" y1="38" x2="152" y2="38" />
        <line className="kk-art-fine" x1="110" y1="32" x2="146" y2="32" />
        {[51, 57, 63].map((x) => (
          <circle key={x} cx={x} cy="32" r="1.5" />
        ))}
        {WORDS.map(([x, y, end]) => (
          <line
            key={`${x}-${y}`}
            className={y > 60 && y < 96 ? "kk-art-lit" : "kk-art-fine"}
            x1={x}
            y1={y}
            x2={end}
            y2={y}
          />
        ))}
      </g>
      <g className="kk-art-layer" data-depth="1.3">
        <rect className="kk-art-face" x="162" y="52" width="34" height="62" rx="3" />
        <rect className="kk-art-face" x="172" y="48" width="14" height="7" rx="2" />
        <line className="kk-art-fine" x1="168" y1="104" x2="190" y2="104" />
      </g>
      <g className="kk-art-layer" data-depth="1.8">
        <circle className="kk-art-face" cx="206" cy="148" r="9" />
        <rect x="201.5" y="147" width="9" height="7" rx="1.5" />
        <path d="M203.5 147 V144.5 A2.5 2.5 0 0 1 208.5 144.5 V147" />
      </g>
      <g className="kk-art-layer" data-depth="1.1">
        <g className="kk-art-late">
          <g className="kk-art-select">
            <rect className="kk-art-dash" x="62" y="60" width="74" height="36" />
          </g>
          <g transform="translate(62 60)">
            <path
              className="kk-art-cursor"
              d="M0 0 V9.4 L2.6 7 L4.4 10.8 L6 10 L4.2 6.3 L7.6 6.2 Z"
            />
          </g>
          <g className="kk-art-grip">
            {SELECTED.map(([x, y]) => (
              <rect
                key={`${x}-${y}`}
                className="kk-art-handle"
                x={x - 3}
                y={y - 3}
                width="6"
                height="6"
              />
            ))}
          </g>
          <line className="kk-art-scan" x1="64" y1="62" x2="134" y2="62" />
          <line className="kk-art-progress" x1="110" y1="32" x2="146" y2="32" />
          {DETECTED.map(([x, y, end]) => (
            <rect
              key={`${x}-${y}`}
              className="kk-art-detect"
              x={x - 2}
              y={y - 4}
              width={end - x + 4}
              height="8"
              rx="1"
              style={{ animationDelay: `${((y - 64) / 12) * 540}ms` }}
            />
          ))}
          <path className="kk-art-check" d="M174 63 L177 66 L183 60" />
          <g className="kk-art-done">
            <Label x={179} y={126} text="copied" middle />
          </g>
          <Label x={44} y={145} text="tesseract, eng + nld" />
          <line className="kk-art-dash kk-art-fine" x1="140" y1="78" x2="160" y2="78" />
          <circle className="kk-art-hop" cx="140" cy="78" r="1.8" />
          {COPIED.map(([y, end], index) => (
            <line
              key={y}
              className="kk-art-copied"
              x1="168"
              y1={y}
              x2={end}
              y2={y}
              style={{ animationDelay: `${index * 180}ms` }}
            />
          ))}
        </g>
      </g>
    </>
  );
}

const ART = { telemetry: Telemetry, update: Update, upload: Upload, text: Text };

export function PrivacyArt({ name }: { name: Art }) {
  const Drawing = ART[name];
  const id = `kk-art-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const art = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const node = art.current;
    const card = node?.closest("li");
    if (!node || !card) return;
    const stopFloat = float(node, card);
    const stopInk = ink(node, card);
    return () => {
      stopFloat?.();
      stopInk?.();
    };
  }, []);

  return (
    <svg ref={art} className="kk-art" viewBox="0 0 240 160" aria-hidden="true" focusable="false">
      <Backdrop id={id} name={name} />
      <Drawing />
    </svg>
  );
}
