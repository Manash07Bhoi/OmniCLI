#!/usr/bin/env python3
"""One-off: rename the *spoken* binary name from `omni` to `omnicli` in prose,
doc comments, help strings and documentation — never in a path, an extension or
a string literal, so no data/config location changes."""
import pathlib
import re
import sys

LEFT = r'A-Za-z0-9_./\\$"-'     # path separators, extensions, quotes, word chars
RIGHT = 'A-Za-z0-9_.:/"-'       # keep /omni/, omni.db, "omni", omni.toml intact
PAT = re.compile(f'(?<![{re.escape(LEFT) if False else LEFT}])omni(?![{RIGHT}])')

TARGETS = list(pathlib.Path("crates").rglob("*.rs")) + [
    pathlib.Path("docs/USAGE.md"),
    pathlib.Path("README.md"),
]

dry = "--apply" not in sys.argv
total = 0
for path in TARGETS:
    if not path.exists():
        continue
    text = path.read_text(encoding="utf-8")
    new, count = PAT.subn("omnicli", text)
    if count:
        total += count
        print(f"{count:4d}  {path}")
        if dry:
            for m in PAT.finditer(text):
                line = text[: m.start()].count("\n") + 1
                snippet = text.splitlines()[line - 1].strip()[:96]
                print(f"        L{line}: {snippet}")
                break
        else:
            path.write_text(new, encoding="utf-8")
print(("would replace " if dry else "replaced ") + str(total))
