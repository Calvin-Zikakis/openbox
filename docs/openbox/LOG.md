# Progress log

Newest first. Each entry says what was done, what was measured, and what comes next.

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
