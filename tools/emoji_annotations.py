"""Builds resources/emoji/annotations.tsv from the Unicode emoji list and CLDR annotations.

Usage:
  python3 -I tools/emoji_annotations.py emoji-test.txt en.json nl.json en-derived.json nl-derived.json

Fetch the inputs first:
  https://unicode.org/Public/emoji/16.0/emoji-test.txt
  https://cdn.jsdelivr.net/npm/cldr-annotations-full/annotations/<lang>/annotations.json
  https://cdn.jsdelivr.net/npm/cldr-annotations-derived-full/annotationsDerived/<lang>/annotations.json
"""

import json
import pathlib
import sys

SKIN_TONES = {chr(code) for code in range(0x1F3FB, 0x1F400)}
VARIATION_SELECTOR = "️"
OUTPUT = pathlib.Path(__file__).resolve().parent.parent / "resources" / "emoji" / "annotations.tsv"


def load_annotations(base_path, derived_path):
    base = json.loads(pathlib.Path(base_path).read_text())["annotations"]["annotations"]
    derived = json.loads(pathlib.Path(derived_path).read_text())["annotationsDerived"]["annotations"]
    return {**derived, **base}


def describe(annotations, glyph):
    entry = annotations.get(glyph) or annotations.get(glyph.replace(VARIATION_SELECTOR, ""))
    if entry is None:
        return "", ""
    name = " ".join(entry.get("tts", []))
    keywords = [word for word in entry.get("default", []) if word != name]
    return name, "|".join(keywords)


def glyphs(emoji_test_path):
    group = ""
    for line in pathlib.Path(emoji_test_path).read_text().splitlines():
        if line.startswith("# group:"):
            group = line.split(":", 1)[1].strip()
            continue
        if not line or line.startswith("#") or "; fully-qualified" not in line:
            continue
        codes, _, rest = line.partition(";")
        glyph = "".join(chr(int(code, 16)) for code in codes.split())
        if group == "Component" or any(char in SKIN_TONES for char in glyph):
            continue
        yield glyph, group


def main(args):
    emoji_test, en, nl, en_derived, nl_derived = args
    english = load_annotations(en, en_derived)
    dutch = load_annotations(nl, nl_derived)
    lines = []
    for glyph, group in glyphs(emoji_test):
        en_name, en_keywords = describe(english, glyph)
        if not en_name:
            continue
        nl_name, nl_keywords = describe(dutch, glyph)
        lines.append("\t".join([glyph, group, en_name, en_keywords, nl_name, nl_keywords]))
    OUTPUT.write_text("\n".join(lines) + "\n")
    print(f"{len(lines)} emoji written to {OUTPUT}")


if __name__ == "__main__":
    if len(sys.argv) != 6:
        sys.exit(__doc__)
    main(sys.argv[1:])
