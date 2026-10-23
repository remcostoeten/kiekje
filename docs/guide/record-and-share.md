---
title: Record and share
description: WebM screen recording through the ScreenCast portal and opt-in temporary upload links.
---

# Record and share

Both features landed in 0.1.0-alpha.6. The recording pipeline is lab tested against a PipeWire source; the portal handshake and the live upload are unverified on a real desktop.

## Screen recording

```bash
kiekje record
```

`kiekje record`, Record screen in the tray menu or the capture menu starts a recording. Your desktop's own ScreenCast picker chooses a screen or a window. Run the command again, or pick Stop recording, to stop and save. The file is WebM (VP8) in `~/Videos/Kiekje`, and a notification offers Open and Show in folder.

Recording goes through the ScreenCast portal and a GStreamer pipeline with `pipewiresrc`. GStreamer is loaded at runtime with `dlopen`, like the capture sound, so it stays optional.

| Needs | Package on Arch |
| --- | --- |
| GStreamer PipeWire plugin | `gst-plugin-pipewire` |
| GStreamer good plugins | `gst-plugins-good` |
| A ScreenCast portal | `xdg-desktop-portal-kde`, `-hyprland`, `-wlr` or `-gnome` |

Without them the recording fails with a notification naming the package, and `kiekje doctor` shows the same under `recording`.

Next on the list: a region crop, MP4 with H.264 through VA-API, pause and resume, microphone and desktop audio through PipeWire, GIF export, and recordings in the gallery. None of that exists yet.

## Temporary upload links

Off by default. Settings, General, Sharing has **Upload to a temporary link** and **Link expires after**. With it on, **Upload and copy link** (<kbd>Ctrl</kbd>+<kbd>U</kbd>) in the gallery sends the capture to [Litterbox](https://litterbox.catbox.moe) and copies the link.

```toml
[upload]
enabled = true
expiry = "24h"   # "1h", "12h", "24h" or "72h"
```

What leaves your machine is the one file you picked, uploaded as `capture.<extension>`, so the file name stays local. Anyone with the link can open the file until it expires, and Kiekje can't delete it earlier. The [privacy page](/guide/privacy) lists every request Kiekje can make.
