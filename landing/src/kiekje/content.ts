export const REPO = "https://github.com/remcostoeten/kiekje";
export const DOCS = "/docs";

export const LINKS = {
  repo: REPO,
  releases: `${REPO}/releases`,
  issues: `${REPO}/issues`,
  license: `${REPO}/blob/master/LICENSE`,
  docs: `${DOCS}/`,
  install: `${DOCS}/guide/install`,
  desktops: `${DOCS}/guide/desktops`,
  annotate: `${DOCS}/guide/annotate`,
  capture: `${DOCS}/guide/capture`,
  privacy: `${DOCS}/guide/privacy`,
  keys: `${DOCS}/reference/keys`,
  start: `${DOCS}/guide/getting-started`,
  cli: `${DOCS}/reference/cli`,
  notes: `${DOCS}/releases/`,
};

export type Release = {
  version: string;
  url: string;
};

export const FALLBACK_RELEASE: Release = {
  version: KIEKJE_APP_VERSION,
  url: `${REPO}/releases/tag/v${KIEKJE_APP_VERSION}`,
};

export type Showcase = {
  id: string;
  title: string;
  body: string;
  points: string[];
  image: string;
  alt: string;
  width: number;
  height: number;
  link: { label: string; href: string };
};

export const SHOWCASES: Showcase[] = [
  {
    id: "annotate",
    title: "Annotate before you keep it",
    body: "Draw on the region while the overlay is still up, or open any capture from the gallery in the editor. Shapes stay editable until you copy or save.",
    points: [
      "Box, ellipse, arrow, line, pen, marker, numbered steps, text, pixelate and crop",
      "Check, cross and emoji stamps that turn freely or in 15 degree steps",
      "Select several shapes and move, turn, restyle or delete them as one undo step",
      "Seven colours, size and opacity on the keyboard, and every tool key can be rebound",
    ],
    image: "/shots/editor.webp",
    alt: "The Kiekje editor with a box, an arrow, two numbered steps, a text label and a pixelated line on a code capture",
    width: 1100,
    height: 760,
    link: { label: "Annotation guide", href: LINKS.annotate },
  },
  {
    id: "gallery",
    title: "Every capture in one gallery",
    body: "Kiekje keeps an index of what you captured, with thumbnails, so you can find a screenshot from last week without opening a file manager.",
    points: [
      "Search by name, filter by kind, period, dimensions, file size, annotated and unsaved",
      "Grid and list views, copy, pin, rename, show in folder or move to trash",
      "Files you moved or deleted are marked missing, with Locate and Remove",
    ],
    image: "/shots/gallery.webp",
    alt: "The Kiekje gallery in grid view with search, kind filters and eight captures",
    width: 980,
    height: 680,
    link: { label: "Gallery guide", href: `${DOCS}/guide/gallery` },
  },
  {
    id: "settings",
    title: "Settings that show what they change",
    body: "The Capture page draws the overlay and lights up the part a setting controls. Everything is also plain TOML in ~/.config/kiekje/config.toml.",
    points: [
      "Edit first, or copy and save the moment you let go of the mouse",
      "Record shortcuts in place, with conflicts flagged before you save",
      "Doctor page that lists what your session supports and how to fix the rest",
    ],
    image: "/shots/settings.webp",
    alt: "The Kiekje settings window on the Capture page, with a preview of the overlay",
    width: 958,
    height: 698,
    link: { label: "Configuration reference", href: `${DOCS}/reference/configuration` },
  },
];

export type StepId = "trigger" | "freeze" | "select" | "keep";

export type Step = {
  id: StepId;
  title: string;
  body: string;
};

export const STEPS: Step[] = [
  {
    id: "trigger",
    title: "Trigger",
    body: "A shortcut, the tray, the command line or a script sends one action to the running instance. A second press while a capture runs is ignored.",
  },
  {
    id: "freeze",
    title: "Freeze",
    body: "Every monitor is grabbed in one call and kept as a raw frame. Menus, tooltips and video stay exactly as they were.",
  },
  {
    id: "select",
    title: "Select",
    body: "Drag a region on the frozen frame, take a monitor or a window with one key, or draw on it first.",
  },
  {
    id: "keep",
    title: "Keep",
    body: "The crop goes to the clipboard, is encoded once, saved without overwriting anything and listed in the gallery.",
  },
];

export type FeatureIcon =
  | "window"
  | "monitors"
  | "text"
  | "record"
  | "loupe"
  | "timer"
  | "stamp"
  | "pin"
  | "file"
  | "clipboard"
  | "upload"
  | "theme";

export type Feature = {
  icon: FeatureIcon;
  title: string;
  body: string;
};

export const FEATURES: Feature[] = [
  {
    icon: "window",
    title: "Window capture",
    body: "Hover a window and click or press Enter. The selection snaps to windows on Plasma 6 and Hyprland.",
  },
  {
    icon: "monitors",
    title: "Across monitors",
    body: "Drag a region over two screens with different scales, or a rotated one. Each part is cut from its own frame at full resolution and stitched.",
  },
  {
    icon: "text",
    title: "Extract text",
    body: "Drag over text and Kiekje copies it as plain text. Tesseract runs on your machine, in the languages you pick.",
  },
  {
    icon: "record",
    title: "Recording and GIF",
    body: "Record a region or a screen to WebM through the ScreenCast portal. Record GIF, or Export GIF later, writes a looping GIF next to it.",
  },
  {
    icon: "loupe",
    title: "Loupe and colours",
    body: "A pixel loupe follows the pointer with the colour under it. C copies the hex and keeps it under Colours.",
  },
  {
    icon: "timer",
    title: "Delay and repeat",
    body: "Wait up to 60 seconds with a countdown you can cancel, or take the last region again with Shift+R.",
  },
  {
    icon: "stamp",
    title: "Stamps and emoji",
    body: "Place a check, a cross or any emoji and turn it freely. The picker searches in English and Dutch and knows :) and <3.",
  },
  {
    icon: "pin",
    title: "Pin on screen",
    body: "Keep a capture floating above your windows. Page Up and Page Down change its opacity, arrows move it on Plasma and Hyprland.",
  },
  {
    icon: "file",
    title: "Names and formats",
    body: "Name files with tokens like {date}, {mode} and {width}, with a live preview. Save PNG or JPEG; the clipboard always gets PNG.",
  },
  {
    icon: "clipboard",
    title: "Clipboard only",
    body: "Turn saving off and captures stay in the cache with an Unsaved badge and a Keep action, pruned after 7 days.",
  },
  {
    icon: "upload",
    title: "Temporary links",
    body: "Opt in and the gallery uploads one capture to Litterbox for 1 to 72 hours and copies the link.",
  },
  {
    icon: "theme",
    title: "Dark and light",
    body: "One setting switches every window between the dark and the light theme, also from a script over D-Bus.",
  },
];

export type FeatureGroup = {
  title: string;
  items: string[];
};

export const MORE_FEATURES: FeatureGroup[] = [
  {
    title: "Capture",
    items: [
      "Full screen with Shift+Print, every monitor in one image",
      "Hold Shift to slow the pointer to an eighth of its speed",
      "Choose screenshot, video, GIF or text before you select",
      "Draw on the frame first and pick the region afterwards",
      "Unplugging a monitor mid-capture cancels it without a stray file",
    ],
  },
  {
    title: "Editor",
    items: [
      "Move or widen the crop after drawing, also with the arrow keys",
      "Copy the text in the region while drawing with Ctrl+T",
      "Right-click the canvas for every tool, size, undo and delete",
      "Edits from the gallery are saved as a new file and the original stays",
      "Page Up and Page Down step through shapes and describe each one",
    ],
  },
  {
    title: "Gallery",
    items: [
      "Recordings and GIFs with a poster frame and their own filter",
      "Files moved within the capture folders are relinked by size and hash",
      "Rename with F2, copy the image or its path, move to trash",
      "Sort by date, name, size or dimensions",
      "Clear the gallery, colours or thumbnails behind a two-click guard",
    ],
  },
  {
    title: "Desktop",
    items: [
      "A tray popover with a delay for each capture mode",
      "Notifications with the image, Open and Show in folder, and an optional shutter sound",
      "Press / in Settings to search every setting",
      "Tells you whether your clipboard manager keeps copies after Quit",
      "One running instance that the CLI and later launches talk to over D-Bus",
    ],
  },
];

export type Key = {
  keys: string[];
  action: string;
};

export const OVERLAY_KEYS: Key[] = [
  { keys: ["Print"], action: "Freeze the screen and select a region" },
  { keys: ["Shift", "Print"], action: "Capture every screen" },
  { keys: ["Enter"], action: "Capture the selection" },
  { keys: ["F"], action: "Take the monitor under the pointer" },
  { keys: ["W"], action: "Cycle windows, Enter takes one" },
  { keys: ["Shift", "R"], action: "Take the last region" },
  { keys: ["Arrows", "Space"], action: "Draw a region without a mouse" },
  { keys: ["Shift"], action: "Hold to slow the pointer for single pixels" },
  { keys: ["D"], action: "Draw first, pick the region afterwards with S" },
  { keys: ["G"], action: "Cycle screenshot, video, GIF and text" },
  { keys: ["Z"], action: "Show or hide the loupe" },
  { keys: ["C"], action: "Copy the colour under the pointer" },
  { keys: ["Esc"], action: "Cancel, nothing is saved" },
];

export const EDITOR_KEYS: Key[] = [
  { keys: ["V"], action: "Select, move and turn shapes" },
  { keys: ["R"], action: "Box" },
  { keys: ["E"], action: "Ellipse" },
  { keys: ["A"], action: "Arrow" },
  { keys: ["L"], action: "Line" },
  { keys: ["P"], action: "Pen" },
  { keys: ["M"], action: "Marker" },
  { keys: ["N"], action: "Numbered step" },
  { keys: ["T"], action: "Text" },
  { keys: ["Y"], action: "Check stamp" },
  { keys: ["U"], action: "Cross stamp" },
  { keys: ["I"], action: "Emoji stamp and picker" },
  { keys: ["X"], action: "Pixelate" },
  { keys: ["C"], action: "Crop, in the editor window" },
  { keys: ["1–7"], action: "Pick a colour" },
  { keys: ["Ctrl", "Z"], action: "Undo, Ctrl+Shift+Z redoes" },
];

export type Support = "lab" | "partial" | "unverified";

export type Desktop = {
  name: string;
  capture: string;
  shortcuts: string;
  status: string;
  support: Support;
};

export const DESKTOPS: Desktop[] = [
  {
    name: "KDE Plasma 6, Wayland",
    capture: "KWin ScreenShot2 over D-Bus",
    shortcuts: "GlobalShortcuts portal",
    status: "Lab tested on Plasma 6.7.5 in a VM",
    support: "lab",
  },
  {
    name: "Hyprland, Omarchy",
    capture: "ext-image-copy-capture, wlr-screencopy",
    shortcuts: "Managed hyprland.conf include",
    status: "Lab tested on Hyprland 0.56.2 in a VM, Omarchy unverified",
    support: "lab",
  },
  {
    name: "Sway and wlroots",
    capture: "ext-image-copy-capture, wlr-screencopy",
    shortcuts: "Bind kiekje capture in your config",
    status: "Lab tested on sway 1.9 and 1.12, no window capture",
    support: "partial",
  },
  {
    name: "GNOME, Wayland",
    capture: "Screenshot portal",
    shortcuts: "GlobalShortcuts portal, GNOME 48+",
    status: "Lab tested on GNOME 50.5, no window capture",
    support: "partial",
  },
  {
    name: "KDE Plasma, X11",
    capture: "KWin ScreenShot2",
    shortcuts: "kglobalaccel",
    status: "Unverified",
    support: "unverified",
  },
];

export const DOCTOR_OUTPUT = `$ kiekje doctor
Session
  type              wayland
  desktop           KDE
  overlay           layer-shell
  window capture    kwin-scripting
  clipboard         Klipper
  recording         ScreenCast portal, WebM
  text capture      Tesseract 5.5.3, eng nld
  kwin access       desktop file installed

Portals
  Screenshot        v2
  ScreenCast        v5
  GlobalShortcuts   v2`;

export type Command = {
  command: string;
  description: string;
};

export const COMMANDS: Command[] = [
  { command: "kiekje capture", description: "Freeze the screen and select a region" },
  { command: "kiekje capture --edit", description: "Select a region and annotate it" },
  { command: "kiekje capture text", description: "Select a region and copy its text" },
  { command: "kiekje capture repeat", description: "Take the last region again, no overlay" },
  { command: "kiekje capture --delay 5", description: "Wait five seconds first" },
  { command: "kiekje capture --no-save", description: "Copy to the clipboard, write no file" },
  { command: "kiekje record", description: "Start recording, run again to stop and save" },
  { command: "kiekje pin shot.png", description: "Keep an image on screen" },
  { command: "kiekje windows", description: "List every window with its id, geometry and title" },
  {
    command: "kiekje capture window --id ID --output shot.png",
    description: "Write one window to a file, even when covered, on Plasma",
  },
  {
    command: "kiekje capture fullscreen --output shot.png --crop 0,0,800,600",
    description: "Write part of the screen to a file, nothing on screen",
  },
];

export type FooterColumn = {
  title: string;
  links: { label: string; href: string }[];
};

export const FOOTER: FooterColumn[] = [
  {
    title: "This page",
    links: [
      { label: "How it works", href: "#how" },
      { label: "Features", href: "#features" },
      { label: "Keys", href: "#keyboard" },
      { label: "Desktops", href: "#desktops" },
      { label: "Privacy", href: "#privacy" },
      { label: "Install", href: "#install" },
    ],
  },
  {
    title: "Guides",
    links: [
      { label: "Getting started", href: LINKS.start },
      { label: "Capture", href: LINKS.capture },
      { label: "Annotate", href: LINKS.annotate },
      { label: "Gallery", href: `${DOCS}/guide/gallery` },
      { label: "Record and share", href: `${DOCS}/guide/record-and-share` },
      { label: "Troubleshooting", href: `${DOCS}/guide/troubleshooting` },
    ],
  },
  {
    title: "Reference",
    links: [
      { label: "Command line", href: LINKS.cli },
      { label: "Keys", href: LINKS.keys },
      { label: "Configuration", href: `${DOCS}/reference/configuration` },
      { label: "Support matrix", href: `${DOCS}/reference/support` },
    ],
  },
  {
    title: "Project",
    links: [
      { label: "Releases", href: LINKS.releases },
      { label: "Release notes", href: LINKS.notes },
      { label: "Report an issue", href: LINKS.issues },
      { label: "Source on GitHub", href: LINKS.repo },
      { label: "MIT license", href: LINKS.license },
    ],
  },
];

export type PrivacyArt = "telemetry" | "update" | "upload" | "text";

export type Privacy = {
  title: string;
  body: string;
  key: string;
  art: PrivacyArt;
};

export const PRIVACY: Privacy[] = [
  {
    title: "No telemetry",
    art: "telemetry",
    body: "No analytics and no crash reporting. Kiekje makes no network requests unless you turn one of the two features below on.",
    key: "no network requests",
  },
  {
    title: "Update check is off",
    art: "update",
    body: "When on, it asks api.github.com for the release list at most once a day, with nothing about you or your captures.",
    key: "at most once a day",
  },
  {
    title: "Upload is off",
    art: "upload",
    body: "When on, it sends only the file you pick to litterbox.catbox.moe, renamed so the file name stays on your machine.",
    key: "stays on your machine",
  },
  {
    title: "Text stays local",
    art: "text",
    body: "Text extraction runs Tesseract inside Kiekje. The region and the text it reads never leave your computer.",
    key: "runs Tesseract inside Kiekje",
  },
];

export type InstallMark = "arch" | "terminal" | "debian" | "fedora" | "box" | "nix" | "code";

export type InstallMethod = {
  id: string;
  label: string;
  mark: InstallMark;
  commands: string;
  note: string;
  file?: string;
};

export const INSTALL_METHODS: InstallMethod[] = [
  {
    id: "arch",
    label: "Arch",
    mark: "arch",
    commands: "yay -S kiekje-bin",
    note: "Installs the release binary from the AUR. yay -S kiekje builds the same version from source.",
  },
  {
    id: "script",
    label: "Any distro",
    mark: "terminal",
    commands:
      "curl -fsSL https://raw.githubusercontent.com/remcostoeten/kiekje/master/get.sh | bash",
    note: "Picks the .deb, .rpm or tarball for your system from the newest release, checks it against SHA256SUMS and installs it with sudo.",
  },
  {
    id: "debian",
    label: "Debian, Ubuntu",
    mark: "debian",
    commands: "sudo apt install ./kiekje_{PACKAGE}-1_amd64.deb",
    note: "Download the .deb first. The release also has an arm64 build.",
    file: "kiekje_{PACKAGE}-1_amd64.deb",
  },
  {
    id: "fedora",
    label: "Fedora",
    mark: "fedora",
    commands: "sudo dnf install ./kiekje-{PACKAGE}-1.x86_64.rpm",
    note: "Download the .rpm first. On openSUSE use sudo zypper install --allow-unsigned-rpm with the same file.",
    file: "kiekje-{PACKAGE}-1.x86_64.rpm",
  },
  {
    id: "flatpak",
    label: "Flatpak",
    mark: "box",
    commands: "flatpak install --user ./kiekje.flatpak",
    note: "Download kiekje.flatpak first. The sandbox captures through the portals, which is slower than the native package on Plasma and Hyprland.",
    file: "kiekje.flatpak",
  },
  {
    id: "nix",
    label: "Nix",
    mark: "nix",
    commands: "nix profile install github:remcostoeten/kiekje",
    note: "Needs the nix-command and flakes experimental features.",
  },
  {
    id: "source",
    label: "Source",
    mark: "code",
    commands:
      "git clone https://github.com/remcostoeten/kiekje.git\ncd kiekje\ncargo install --path . --locked\nkiekje install-desktop-file",
    note: "Needs Rust 1.85 or newer, GTK 4.14 and gtk4-layer-shell 1.1.",
  },
];

export const DOCTOR_SAMPLE = {
  where: "KDE on Wayland, trimmed",
  session: [
    ["desktop", "KDE"],
    ["overlay", "layer-shell"],
    ["daemon", "running"],
  ],
  backends: [
    { name: "kwin-screenshot2", role: "used first", first: true },
    { name: "xdg-desktop-portal", role: "fallback", first: false },
  ],
  fallback: "If the first one fails, the next one down takes over.",
  result: "No problems found.",
} as const;
