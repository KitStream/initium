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
3. Run `cargo check` to ensure the project builds successfully after the version bump.
4. Update `CHANGELOG.md`:
   - Move everything under `## [Unreleased]` into a new `## [X.Y.Z] - YYYY-MM-DD` section (use today's date).
   - Leave `## [Unreleased]` empty (with just the heading).
5. Run `cargo test` to verify nothing is broken.
6. Run `cargo clippy -- -D warnings` and `cargo fmt -- --check`.
7. Commit: `release: vX.Y.Z`
8. Push the branch and create a PR with title `release: vX.Y.Z`.
9. The PR body should include the changelog entries for this version.

When the PR merges, the auto-tag workflow detects the version bump in `Cargo.toml` and creates the `vX.Y.Z` tag, which triggers the release workflow (Docker build + crates.io publish).
