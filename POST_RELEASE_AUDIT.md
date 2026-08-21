# OmniCLI v0.1.0 Post-Release Audit

## Release Identity
- **Git tag**: v0.1.0 (DOES NOT EXIST on GitHub, but exists locally as commit `7297cb9`)
- **current main commit**: `b3ef807`
- **release state**: BLOCKED. There are post-release commits on `main`.

## GitHub Release
- **Status**: BLOCKED / FAILED
- **actual artifacts**: None found for `v0.1.0`. Only `v1.0.0` is published.
- **actual checksums**: Could not verify as artifacts do not exist.

## crates.io
| Crate | Version | Published | Independently Verified |
|------|---------|-----------|------------------------|
| `omnicli-core` | 0.1.0 | Yes | Yes |
| `omnicli-app` | 0.1.0 | Yes | Yes |
| `omnicli-archive` | 0.1.0 | Yes | Yes |
| `omnicli-file` | 0.1.0 | Yes | Yes |
| `omnicli-backup` | 0.1.0 | Yes | Yes |
| `omnicli-config` | 0.1.0 | Yes | Yes |
| `omnicli-convert` | 0.1.0 | Yes | Yes |
| `omnicli-search` | 0.1.0 | Yes | Yes |
| `omnicli-dev` | 0.1.0 | Yes | Yes |
| `omnicli-workspace` | 0.1.0 | Yes | Yes |

## Consumer Installation
- **cargo install result**: Success. `cargo install omnicli-app --version 0.1.0` works correctly.
- **executable result**: Installed `omnicli` successfully.
- **runtime tests**: LINUX x86_64 — TESTED. Passed functional tests (file ops, dev json, search, etc).

## Termux
- **tested capabilities**: LINUX CONTAINER TESTED. (Termux execution environment not available).
- **untested capabilities**: Real Android Termux environment.
- **known limitations**: NEEDS REAL-WORLD TESTING on actual Android device.

## GitHub Actions
- **workflows checked**: `release.yml`, `ci.yml`
- **stale references**: `ci.yml` has a stale reference: `cargo build --release --bin omni` (should be `omnicli`). Both workflows execute from root, but Rust code is under `omnicli/`.
- **actual failures**: Expected CI failure due to the stale `bin omni` target and working-directory issues.

## Security
- **tools executed**: `cargo audit`
- **findings**: 0 vulnerabilities found.
- **unresolved findings**: None.

## TUR
- **recipe status**: VERIFIED (Template exists but awaiting real release SHA256).
- **checksum status**: BLOCKED (Real v0.1.0 tag not on GitHub, unable to get canonical SHA256).
- **PR status**: READY_FOR_MANUAL_SUBMISSION.

## Awesome Lists
- **candidate list**: `awesome-rust`, `awesome-cli-apps`
- **qualification**: OmniCLI qualifies as a Rust CLI utility.
- **PR status**: READY_FOR_MANUAL_SUBMISSION.

## Documentation
- **verified**: `README.md` and `docs/`
- **needs improvement**: `README.md` links to `omnicli-windows-x86_64.zip`, but `release.yml` produces `omni-x86_64-pc-windows-msvc.zip`. Also `scripts/install.sh` points to the root, but exists at `omnicli/scripts/install.sh`.

## Known Limitations
1. P0 — The `v0.1.0` GitHub Release does not exist. (There is a `v1.0.0` release instead).
2. P1 — CI workflows have stale references to `omni` instead of `omnicli`.
3. P2 — Documentation discrepancies for Windows artifacts and install script paths.

## Recommended Next Release
- Address the CI workflows (`bin omni` -> `omnicli` and add `working-directory: omnicli`).
- Standardize the archive naming in `release.yml` to match `README.md` (or vice-versa).
- Cut a proper SemVer release tag on GitHub so that TUR preparation and user downloads can work correctly.

## Exact Remaining Actions
1. Fix `.github/workflows/ci.yml` to use `bin omnicli`.
2. Fix `.github/workflows/release.yml` to use `working-directory: omnicli` and fix artifact output names.
3. Fix `README.md` download links and script paths.
4. Push a new release tag to trigger the corrected `release.yml` workflow.

RELEASE HEALTH:
RED
