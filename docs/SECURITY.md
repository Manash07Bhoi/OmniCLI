# Security policy and posture

## Reporting a vulnerability

Open a [private security advisory](https://github.com/Manash07Bhoi/OmniCLI/security/advisories/new)
or email the maintainer. Please include the version (`omni --version`), the
platform, and a reproduction. Do not open a public issue for a vulnerability.

## What this project is, and what it is not

OmniCLI is a **local, single-user toolkit**: a CLI that operates on files you
point it at, plus an optional local HTTP API and dashboard for the same
operations. It has:

- **no authentication layer** — the API is designed to be bound to
  `127.0.0.1`, or reachable only through a proxy that authenticates;
- **no multi-tenancy** — every process that can run the CLI can read and write
  the same data;
- **no telemetry** — nothing is sent anywhere. The only network requests the
  project makes are the ones you ask for (the installer downloading a release,
  and `omni security headers`/`tls` when you point them at a URL).

Anything that needs accounts, isolation between users, or exposure to the
public internet belongs in front of this tool, not inside it.

## Defensive scope of the security commands

`omni security …` is intentionally limited to work that a system owner can do
on their own machine:

| Command | What it does | What it will never do |
|---------|--------------|-----------------------|
| `security secrets [PATH]` | Scans *your* files for credential-shaped strings and prints redacted evidence | No exfiltration, no network calls, no attempt to validate a found key |
| `security permissions [PATH]` | Reports setuid/setgid and world-writable entries | Does not change permissions |
| `security config` | Audits config-file and data-directory permissions | Does not rewrite your configuration |
| `security headers URL` | Reads response headers from a URL you name | No fuzzing, no payloads, a single request |
| `security tls HOST` | Reads the certificate chain of a host you name | No interception, no key material handling |

There is no scanning of third-party systems, no exploitation, no credential
capture, no evasion and no destructive action anywhere in this project. Pull
requests adding such features will be declined.

## Built-in protections (and where they are tested)

| Area | Protection | Test |
|------|-----------|------|
| Archive extraction | Rejects `..` components, absolute paths, symlinks that escape, hardlinks to outside targets, device nodes; entry-count and expansion-ratio limits (`ExtractLimits`: 1M entries, 64 GiB, 10000:1) | `omnicli-archive` unit tests, `archive.test.ts`, `test-installer.sh` scenario 4 |
| Installer | Verifies the archive against the release's `SHA256SUMS.txt` before extracting; refuses when the checksum file is missing; rejects traversal members; installs with mode 755 | `scripts/test-installer.sh` (4 scenarios, run in CI) |
| API input | Zod validation on every route, 1 MB body cap, JSON 404s, no stack traces in production, `{error, issues}` for validation failures | `dev-and-errors.test.ts`, `files.test.ts`, `search.test.ts` |
| API headers | `X-Content-Type-Options`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`, `Cross-Origin-Resource-Policy: same-origin`, `default-src 'none'` CSP; no `X-Powered-By` | `observability.test.ts` |
| CORS | Deny-by-default in production; `CORS_ORIGIN` opt-in; dev origins allowed only in development | `observability.test.ts` (live-verified with `curl`) |
| Logs | pino redacts `authorization` and `cookie`; secret-scan matches are always redacted in output | `observability.test.ts`, CLI security tests |
| Config writes | Atomic write (temp file + rename), so an interrupted write cannot leave a truncated config | `omnicli-config` tests |
| Destructive operations | `--dry-run` support, `--yes` required for duplicate deletion, `--delete-dupes` never runs unattended | CLI tests |
| Provenance | Release archives are attested by the release workflow; `gh attestation verify` is documented in the release notes | `.github/workflows/release.yml` |

## Dependency security

- `pnpm audit` is run in CI and currently reports **no known vulnerabilities**.
  Reaching that state involved tracing every advisory to its dependency path,
  pinning patched releases with overrides, and isolating the one design-only
  package that carried an advisory with no patched release — not an allowlist.
- `cargo audit` and `cargo deny` run in `.github/workflows/security.yml`
  (license gate: GPL/AGPL/LGPL and the `openssl` crate are rejected).
- No dependency is upgraded blindly: every override in `package.json` was
  followed by typecheck, the full test suite, both builds, and — for the
  `drizzle-kit` chain — an actual `drizzle-kit push` run against a scratch
  database.

## Known limitations

- **No authentication.** See above; put the API behind a proxy if it must be
  reachable from another host.
- **macOS, Windows and Android/Termux binaries are built by CI but have not
  been executed** on the machine where this documentation was written. See the
  platform support matrix in the README.
- **The dashboard is a local admin surface.** It ships `noindex, nofollow` and
  must not be exposed publicly.
- **Secret scanning is heuristic.** It catches common key formats and
  high-entropy assignments; it will miss obfuscated secrets and can flag
  random-looking test data.
