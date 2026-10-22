# 0001: Build openbox as a fork of rbxport

Date: 2026-10-21. Status: accepted.

## Context

The goal is a rekordbox replacement for gig prep, with a much better beat-grid analyzer
and Lexicon-class library features. rbxport (chrisle/rbxport, GPL-2.0-or-later) already
provides, in Rust and Tauri 2:

- the device formats (`export.pdb`, OneLibrary, ANLZ)
- LINK Export
- guarded `master.db` writes
- backups
- a fast React UI

Its README says its vision is "fewer features" and invites forks for more.

## Decision

Fork it as github.com/Calvin-Zikakis/openbox. openbox code lives in `ob-*` crates and
`docs/openbox/`. Upstream files get only thin hooks, listed in
[upstream-hooks.md](../upstream-hooks.md). Merge upstream regularly.

## Consequences

- openbox is GPL-2.0-or-later.
- Phase 1 of the original plan (format writers) and most UI work are inherited.
- The fork's value is the analyzer and the features upstream declines. It must keep up
  with a fast-moving upstream (about 1,500 commits in its first four weeks).
