# Changelog
## 0.1.2 (2026-02-10)

### Features

- add Claude Code plugin with skills, commands, and agent

## 0.1.1 (2026-02-08)

### Features

- implement mcplug CLI and library crate
- add GitHub Actions CI and release workflows
- set up knope for PR-based release management
- add trusted publishing to crates.io in release workflow
- upgrade rand 0.8→0.9 and reqwest 0.12→0.13

### Fixes

- gate POSIX-only code with cfg(unix) and resolve all clippy warnings
- install aarch64 OpenSSL dev libs for linux cross-compilation
- use vendored OpenSSL for aarch64-linux cross-compilation
- use single glob string for knope assets config
- add license and repository metadata to Cargo.toml
