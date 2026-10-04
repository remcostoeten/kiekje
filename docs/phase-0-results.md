# Phase 0 results

Machine: Arch Linux, Plasma 6.7.5, KWin 6.7.5 on Wayland, GTK 4.22.5, one active monitor DP-2 (3840x2160). The display scale changed from 1.0 to 1.5 between runs, so both were exercised. Release build. All numbers are from `kiekje-next` log output on 2026-10-02 and are single-machine samples, not benchmarks.

## Measured

| Path | Samples | Result |
|---|---|---|
| CLI trigger to running instance (D-Bus action) | 12 | 1.2 to 2.3 ms |
| KWin ScreenShot2, one 4K frame, raw pixels over a pipe | 10 | 36 to 57 ms |
| Portal Screenshot through the same pipeline, including PNG decode | 5 | 591 to 1046 ms |
| Trigger to overlay painted, first overlay after daemon start | 1 | 290 ms |
| Trigger to overlay painted, later overlays | 3 | 76 to 99 ms |
| Idle daemon, fresh | 30 s | 41 MB PSS (22 MB anonymous), 0 CPU ticks |
| Idle daemon after 3 fullscreen captures | 1 | 60 MB PSS (41 MB anonymous) |
| Idle daemon after 5 region overlays | 1 | 271 MB RSS, PSS not measured |

## Memory after the allocator fix

Measured later on 2026-10-02 with two active monitors (DP-2 3840x2160 at 1.25, HDMI-A-1 2560x1440 at 1.0), so a fullscreen capture stitches a 3850x3960 image. Region captures were driven with `ydotool` drags.

| Path | Before | After |
|---|---|---|
| Idle daemon, fresh | 22 MB anonymous | 22 MB anonymous, 67 MB PSS |
| After 3 fullscreen captures | 97 MB anonymous | 27 MB anonymous |
| After 8 fullscreen captures | 133 MB anonymous | 27 MB anonymous |
| After 10 region overlays, settled | not measured | 28 MB anonymous, 96 MB PSS |

- Cause: glibc raises its mmap threshold after a large buffer is freed, so later frame, stitch and PNG buffers came from the heap and stayed resident. The daemon now pins `M_MMAP_THRESHOLD` to 128 KiB at startup, which keeps large buffers on `mmap` and returns them on free.
- While an overlay is open, RSS rises by about 47 MB and drops back a few seconds after it closes. GTK releases those textures by itself.
- The first overlay loads the NVIDIA GL driver, which stays mapped: about 45 MB PSS, mostly `libnvidia-gpucomp` and `/dev/nvidiactl`. This is a one-time cost and does not grow with use.
- The remaining 5 MB of anonymous growth is the last PNG, kept for the clipboard.

## Fullscreen latency

Same two-monitor setup, `capture fullscreen --no-save`, 3850x3960 output, 16 cores.

| Stage | Before | After |
|---|---|---|
| KWin ScreenShot2, both frames | 46 to 60 ms | unchanged |
| Stitch | 100 to 104 ms, on the GTK main thread | 12 to 23 ms, on a worker, split over row bands |
| PNG encode | 98 to 121 ms | 58 to 60 ms |
| Trigger to clipboard offered | 249 to 276 ms | 124 to 149 ms, one outlier at 219 ms |

- Encode no longer makes a converted copy of the image; pixels are converted in place.
- The PNG filter is Paeth instead of adaptive. On this desktop that encoded in 45 ms instead of 63 ms for a 3% larger file (5.16 MB against 5.03 MB).
- The GTK clipboard fallback still renders on the main thread, because Wayland only accepts a clipboard change while the overlay has focus.
- Of the GTK renderers, the default used the least memory after four overlays (93 MB PSS). `ngl`, `gl`, `cairo` and `vulkan` used 203 to 461 MB and were not faster.

## Verified behavior on KDE

- KWin grants ScreenShot2 to any executable whose installed desktop file lists `X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2` and whose `Exec` points at it. Without the file KWin returns `NoAuthorized`, which maps to a typed error.
- KWin returns `QImage::Format_ARGB32_Premultiplied` (6). It maps directly to `GDK_MEMORY_B8G8R8A8_PREMULTIPLIED`, so the overlay shows the frame with no conversion.
- `native-resolution` returns 3840x2160 at scale 1.5, with the monitor reporting 2560x1440 logical. Selection mapping produced a sharp native-resolution crop.
- The layer-shell overlay appears on KWin, takes keyboard focus, Escape cancels without saving, and a second trigger during selection is ignored.
- The portal writes its PNG into `~/Pictures`. The backend deletes it only when it was created during the request; the folder count was unchanged after 5 runs.
- Clipboard: KWin ignores a selection set by a client without keyboard focus. Setting it while the overlay still has focus works (`image/png`, `image/tiff`, `image/jpeg` offered). A fullscreen capture from the background daemon does not reach the clipboard.
- KWin exposes `ext_data_control_manager_v1` to normal clients. This is the route for setting the clipboard from the tray without focus.
- Checked 2026-10-03 on Plasma Wayland: three `capture fullscreen --no-save` runs sent to the background daemon each left a 3840x2160 `image/png` that `wl-paste` read back, with no window focused. Klipper also requests the PNG once per capture. The debug build offers it about 2 s after the trigger, mostly PNG encode.

## Backend contract and detection

Every backend implements `CaptureBackend` (`capabilities`, `capture`). `backends::candidates` picks the order from what the session exposes, never from `XDG_CURRENT_DESKTOP`:

1. `kwin-screenshot2` when `org.kde.KWin` owns its D-Bus name and the Wayland registry has `org_kde_plasma_shell` (or there is no Wayland display, as on Plasma X11). A KWin on a shared session bus under another compositor is ignored.
2. `ext-image-copy-capture` when both `ext_image_copy_capture_manager_v1` and `ext_output_image_capture_source_manager_v1` are advertised.
3. `wlr-screencopy` when `zwlr_screencopy_manager_v1` is advertised.
4. `xdg-desktop-portal` when the Screenshot portal reports a version.

Auto mode moves to the next candidate when a backend is unavailable, unauthorized, offers no usable format or hits a Wayland error. `--backend wlroots` tries 2 then 3. KWin on this machine advertises neither capture global to normal clients, so `--backend wlroots` reports no backend here.

## wlroots backend in nested Hyprland

Hyprland 0.56.2 running as a 1280x720 window inside this Plasma session, release build. Nested, so these are functional checks, not Omarchy numbers.

| Path | Samples | Result |
|---|---|---|
| ext-image-copy-capture, one frame | 2 | 7.6 and 12.3 ms, `Xrgb8888` |
| wlr-screencopy v3, one frame | 1 | 5.6 ms, `Argb8888` |

- Channel order checked with a known background colour: `rgb(ff4000)` came out as `255, 64, 0` in the PNG through ext-image-copy-capture.
- The cursor appeared in both captures with cursor painting off. The nested compositor draws a software cursor into its output, so this needs a check on a real session.
- When the nested window is hidden, KWin stops its frame callbacks, Hyprland renders nothing, and both protocols wait for a frame. The backend gives up after 5 s with a timeout.
- After the nested output disappeared, Hyprland offered a 0x0 buffer on its `FALLBACK` output. An empty offer now fails that monitor instead of allocating an empty pool, which Hyprland answered with a protocol error.
- Output rotation and flips are undone from the frame `transform` event (ext) or the output transform plus `y_invert` (screencopy). Unit tests cover the pixel mapping; no rotated monitor was tested.

## Overlay without layer-shell

When GDK is not on Wayland or the compositor has no `zwlr_layer_shell_v1`, the overlay opens one fullscreen window per monitor. Checked under XWayland on KWin (`GDK_BACKEND=x11`): both monitors showed the dimmed frozen frame. Escape did not cancel it when launched from a terminal, because KWin did not give the window keyboard focus. The layer-shell overlay on the same machine cancels on Escape. Focus on GNOME depends on the activation token from the launching shortcut and is unverified.

## Not verified

- Omarchy, Hyprland on real hardware, and X11. Hyprland 0.56.2 and Plasma 6.7.5 run in VMs with software rendering, see "KDE and Hyprland VMs" below.
- Keyboard focus for the fullscreen overlay on GNOME when triggered by a real global shortcut.
- Rotated monitors on KWin, the portal and real hardware. Sway and Hyprland virtual outputs pass, see "Layouts" below.
- Two or more physical monitors outside KDE, and any multi-monitor layout on GNOME.
- A selection across a monitor edge on real hardware, on Hyprland, Sway and GNOME, and on the fullscreen-window overlay without layer-shell. It passes on KWin in `test-kde`, see "Selection across monitors" below.
- Global shortcuts on GNOME and on this KDE host. KDE and Hyprland pass in VMs, see "Global shortcuts" below.
- The tray on Waybar, GNOME with the AppIndicator extension, and real hardware. It passes on Plasma in `test-kde`, see "Tray" below.

## GNOME 50 in a VM

Fedora 44, GNOME Shell 50.5, KVM VM `kiekje-gnome` with one 1280x800 virtio display, 4 vCPUs. Results come from `lab/kiekje-lab` and are in `lab-results/fedora-44_gnome-50.5_kvm/`.

- GNOME advertises no capture protocol and no layer-shell, so auto mode uses the portal and the overlay uses fullscreen windows. `ext_data_control_manager_v1` is missing, so the clipboard falls back to GTK. GlobalShortcuts is v1.
- Portal fullscreen capture with the permission granted: 196 to 239 ms over 6 samples.
- The first non-interactive Screenshot request showed a permission dialog. With the screen blanked nobody answered it, the call never returned, and every later trigger was dropped as "already running". The portal call now has a 30 s method timeout, and detection and `doctor` give up on a hung bus call after 2 s.
- The VM has idle blanking and the lock screen turned off, and the screenshot permission for host apps is granted in the permission store, so automated runs don't wait on the dialog. Reset it with `flatpak permission-reset screenshot` to retest first use.
- After a snapshot revert GNOME shows the Activities overview. It hides the pattern window and takes Escape, which made run `20261003-121456` fail `pixels_fullscreen` and `escape`. `run` now closes the overview through the `OverviewActive` property of `org.gnome.Shell` first. Run `20261003-121823` in `test-fedora` then passed `escape` without a click: the fullscreen-window overlay, triggered from a CLI over SSH, got keyboard focus. Focus when triggered by a real global shortcut is still unverified.

## Lab CLI

`lab/kiekje-lab` runs the same tests on any machine and writes each run to `lab-results/<os>_<desktop>-<version>_<virt>/<timestamp>/`: `env.txt`, `doctor.txt`, `daemon.log`, `summary.tsv` and, for interactive runs, `manual.txt` with the answers to the Escape and selection checks. Inside a VM it builds into `~/kiekje-target`, so host and VM builds don't mix.

Quick start for a new VM from any desktop ISO:

```
lab/kiekje-lab doctor-host                          # check KVM, libvirt and tools, prints the pacman fix
lab/kiekje-lab up my-test ~/vm/isos/some-distro.iso # install the OS by hand, log in, paste one line
lab/kiekje-lab run-vm my-test --auto                # run the tests, results land in lab-results/
lab/kiekje-lab all                                  # reset, sync and test every ready VM in turn
lab/kiekje-lab reset my-test                        # back to the clean snapshot
lab/kiekje-lab rm my-test                           # delete the VM and its disk
```

`up` chains `new`, a wait for the OS install and a login, then `bootstrap`, `provision`, `sync`, `install` and `snapshot clean`. It prints each step and resumes at the first unfinished one when rerun. Every step is also its own command:

| Command | What it does |
| --- | --- |
| `doctor-host` | Checks KVM, libvirtd, the default network, virt-install, virt-viewer, `/usr/bin/qemu-img`, `/usr/lib/virtiofsd`, OVMF, ssh, an SSH key, rsync, curl, setfacl, osinfo and python3. Exits non-zero with the Arch fix for each gap. |
| `new <vm> <iso\|url> [--ram 6G] [--cpus 4] [--disk 40G] [--no-3d]` | UEFI q35 VM with a virtio disk and network, a guest agent channel and spice. Tries virtio-gpu with virgl first and falls back to plain virtio-gpu when the host can't start it. `--osinfo` is guessed from the ISO name, else `linux2024`. A URL is downloaded once into `~/vm/isos` (`KIEKJE_LAB_ISOS`). |
| `iso <vm> <iso\|url\|--eject>` | Swaps or ejects the CD. |
| `bootstrap <vm>` | Installs and enables openssh and qemu-guest-agent, adds your public key (`KIEKJE_LAB_KEY`, default `~/.ssh/id_ed25519`) and gives the guest user passwordless sudo. Runs over SSH when port 22 is open, otherwise serves the script on the libvirt bridge and prints a `curl ... \| bash` line to type in the guest. |
| `provision <vm>` | Reads `ID` and `ID_LIKE`, maps them to the arch, fedora, debian or suse family and installs the build dependencies plus the portal backend for the detected desktop. Rust comes from rustup in the user's home. Idempotent. |
| `sync <vm> [--virtiofs]` | rsync over SSH into `~/kiekje-next`, without `target/` and `lab-results/`. `--virtiofs` adds a read-only share instead, restarts the VM once and mounts it at the same path. |
| `install <vm>` | Release build in the VM, installs `~/.local/bin/kiekje-next`, writes the desktop file with the matching `Exec` and `X-KDE-DBUS-Restricted-Interfaces`, then prints `doctor`. |
| `snapshot <vm> [label]`, `reset <vm> [label]` | libvirt internal snapshots, default label `clean`. `reset` asks first for any other label. When a live snapshot is refused, `snapshot` shuts the VM down, takes it and boots again. |
| `ssh <vm> [cmd]`, `rm <vm>` | Shell or command in the VM; delete the VM, its snapshots and its disk after typing its name. The ISO stays. |
| `vms` | State, address, distro, desktop and whether a `clean` snapshot exists. |
| `run-vm <vm> --auto` | Runs `run --driven` in the guest. When the overlay has painted, the guest prints an inject line and the host answers through QEMU: `virsh send-key KEY_ESC` for the Escape check, `input-send-event` on the USB tablet for a drag from 25% to 75% of the screen. The guest writes `escape=pass` to `checks.txt` when the overlay canceled without a click. The drag is saved and judged as `pixels_region`, see the pixel checks below. Runs before this change wrote `region.txt`. |
| `run-vm <vm> --auto --layout <name>` | Same, after switching the guest's outputs to a test layout, see "Layouts" below. Sway and Hyprland only. |
| `all [vm...]` | For every VM with a finished `up` and a `clean` snapshot, or the ones named: revert to `clean`, set the guest clock through the agent, sync, `run-vm --auto`, then power the VM off. Prints `results` at the end and exits non-zero when a VM failed. |

Per-VM state (user, distro, desktop, sync mode, last finished step) lives in `~/.local/share/kiekje-lab/<vm>.env`, next to the lab's own `known_hosts` and the qcow2 copy of the OVMF variable store. Internal snapshots of a UEFI VM need qcow2 NVRAM, and libvirt can't convert the raw Arch template itself. For VMs made by `new`, `run-vm` writes results to `~/kiekje-results` in the guest and copies them back into `lab-results/` with the same layout. VMs without a state file, like `kiekje-gnome`, keep the old behavior.

`all` takes `--layout` too. `KIEKJE_LAB_CAPTURES=10` changes the number of fullscreen captures per backend (default 5). `--no-interactive` skips the region checks. Only the OS install and the first bootstrap still need a person; everything after the `clean` snapshot runs unattended with `all`.

### What was run on 2026-10-03

Host: Arch, KDE Plasma 6.7.5, libvirt 12.7, QEMU 11.1, RTX 3060 Ti with the NVIDIA driver.

- `new` with the Arch ISO 2026.10.01 URL: downloaded, booted the UEFI installer, fell back to non-3D because virgl fails with `EGL_NOT_INITIALIZED` on this host. `iso` eject and insert, the eject-and-boot path when the VM powers off during the wait, and `rm` all worked.
- `test-fedora`: a clone of `kiekje-gnome` (Fedora 44, GNOME 50.5, BIOS), adopted by `up` from the wait step. Bootstrap over SSH, provision, sync, install, snapshot, then `run-vm --no-interactive` 10/10 through the portal. `reset` took 5 s.
- `test-ubuntu`: Ubuntu 26.04 cloud image on the same UEFI setup as `new`. Bootstrap through the served `curl` line, run by cloud-init in place of a person. `run-vm` 10/10 on sway through `ext-image-copy-capture`. `reset` took 21 s.
- `test-arch`: Arch cloud image, same setup, bootstrap over SSH with the guest password. `run-vm` 15/15 on sway. `sync --virtiofs` shared the repo read-only; the live snapshot still worked with the share attached, and `reset` took 3 to 19 s depending on host memory pressure.
- `run-vm kiekje-gnome` still writes through the shared repo as before.
- shellcheck 0.11 passes.

Sway on the Ubuntu and Arch VMs was installed by hand with tty autologin, because cloud images have no desktop. On Ubuntu with sway, `doctor` reports the Screenshot portal as unavailable even though `xdg-desktop-portal-wlr` is installed.

Unverified: the graphical OS install from the Fedora Workstation Live, Ubuntu Desktop, EndeavourOS, CachyOS and Omarchy ISOs, so also whether the login wait sees the guest agent on those images (pressing Enter always works). Not run at all: the suse family, Debian proper, KDE, Hyprland or GNOME on Ubuntu and Arch, and virgl with live snapshots.

### Pixel and clipboard checks

Every `run` builds `examples/lab_probe.rs` next to the app. `lab_probe show` opens a fullscreen window on every monitor whose pixels encode their own position on the desktop: red is x, green is y, both 0 to 255 across the bounding box of all monitors, and blue is a constant 128. With the pattern on screen the run makes a saved fullscreen capture, and in `--driven` mode the 25% to 75% drag is saved too. `lab_probe check` then samples a 16x16 grid of the PNG and decodes where each sample came from. It gets the daemon's `frame` lines, so it knows each monitor's logical rectangle and native size, and judges either the stitched desktop or a crop of one monitor. Samples that fall between monitors are skipped. It writes these lines to `checks.txt`:

- `pixels_fullscreen` and `pixels_region` pass when the PNG has the expected size (exactly the stitched desktop at the highest monitor density for fullscreen, within 2% of the dragged monitor's native frame for the drag) and at least 90% of the samples decode to their expected position, within 1% for fullscreen and 2% for the drag. Swapped channels, mirroring, rotation, a logical-size capture on a scaled monitor and a crop from the wrong place all fail, with the worst sample in the message. The outlier budget covers the cursor.
- `clipboard_fullscreen` and `clipboard_region` read `image/png` back through `ext-data-control-v1` and pass when it is byte-identical to the saved file. Without that protocol (GNOME) the line says `unverified`.

The checked PNGs stay in `png/` inside the run folder, which git ignores.

### Layouts

`--layout` reconfigures the guest's outputs before the run and restores them after. Run folders get the layout as a suffix, like `20261003-133355-multi`.

| Layout | Outputs | Region drag on |
| --- | --- | --- |
| `single` | as booted | the only output |
| `scale` | the first output at scale 1.5 (1280x800 native, 853x533 logical) | that output |
| `multi` | adds a headless 1920x1200 output at 1.5, logical 1280x800 at -1280,0 | the headless output |
| `rotated` | adds a headless 1920x1200 output at 2.0, turned 90 degrees, logical 600x960 at -600,0 | the headless output |

On sway the extra output comes from `create_output` and the QEMU tablet is mapped to it with `map_to_output`. On Hyprland it comes from `hyprctl output create headless`, and the drag is aimed inside the tablet's whole-desktop range instead.

Results on 2026-10-03, every check passed:

| VM | Layout | Fullscreen | Region |
| --- | --- | --- | --- |
| test-arch, sway 1.12 | scale | 1280x800, 256/256 | 642x402 |
| test-arch | multi | 3840x1200, 256/256 | 962x602 on HEADLESS-1 |
| test-arch | rotated | 3760x1920, 223/223 (33 samples fall between monitors) | 602x962 on HEADLESS-1 |
| test-ubuntu, sway | scale | 1280x800, 256/256 | 642x402 |
| test-ubuntu | multi | 3840x1200, 256/256 | 962x602 on HEADLESS-1 |
| test-hypr, Hyprland 0.56.2 | scale | 1280x800, 256/256 | 642x402 |
| test-hypr | multi | 3840x1200, 256/256 | 962x602 on KIEKJE-LAB |
| test-hypr | rotated | 3760x1920, 223/223 | 602x962 on KIEKJE-LAB |

This covers mixed scale 1.0 + 1.5 and 1.0 + 2.0, a negative origin, a rotated output and a crop on a fractional-scaled monitor. Clipboard readback matched the saved file in every run. GDK reported scale 3.2 for the rotated headless output at scale 2; Kiekje derives its scale from frame size over logical size, so this did not affect it.

### Save, error paths, memory and lifecycle

Every `run` now also writes these lines to `checks.txt`:

| Check | Passes when |
| --- | --- |
| `save_burst` | 10 back-to-back saved captures give 10 distinct, valid PNG files named `kiekje-fullscreen-<date>-<time>[-n].png` in the Kiekje pictures folder. |
| `readonly_save` | With that folder read-only, the daemon logs `save failed`, still puts the image on the clipboard, and the next capture saves normally. |
| `missing_backend` | `--backend` with a backend this session lacks logs `capture failed` with the reason, and the next capture works. |
| `double_trigger` | A trigger while the region overlay is open is dropped and logged (driven runs only). |
| `escape` | Also fails when canceling left a file behind. |
| `memory` | Anonymous memory grows less than 4 MB over a second round of 10 fullscreen captures. Idle and per-round numbers are in the line. |
| `single_instance` | A second `daemon` exits with `already running` and the first keeps running. |
| `quit` | The daemon exits within 10 s of `quit` and a new client reports `not running`. |
| `cli_exit` | No CLI client printed the GLib D-Bus unregister warning. |

Results on 2026-10-03, every check passed on test-arch, test-ubuntu, test-fedora, test-kde and test-hypr. On test-fedora the clipboard lines are `unverified` (no data-control protocol) and `shortcuts` is `unverified` (GNOME asks in a dialog). On the sway VMs `shortcuts` is `unavailable` because `xdg-desktop-portal-wlr` has no GlobalShortcuts. Memory on the 1280x800 VMs, anonymous:

| VM | Idle | After backend loops and overlays | Round 1 growth | Round 2 growth |
| --- | --- | --- | --- | --- |
| test-arch | 6 MB | 55 MB | 7 MB | 0 MB |
| test-ubuntu | 5 MB | 40 MB | 0 MB | 0 MB |
| test-fedora | 6 MB | 43 MB | 0 MB | 0 MB |

The save burst landed up to 2 captures in the same second; the second got the `-2` suffix from the exclusive create.

## KDE and Hyprland VMs

Both are clones of `test-arch` with the desktop installed over SSH instead of from an installer ISO, so no installer was clicked through.

- `test-kde`: Plasma 6.7.5 on Wayland, started by `startplasma-wayland` from tty autologin, screen locking and display power-off disabled. KWin ScreenShot2 and the portal both work. The first portal Screenshot showed Plasma's "Allow Apps to Take Screenshots?" dialog; it was allowed once and the grant is in the clean snapshot.
- `test-hypr`: Hyprland 0.56.2 with `xdg-desktop-portal-hyprland`, started by `start-hyprland` from tty autologin. `ext-image-copy-capture`, `wlr-screencopy` and the portal work. GlobalShortcuts is v1.
- Cloning broke the network: cloud-init's `10-cloud-init-eth0.network` matches the old MAC address. The match line was removed through the guest agent.
- Omarchy itself was not installed. It needs its own installer on top of Arch.

Lab fixes found on the way:

- The virtiofs share was not mounted in the clones, so the guest ran a stale rsync copy of the lab script. `sync` now mounts the share on every run.
- A live snapshot taken with the share mounted could not be restored (`vhost-user-fs` back-end state). `snapshot` now unmounts the share first.
- On Hyprland an unanswered portal permission dialog from `xdg-desktop-portal-gtk` stalled every later portal call, including the Settings calls GTK makes at startup. The pattern window then never showed, `capture` took 2 s per detection timeout, and `quit` with no daemon hung. `provision` now stores the screenshot grant for host apps in the permission store.
- Hyprland has no way to map the QEMU tablet to one output, so the tablet spans the whole desktop. For the `multi` and `rotated` layouts the guest now computes the drag from the monitor geometry that `lab_probe show` prints and sends it with the inject line.

## Global shortcuts

The daemon registers `capture-region` (preferred `Print`) and `capture-fullscreen` (preferred `Shift+Print`) through the GlobalShortcuts portal at startup. It uses the GTK application's own D-Bus connection, because GTK registers the app id `dev.remcostoeten.KiekjeNext` with `org.freedesktop.host.portal.Registry` on it; the portal then names the shortcuts after the app. Activations arrive as signals on the main loop and start a saved capture through the same path as the CLI. Without a portal the daemon logs `global shortcuts are off` and keeps running.

- KDE (`test-kde`): Plasma showed "Global Shortcuts Requested" with both shortcuts and their preferred keys. After OK, `BindShortcuts` returned `Print` and `Shift+Print`. Pressing Shift+Print through QEMU made a KWin fullscreen capture; Print opened the region overlay with keyboard focus, and Escape canceled it. The accepted bindings are in the clean snapshot.
- Hyprland (`test-hypr`): xdph registered `dev.remcostoeten.KiekjeNext:capture-region` and `:capture-fullscreen` without keys, as `hyprctl globalshortcuts` shows. `hyprctl dispatch global dev.remcostoeten.KiekjeNext:capture-fullscreen` made a saved capture. On Hyprland the daemon writes the binds to a managed include, `~/.config/kiekje/hyprland.conf`, and only rewrites it when its content changes. If `~/.config/hypr/hyprland.conf`, or a file it sources up to four levels deep, has no `source` line for it, the daemon logs the exact line to add and `doctor` reports `hyprland binds: not sourced` with that fix. Checked on 2026-10-04 with unit tests and `doctor` under headless sway with `HYPRLAND_INSTANCE_SIGNATURE` set. Unverified on a real Hyprland: whether Hyprland reloads the include on change, and whether Omarchy's own `Print` bind fires alongside it.
- Not tested: GNOME (GlobalShortcuts v1 in the Fedora VM), real key presses on Hyprland, and the KDE host, where registering would add bindings to the real `kglobalshortcutsrc`.

Every lab run now writes a `shortcuts` line: `pass` with the bound triggers, `unavailable` when the session has no portal for it, or `unverified` when nothing came back within 15 s because the desktop is asking in a dialog.

## Selection across monitors

The overlay keeps the drag in desktop coordinates. Each surface adds its monitor's logical origin to the pointer position, and the Wayland implicit grab keeps motion going to the surface where the press started, so a drag continues past its edge. Every surface draws the part of the rectangle that falls on it, and the size label sits on the monitor under the rectangle's bottom-left corner.

On release the selection is clipped to the monitors it touches. When the clipped rectangle fits one monitor it is cropped from that frame at native resolution, as before. Otherwise it is composed from every touched frame at the highest pixel density among them, with nearest-neighbour upscaling for the others and transparent pixels in gaps between monitors. `kiekje: selected WxH across monitors from logical ...` marks the second case in the log.

Tested on 2026-10-03 in `test-kde` with a second virtio-gpu device, because the second head of one virtio-vga device stays disconnected without a SPICE client that opens it. Layout: Virtual-1 1280x800 at scale 1 and Virtual-2 1280x800 at scale 1.5 (GDK reports 853x533 logical) to its right. The domain keeps both GPUs and has a `multi` snapshot; `clean` still restores the one-monitor setup. With `lab_probe show` on screen, drags were sent through QEMU, whose tablet spans the whole desktop on KWin:

| Drag | Result | Pattern check |
| --- | --- | --- |
| Virtual-1 into Virtual-2 | 602x302 from logical 401x201+1099+199 | 171 of 171 samples within 0.6/255 |
| Virtual-2 back into Virtual-1 | 1202x602 from logical 801x401+899+99 | 171 of 171 samples within 0.6/255 |
| Inside Virtual-1 | 427x241 native crop on Virtual-1 | not sampled |

A screenshot of both QEMU heads mid-drag showed the rectangle on each monitor, meeting at the edge, with the label `1202 × 602` on Virtual-1.

## Tray

The daemon shows a StatusNotifierItem through `ksni`. Its `spawn` blocks on D-Bus, so it runs on a separate thread, and the tray only sends commands to the main loop over a channel. Left click opens the region overlay. The right-click menu is the native one drawn by the tray host: Capture region, Capture every screen, Open captures folder, Quit. Captures picked from the menu start 150 ms after the click so the menu is gone first. Quit removes the item and stops the daemon. Without a StatusNotifierWatcher the daemon logs `tray is off` and keeps running.

Tested on 2026-10-03 on Plasma 6.7.5 in `test-kde`, clicking through QEMU:

- The `camera-photo-symbolic` icon showed in the system tray and the menu opened on right click.
- Left click painted the overlay in 78 ms; Escape canceled it.
- Capture every screen from the menu saved a 3201x1200 stitched capture without the menu in it. With the delay set to 0 ms the menu was also absent in 3 of 3 captures, because Plasma closes the menu before it sends the click. The 150 ms stays as margin until Waybar and real hardware are measured.
- Open captures folder failed with `Failed to find default application for content type 'inode/directory'` on the VM image, which has no file manager. After installing Dolphin in the guest it opened `~/Pictures/Kiekje`.
- Quit from the menu ended the daemon and the icon disappeared.
- The full lab run passed every check with the tray running; idle memory was 47 MB anonymous.

Left click now opens the styled popover, see "Tray popover".

## Capture delay and repeat last region

`capture --delay <seconds>` waits up to 60 s on the main loop before grabbing the frame, so the frozen frame shows what was on screen after the delay. The capture counts as in flight while it waits, so other triggers are dropped. `capture repeat` and the tray item "Repeat last region" grab every monitor and crop the last selected region without opening the overlay. The region is kept in desktop logical coordinates in `$XDG_DATA_HOME/kiekje/last-region.toml`, written atomically after each region selection, and the crop goes through the same plan as a drag, so it stays native-resolution and may cross monitors. Files are named like region captures.

Checked on 2026-10-04 in a cloud container with sway 1.9 headless (pixman, one 1280x800 output) through `wlr-screencopy`:

- `capture repeat` with no stored region logged `no region was selected yet` and the next trigger worked.
- With a stored 300x200+100+50 region, `capture repeat --delay 2` logged `capturing in 2.0 s`, dropped a `capture fullscreen` sent during the wait, then saved a 300x200 PNG named `kiekje-region-...`.
- A stored region outside every monitor logged `the last region ... is not on any monitor now` and saved nothing.

Unverified: storing the region from a real drag, the tray item, and any desktop other than headless sway.

## Notifications and sound

After every capture the daemon sends one notification through `org.freedesktop.Notifications` on the app's own D-Bus connection, from the main loop and without waiting on the reply. The notice depends on the outcome:

- saved: the image as `image-path`, with Open and Show in folder actions. Show in folder calls `org.freedesktop.FileManager1.ShowItems` and opens the folder when no file manager service is running.
- copied only, with `--no-save`.
- save failed, at critical urgency, saying whether the image is still on the clipboard.
- capture failed, at critical urgency, with the reason.

Successful notices ask the daemon for the `screen-capture` sound theme event through `sound-name`. Failures and `sound = false` send `suppress-sound`. A canceled selection sends nothing. `[notifications] enabled` and `sound` in the config switch both, read once per capture.

Checked on 2026-10-04 with mako 1.8 on headless sway 1.9 in a cloud container. Saved, copied, save failed (folder `/proc/kiekje`) and capture failed (`repeat` with no region) each showed one notification with the expected text, urgency and actions. Both actions reached the daemon: with no FileManager1 service and no default apps, they logged the folder fallback and the open error. With `enabled = false`, nothing was sent.

Unverified: sound playback (mako plays no sounds), and notifications on Plasma, GNOME and the Omarchy mako setup.

## Monitor unplugged during selection

The overlay watches the GDK monitor behind each frame. If one is invalidated while the selection is open, the overlay closes every surface, saves nothing, logs `monitor <connector> was disconnected during the selection` and sends a capture-failed notification. The daemon then takes the next trigger as usual. The signal handlers are disconnected when the overlay finishes, so they don't keep the frames alive.

Checked on 2026-10-04 on headless sway 1.9 with two outputs, HEADLESS-1 at 1280x800 and HEADLESS-2 at 1920x1080 to its right. I opened the region overlay, then ran `swaymsg output HEADLESS-2 unplug`. The overlay closed, the notification showed, and a fullscreen capture right after worked on the remaining monitor. `quit` exited cleanly. Unverified: a physical unplug on KDE and Hyprland.

## Tray popover

Left click on the tray icon, or `kiekje menu`, opens Kiekje's own popover; a second trigger closes it. It lists Capture region (the primary button), Capture every screen and Repeat last region with their shortcuts in mono, up to four recent captures as thumbnails, then Open folder and Quit. Gallery and Settings join it when they exist. Thumbnails are decoded on a worker and clicking one opens the file.

On layer-shell compositors the popover is a transparent `Top` layer surface over the whole monitor with exclusive keyboard focus. The panel sits in the corner nearest the tray click, 8 px from the edges, and a click anywhere outside it closes it, as does Escape. Without click coordinates, or when they are `(0, 0)` or outside every monitor, it uses the first monitor's top right. Elsewhere it is an undecorated window. Captures picked from it start 150 ms later so the popover is gone first.

The styling comes from `resources/style.css`, compiled in with GResource. It needs GTK 4.16 or newer for `var()` and `color-mix()`; older GTK logs the parse error and draws the popover unstyled.

Checked on 2026-10-04 on headless sway 1.9 with GTK 4.16.12 built from source, in a cloud container, using a fullscreen capture taken while the popover was open:

- The panel draws in the top-right corner at 320 px wide, with the tokens from the stylesheet and four cropped thumbnails.
- Escape closed it. Down then Return picked Capture every screen, and the saved capture had no popover in it.

Unverified: closing on a click outside (virtual pointer clicks didn't reach clients in the container), tray click coordinates from Plasma and Waybar, and every real desktop.

## Shortcut recorder and settings

`[shortcuts]` in the config holds one trigger per action in the XDG format, such as `capture-region = "SHIFT+LOGO+s"`; an empty string turns the shortcut off. The rules live in one place and apply to the file and the recorder alike: a key needs Ctrl, Alt or Super (Shift alone doesn't count) except Print, desktop combos like Alt+Tab are refused, and two actions can't share a trigger. A config that breaks a rule is reported and the defaults are used, as with any invalid config.

The triggers are what Kiekje suggests to the GlobalShortcuts portal and what it writes to the managed Hyprland include, so an action turned off gets no `bind` line.

`kiekje settings`, or Settings in the tray popover, opens a settings window with a Shortcuts section. On Hyprland, clicking a shortcut records the next key combo: Escape cancels and Backspace turns it off. The recorder stores the key as it is without Shift, so a combo with Shift names the key, not the symbol it types. A saved change rewrites `config.toml` (keeping the rest of the file) and the include, which Hyprland reloads by itself. Where the portal owns the keys (KDE, GNOME), the rows show the desktop's bindings, read-only, and say to change them in the desktop's shortcut settings, because the portal only takes Kiekje's suggestion once. When shortcuts are off, the window says why.

Checked on 2026-10-04 on headless sway 1.9 with GTK 4.16.12 in a cloud container, with the shortcut host forced to Hyprland in a local build (sway has no GlobalShortcuts portal): recording `s` showed the Ctrl, Alt or Super rule and kept recording; Ctrl+Alt+K saved to both files; the same combo on the other action showed `Ctrl+Alt+K is already set for Capture a region.` and saved nothing; Backspace turned it off and removed its `bind` line; Reset restored the default. Without the override the rows were disabled with the portal error.

Unverified: recording on a real Hyprland session, including Super combos the compositor might grab first, Hyprland reloading the include, and the read-only view on Plasma and GNOME.

## Bundled fonts

Geist and Geist Mono 1.7.2 (variable TTFs, SIL OFL 1.1, licence in `resources/fonts/OFL.txt`) live in `resources/fonts`. At startup the primary instance registers them with fontconfig for this process only, through `FcConfigAppFontAddFile`, before any text is laid out; nothing is installed for the user. It looks in `$XDG_DATA_HOME/kiekje/fonts`, then each `$XDG_DATA_DIRS/kiekje/fonts`, then the source tree, so packages must install them to `/usr/share/kiekje/fonts`. A missing file is logged, the UI falls back to the system font, and `doctor` shows it with a fix. `pango_font_map_add_font_file` would avoid the fontconfig call but needs Pango 1.56, and CI's Ubuntu 24.04 has 1.52.

Checked on 2026-10-04 on headless sway 1.9 with GTK 4.16.12: the popover and settings window render in Geist and Geist Mono, and `doctor` reports both fonts. Unverified: a packaged install reading `/usr/share/kiekje/fonts`.

## Changing portal shortcuts from settings

Where the GlobalShortcuts portal is version 2 or newer, the settings window shows a Change in desktop settings button. It calls `ConfigureShortcuts` on Kiekje's session, so the desktop opens its own dialog. The daemon listens for `ShortcutsChanged` on that session: it logs each new trigger, updates the popover labels, and refreshes the open settings window. On version 1 the button is hidden and the note says to use the desktop's shortcut settings. Reset only shows where Kiekje owns the keys (Hyprland).

Checked on 2026-10-04 on headless sway 1.9 with GTK 4.16.12, against a small fake portal on the session bus (version 2, answering CreateSession and BindShortcuts, then emitting ShortcutsChanged with `Ctrl+Alt+P` 300 ms after ConfigureShortcuts). The button sent ConfigureShortcuts, the daemon logged the change, and the open window showed `Ctrl+Alt+P`. Unverified: the real KDE and GNOME dialogs, and which portal versions they ship.

## Capture library

Phase 2 starts with the metadata store. Every saved capture gets a row in `$XDG_DATA_HOME/kiekje/library.db` (SQLite through bundled `rusqlite`, WAL mode): path, mode, size in pixels and bytes, and creation time. The row is written on the capture worker right after the file, and a failed write is logged without affecting the capture. The schema is versioned with `user_version`, and a database from a newer build is refused rather than changed. At daemon start a worker checks every row against disk and sets or clears its missing flag. The popover's Recent row now reads the four newest rows that are not missing, instead of listing the captures folder, so files that aren't Kiekje's no longer show up. Captures saved before the library existed are not imported.

Thumbnails are cached in `$XDG_CACHE_HOME/kiekje/thumbs`, one PNG per source file that fits in 320 by 320 and never upscales. The source is decoded with GdkPixbuf, scaled with `fast_image_resize` (Lanczos3, alpha premultiplied) and encoded with `png`. The cache key hashes the path, file size, modification time and edge size, so a changed file gets a new thumbnail. At daemon start, after the missing check, cache entries that no longer match a present library row are deleted. The popover's Recent row reads through this cache, so after the first open it no longer decodes full screenshots. Measured only on headless sway in a debug build: the first open with a cold cache shows the Recent row noticeably later than a warm one; no p50 or p95 yet.

Checked on 2026-10-04 on headless sway 1.9: two fullscreen captures made two rows. After deleting one file and restarting the daemon, it logged one entry changed state, and the popover showed only the remaining capture. Not built yet: thumbnails cache, gallery views, filters, file actions and re-linking moved files.

## Follow-up for Phase 1

1. Done: clipboard through `ext-data-control-v1` for captures that have no focused window, with a focused-window fallback on desktops without it (GNOME).
2. Done: the daemon warms the overlay when it starts idle. It realizes a window that is never mapped and renders one small overlay-like frame offscreen, which loads the GL driver, the renderer and the label font. Measured on this desktop (NVIDIA, Plasma 6.7.5, 4K at 1.5), release build, fresh daemon each run:

   | | Samples | First overlay painted | Idle after 6 s |
   | --- | --- | --- | --- |
   | Without warm-up | 4 | 246 to 432 ms | 72 MB PSS, 21 MB anonymous |
   | With warm-up | 4 | 98 to 118 ms | 117 to 132 MB PSS, 36 to 37 MB anonymous |

   The warm-up itself takes 222 to 313 ms on the main thread right after start. A trigger in that window waits for it, which is no slower than a cold overlay. The extra memory is the same driver cost every daemon paid after its first region capture anyway. A daemon started by `capture` skips the warm-up, because its first overlay does the same work.
3. Done: memory growth came from the glibc heap, see above.
4. Done for registration: GlobalShortcuts through the portal on KDE and Hyprland, see "Global shortcuts". Done: the Hyprland managed `bind` include. Done: rebinding from settings on Hyprland and turning shortcuts off, see "Shortcut recorder and settings". Done: `ConfigureShortcuts` on portal v2 desktops, see "Changing portal shortcuts from settings".
5. Done: CLI clients that forward to the running instance, and `quit` with nothing running, now leave through `g_application_run` with a `handle-local-options` handler that exits at once. GLib no longer warns about the D-Bus registration.
6. Partly done: Hyprland runs in `test-hypr`. Still open: an Omarchy session, and GNOME overlay focus when triggered by a real global shortcut.
7. Done on KWin: selection across monitors and the StatusNotifierItem tray with its native menu, see "Selection across monitors" and "Tray". Done: the styled tray popover, see "Tray popover". Open: all three on Hyprland, Sway, GNOME and real hardware.

## Gallery window

`kiekje gallery`, the tray menu's Open gallery item and the Gallery button next to Recent in the popover open one gallery window (a second request brings it forward). It lists every library row that is not missing, newest first, as a `GtkGridView` of cards: the cached thumbnail, the file name, and the pixel size, file size and date in mono. The grid only binds visible cards, and each bound card loads its thumbnail on `gio::spawn_blocking`; the window keeps at most 240 decoded textures and drops the oldest. Cards show a skeleton until their thumbnail arrives. Enter or a double click opens the file with the default app, Esc closes the window. The list reloads when the window becomes active and after each saved capture, and keeps its scroll position when nothing changed. Tested on headless sway only: grid, skeleton-free warm load, keyboard selection with the accent tint, live refresh after two captures, and the popover button. Unverified: Plasma, Hyprland, GNOME, double click (wlrctl clicks don't reach clients), and scrolling a 10,000-item library.

The gallery has a toolbar: a search field (typing anywhere in the window starts a search, Ctrl+F focuses it) that matches any part of the file name ignoring case; a sort menu (newest, oldest, largest, name); Grid and List chips; kind chips (All, Region, Screen, where a repeated region counts as a region); and period chips (any time, 24 h, 7 days, 30 days, as rolling windows). Filtering and sorting run in memory over the loaded rows in `library::query`, which is unit-tested; the header shows "N of M captures" while a filter is active and a "No captures match" state when nothing is left. Grid and list share one selection model. Tested on headless sway: search narrowing, list view through keyboard Tab and Space, chip and sort rendering. Unverified on real desktops, and the sort menu's popover was not opened there.

File actions work on the selected capture, from a right-click menu, Shift+F10 or the Menu key, or shortcuts that only apply while the grid or list has focus: Enter opens, Ctrl+Return shows it in the file manager (`FileManager1.ShowItems`, falling back to opening the folder), Ctrl+C copies the image, Ctrl+Alt+C copies the path (Dolphin's Copy Location key; Ctrl+Shift+C did not match with the test keyboard), Delete moves the file to the trash through GIO and drops its record, and "Remove from gallery" drops only the record. A status line under the grid reports each result or error. The selection survives a reload, and a reload requested while one runs is queued instead of dropped. Notifications and the gallery share `ui::files` for open and reveal. Tested on headless sway: menu by keyboard, copy image, copy path, trash (file landed in the XDG trash), remove from gallery, reveal falling back to the folder. Unverified: right click with a real pointer, Plasma, Hyprland, GNOME, and the image on the clipboard as seen by another app.

Missing files: every gallery load first runs the library's missing check, so files deleted or moved outside Kiekje are noticed without a restart. When any are missing, a "Missing N" chip appears at the end of the filter row; it switches the views to missing captures only (search, kind, period and sort still apply). A missing card has no thumbnail and reads "Missing from" its old folder. On a missing capture, Open, Show in folder, Copy and Trash are disabled; "Locate file" (Ctrl+L) opens the GTK file dialog in the old folder and relinks the record to the picked file (`Library::relink`, which refuses a path another record already uses or a file that doesn't exist), "Remove from gallery" forgets one, and "Remove all missing" forgets them all. Tested on headless sway: two files moved away showed under Missing 2, the menu showed the right actions enabled, and locating one through the dialog relinked it (Missing 1). Unverified: the portal file chooser on Plasma, Hyprland and GNOME.
