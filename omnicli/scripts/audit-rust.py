#!/usr/bin/env python3
"""Check the Rust dependency graph against the OSV vulnerability database.

    python3 scripts/audit-rust.py        # reads ./Cargo.lock

`cargo audit` is the canonical tool and runs in CI (.github/workflows/security.yml),
but it needs ~1 GB of RAM to build (rustls/aws-lc-sys) and cannot be installed on
every machine. OSV aggregates the RustSec advisories that `cargo audit` uses, so
this script answers the same question for the exact versions in Cargo.lock with
no local build. It exits 1 when it finds an advisory, so it can gate a script.

Nothing is upgraded here: this reports, it does not modify.
"""
from __future__ import annotations

import json
import pathlib
import sys
import urllib.request

LOCK = pathlib.Path(__file__).resolve().parent.parent / "Cargo.lock"
OSV_BATCH = "https://api.osv.dev/v1/querybatch"


def packages(lock_path: pathlib.Path) -> list[tuple[str, str]]:
    """Parse Cargo.lock without a TOML dependency (the format is regular)."""
    out: list[tuple[str, str]] = []
    name = version = None
    for line in lock_path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line.startswith("name = "):
            parts = line.split('"')
            name = parts[1] if len(parts) > 1 else None
        elif line.startswith("version = "):
            parts = line.split('"')
            version = parts[1] if len(parts) > 1 else None
            if name and version:
                out.append((name, version))
                name = version = None
    return out


def main() -> int:
    if not LOCK.exists():
        print(f"error: {LOCK} not found (run cargo generate-lockfile)", file=sys.stderr)
        return 2

    pkgs = packages(LOCK)
    print(f"checking {len(pkgs)} locked crates against OSV (RustSec)…")

    body = json.dumps({"queries": [{"package": {"ecosystem": "crates.io", "name": n}, "version": v} for n, v in pkgs]}).encode()
    req = urllib.request.Request(OSV_BATCH, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as resp:
        results = json.load(resp).get("results", [])

    findings = []
    for (name, version), result in zip(pkgs, results):
        for vuln in result.get("vulns", []):
            severity = "unknown"
            for sev in vuln.get("severity", []) or []:
                severity = sev.get("score", severity)
            findings.append(
                (
                    name,
                    version,
                    vuln.get("id", "?"),
                    (vuln.get("summary") or "").strip(),
                    severity,
                    [a.get("type") for a in vuln.get("affected", []) if a.get("ranges")][:1],
                )
            )

    if not findings:
        print(f"no known vulnerabilities for the {len(pkgs)} crates in Cargo.lock")
        return 0

    print(f"\n{len(findings)} advisory match(es):")
    for name, version, vid, summary, severity, _ in findings:
        print(f"  {name} {version}: {vid} — {summary}  [{severity}]")
        print(f"    https://osv.dev/vulnerability/{vid}")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
