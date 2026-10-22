# Progress log

Newest first. Each entry says what was done, what was measured, and what comes next.

## 2026-10-22 (repository renamed)

- The GitHub repository is now Calvin-Zikakis/rbxport-beat-analysis, and the local
  checkout moved to `~/repositories/rbxport-beat-analysis`. `origin` points to the new
  URL. The project, crates (`ob-*`) and app mode keep the name openbox.
- The rustup directory override is keyed by path, so it was set again for the new
  folder (`rustup override set 1.99.0-aarch64-apple-darwin`).

**Library comparison (in progress)**

- The owner's rekordbox library on this Mac is now usable for `ob-bench library`: dead
  entries were removed and more tracks were imported from USB, keeping rekordbox's grids.
- A 20-track sample (before the cleanup) put rekordbox's grids about 1.1 ms late on the
  kick attack (median); openbox with the neural judge was about 0.5 ms late, rbxport
  about 0. Where openbox and rekordbox disagreed by half a beat (3 tracks), the kick band
  sided with openbox each time. Too small to conclude anything.
- A full run on 464 tracks (0 missing, 39 without a rekordbox grid) was started with
  `nohup target/release/ob-bench library --jobs 4`, writing
  `target/ob-library/summary-full.txt` (no titles) and `target/ob-library/results.jsonl`
  (private, names tracks). It must run outside the VS Code terminal sandbox, which hides
- The full run finished: 464 tracks in 748 s. Results are in
  [research note 005](research/005-placement.md). In short: where openbox and rekordbox
  differ by 5 to 25 ms on the same beat (105 tracks), 64 % of openbox's beats sit within
  5 ms of a kick attack against 34 % of rekordbox's. The neural judge cuts off-beat
  disagreements with rekordbox from 43 (rbxport) to 15. Beat 1 agrees with rekordbox on
  83 %, about as often as rbxport. **23 tracks (5 %) have a tempo ratio error** (half,
  two thirds, four fifths) from the shared tempo stage, now the largest failure class.
  `cargo clippy -p ob-ml -p ob-analysis -p ob-bench --release --all-targets` is clean.

**Placement stage measured**

- [Research note 005](research/005-placement.md). On the synthetic suite the median
  signed error fell from 0.97 ms to 0.07 ms, and "max<=2" rose from 34 to 35 of 40.
  Beat 1 (37) and octave (37) are unchanged.
- Fixed a regression the first run showed: on tracks that start on a beat at 0 s, the
  stage moved `anchor_file_start`'s one-beat opening segment about 0.9 ms late. It now
  leaves that segment where it is.
- `ob-bench files` now prints the placement measurement. Not yet run on real masters.

## 2026-10-21 (paused here)

Where things stand when work paused. Everything below this entry is committed and pushed
to `origin/main` (last commit `09c8436b`). The work in this entry is **not committed**.

**App integration (uncommitted, built and tested)**

- A third analysis mode, "openbox (Precise Grid)", in the Analysis Setting dialog and in
  Preferences > Track Analysis, and the fork's default mode. It runs `ob-analysis` on
  rbxport's Normal (constant-grid) options. The result goes through rbxport's own ANLZ,
  `master.db` and USB-export writers unchanged.
- Rust: `src-tauri/src/analysis.rs` maps mode `"openbox"` to `Engine::Openbox`. The
  engine travels in `AnalysisSettings` as a field the frontend never sends, so
  `analyse_and_save`'s signature and its upstream tests are unchanged. `analyse_file`
  remains as a test-only wrapper around `analyse_file_with`. A new test,
  `the_openbox_engine_grids_the_fixture_track_as_rbxport_does`, was added.
- Frontend: `AnalysisMode` gains `"openbox"` in `src/lib/preferences.ts`, along with
  validation, the default, the remembered dialog choice, the IPC type and the two
  selectors.
- Checks run: `tsc --noEmit` passes, and `cargo clippy -p rbxport --all-targets` is clean.
  The vitest run of the affected folders shows 15 failures in 5 files, but **the same 15
  fail on a clean tree**. They are `localStorage` and React `act` tests, consistent with
  Node 26 on this machine, while the project pins Node 24.
- `RB_LITE_TEST=1 cargo test -p rbxport --lib analysis::` passes: 8 of 8, including the
  new openbox engine test.
- The models are found through `OPENBOX_MODELS`, `./models`, or the workspace's
  `models/` at build time. That works for a development build on this machine. A
  packaged app will need to ship or download them.

**Placement refinement (uncommitted, wired in, not measured)**

- `crates/ob-analysis/src/placement.rs` measures each beat's transient onset at
  sub-millisecond resolution, takes the median offset, and moves the whole grid by it
  when at least 16 beats agree within 1 ms (move capped at 4 ms). It moves segment
  boundaries with the beats, and keeps beat 1 by time.
- It is wired in: `ob-analysis::analyse` calls `placement::refine` after
  `anchor_file_start`, and `Report.placement` carries its measurement.
  `cargo clippy -p ob-analysis -p ob-bench --release --all-targets` is clean.
- **Not measured yet.** Run `ob-bench eval`. The target is the synthetic suite's 1 ms
  early bias going to about 0.2 ms, with no change in downbeats.

**Needs the owner**

- Where beat 1 is on "ROUZ - The One" and "ROUZ & Wyllie - Perceive" (research note
  004). Both are flagged as conflicts between structure and the neural model.
- The drive or rekordbox USB stick holding the DJ library, for `ob-bench library`.
- Whether to commit and push the uncommitted work above as it stands.

**To resume**

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
git status                                             # the uncommitted files listed above
RB_LITE_TEST=1 cargo test -p rbxport --lib analysis::  # confirm the app tests
cargo build --release -p ob-bench && target/release/ob-bench eval --quiet --jobs 3
```

Then measure the placement stage, tune the CoreML provider settings, and make one
constant grid the default with tempo changes flagged for review. The full handoff for
another agent is [HANDOFF.md](HANDOFF.md).

## 2026-10-21 (evening)

**Neural judge**

- `crates/ob-ml`: Beat This! activations (MIT, pure-Rust inference), with models fetched
  and checksum-checked by `scripts/openbox/fetch-models.sh`. Decision
  [0003](decisions/0003-neural-judge.md), research note [004](research/004-neural-judge.md).
- On the true synthetic grids it is right 40 of 40 on the half beat and 40 of 40 on the
  downbeat (full model).
- `ob-analysis` lets it decide the half beat when decisive, and blends it with structure
  for beat 1, with `bar_conflict` and `half_beat_conflict` flags.
- On real masters it moves one off-beat grid (ROUZ - The One) back onto the kick, and
  changes beat 1 on two of the owner's tracks. Those two are flagged as conflicts, and
  only the owner can confirm them.
- Cost: four 20-second excerpts, about 1.5 s per track on the CPU.

**Needs the owner**

- Where beat 1 is on "ROUZ - The One" and "ROUZ & Wyllie - Perceive" (see the end of
  note 004).
- The drive or USB stick holding the DJ library, for `ob-bench library`.

**GPU, first attempt**

- CoreML through ONNX Runtime 1.24.2 (`scripts/openbox/fetch-onnxruntime.sh`, vendored,
  git-ignored) works, with outputs within 0.013 of the CPU's. But it runs 2.4 times
  *slower* with default provider settings, so it is opt-in
  (`OPENBOX_NEURAL_BACKEND=coreml`). Tuning the provider options is a follow-up.

**Next**

1. Tune the CoreML provider settings (MLProgram, compute units, static shapes, cache).
2. The 1 ms placement bias.
3. Make the openbox analyzer selectable in the app.
4. Default to one constant grid, and flag possible tempo changes.

## 2026-10-21 (later)

**Real music**

- `ob-bench library` compares analyzers against the grids in the installed rekordbox
  library, reading only a copy of `master.db`. **Blocked:** 929 of the 947 analyzed
  tracks' files are not on this machine. Needs the drive with the DJ collection, or a
  rekordbox USB export.
- `ob-bench files` runs every analyzer on any audio files. Research note
  [003](research/003-real-masters.md), on 11 real masters plus 6 renders of one: openbox
  and rbxport agree on every Normal-preset grid. Tempo following split 2 constant
  masters into segments. One track is a genuine half-beat coin flip.

**Next**

1. Confidence and flags in `ob-analysis` (half-beat ambiguity, bar ambiguity, possible
   tempo change), and default to one constant grid.
2. A better kick-versus-bass judge for the ambiguous case, including neural beat
   activations (Beat This!).
3. The 1 ms placement bias.
4. Make the openbox analyzer selectable in the app.

## 2026-10-21

**Fork and setup**

- Forked chrisle/rbxport to github.com/Calvin-Zikakis/openbox. `upstream` points to
  chrisle/rbxport. This folder is the checkout. Decision
  [0001](decisions/0001-fork-rbxport.md).
- Rust: this Mac's rustup default host is x86_64 with no Rosetta, so x86 toolchains
  cannot run. Added a directory override:
  `rustup override set 1.99.0-aarch64-apple-darwin` (matches `rust-toolchain.toml`). The
  global default is unchanged; consider `rustup set default-host aarch64-apple-darwin`.
- rbxport's analysis tests pass on this machine (33 tests).

**Benchmark (`crates/ob-bench`)**

- Synthetic tracks with exact ground truth across 9 styles, plus scoring at full
  precision and at USB-export precision. Placement and numbering are scored
  separately. See the [crate README](../../crates/ob-bench/README.md).
- Baseline: research note [001](research/001-synthetic-baseline.md). rbxport is about
  1 ms early everywhere. Its BPM is right. Its main failure is an off-beat grid on
  house music (3 of 10).

**Analyzer (`crates/ob-analysis`)**

- openbox's analyzer: rbxport's stages with separate half-beat and bar judges.
  Decision [0002](decisions/0002-separate-half-beat-and-bar-judges.md), research note
  [002](research/002-half-beat-judge.md). On synthetic tracks, beat 1 is right on 36 of
  40 instead of 33, with no regressions.
- Upstream hooks: 6 items made `pub` in `rbl-analysis`, no behavior change
  ([upstream-hooks.md](upstream-hooks.md)).

**Checks for this state**

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo clippy -p ob-analysis -p ob-bench -p rbl-analysis --release --all-targets
RB_LITE_TEST=1 cargo test -p rbl-analysis --release
cargo run --release -p ob-bench -- eval --quiet
```

**Next**

1. Research note 003: compare rbxport and openbox with the grids in the local rekordbox
   6 library (947 analyzed tracks), read-only. Check the rekordbox 6 MP3 offset of 1105
   samples. Measure how often each analyzer lands on the off-beat, and record kick
   ratios to calibrate the threshold of 2.
2. Remove the 1 ms early placement bias with sub-sample attack refinement.
3. Neural beat and downbeat evidence (Beat This!) as a third judge, for soft kicks,
   broken beats and tempo changes.
4. Confidence score and flags from the phase report, for the review queue.
