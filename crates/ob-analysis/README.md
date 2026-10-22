# ob-analysis

openbox's track analyzer. It runs rbxport's analysis stages from `rbl-analysis`: onset
and kick-band envelopes, the kick-attack tempo fit, band frames, key and waveform. It
makes its own decisions about where the grid sits, and returns rbxport's `Analysis`
type, so ANLZ writing, `master.db` and USB export take it unchanged.

```rust
let options = ob_analysis::Options::preset(rbl_analysis::AnalysisPreset::Rekordbox);
let result = ob_analysis::analyse(&mono_samples, sample_rate, &options);
let analysis = result.analysis; // rbl_analysis::Analysis
let report = result.report;     // the evidence behind the grid, and its flags
```

`Options::preset` uses the neural judge when its models are installed (see
[`ob-ml`](../ob-ml/README.md)); `.without_neural()` turns it off.

## What differs from rbxport

| Stage | rbxport | openbox |
| --- | --- | --- |
| On the beat or off it | the half-beat position where the arrangement changes most | the neural model decides when decisive (a ratio of at least 2), then the kick band when decisive, then structure (total change on beats against between them) |
| Which beat is 1 | the same vote | structure, with each beat's score including half of each neighbouring half beat's, blended 0.4 / 0.6 with the neural downbeat. Flagged when they disagree |

See [`src/phase.rs`](src/phase.rs) and research notes
[002](../../docs/openbox/research/002-half-beat-judge.md) and
[004](../../docs/openbox/research/004-neural-judge.md).

## Checks

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo clippy -p ob-analysis --release --all-targets
cargo run --release -p ob-bench -- eval --quiet
```
