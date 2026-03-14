---
name: release
description: Release
user_invocable: true
---

# Release

Prepare a release PR for initium. This command handles version bumping, changelog updates, and PR creation. The release is published automatically when the PR merges (CI detects the version bump and creates the tag, which triggers the release workflow).

## Determine the next version number

Follow semantic versioning (MAJOR.MINOR.PATCH):

1. Read `CHANGELOG.md` under `## [Unreleased]` to see what has changed since the last release.
2. Read recent commits since the last tag: `git log $(git describe --tags --abbrev=0)..HEAD --oneline`
3. Determine the version bump:
   - **PATCH** (x.y.Z): Only bug fixes, documentation, or internal changes with no user-facing behavior change.
   - **MINOR** (x.Y.0): New features, new CLI flags, new configuration options, or backward-compatible enhancements.
   - **MAJOR** (X.0.0): Breaking changes — removed features, changed defaults, incompatible schema/config changes, or renamed CLI flags.
4. Read the current version from `Cargo.toml` and compute the next version.

## Confirmation phase

Before making any changes, present to the user:

- The **current version** and the **proposed next version** with reasoning.
- A **summary of changes** that will go into the release (from Unreleased changelog + commit log).
- Ask: "Proceed with version X.Y.Z?" and wait for confirmation.
- If the user suggests a different version, use that instead.

## Execute the release

Once confirmed:

1. Fetch origin and create a branch: `release/vX.Y.Z` from `origin/main`.
2. Bump version in `Cargo.toml` (the `version = "..."` field under `[package]`).
3. Run `cargo check` to ensure the project builds successfully after the version bump. This also updates `Cargo.lock` — always include `Cargo.lock` in the release commit.
4. Update `CHANGELOG.md`:
   - Move everything under `## [Unreleased]` into a new `## [X.Y.Z] - YYYY-MM-DD` section (use today's date), placed immediately below `## [Unreleased]` (i.e., at the top of the released versions list).
   - Leave `## [Unreleased]` empty (with just the heading).
   - Each released version must have its own section with its changes — never merge entries across versions.
5. Search all documentation files (`docs/`, `README.md`, `examples/`) and the `Makefile` for references to the previous version (e.g. image tags like `initium:1.3.1`, version strings, cosign `--certificate-identity` tag refs in `docs/security.md`) and update them to the new version. Exclude `CHANGELOG.md` (historical entries should keep their original versions).
6. Run `cargo test` to verify nothing is broken.
7. Run `cargo clippy -- -D warnings` and `cargo fmt -- --check`.
8. Commit all changed files (`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, docs): `release: vX.Y.Z`
9. Push the branch and create a PR with title `release: vX.Y.Z`.
10. The PR body should include the changelog entries for this version.

When the PR merges, the auto-tag workflow detects the version bump in `Cargo.toml` and creates the `vX.Y.Z` tag, which triggers the release workflow (Docker build + crates.io publish).
