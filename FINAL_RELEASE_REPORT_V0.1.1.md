# TUR PR #2763 - CI Failure Investigation and Fix Report

## Overview
This report details the investigation and fix applied to Termux User Repository (TUR) Pull Request #2763 to resolve CI failures across all architectures.

## Current State
- **PR #2763 Status**: OPEN
- **Final PR Head Commit SHA**: `525b76f4ebaba22b7d679dc39d78368c779d408a`
- **Merge Conflicts**: NONE

## Investigation
### Exact Failure Cause
The CI jobs for `build (aarch64)`, `build (i686)`, `build (x86_64)`, and `build (arm)` were previously failing with the following error:
```
error: cannot create the lock file /home/builder/.termux-build/omnicli/src/omnicli/Cargo.lock because --locked was passed to prevent this
```
### Root Cause Proof
An investigation of the OmniCLI v0.1.1 source release archive at `https://github.com/Manash07Bhoi/OmniCLI/archive/refs/tags/v0.1.1.tar.gz` (SHA256: `9732f3b96f51246f878d00af15c2245f8df6cdc62535dfb067b82e6e63614a6c`) proved that the `Cargo.lock` file is **not included** in the release archive.
Because the source archive does not contain a `Cargo.lock`, running `cargo build` with the `--locked` flag inevitably triggers the observed error (exit code 101).

## Fix Performed
The fix involved modifying the `omnicli/build.sh` recipe purely within the TUR PR branch `add-omnicli` (PR #2763).
- **Files Changed**: `tur/omnicli/build.sh`
- **Modification**: Removed the `--locked` flag from the `cargo build` command in `termux_step_make()`.
- **Reasoning**: This allows Cargo to resolve and generate a lockfile on the fly during the TUR build process, which is the correct and standard Termux/TUR approach when packaging source archives lacking a pre-existing lockfile.
- **Commits Pushed**: 1 commit (`525b76f4ebaba22b7d679dc39d78368c779d408a`) pushed to the `add-omnicli` head branch.
- **Integrity Kept**: The original v0.1.1 source URL and its verified SHA256 checksum remained completely unchanged. The original repo was not modified.

## Final CI Results After Fix
The new commits triggered new GitHub Actions workflows for PR #2763.
- **Workflow Run IDs**: `32694798519` (Package updates TUR), `32694798612` (Packages-TUR)
- **Status of New Jobs**: **PENDING MAINTAINER APPROVAL**
- **Conclusion**: `action_required`

## Conclusion
The technical defect within the TUR recipe has been fixed, pushed, and correctly linked to the PR. However, due to Termux/TUR GitHub security policies on first-time/external contributors, the actual CI jobs are paused and require maintainer action to execute.

**MAINTAINER APPROVAL REQUIRED**: The PR is currently blocked solely by external infrastructure waiting for a TUR maintainer to click "Approve and run" on the workflows. Once approved, the checks will execute against the fixed recipe and validation will resume.
