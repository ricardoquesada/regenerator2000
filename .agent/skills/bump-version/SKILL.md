---
name: bump-version
description: >-
  Bumps workspace crate versions and writes user-facing CHANGELOG.md release
  notes in Simplified Technical English (ASD-STE100). Use when the user asks to
  bump the version or prepare a release (for example, /bump-version v0.9.26).
  Don't use for git commits or tags unless explicitly requested.
---

# Bump Version Skill

## 1. Verify the New Version

Check that the user provided a valid semantic version (strip any leading `v`,
such as `v0.9.26` -> `0.9.26`). If the user omitted the version, ask for
clarification first.

## 2. Update Cargo.toml and Cargo.lock

Update the `version` field in all three workspace manifests:

- Root `Cargo.toml`:
  - Update `[package].version`.
  - Update `regenerator2000-core` and `regenerator2000-tui` in `[dependencies]`.
- `crates/regenerator2000-core/Cargo.toml`:
  - Update `[package].version`.
- `crates/regenerator2000-tui/Cargo.toml`:
  - Update `[package].version`.
  - Update `regenerator2000-core` in `[dependencies]`.

Run `cargo check --workspace` so `Cargo.lock` updates to the new version.

## 3. Review Recent Changes

List all commits since the previous release tag:

```bash
git log $(git describe --tags --abbrev=0)..HEAD --format='=== %h %s ===%n%b'
```

Group the changes by user impact rather than one bullet per commit. Omit fixes
for bugs introduced in the same unreleased cycle.

## 4. Update CHANGELOG.md (User-Facing "What" + ASD-STE100)

Insert `## [x.y.z] - YYYY-MM-DD` (with the current date) above the previous
release header in `CHANGELOG.md`. Group entries under `### Features`,
`### Fixes`, `### Performance`, or `### Documentation`.

Apply the `asd-ste100` skill (when available) and enforce both rules below:

1. **Focus on "What", Not "How"**: `CHANGELOG.md` serves users of Regenerator
   2000, not maintainers who read `git log`. State user-visible capabilities,
   supported formats or protocols, UI behavior, keyboard shortcuts, MCP tools,
   and fixed symptoms. Omit internal Rust struct names, helper functions,
   module paths, and implementation mechanics.
2. **Use ASD-STE100 Simplified Technical English**:
   - Start entries with active imperative verbs (`Support`, `Add`, `Sync`,
     `Fix`, `Keep`).
   - Keep sentences short (`<= 20` words for actions, `<= 25` words for
     descriptions), with one main idea per sentence.
   - Use simple tenses only. Avoid `-ing` participles, passive voice, phrasal
     verbs (`fall back`, `spin up`), prose semicolons, and filler adjectives
     (`comprehensive`, `seamless`, `robust`).

### Example

- **Avoid ("how" + internal symbols)**:
  `Add startup probing in terminal_graphics with Iterm2CellProtocol Retina 2x
  OSC 1337 PNGs and KittyDoubleBufferProtocol ID flips.`
- **Prefer ("what" + ASD-STE100)**:
  `Support Kitty, iTerm2, and Sixel terminal graphics so the About dialog and
  Bitmap pane display high-resolution images.`

## 5. Verify and Notify User

Verify that no previous version strings remain in `Cargo.toml` or `Cargo.lock`.
Summarize the updated files, and do not commit or tag unless asked.
