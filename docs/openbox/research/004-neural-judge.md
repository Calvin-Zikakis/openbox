# 004: Beat This! as a third phase judge

Date: 2026-10-21. Machine: Apple M3 Pro, CPU inference (`rten`).

```sh
scripts/openbox/fetch-models.sh --full
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo build --release -p ob-bench
target/release/ob-bench neural                    # judge the true synthetic grids
target/release/ob-bench neural --analyzer small   # the same with the small model
target/release/ob-bench neural-speed FILE         # inference time by audio length
target/release/ob-bench eval --quiet --jobs 3
target/release/ob-bench files [--same-song] FILE...
```

## Question

The DSP judges (kick band and structure) split on one real track: the kick band could
not tell kick from bass, and structure decided 1.96 against 2.04. Can a neural beat
tracker settle these cases, and at what cost?

## Model

[Beat This!](https://github.com/CPJKU/beat_this) (ISMIR 2024, MIT), run as ONNX through
the `beat-this` crate (MIT, pure-Rust `rten` backend). openbox uses only its per-frame
beat and downbeat probabilities (50 frames per second), never its picked beats. Grid
placement stays on kick attacks, which are far finer than a 20 ms frame.

## How good it is on known grids

Scored on the true grid of each of the 40 synthetic tracks:

| Model | Beats on the true beats, not midpoints | Downbeat on the true bar position | Time per track |
| --- | --- | --- | --- |
| Full (`beat_this.onnx`, 83 MB) | 40 / 40 (ratios 3.9 to 415) | 40 / 40 | about 5 s for the whole track |
| Small (`beat_this_small.onnx`, 10 MB) | 40 / 40 | 38 / 40 | about 3 s |

It is right even on the hip-hop tracks, where the DSP fails.

## Cost, and excerpts

Inference time grows faster than the input. The full model on one track:

| Audio | 8 s | 10 s | 15 s | 20 s | 25 s | 30 s | whole (378 s) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| ms | 127 | 161 | 262 | 387 | 515 | 1,262 | 8,433 |

A 30-second input spills into a second model chunk. openbox therefore runs the full model
on **four 20-second excerpts**, the loudest 20 seconds of each quarter of the track
(where the drums play), or on the whole track when it is under 100 seconds. That costs
about 1.5 s per track. On the synthetic suite with three parallel jobs, throughput is
about 1.2 s per track. Inference runs beside the DSP stages, on all cores.

## Fusion (in `ob-analysis::phase`)

- **Half beat:** if the neural ratio (beat activation on beats over midpoints) is at
  least 2 or at most 0.5, the neural model decides. Otherwise the kick band decides
  when decisive, then structure. A decisive kick band and a decisive neural model that
  disagree raise `half_beat_conflict`. This has never happened so far.
- **Beat 1:** the structure scores and the neural downbeat means are each normalized to
  sum to 1, then blended 0.4 / 0.6. When the two judges' own picks differ, the track is
  flagged (`bar_conflict`), and both candidates are kept for the review screen.

## Results

Synthetic suite: no change. openbox without the neural judge already gets every
downbeat a constant grid can get (37 of 40; the 3 misses are the tempo-change edits).

Real masters (11 tracks plus renders), Normal preset:

| Track | DSP openbox and rbxport | With the neural judge |
| --- | --- | --- |
| ROUZ - The One | grid on the off-beat (kick ratio 0.93 after structure moved it) | kept on the fitted grid: neural ratio about 170,000 on the beat, kick ratio 1.08. Beat 1 moves; flagged (structure 3, neural 0) |
| Perceive (5 renders) | beat 1 at structure's position 3, consistent | beat 1 one beat later, consistent across all 5 renders. Flagged: structure 3, neural 0 or 2 depending on the render |
| 8 other tracks | | identical grid and beat 1 |

The neural half-beat verdicts are emphatic: ratios of at least 54, or about 0.00 for a
grid that must move. On every track where the kick band is decisive, they agree with
it.

The neural *downbeat* is less stable on real music. Between renders of one song, and
between excerpt choices, it can swing between positions 0 and 2 (beat 1 against beat
3), a known half-bar weakness of neural downbeat trackers on electronic music. Weighted
at 0.6 it still decided consistently across renders here, but these are exactly the
tracks to put in the review queue.

In both beat-1 conflicts, structure picked the beat *before* the neural model's choice,
and the neural model gave structure's pick almost no downbeat probability (0.12 to 0.15).
rbxport documents this pattern: novelty peaking one beat early, at a fill or riser into
the phrase. The blend sides with the neural model. **Unverified:** it needs ground truth.
Both conflicting tracks are the library owner's own productions, so they can say where
beat 1 is.

## GPU: CoreML through ONNX Runtime

`ob-ml` can also run inference through Microsoft's ONNX Runtime 1.24.2 build for Apple
silicon (`scripts/openbox/fetch-onnxruntime.sh`) with its CoreML provider. With the
provider's default settings, as the `beat-this` crate sets them:

| Model, input | CPU (`rten`) | CoreML |
| --- | --- | --- |
| Full, whole 378 s track | 8.4 s | 20.1 s |
| Full, 20 s | 0.39 s | slower at every length tried |

The outputs agree to within 0.013 in probability over 18,901 frames, so the path is
correct, only slow. ONNX Runtime's default CoreML settings commonly send unsupported
operators back to the CPU and recompile for each new input shape. CoreML therefore stays
opt-in (`OPENBOX_NEURAL_BACKEND=coreml`) until the provider's options (MLProgram model
format, compute units, static shapes, compiled-model cache) have been tried.

## Next

- Ground truth for beat 1 on real tracks (owner verification, then the DJ library).
- GPU inference (ONNX Runtime with CoreML) to bring the neural judge's cost below the
  DSP's.
