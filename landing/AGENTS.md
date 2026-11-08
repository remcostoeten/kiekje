<!-- LOVABLE:BEGIN -->

> [!IMPORTANT]
> This project is connected to [Lovable](https://lovable.dev). Avoid rewriting
> published git history — force pushing, or rebasing/amending/squashing commits
> that are already pushed — as it rewrites history on Lovable's side and the
> user will likely lose their project history.
>
> Commits you push to the connected branch sync back to Lovable and show up in
> the editor, so keep the branch in a working state.

<!-- LOVABLE:END -->

## Decisions

- The Kiekje showcase stage keeps a fixed 1200 px logical height and derives its width from the aspect of its own box, so the full desktop is visible instead of cover-cropped. It fills the hero on wide screens and sits below the hero text in a 5:4 frame on phones.
- The page follows the app's design: the near-black palette, one accent (`#fe5101`), Geist in sentence case, state feedback as a background tint. The hero wordmark is Geist Pixel in an overlay-style selection frame, with one quiet 15px line of copy under it in the faint text colour. No uppercase labels.
- The version on the page comes from the GitHub releases API, pre-releases included. `FALLBACK_RELEASE` in `src/kiekje/content.ts` renders first and is filled at build time from `../Cargo.toml` (falling back to the newest `v*` git tag) by `scripts/app-version.ts`, so it never needs a manual bump.
- The install section is a centered closing poster (icon, "Your desktop next." in Geist Pixel, distro marks as the method picker, the command chip with Copy) followed by a trimmed `kiekje doctor` card. The doctor rows in `DOCTOR_SAMPLE` are verbatim from a real run on Plasma 6; never invent rows for other desktops.
- Screenshots in `public/shots` are real app windows taken with the app repo's `lab/ui-shot`. Retake them when the UI changes; never draw fake UI for them.
- Feature copy follows the docs guide and reference pages in the app repo. Standout features get a card in `FEATURES`, the rest a line in `MORE_FEATURES`; add new app features there when they ship.
- The "Capture first, select second" section is a step list beside one stage (`step-stage.tsx`, `src/styles/steps.css`): a 720×450 miniature desktop, scaled to fit, that acts out each step with rebuilt Kiekje UI (tray popover, overlay mode and hint bars, window snap, region with loupe, inline editor bars, notification, gallery). Copy its labels and sizes from the app and `showcase.ts`, not from imagination. The active step's `--kk-play` drives its keyframes and its progress bar, and the bar's end starts the next step. Each step's keyframes begin where the previous step ends; hand-off exits only run on `data-flow="next"`, hover jumps snap. The trigger rotates tray, Print key and CLI per loop. Reduced motion shows the finished select state. Hover pins a step and the stage pauses off screen.
- The docs live at `/docs` on the same domain. `bun run build` builds `../docs` with `DOCS_BASE=/docs/` into `public/docs`; links in `content.ts` are relative to it. Do not add the docs as TanStack routes.
- No comparison tables, benchmarks or "better than" claims. State what Kiekje does and where it is tested.
- Lint with Oxlint and format with oxfmt. No ESLint or Prettier.
