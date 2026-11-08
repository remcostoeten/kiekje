import type { FeatureIcon } from "@/kiekje/content";

type Glyph = FeatureIcon | "copy" | "check" | "arrow" | "external" | "download" | "terminal";

type Props = {
  name: Glyph | "github";
  size?: number;
};

const PATHS: Record<Glyph, string> = {
  window:
    "M3 5.5A1.5 1.5 0 0 1 4.5 4h11A1.5 1.5 0 0 1 17 5.5v9a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 3 14.5zM3 8h14M5.5 6h.01M7.5 6h.01",
  text: "M4 5h12M10 5v11M7.5 16h5M3 3v4M17 3v4",
  monitors: "M2 6h7.5v6H2zM10.5 4H18v8h-7.5zM5.75 12v3M4 15h3.5M14.25 12v3M12.5 15h3.5",
  stamp: "M10 17a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM7.5 8.5h.01M12.5 8.5h.01M7 11.75a3.5 3.5 0 0 0 6 0",
  file: "M11.5 3H6a1.5 1.5 0 0 0-1.5 1.5v11A1.5 1.5 0 0 0 6 17h8a1.5 1.5 0 0 0 1.5-1.5V7zM11.5 3v4h4M7.5 11h5M7.5 14h3",
  theme: "M10 17a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM10 3v14M10 6h3.5M10 9h4.75M10 12h4.5M10 15h2.5",
  record:
    "M10 16a6 6 0 1 0 0-12 6 6 0 0 0 0 12zM10 12.25a2.25 2.25 0 1 0 0-4.5 2.25 2.25 0 0 0 0 4.5z",
  pin: "M7 3h6M8 3v5l-3 3h10l-3-3V3M10 11v6",
  loupe: "M9 15A6 6 0 1 0 9 3a6 6 0 0 0 0 12zM13.5 13.5 17 17M9 6.5v5M6.5 9h5",
  timer: "M10 17a6 6 0 1 0 0-12 6 6 0 0 0 0 12zM10 8v3l2 1.5M8 2.5h4",
  clipboard:
    "M7 4H5.5A1.5 1.5 0 0 0 4 5.5v10A1.5 1.5 0 0 0 5.5 17h9a1.5 1.5 0 0 0 1.5-1.5v-10A1.5 1.5 0 0 0 14.5 4H13M7.5 3h5v2.5h-5zM7 10h6M7 13h4",
  upload: "M10 13V3.5M6.5 7 10 3.5 13.5 7M4 12.5v2A2.5 2.5 0 0 0 6.5 17h7a2.5 2.5 0 0 0 2.5-2.5v-2",
  copy: "M7 7V5.5A1.5 1.5 0 0 1 8.5 4h6A1.5 1.5 0 0 1 16 5.5v6a1.5 1.5 0 0 1-1.5 1.5H13M5.5 7h6A1.5 1.5 0 0 1 13 8.5v6a1.5 1.5 0 0 1-1.5 1.5h-6A1.5 1.5 0 0 1 4 14.5v-6A1.5 1.5 0 0 1 5.5 7z",
  download:
    "M10 3.5V13M6.5 9.5 10 13l3.5-3.5M4 12.5v2A2.5 2.5 0 0 0 6.5 17h7a2.5 2.5 0 0 0 2.5-2.5v-2",
  check: "M4.5 10.5 8 14l7.5-8",
  terminal: "M3.5 4.5h13v11h-13zM6.5 8l2.5 2-2.5 2M10.5 12.5h3",
  arrow: "M4 10h12m0 0-4.5-4.5M16 10l-4.5 4.5",
  external:
    "M8 4H5.5A1.5 1.5 0 0 0 4 5.5v9A1.5 1.5 0 0 0 5.5 16h9a1.5 1.5 0 0 0 1.5-1.5V12M11 4h5v5M16 4l-7 7",
};

const GITHUB =
  "M10 1.5a8.5 8.5 0 0 0-2.69 16.57c.43.08.58-.18.58-.41v-1.45c-2.36.51-2.86-1.14-2.86-1.14-.39-.98-.95-1.24-.95-1.24-.77-.53.06-.52.06-.52.85.06 1.3.88 1.3.88.76 1.3 1.99.92 2.47.71.08-.55.3-.92.54-1.14-1.89-.21-3.87-.94-3.87-4.2 0-.93.33-1.69.88-2.28-.09-.22-.38-1.08.08-2.25 0 0 .72-.23 2.34.87a8.1 8.1 0 0 1 4.25 0c1.62-1.1 2.33-.87 2.33-.87.47 1.17.17 2.03.09 2.25.55.59.88 1.35.88 2.28 0 3.27-1.99 3.99-3.88 4.2.3.26.58.78.58 1.57v2.33c0 .23.15.5.59.41A8.5 8.5 0 0 0 10 1.5z";

export function Icon({ name, size = 20 }: Props) {
  if (name === "github") {
    return (
      <svg width={size} height={size} viewBox="0 0 20 20" aria-hidden="true">
        <path d={GITHUB} fill="currentColor" />
      </svg>
    );
  }
  return (
    <svg width={size} height={size} viewBox="0 0 20 20" aria-hidden="true">
      <path
        d={PATHS[name]}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
