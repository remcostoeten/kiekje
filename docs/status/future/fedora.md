# Fedora support path

This page describes what Fedora support would require. Fedora is attractive
because it is Linux-first and Wayland-forward, but distro reputation still does
not remove backend constraints.

## Current reality

Fedora often ships modern Wayland tooling, but its default GNOME desktop is not
the same thing as Hyprland support.

- Fedora with Hyprland or another compatible wlroots session is closest to
  working path.
- Fedora Workstation on GNOME Wayland still runs into the current backend
  mismatch because capture depends on `grim`.

## What is needed

These items are required before Fedora can be treated as supported target.

- Test runtime behavior on Fedora Hyprland and Fedora GNOME separately.
- Add portal-backed or compositor-specific capture for GNOME-first setups.
- Keep tray and GTK behavior validated on Fedora package versions.
- Add installation and packaging docs for Fedora-specific dependencies and
  release flows.

## Recommended support policy

Fedora is good candidate for second official distro wave after Hyprland-first
Linux support is stable.

- Support Fedora Hyprland path first.
- Add Fedora GNOME only after backend abstraction and portal capture exist.

## Bottom line

Fedora is more realistic than broad Ubuntu or Debian claims in the near term,
but official Fedora support still needs backend work beyond current `grim`
assumptions.
