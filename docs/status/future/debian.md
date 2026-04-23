# Debian support path

This page describes what Debian support would require. Debian is possible, but
it raises more packaging and version-drift questions than rolling-release
Wayland setups.

## Current reality

Debian can run Kiekje only if the session and package set match current Linux
requirements. The project does not yet publish a Debian-specific support path.

- Debian with Hyprland or another compatible Wayland session is closer to
  viability than Debian with default desktop assumptions.
- Stable Debian releases may lag on Rust, GTK, libadwaita, or related native
  packages compared with faster-moving Wayland setups.

## What is needed

These items are required before Debian can be treated as supported target.

- Test build and runtime dependencies against specific Debian releases.
- Decide minimum supported Debian release and Rust toolchain.
- Validate availability of `grim`, `wl-copy`, GTK4, and libadwaita runtime
  packages.
- Add backend path that does not require Hyprland for core workflows.
- Add packaging and install docs that are specific to Debian, not generic to
  all Linux systems.

## Recommended support policy

Debian support should be explicit and release-based.

- Start with one tested Debian release, not "Debian" as broad claim.
- Treat Hyprland-compatible sessions as first target.
- Add GNOME or KDE desktop support only after backend abstraction work lands.

## Bottom line

Debian support is possible, but the project needs versioned testing and a less
Hyprland-specific backend before Debian can be called supported with confidence.
