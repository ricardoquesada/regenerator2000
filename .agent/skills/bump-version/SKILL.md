---
name: bump-version
description: Automates the process of bumping the version and updating the changelog.
---

# Bump Version Skill

Use this skill when the user requests to bump the project version (e.g., "bump version to v0.8.0" or "prepare release x.y.z").

## 1. Verify the New Version

Ensure the user provided a valid semantic version (e.g., `0.8.0`). If they didn't specify one, determine the correct next version based on recent changes (major/minor/patch) or ask the user for clarification.

## 2. Update Cargo.toml

In `Cargo.toml`:

- Locate the `[package]` section.
- Update the `version = "..."` field to the new version.

## 3. Review Recent Changes

Run the following command to get a list of commits since the last tag:

```bash
git log $(git describe --tags --abbrev=0)..HEAD --oneline
```

Review the commit messages to understand the scope of what features, fixes, and other changes are included in this release.

## 4. Update CHANGELOG.md

In `CHANGELOG.md`:

- Create a new header section for the new version directly below the project headers or the `[Unreleased]` section.
- Use the format: `## [x.y.z] - YYYY-MM-DD` using the current date.
- Categorize the changes based on the commit history into the appropriate sections, such as:
  - `### Features`
  - `### Fixes`
  - `### Changes`
  - `### Refactor / Internal`
  - `### Documentation`
  - etc.
- Ensure the descriptions are concise and match the existing changelog formatting conventions.

## 5. Verify and Notify User

Summarize the updates made and notify the user that `Cargo.toml` and `CHANGELOG.md` have been successfully updated for the new version.
