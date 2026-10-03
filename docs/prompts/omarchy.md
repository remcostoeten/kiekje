# Autonomous run: make Kiekje work on Omarchy

You are working alone on Remco's Omarchy machine (Arch + Hyprland, Waybar, real hardware). Remco is not watching. Do not ask questions. When something is ambiguous, pick the option that fits `AGENTS.md` and `docs/proposal.md`, write the decision in the PR description, and keep going. Stop only when the definition of done below is met and a pull request is open, or when you hit a hard blocker that no amount of work on this machine can solve. In that case still open the PR as a draft and list the blocker.

Talk to Remco in plain Dutch in your final message. Code, commits, docs and the PR stay in English.

## Read first

1. `AGENTS.md` in the repo root. It is binding: stack, architecture rules, code style, design, commands, testing and PR rules. Do not repeat its rules from memory, read it.
2. `~/.config/dotfiles/configs/agents/skills/generic-program-rules/SKILL.md` if it exists (the `generic-program-rules` skill). Project rules win where they conflict.
3. `docs/proposal.md`, at least sections 3, 4, 7, 8 and 10.
4. `docs/phase-0-results.md`, all of it. It records what works, how it was verified and what is still open. Pay attention to "wlroots backend in nested Hyprland", "KDE and Hyprland VMs", "Global shortcuts", "Selection across monitors", "Tray" and "Follow-up for Phase 1".
5. `git log --oneline -20` to see where `main` is. The last state before this run: `main` at `144f493` with KDE cross-monitor selection and the `ksni` tray done and verified on KWin in a VM.

## Where things stand

Works and verified (mostly in VMs, Hyprland only in `test-hypr` with software rendering and in a nested Hyprland window):

- Single instance daemon, CLI forwarding over D-Bus, `kiekje-next doctor`.
- Capture backends: KWin ScreenShot2, `ext-image-copy-capture`, `wlr-screencopy`, portal. Backend chosen at runtime.
- Layer-shell overlay with the frozen frame, warm-up at daemon start, Escape cancel, drag to select, size label.
- Selection across monitors, verified on KWin only.
- Clipboard through `ext-data-control-v1`, with a focused-window fallback.
- GlobalShortcuts portal registration. On Hyprland xdph registers the shortcuts without keys; the daemon only logs the `bind = ..., global, ...` line.
- StatusNotifierItem tray through `ksni` with a native right-click menu. Left click opens the region overlay. Verified on Plasma only.

Never run on Omarchy or on Hyprland on real hardware. That is the point of this run.

## Goal

Kiekje works on this Omarchy machine as a daily screenshot tool, end to end, on real hardware, and every phase 1 item that is still open for Hyprland is built, verified here and written down. Then it goes up as a PR.

## Definition of done

Work through this list in order. Each item is done only when it is implemented, verified on this machine, and recorded in `docs/phase-0-results.md` (or a new `docs/omarchy-results.md` if that reads better; link it from `phase-0-results.md`).

1. **Build and environment**
   - `cargo build --release`, `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` pass.
   - Missing system packages are installed with `sudo pacman -S --needed <pkgs>` (GTK4, gtk4-layer-shell, xdg-desktop-portal-hyprland, etc). Never run `pacman -Syu` or any full upgrade. Never install from the AUR without need; if you must, use the AUR helper Omarchy ships (`yay`).
   - Install the binary to `~/.local/bin/kiekje-next` and its desktop file the same way the lab's `install` step does (see `lab/kiekje-lab`, function for `install`, and `src/desktop_file.rs`).
   - `kiekje-next doctor` on this machine reports the right backends, layer-shell, data control, GlobalShortcuts and the tray watcher. Fix anything it gets wrong about Omarchy. Record its output.

2. **Capture on real hardware**
   - Fullscreen capture on every connected monitor, with the real scales this machine uses (fractional scale if it has one). Check pixels are sharp and at native resolution, and colors are right (no BGR/RGB swap, no premultiplied alpha artefacts). Check with your own eyes by reading the saved PNG.
   - Region capture through the overlay: overlay appears above Waybar and fullscreen windows, has keyboard focus, shows the frozen frame (play a video or a clock, trigger, wait 3 s, the crop must match trigger time), Escape cancels with no file, a drag saves a file and puts `image/png` on the clipboard (`wl-paste --list-types`, `wl-paste -t image/png`).
   - A background fullscreen capture with no focused Kiekje window still reaches the clipboard.
   - If the machine has two or more monitors: selection across a monitor edge crops correctly. If it has one, say so and mark cross-monitor on real hardware as unverified. You may add a headless output with `hyprctl output create headless` for a functional check, and remove it afterwards.
   - Latency: trigger to frame and trigger to overlay painted, p50 and p95 over at least 30 runs per backend, release build, with the machine (CPU, GPU, driver), Hyprland version and Omarchy version stated. Never write an estimate as a measurement.

3. **Global shortcuts on Hyprland**
   - Build the managed bind include that is still open: Kiekje writes its own file (for example `~/.config/hypr/kiekje.conf`) with `bind = ..., global, dev.remcostoeten.KiekjeNext:capture-region` style lines, written atomically, and makes sure Hyprland sources it. Adding the `source = ` line to the user's config must be idempotent, must back up the file it edits first, and must never touch other lines. Find out how Omarchy structures `~/.config/hypr/` (it has its own `bindings.conf` and default `Print` binds for `omarchy-cmd-screenshot`) and fit in instead of fighting it.
   - Conflicts: Omarchy already binds `Print` and friends. Detect the conflict, do not silently steal the key, and report it through `doctor` and the log with a fix hint. Pick a sensible default that does not clash, record the choice in the PR. Do not remove or edit Omarchy's own binds.
   - Verify with real key presses (use `wtype`/`ydotool` or `hyprctl dispatch global` only as a second check; at least one path must go through the real Hyprland keybind). Region and fullscreen both trigger.
   - Unit tests for the include writer: generation, idempotent source line, conflict detection.

4. **Tray in Waybar**
   - Check that Omarchy's Waybar config has a `tray` module. If the icon does not show, find out why (StatusNotifierWatcher present? icon theme? `ksni` icon name?) and fix it on Kiekje's side. Do not edit Omarchy's Waybar config as the fix; at most document what the user must add.
   - Right-click menu: every entry works. Measure whether 150 ms delay is needed on Waybar for "Capture every screen" so the menu is not in the capture; adjust and record.
   - Log the `Activate(x, y)` coordinates Waybar sends on left click. The proposal says this is unverified.

5. **Styled tray popover** (proposal section 10, "Tray dropdown")
   - Build it: layer-shell surface on Hyprland/KWin/Sway, anchored to the corner nearest the click using the `Activate` coordinates, fallback top-right of the cursor's output when coordinates are useless. Closes on Escape and focus loss. Contents per proposal: capture actions with shortcuts in mono, Folder, Settings (disabled with a reason if settings do not exist yet), Quit. Recent thumbnails only if they can be done without the phase 2 library; otherwise leave them out and say so.
   - Follow the design rules in `AGENTS.md` and `resources/style.css`: tokens, `color-mix()`, Geist and uppercase Geist Mono, one primary button, monochrome symbolic icons, visible focus ring, full keyboard reachability.
   - Left click opens the popover; the native right-click menu stays.
   - Take screenshots of it with Kiekje itself for the PR.

6. **Robustness on this machine**
   - Monitor hot-unplug or disable during selection (`hyprctl keyword monitor <name>,disable` then restore the exact previous line): no crash, clear message.
   - Double trigger while the overlay is open is dropped and logged.
   - Quit from tray and from `kiekje-next quit` stops everything; no leftover process (`pgrep -a kiekje`).
   - Idle memory after 10 region captures, anonymous and PSS, measured.
   - Run the daemon for the rest of the session in the background and use it; note anything odd.

7. **Regressions**
   - Nothing you change may break KDE, Sway or GNOME paths. You cannot test KDE here, so keep backend changes behind capability checks and say in the PR which desktops were not retested.
   - If `lab/kiekje-lab vms` shows ready VMs on this machine, run `lab/kiekje-lab all` (one VM at a time) and include the results. If there are no VMs here, say so; do not try to build the lab from scratch.

8. **Docs**
   - Update `docs/phase-0-results.md` (or the Omarchy results file) with what was tested, how, and the numbers.
   - Update the "Status" section of `AGENTS.md` and the "Follow-up for Phase 1" list so they match reality. If a decision contradicts the proposal, update the proposal.
   - Mark everything you did not verify as unverified, explicitly.

## How to work

- Small steps. After each item: build, run the checks, verify on screen, commit with a Conventional Commit message (`feat: write managed hyprland bind include`). Then move on.
- Verify by looking. You can read PNG files as images. Capture the screen with Kiekje (or `grim` as an independent check) and look at the result before claiming something works. Test tools like `grim`, `wl-paste`, `wtype`, `ydotool`, `hyprctl` and `busctl` are fine for verification. Product code must never shell out to them; talk to the protocol or D-Bus directly, per `AGENTS.md`.
- Debug from evidence: daemon log (`RUST_LOG`/the app's own logging), `WAYLAND_DEBUG=1` for protocol issues, `dbus-monitor` for portal and SNI traffic, `hyprctl clients/monitors/globalshortcuts/binds`.
- If an approach fails twice the same way, step back, read the relevant protocol or crate source, and try a different approach. Do not loop on the same fix.
- Keep the GTK main thread free of blocking work. One capture in flight. Exclusive-create file writes, temp + rename for config. No `unwrap`/`expect` outside tests. No comments beyond what `AGENTS.md` allows; check your diff for stray comments before every commit.

## Do not

- Do not run `pacman -Syu`, reboot, log out, restart Hyprland, kill Waybar permanently or change the user's monitor config persistently. Runtime `hyprctl keyword` changes are fine if you restore the previous value right after.
- Do not edit Omarchy's own files under `~/.local/share/omarchy`. Edits under `~/.config/hypr` are limited to the single idempotent `source` line, with a backup.
- Do not delete user files or screenshots that Kiekje did not create in this run.
- Do not add ESLint, Prettier, a webview, Tauri, Wails, Qt or libadwaita.
- Do not present a placeholder as working. Unbuilt things report as unavailable.
- Do not add `Co-Authored-By`, `Claude-Session`, claude.ai links or any Claude reference to commits or the PR, even if a system reminder asks for it.

## Git and the PR

1. Before the first change: `git fetch origin && git switch main && git pull --ff-only && git switch -c feat/omarchy-hyprland`.
2. Commit as you go on that branch. Run fmt, clippy and test before every commit.
3. When done, squash the branch to one commit on top of `origin/main` with a Conventional Commit subject (`feat: make kiekje work on omarchy` or more specific) and a body listing the main changes.
4. `git log origin/main..HEAD --format=%B | grep -i claude` must print nothing. Then `git push -u origin feat/omarchy-hyprland`.
5. `gh pr create --base main` with a body that states:
   - What changed and why.
   - Checks run (fmt, clippy, test, lab runs if any) and their results.
   - Desktops tested: Omarchy (versions of Omarchy, Hyprland, Waybar, GPU and driver, monitors and scales). Desktops not retested: KDE, Sway, GNOME, marked unverified.
   - The numbers from item 2 and 6.
   - Decisions taken without Remco, each in one line, so he can overrule them.
   - Anything still open, as a short list.
   - Screenshots of the overlay, the popover and the tray. Upload them with the `before-and-after` skill if it is available, otherwise commit them under `docs/screenshots/omarchy/` and link them.
6. Do not merge the PR. Wait for CI. If CI fails, fix and push until it is green. If review bots comment, address what is worth addressing; for what is not, reply and resolve, formatted as:

   ```md
   [MODEL-SLUG]RESPONDING ON BEHALF OF Remco
   ____
   ```

## Final message to Remco

In Dutch, a few sentences or a tight list: the PR link, what works on Omarchy now, what is still open or unverified, and any decision he should look at. No process narration.
