## What does this PR do?

<!-- A concise description of the change and the motivation behind it. -->

Fixes # <!-- Issue number, e.g. Fixes #12 -->

---

## Type of change

- [ ] 🐛 Bug fix (non-breaking)
- [ ] ✨ New feature (non-breaking)
- [ ] 💥 Breaking change (existing behaviour changes)
- [ ] 📝 Documentation / comments only
- [ ] 🔒 Security fix
- [ ] ⚡ Performance improvement
- [ ] 🧹 Refactor / clean-up (no behaviour change)

---

## Checklist

### Rust (if applicable)
- [ ] `cargo clippy -- -D warnings` passes with zero diagnostics
- [ ] `cargo test` passes (all crates)
- [ ] `cargo build --release` succeeds
- [ ] `cargo fmt --check` passes
- [ ] No `unwrap()` / `expect()` in library code (only `main.rs` / `#[cfg(test)]`)
- [ ] Every new public function has at least one unit test
- [ ] Every new command has `--json` output

### TypeScript (if applicable)
- [ ] `pnpm typecheck` passes
- [ ] `pnpm build` succeeds

### Documentation
- [ ] `omnicli/docs/USAGE.md` updated if command syntax changed
- [ ] `README.md` updated if user-facing behaviour changed
- [ ] `CHANGELOG.md` entry added under `## [Unreleased]`

---

## Test plan

<!-- Describe what you tested and on which platforms. Paste key command output. -->

**Platform tested:** <!-- e.g. Ubuntu 22.04, Termux aarch64, Kali Linux -->

```bash
# paste relevant command output here
```

---

## Screenshots / output (if UI or output format changed)

<!-- Delete if not applicable -->
