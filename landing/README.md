# Kiekje landing

The one-page site for [Kiekje](https://github.com/remcostoeten/kiekje), a screenshot tray app for Linux. It opens with a canvas replay of a capture on a Plasma desktop, then shows the editor, gallery and settings with real screenshots, the keyboard map, desktop support, the CLI, privacy and install steps.

Built with TanStack Start, React and plain CSS on top of Tailwind's reset. Connected to [Lovable](https://lovable.dev).

## Development

```bash
bun install
bun run dev
bun run lint
bun run format
bun run test
bun run build
```

- The docs (`../docs`, VitePress) are built into `public/docs` by `scripts/build-docs.ts` before every `build` and served at `/docs`. The output is gitignored. `DOCS_BASE` and `DOCS_OUT_DIR` set the base path and output folder; without them the docs build for GitHub Pages at `/kiekje/`.
- Page copy, links, install commands and the fallback release live in `src/kiekje/content.ts`.
- The hero animation is `src/kiekje/showcase.ts`, one canvas drawn from a single timeline. `?dev` adds a frame-time overlay on F.
- Screenshots in `public/shots` come from the app's `lab/ui-shot` harness on a private Xvfb display.
