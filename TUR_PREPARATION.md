# Termux User Repository (TUR) Packaging Plan
**NOT FOR SUBMISSION - PREPARATION TEMPLATE ONLY**

This document outlines the exact packaging recipe for `omnicli` in the Termux User Repository (TUR).

Because TUR requires an immutable source archive and a real SHA-256 checksum to guarantee deterministic builds, the final `build.sh` script **MUST NOT** be generated until the official GitHub release tag is created.

## Target Location
Once the release is cut, create this file in the `termux-user-repository/tur` fork at:
`tur/omnicli/build.sh`

## `build.sh` Template

```bash
TERMUX_PKG_HOMEPAGE="https://github.com/Manash07Bhoi/OmniCLI"
TERMUX_PKG_DESCRIPTION="Professional-grade, full-stack command-line toolkit for file ops, search, conversion, and archives"
TERMUX_PKG_LICENSE="MIT"
TERMUX_PKG_MAINTAINER="Manash07Bhoi <no-reply@github.com>"
# IMPORTANT: Update TERMUX_PKG_VERSION to the exact SemVer tag (e.g. "0.1.0" or "1.0.0") without the 'v'
TERMUX_PKG_VERSION="<ACTUAL_RELEASE_VERSION_HERE>"
TERMUX_PKG_SRCURL="https://github.com/Manash07Bhoi/OmniCLI/archive/refs/tags/v${TERMUX_PKG_VERSION}.tar.gz"
# IMPORTANT: Calculate the SHA-256 checksum of the downloaded tar.gz archive and paste it below
TERMUX_PKG_SHA256="<ACTUAL_64_CHARACTER_SHA256_HERE>"
TERMUX_PKG_BUILD_IN_SRC=true
TERMUX_PKG_AUTO_UPDATE=true
TERMUX_PKG_DEPENDS=""
TERMUX_PKG_BUILD_DEPENDS="rust"

termux_step_make() {
    cargo build --jobs $TERMUX_PKG_MAKE_PROCESSES --target $CARGO_TARGET_NAME --release --manifest-path omnicli/Cargo.toml
}

termux_step_make_install() {
    install -Dm755 omnicli/target/${CARGO_TARGET_NAME}/release/omnicli -t $TERMUX_PREFIX/bin/
}
```

## Procedure to Finalize

1. Ensure the Git tag (e.g., `v0.1.1`) has been pushed to GitHub.
2. Download the source archive:
   ```bash
   curl -sL https://github.com/Manash07Bhoi/OmniCLI/archive/refs/tags/v0.1.1.tar.gz -o omnicli.tar.gz
   ```
3. Calculate the true SHA-256 checksum:
   ```bash
   sha256sum omnicli.tar.gz
   ```
4. Copy the `build.sh` template into the `tur` repository, replacing `<ACTUAL_RELEASE_VERSION_HERE>` and `<ACTUAL_64_CHARACTER_SHA256_HERE>`.
5. Test the build in the TUR Docker container (if available).
6. Submit the PR to the `termux-user-repository/tur` repository.
