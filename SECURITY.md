# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| `main` (latest) | ✅ Active |
| Older releases | ❌ Not supported — please upgrade |

We follow a rolling-release model. Security fixes land on `main` and are tagged for release. There are no long-term-support branches.

---

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Use GitHub's private advisory flow instead:

👉 **[Report a security vulnerability](https://github.com/Manash07Bhoi/OmniCLI/security/advisories/new)**

This creates a private draft advisory that only you and the maintainers can see. We aim to:

1. **Acknowledge** your report within **48 hours**.
2. **Confirm** the vulnerability and assess severity within **5 business days**.
3. **Release a fix** within **14 days** for critical/high severity.
4. **Credit you** in the advisory and `CHANGELOG.md` (unless you prefer to remain anonymous).

---

## Security Design

OmniCLI is designed with security in mind at every layer:

| Concern | Mitigation |
|---------|-----------|
| **Zip-slip** | Archive extraction rejects `..` path components and absolute entry paths |
| **Encryption** | age X25519 asymmetric encryption via the audited [`age`](https://crates.io/crates/age) crate (IETF RFC 9180) |
| **Hash integrity** | BLAKE3 used for all content comparisons; SHA-256 and MD5 available for compatibility |
| **No `openssl`** | Prohibited via `cargo deny` — rustls/ring only to avoid cross-compilation pain and OpenSSL CVEs |
| **Key exposure** | `--identity` keys visible in `ps aux` — documentation advises shell substitution for production |
| **Dependency auditing** | Weekly `cargo audit`, `cargo deny`, and `pnpm audit` via GitHub Actions |
| **License gate** | GPL/AGPL/LGPL dependencies are blocked by `cargo deny` |
| **No `unsafe` in library code** | `unsafe` blocks are forbidden outside `main.rs` — enforced by CI |

---

## Scope

In-scope for security reports:
- Remote code execution or privilege escalation via any `omni` command
- Path traversal vulnerabilities in archive extraction or file operations
- Cryptographic weaknesses in the `omni file encrypt/decrypt` flow
- Authentication/authorisation bypass in the REST API server
- Dependency vulnerabilities with a CVE score ≥ 7.0

Out of scope:
- Vulnerabilities requiring local root access to exploit
- Denial-of-service via resource exhaustion with attacker-controlled input (report, but lower priority)
- Issues in dependencies we cannot patch ourselves (report upstream; we'll update the dep)

---

## Dependency Audit

Automated audits run on every push to `main` and weekly:

- **Rust**: `cargo audit` + `cargo deny` (advisories, licenses, banned crates)
- **Node.js**: `pnpm audit`

See [`.github/workflows/security.yml`](.github/workflows/security.yml).
