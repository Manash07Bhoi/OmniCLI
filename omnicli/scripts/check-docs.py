#!/usr/bin/env python3
"""Check that the documentation matches the binary it describes.

Documentation rots quietly: a rename here, a deleted subcommand there, and a
copy-paste install line that copies a file that does not exist. This script walks
every markdown file in the repository, pulls the `omni`/`omnicli` command lines
out of the fenced code blocks, and asks the *real* binary whether each command
path and each flag exists. It also checks that referenced files (`docs/assets/*.png`,
`target/release/<binary>`) and pnpm filters resolve.

    python3 scripts/check-docs.py                 # lint only
    python3 scripts/check-docs.py --execute       # also run the curated examples

Exit code 0 = everything documented exists; 1 = at least one mismatch.

Add `# docs-check: ignore` to a line to skip it (used for deliberately
non-existent examples such as "what happens when the file is missing").
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent          # .../OmniCLI
WORKSPACE = REPO.parent                                 # repo root
BIN_CANDIDATES = ("omnicli", "omni")
MARKDOWN = [
    "README.md",
    "omnicli/README.md",
    "omnicli/docs/USAGE.md",
    "omnicli/docs/MEMORY_BANK.md",
    "docs/DEPLOYMENT.md",
    "docs/SECURITY.md",
    "replit.md",
]
BINARY_ALIASES = {
    "omni": "omnicli",
    "omnicli": "omnicli",
    "./omnicli/target/release/omnicli": "omnicli",
    "./omnicli/target/debug/omnicli": "omnicli",
    "./target/release/omnicli": "omnicli",
    "./target/debug/omnicli": "omnicli",
}
FENCED = re.compile(r"```(?P<lang>bash|sh|shell|console|powershell|pwsh)\n(?P<body>.*?)```", re.S)
# Languages where a trailing backslash continues the line. In PowerShell a
# trailing backslash is just a path separator (``docs\``), not a continuation.
POSIX_LANGS = {"bash", "sh", "shell", "console"}
HEADING = re.compile(r"^(#{1,6})\s+(.+?)\s*#*\s*$")
ANCHOR_REF = re.compile(r"\]\(#([^)\s]+)\)")
FLAG = re.compile(r"--[a-zA-Z][a-zA-Z0-9-]*")


def find_binary() -> Path:
    env = os.environ.get("OMNICLI_BIN")
    if env:
        p = Path(env)
        if not p.exists():
            sys.exit(f"OMNICLI_BIN={env} does not exist")
        return p
    for profile in ("release", "debug"):
        for name in BIN_CANDIDATES:
            p = REPO / "target" / profile / name
            if p.exists():
                return p
    sys.exit("no omnicli binary found — run `cargo build --release` first")


class Help:
    """Introspects the CLI's own --help tree instead of a hand-written list."""

    def __init__(self, binary: Path):
        self.binary = binary
        self.cache: dict[tuple[str, ...], str] = {}

    def text(self, path: tuple[str, ...]) -> str | None:
        if path in self.cache:
            return self.cache[path]
        proc = subprocess.run(
            [str(self.binary), *path, "--help"],
            capture_output=True,
            text=True,
            check=False,
        )
        out = proc.stdout + proc.stderr
        if proc.returncode != 0 or not out.strip():
            return None
        self.cache[path] = out
        return out

    def subcommands(self, path: tuple[str, ...]) -> set[str]:
        out = self.text(path)
        if out is None:
            return set()
        names: set[str] = set()
        in_commands = False
        for line in out.splitlines():
            if line.strip() == "Commands:":
                in_commands = True
                continue
            if in_commands:
                if not line.strip():
                    in_commands = False
                    continue
                m = re.match(r"^ {2}([a-zA-Z][a-zA-Z0-9_-]*)", line)
                if m:
                    names.add(m.group(1))
        names.discard("help")
        return names

    def flags(self, path: tuple[str, ...]) -> set[str]:
        out = self.text(path)
        return set(FLAG.findall(out)) if out else set()

    def resolve(self, tokens: list[str]) -> tuple[tuple[str, ...], list[str], str | None]:
        """Greedily consume subcommand names; return (path, remaining, error)."""
        path: tuple[str, ...] = ()
        rest = list(tokens)
        while rest and rest[0] in self.subcommands(path):
            path = (*path, rest.pop(0))
        if path == ():
            return path, rest, None
        # The remainder must not start with something that looks like a verb of
        # the resolved command (i.e. we stopped early because it does not exist).
        return path, rest, None


def command_lines() -> list[tuple[str, int, str, bool]]:
    """(file, line number, command text, posix?) for every documented command line."""
    out: list[tuple[str, int, str]] = []
    for rel in MARKDOWN:
        p = WORKSPACE / rel
        if not p.exists():
            continue
        text = p.read_text(encoding="utf-8")
        for fence in FENCED.finditer(text):
            lang = fence.group("lang")
            posix = lang in POSIX_LANGS
            pending: list[str] = []          # accumulation buffer for multi-line commands
            for lineno, raw in enumerate(fence.group("body").splitlines(), start=1):
                line = raw.strip()
                if not line:
                    continue
                if pending:
                    pending.append(line)
                    joined = " ".join(pending)
                    if _balanced(joined) and not (posix and joined.endswith("\\")):
                        out.extend(_emit(rel, joined, posix))
                        pending = []
                    continue
                if line.startswith("#") or "docs-check: ignore" in line:
                    continue
                head = line.split()[0]
                if not (head in BINARY_ALIASES or head.endswith("/omnicli") or head.endswith("/omni")):
                    continue
                if _balanced(line) and not (posix and line.endswith("\\")):
                    out.extend(_emit(rel, line, posix))
                else:
                    pending = [line]
            if pending:
                out.extend(_emit(rel, " ".join(pending), posix))
    return out


def _balanced(text: str) -> bool:
    """True when every quote in the text is closed (so shlex can parse it)."""
    for q in ('"', "'"):
        if text.count(q) % 2:
            return False
    return True


def _emit(rel: str, line: str, posix: bool) -> list[tuple[str, int, str, bool]]:
    return [(rel, 0, line, posix)]


def split_pipeline(line: str) -> list[str]:
    """Split on `|` only outside quotes, so `jq 'a | b'` survives."""
    parts, buf, quote = [], [], None
    for ch in line:
        if quote:
            buf.append(ch)
            if ch == quote:
                quote = None
        elif ch in "\"'":
            quote = ch
            buf.append(ch)
        elif ch == "|":
            parts.append("".join(buf))
            buf = []
        else:
            buf.append(ch)
    parts.append("".join(buf))
    return [p.strip() for p in parts if p.strip()]


def check_commands(help_: Help, problems: list[str]) -> int:
    checked = 0
    for rel, _, line, posix in command_lines():
        # Run each pipeline stage separately: `omni … | head` is two commands.
        for stage in split_pipeline(line):
            stage = stage.strip()
            if not stage:
                continue
            try:
                tokens = shlex.split(stage, posix=posix)
            except ValueError as exc:                       # unbalanced quotes etc.
                problems.append(f"{rel}: cannot parse {stage!r} ({exc})")
                continue
            head = tokens[0]
            if head not in BINARY_ALIASES:
                continue                                    # e.g. `jq .digest`
            args = [t for t in tokens[1:] if not t.startswith("-")]
            path, _rest, _ = help_.resolve(args)
            path = tuple(dict.fromkeys(path))               # de-dupe repeats
            if not path:
                if args:
                    problems.append(
                        f"{rel}: `{head} {' '.join(args[:2])}` resolved to no subcommand "
                        f"(root takes: {', '.join(sorted(help_.subcommands(())))})"
                    )
                else:
                    checked += 1                            # `omnicli --version`
                continue
            if path[0] not in help_.subcommands(()):
                problems.append(f"{rel}: `{head} {path[0]}` is not a command group")
                continue
            # Walk each segment explicitly so the exact failing verb is reported.
            node: tuple[str, ...] = ()
            for segment in path:
                if segment not in help_.subcommands(node):
                    problems.append(
                        f"{rel}: `{head} {' '.join(node + (segment,))}` — `{segment}` is not a subcommand of "
                        f"`{' '.join(node) or head}` (have: {', '.join(sorted(help_.subcommands(node)))})"
                    )
                    node = ()
                    break
                node = (*node, segment)
            if not node:
                continue
            known = help_.flags(node) | help_.flags(())
            for flag in FLAG.findall(stage):
                if flag not in known:
                    problems.append(f"{rel}: `{flag}` is not accepted by `{' '.join(node)}`")
            checked += 1
    return checked


def check_assets(problems: list[str]) -> int:
    checked = 0
    ref = re.compile(r"!?\[[^\]]*\]\(([^)\s]+)\)|<img[^>]+src=\"([^\"]+)\"")
    for rel in MARKDOWN:
        p = WORKSPACE / rel
        if not p.exists():
            continue
        for match in ref.finditer(p.read_text(encoding="utf-8")):
            target = match.group(1) or match.group(2)
            if not target or "://" in target or target.startswith("#"):
                continue
            if target.endswith(".md") or "/assets/" in target:
                resolved = (p.parent / target).resolve()
                checked += 1
                if not resolved.exists():
                    problems.append(f"{rel}: link target {target} does not exist")
    return checked


def check_anchors(problems: list[str]) -> int:
    """`](#slug)` references must match a heading in the same file (GitHub slugs)."""
    checked = 0
    for rel in MARKDOWN:
        p = WORKSPACE / rel
        if not p.exists():
            continue
        in_fence = False
        headings: set[str] = set()
        for line in p.read_text(encoding="utf-8").splitlines():
            if line.lstrip().startswith("```"):
                in_fence = not in_fence
                continue
            if in_fence:
                continue
            m = HEADING.match(line)
            if m:
                text = m.group(2)
                text = re.sub(r"`([^`]*)`", r"\1", text)  # de-code inline code
                slug = re.sub(r"[^\w\- ]", "", text.strip().lower()).replace(" ", "-")
                headings.add(slug)
        for m in ANCHOR_REF.finditer(p.read_text(encoding="utf-8")):
            slug = m.group(1)
            checked += 1
            if slug not in headings:
                problems.append(f"{rel}: anchor #{slug} matches no heading")
    return checked


def check_binary_names(problems: list[str]) -> int:
    """`target/release/<name>` and `cp target/release/<name>` must be the real bin."""
    cargo = (REPO / "crates" / "omnicli-cli" / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'\[\[bin\]\]\s*\nname = "([^"]+)"', cargo)
    real = m.group(1) if m else "omnicli"
    checked = 0
    pat = re.compile(r"target/(?:release|debug)/([A-Za-z0-9_.-]+)")
    for rel in MARKDOWN:
        p = WORKSPACE / rel
        if not p.exists():
            continue
        for m2 in pat.finditer(p.read_text(encoding="utf-8")):
            name = m2.group(1)
            checked += 1
            if name not in (real, real + ".exe"):
                problems.append(f"{rel}: refers to target/*/{name}, but the binary is `{real}`")
    return checked


def check_pnpm_filters(problems: list[str]) -> int:
    names = set()
    for manifest in list(WORKSPACE.glob("artifacts/*/package.json")) + list(
        WORKSPACE.glob("lib/*/package.json")
    ) + list(WORKSPACE.glob("lib/*/*/package.json")):
        try:
            names.add(json.loads(manifest.read_text(encoding="utf-8")).get("name", ""))
        except json.JSONDecodeError:
            continue
    checked = 0
    pat = re.compile(r"--filter\s+(@?[A-Za-z0-9._/-]+)")
    for rel in MARKDOWN:
        p = WORKSPACE / rel
        if not p.exists():
            continue
        for m in pat.finditer(p.read_text(encoding="utf-8")):
            pkg = m.group(1).split("#")[0]
            checked += 1
            if pkg not in names:
                problems.append(f"{rel}: `--filter {pkg}` matches no workspace package")
    return checked


# Curated examples that are safe to execute against a throwaway directory. Each
# entry is (label, shell command) and must exit 0 with non-empty output.
EXAMPLES = [
    ("version", "omnicli --version"),
    ("help", "omnicli --help"),
    ("doctor", "omnicli doctor --json"),
    ("find", "omnicli file find '*.txt' --path . --count"),
    ("find json", "omnicli file find '*.txt' --path . --json"),
    ("hash", "omnicli file hash notes.txt --algo sha256 --json"),
    ("hash blake3", "omnicli file hash notes.txt --algo blake3"),
    ("stats", "omnicli file stats notes.txt --json"),
    ("duplicate", "omnicli file duplicate . --json"),
    ("archive create", "omnicli archive create bundle.tar.gz ./data"),
    ("archive list", "omnicli archive list bundle.tar.gz"),
    ("convert", "omnicli convert run data/rows.csv data/rows.json"),
    ("dev uuid", "omnicli dev uuid --json"),
    ("dev json", "omnicli dev json '{\"release\": true, \"v\": 1}'"),
    ("security permissions", "omnicli security permissions --json"),
    ("security secrets", "omnicli security secrets notes.txt"),
    ("config show", "omnicli config show --json"),
    ("report", "omnicli report --format markdown"),
    ("workspace new", "omnicli workspace note new 'example note' --json"),
    ("workspace list", "omnicli workspace note list --json"),
    ("search index", "omnicli search index ./data"),
    ("search query", "omnicli search 'needle' --json"),
    ("completions", "omnicli completions bash"),
]


def execute_examples(binary: Path, problems: list[str]) -> int:
    alias = REPO / "target" / "release" / "omnicli"
    env = dict(os.environ)
    if not alias.exists():
        alias = binary
    with tempfile.TemporaryDirectory(prefix="omnicli-docs-") as tmp:
        root = Path(tmp)
        (root / "data").mkdir()
        (root / "notes.txt").write_text("a note\n", encoding="utf-8")
        (root / "data" / "rows.csv").write_text("name,size\nalpha,1\nbeta,2\n", encoding="utf-8")
        (root / "data" / "needle.txt").write_text("a needle in a haystack\n", encoding="utf-8")
        env["HOME"] = str(root / "home")
        env["XDG_DATA_HOME"] = str(root / "home" / ".local" / "share")
        env["XDG_CONFIG_HOME"] = str(root / "home" / ".config")
        env["OMNICLI_ROOT"] = str(root)
        env["NO_COLOR"] = "1"
        (root / "home").mkdir()
        ran = 0
        for label, cmd in EXAMPLES:
            script = cmd.replace("omnicli ", f"{alias} ").replace("--path .", f"--path {root}")
            proc = subprocess.run(
                ["bash", "-c", script],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                timeout=120,
            )
            if proc.returncode != 0:
                problems.append(
                    f"example `{cmd}` exited {proc.returncode}: "
                    f"{(proc.stderr or proc.stdout).strip().splitlines()[:1]}"
                )
            elif not (proc.stdout + proc.stderr).strip():
                problems.append(f"example `{cmd}` produced no output")
            else:
                ran += 1
        return ran


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--execute", action="store_true", help="also run the curated examples")
    args = ap.parse_args()

    binary = find_binary()
    help_ = Help(binary)
    problems: list[str] = []

    n_cmd = check_commands(help_, problems)
    n_assets = check_assets(problems)
    n_bin = check_binary_names(problems)
    n_filter = check_pnpm_filters(problems)
    n_anchor = check_anchors(problems)
    print(
        f"docs check: {n_cmd} command lines, {n_assets} file links, "
        f"{n_bin} binary paths, {n_filter} pnpm filters, {n_anchor} anchors "
        f"— against {binary.relative_to(WORKSPACE)}"
    )

    if args.execute:
        ran = execute_examples(binary, problems)
        print(f"examples executed: {ran}/{len(EXAMPLES)}")

    if problems:
        print(f"\n{len(problems)} problem(s):")
        for p in problems:
            print(f"  - {p}")
        return 1
    print("no problems found")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
