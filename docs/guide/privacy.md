---
title: Privacy
description: What Kiekje sends over the network, which is nothing unless you ask, and where it keeps your data.
---

# Privacy

**Kiekje makes no network requests unless you ask for them.** There are no analytics and no telemetry. Two features talk to the network, and both are off by default.

| Feature | Setting | What is sent | To |
| --- | --- | --- | --- |
| Update check | `[general] check_for_updates`, or Check now on the About page | One request for the list of Kiekje releases, with a `kiekje/<version>` user agent and nothing about you or your captures. At most once a day | `api.github.com` |
| Temporary upload | `[upload] enabled`, then Upload and copy link in the gallery | The one file you picked, uploaded as `capture.<extension>` so the file name stays on your machine | `litterbox.catbox.moe` |

An uploaded file is public to anyone with the link until it expires after 1, 12, 24 or 72 hours. Kiekje can't delete it earlier.

## Where data lives

| Data | Location |
| --- | --- |
| Captures | `~/Pictures/Kiekje`, configurable |
| Recordings | `~/Videos/Kiekje` |
| Settings | `$XDG_CONFIG_HOME/kiekje/config.toml` |
| Gallery index | `$XDG_DATA_HOME/kiekje/library.db` |
| Last update check | `$XDG_DATA_HOME/kiekje/update-check.toml` |
| Thumbnails | `$XDG_CACHE_HOME/kiekje/thumbs` |
| Unsaved captures | `$XDG_CACHE_HOME/kiekje/unsaved`, pruned after `keep_unsaved_days` |

The guarantee stops at your desktop. A copied image sits on the clipboard where other apps and clipboard managers can read it, and notifications, the screenshot portal and the tray go through your desktop's own services.
