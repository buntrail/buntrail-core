# Changelog

## 0.4.1 — Failed-push review

Align first-party dependencies with the corrective 0.4.1 Git sidebar release; no API or behavior changes in this crate.

## 0.4.0 — Git sidebar

Add explicit byte-preserving Git sidebar history scopes, summary and optional upstream/GitHub contracts. ABI 1 remains unchanged.

## 0.3.0

File-sidebar operations and compatible shared core contracts. C ABI remains version 1.

## 0.2.0 — Git Repository

Add byte-preserving repository, commit, history filter, branch action and merge outcome contracts. This breaks Rust struct/enum constructors; legacy JSON fields remain unchanged when new options are absent.

## 0.1.1

- Equalize same-direction terminal split groups after splitting or closing a pane.
- Preserve perpendicular groups, custom ratios outside the edited group and terminal identities.

## Unreleased — initial 0.1.0 preparation

- Extracted self-contained crate source, resources, documentation and checks.
- Added MIT licensing and versioned Buntrail dependencies.
- Publication and platform acceptance are tracked separately from source preparation.
