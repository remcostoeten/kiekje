#!/usr/bin/env python3
import json
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
lock = (root / "Cargo.lock").read_text()
sources = json.loads((root / "packaging/flatpak/cargo-sources.json").read_text())

vendored = {
    source["dest"].removeprefix("cargo/vendor/")
    for source in sources
    if source.get("type") == "archive" and source.get("dest", "").startswith("cargo/vendor/")
}
locked = {
    f"{name}-{version}"
    for name, version in re.findall(
        r'name = "([^"]+)"\nversion = "([^"]+)"\nsource = "registry', lock
    )
}

missing = sorted(locked - vendored)
if missing:
    print("packaging/flatpak/cargo-sources.json is out of date with Cargo.lock.")
    print("Missing: " + ", ".join(missing))
    print("Regenerate it with flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json")
    sys.exit(1)
