# ob-analysis

openbox's track analyzer. It runs rbxport's analysis stages from `rbl-analysis`: onset
and kick-band envelopes, the kick-attack tempo fit, band frames, key and waveform. It
makes its own decisions about where the grid sits, and returns rbxport's `Analysis`
type, so ANLZ writing, `master.db` and USB export take it unchanged.

```rust
let result = ob_analysis::analyse(&mono_samples, sample_rate, rbl_analysis::AnalysisPreset::Rbxport.options());
let analysis = result.analysis; // rbl_analysis::Analysis
let report = result.report;     // the evidence behind the grid
```

## What differs from rbxport

| Stage | rbxport | openbox |
| --- | --- | --- |
| On the beat or off it | the half-beat position where the arrangement changes most | the kick band decides when decisive (at least 2x more kick onset energy on one side); structure decides otherwise, by total change on beats against between them |
| Which beat is 1 | the same vote | each beat's structure score plus half of each neighbouring half beat's, so a change straddling the downbeat still votes for it |

See [`src/phase.rs`](src/phase.rs) and the research note
[002](../../docs/openbox/research/002-half-beat-judge.md).

## Checks

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo clippy -p ob-analysis --release --all-targets
cargo run --release -p ob-bench -- eval --quiet
```
