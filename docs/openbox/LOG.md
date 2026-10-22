# Progress log

Newest first. Each entry says what was done, what was measured, and what comes next.

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
