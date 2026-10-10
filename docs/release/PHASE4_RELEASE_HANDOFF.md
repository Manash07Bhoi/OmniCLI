# Phase 4 — Release Handoff Report

**Date:** 2026-10-09 · **Repository:** `github.com/Manash07Bhoi/OmniCLI` · **Version prepared:** `0.2.0`

**Final status: `RELEASE CANDIDATE`** — every gate executable in this environment
passes on current evidence; the publication gates (push, CI, release, crates.io)
are blocked by exactly one thing: **no valid GitHub credential exists in this
environment** (the token supplied in conversation returns HTTP 401). Section 14
lists the exact commands that finish the remaining work.

---

## 1. Verified initial repository state

Measured, not assumed (this session, commands shown):

| Check | Command | Result |
|---|---|---|
| Branch / HEAD | `git rev-parse HEAD` | `main` at `9e9b060` at Phase-4 start |
| Working tree | `git status --porcelain` | clean (0 entries) |
| Remote state | GitHub API (unauthenticated) | `origin/main` = `7636a78` (unchanged since 2026-08-24) |
| Local vs remote | `git rev-list --count origin/main..main` | ahead-only; fast-forward integration (no force ever needed) |
| Prior reports' claims | re-measured | "one commit ahead" was stale — local was 2 ahead (`a46c53f` + `9e9b060`) |
| Toolchain | `rustc --version` / `node --version` / `pnpm --version` | 1.99.0 / v20.20.2 / 10.15.1 |

## 2. History-reconstruction disclosure (preserved, not manufactured)

The original Phase-2/3 work existed as **12 local commits** on top of `7636a78`.
A sandbox reset destroyed `.git` **before any push**; origin never contained
them, so no shared history was lost or rewritten and none was manufactured:
the surviving tree was re-committed as a single disclosed reconstruction
commit (`a46c53f`), described in
[`PHASE3_RELEASE_CERTIFICATION.md`](PHASE3_RELEASE_CERTIFICATION.md) §1/§3.
Per-change attribution lives in the reports, not in fabricated commits.
This Phase-4 work is committed on top as normal, individually reviewed commits.

## 3. GitHub branch integration

**Status: BLOCKED — no valid credential.** Evidence:

```
$ GET https://api.github.com/user   (Authorization: Bearer <supplied token>)
→ 401 Bad Credentials   (both Bearer and token schemes; control requests to
                          the public API return 200, so the network is fine)
```

The supplied token was **never stored, logged, or committed**; nothing needs
revoke-and-clean locally. Because it was pasted into a chat, it should be
treated as exposed regardless of validity and revoked if it was ever active.

Prepared integration (safe, no force, no history rewrite):

```bash
git push -u origin main:reconstruction/phase3-4   # create the review branch
# open PR reconstruction/phase3-4 → main (CI runs automatically: all six
# workflows trigger on pull_request)
# after required checks pass: merge via the GitHub UI or
# gh pr merge --merge; then push main (fast-forward) or merge the PR
```

The branch push is a pure fast-forward from `7636a78` (verified:
`git merge-base --is-ancestor origin/main main`).

## 4. Branch protection (planned with REAL check names — not yet active)

Configure after the first PR run reveals actual check names on GitHub
(do not invent names; job ids and display names verified in the workflow
files):

| Workflow | Jobs to require |
|---|---|
| `rust.yml` | `Test (ubuntu-latest)` + `Test (windows-latest)` + `Test (macos-latest)` (matrix), `Minimum Supported Rust Version` |
| `typescript.yml` | `Typecheck & Build`, `API server smoke test`, `API integration tests (real CLI + real SQLite)` |
| `security.yml` | `Rust — cargo-audit`, `Node — pnpm audit`, `Rust — cargo-deny (licenses + advisories)` |
| `ci.yml` | `Rust build & smoke` (matrix), `Installer end-to-end (real archive, real install.sh)` |

Settings to apply (Settings → Branches → Add rule, or the Branch-protection API):
require PR before merge · require the statuses above · require branches
up to date · block force pushes · block deletion · dismiss stale approvals.
**No required-approval count** (single-maintainer repo — a self-approval is
impossible on GitHub; this matches the "supported by the contributor model"
clause). Administrators: allow bypass explicitly so a broken workflow can be
recovered from without disabling protection.

## 5. Repository cleanup (done this phase)

- Moved six superseded working documents to [`docs/archive/`](../archive/README.md)
  with an index: `AUDIT_AND_FIX_REPORT.md`, `PHASE2_REPORT.md`,
  `FINAL_RELEASE_REPORT_V0.1.1.md`, `AWESOME_LIST_SUBMISSIONS.md`,
  `EXTERNAL_SUBMISSIONS_REPORT.md`, `TUR_PREPARATION.md`, `replit.md`.
  Audit trail preserved; nothing historical was deleted.
- Deleted `empty_commit.txt` (zero-byte stray, referenced nowhere).
- Verified: docs linter 23/23 after the moves; the one README reference updated.
- Kept: lockfiles, all tests, release scripts, LICENSE/SECURITY/CODE_OF_CONDUCT,
  `.replit`/`.replitignore` (live Replit configuration), certification evidence.
- `.gitignore` already covers `dist`, `*.tsbuildinfo`, `node_modules`; generated
  files were verified untracked (`git ls-files` clean).

## 6. README and metadata changes

README (all changes evidence-backed, docs linter 23/23):
- Replaced self-asserted badges (clippy-clean → `#`, "security-audited",
  distro-list platform badge) with the **four real workflow badges** + MIT +
  live crates.io badge (file-name badge URLs — valid as soon as these files
  are on the default branch).
- Corrected stale claims: 269 → **276** tests; ARM/Termux rows in the platform
  matrix now state the truth (full 276-suite executed under **qemu emulation,
  labelled as such**); releases section gained an honest status block for
  v1.0.0 (defective, deprecated) and v0.1.1 (valid assets, stale binaries);
  release example tag corrected from `v1.0.0` to the version-gated flow.
- a11y testing recipe no longer dirties the repo lockfile (scratch-dir
  install) and now includes the 15-check e2e smoke script.

GitHub metadata (description/topics) — **pending auth**; prepared values:
- Description: `One Rust binary for file ops, full-text search, format conversion, archives, and dev tools — with a local REST API and React dashboard.`
- Topics: `rust` `cli` `command-line` `developer-tools` `file-management` `search` `backup` `archive` `typescript` `react`
  (each maps to an implemented, documented capability — no keyword stuffing)

## 7. License and security documentation

- Root `LICENSE`: MIT, present; `license = "MIT"` in all ten crate manifests (verified by grep: single unique value).
- `SECURITY.md` (root): private reporting via GitHub Security Advisories — a real route; no invented email. `docs/SECURITY.md` is the technical posture doc (different purpose, kept).
- `CODE_OF_CONDUCT.md`, `CHANGELOG.md`, `CONTRIBUTING.md` (at `omnicli/CONTRIBUTING.md`, linked from README): present.
- Added this phase: `.editorconfig`, `.github/dependabot.yml` (cargo + npm + github-actions, weekly).
- `FUNDING.yml`, issue/PR templates: pre-existing, left intact.

## 8. CI failures found and their fixes (verified present in source; first real runs pending)

| Historical failure | Fix in tree | Where verified |
|---|---|---|
| Release version gate grepped the workspace-root manifest (empty → every tag refused) | gate greps `crates/omnicli-cli/Cargo.toml` | `release.yml` line ~32; tag `v0.2.0` will match manifest `0.2.0` |
| Security workflow swallowed audit failures / yanked `chacha20 0.10.1` | lockfile bumped to 0.10.2; high/critical fail the job | `cargo-deny check licenses advisories` → ok, ok (local, this session) |
| Unpinned `cross` installation | pinned | `release.yml` (version-pinned install step) |
| Pre-release gates that never ran | tag + `workflow_dispatch` triggers added | `pre-release.yml` |
| API health job ran an unnecessary schema-generation step | removed | `typescript.yml` `api-health` job |
| Incomplete release verification | tag↔version gate + binary `--version` checks in release workflow | `release.yml` |

These are local/static verifications. **The first real GitHub Actions run is
itself the remaining proof** (Blocker A).

## 9. Actual test results (this session, this tree)

Executed locally on the `0.2.0` tree:

| Gate | Command | Result |
|---|---|---|
| Rust workspace | `cargo test --workspace` | **276 passed / 0 failed**, exit 0 |
| CLI black-box | included above (`--test cli`) | **65/65** |
| Clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 |
| Formatting | `cargo fmt --all -- --check` | clean |
| Release build | `cargo build --release --workspace` | `omnicli 0.2.0`, exit 0 |
| MSRV | `cargo +1.88.0 check --workspace` | exit 0 |
| Dependency audits | `cargo-deny check licenses advisories` / `cargo-audit 0.22.2` / OSV script / `pnpm audit` | ok,ok / exit 0 (0 vulns, 1 unmaintained warning) / 1 documented exception / no known vulns |
| JS install/typecheck/build | `pnpm install --frozen-lockfile` / `pnpm typecheck` / `pnpm -r build` | 0 / 0 errors / 0 |
| API suite | `vitest run` | **13 files / 104 tests passed** |
| Installer | `omnicli/scripts/test-installer.sh omnicli/target/release/omnicli` | **4/4 scenarios**, exit 0 |
| Docs | `omnicli/scripts/check-docs.py --execute` | **23/23 examples**, no problems |
| Browser | a11y audit + e2e smoke on the shipped dist | **9 pages / 0 problems**, **15/15 checks** |
| Cross-platform (qemu, emulated) | full workspace suite per ARM target | aarch64-gnu **276/276**, aarch64-musl **276/276**, armv7-musl **276/276**; armv7-gnu smoke PASS |

Executed remotely: **none** — impossible without a valid credential. The
numbers above were re-executed this session on the exact current tree, not
carried from history.

## 10. Version, tag, commit

- **Version:** `0.2.0` (all ten crates + inter-crate deps; CHANGELOG cut).
  Justification: 0.x SemVer — new features since the published 0.1.1
  (`doctor`, `report`, `security` commands; search shorthand/--in/--regex
  content search; case-sensitive search; structured API 400s) plus the fixes
  listed under `[0.2.0]`. Not `1.0.0` (no stability commitment has been made
  or evidenced); not a patch (features were added).
- **Tag:** not created — tagging is the release trigger and is gated on
  Blocker A. The tag `v0.2.0` must point at the commit that carries `7419d62`
  (or later) — the workflow refuses mismatches.
- **Release URL:** will be `https://github.com/Manash07Bhoi/OmniCLI/releases/tag/v0.2.0`
  after the workflow completes.

## 11. Expected artifacts (7 targets, from `release.yml`)

`omnicli-linux-x86_64.tar.gz`, `omnicli-linux-aarch64.tar.gz`,
`omnicli-linux-aarch64-musl.tar.gz`, `omnicli-linux-armv7-musl.tar.gz`,
`omnicli-windows-x86_64.zip`, `omnicli-macos-x86_64.tar.gz`,
`omnicli-macos-aarch64.tar.gz`, plus `SHA256SUMS.txt` (well-formed —
the workflow writes it programmatically; the v1.0.0 malformation cannot
recur).

Independent local reference build: `omnicli 0.2.0` x86_64-linux-gnu,
sha256 `f4109036428527ac3ac5333df203f69bfb7537f902a12773bd2881df4baf2206`.
(Release builds are not byte-reproducible; CI hashes are the published truth.)
ARM cross-build hashes from the certification run are recorded in §13 of the
Phase-3 report.

## 12. crates.io publication — PENDING (no registry credential in scope)

Exact procedure, prepared and validated locally (`cargo metadata` dependency
order; every crate `cargo package`-clean candidates — lockfile + license
fields + README/repository metadata verified):

```bash
export CARGO_REGISTRY_TOKEN="<crates.io token>"   # never commit this
cd omnicli
for c in omnicli-core omnicli-archive omnicli-backup omnicli-config \
         omnicli-convert omnicli-dev omnicli-file omnicli-search \
         omnicli-workspace omnicli-app; do
  cargo package -p $c --allow-dirty --no-verify >/dev/null || exit 1
  cargo publish -p $c || exit 1
  sleep 15   # let the index settle between dependent publishes
done
# verify:
for c in omnicli-core … omnicli-app; do
  curl -s -H "User-Agent: check" https://crates.io/api/v1/crates/$c | grep -o '"max_version":"[0-9.]*"'
done
cargo install omnicli-app --version 0.2.0 && omnicli --version   # smoke
```

Order is `omnicli-core` first and `omnicli-app` last (topologically derived
this session). Note: `omnicli` (bare name) on crates.io belongs to an
unrelated package — the workspace correctly publishes `omnicli-app` + the
`omnicli-*` libraries and must never try to claim the bare name.

## 13. Historical release remediation

- **v1.0.0 (defective: stale 0.1.0 binary + malformed SHA256SUMS).**
  Plan (no deletion, no tag rewrite — the tag/rename of a public release is
  effectively immutable): edit the release body to prepend a deprecation
  notice pointing to the correct install path, and mark it *pre-release* so
  GitHub stops surfacing it as latest; keep assets as evidence. Requires the
  maintainer session (Blocker A). After `v0.2.0` publishes, GitHub "latest"
  moves automatically.
- **v0.1.1 (healthy assets, stale binaries).** Keep as-is; superseded by
  `v0.2.0`. New release notes must (and will, via the CHANGELOG) list the
  fixed completions name and the SIGPIPE panic fix; the post-release check
  confirms the published binary contains them (`omnicli completions bash |
  grep _omnicli` and `| head` exits 0).

## 14. Remaining blockers and exact maintainer actions

| # | Blocker | Action |
|---|---|---|
| A | No valid GitHub credential in this environment (supplied token: 401) | From an authenticated machine: `git push -u origin main:reconstruction/phase3-4` → open PR → watch checks → merge → `git push origin main`. Or create a classic PAT with **`repo` + `workflow` scopes** (workflow scope is required — this tree modifies `.github/workflows/`) and provide it via `git credential` / environment to a session — **not in chat** |
| B | Branch protection not yet active | Apply §4 after the first PR run (check names must be read from the real run) |
| C | First real Actions run of the fixed pipeline | Happens automatically with A; all gates are expected green based on local evidence, and any failure is diagnosed per CI-repair rules |
| D | `v0.2.0` tag + release | After C is green on `main`: `git tag v0.2.0 && git push origin v0.2.0`; then verify the 7 assets + checksums + `--version` on the release page |
| E | v1.0.0 deprecation edit | With A's session: edit the v1.0.0 release body + pre-release flag (§13) |
| F | crates.io 0.2.0 family | §12 procedure with a registry token |

## 15. Final release decision

**`RELEASE CANDIDATE`**

Every gate executable in this environment passes on current, re-executed
evidence (§9), the release engineering is prepared down to the exact commands
(§10, §12, §14), and the historical release defects have safe remediation
plans (§13). It is not `RELEASE READY` because the definition requires the
**actual** integration, protection, CI-green, and completed release-workflow
events — and none can execute without a valid GitHub credential (Blocker A).
The moment A–D complete cleanly, this project satisfies every `RELEASE READY`
condition except native Windows/macOS runtime execution, which the tagged
build's own CI either provides or documents.
