# 006: A neural judge for tempo ratio errors

Date: 2026-10-22. Code: `ffa174d5` plus `crates/ob-analysis/src/octave.rs`, the
`detect_tempo_at` hook in `rbl-analysis`, and the stricter agreement rule in
`placement.rs`.

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo build --release -p ob-bench
target/release/ob-bench eval --quiet --jobs 3 --json target/ob-bench-v8.json
target/release/ob-bench library --jobs 4      # outside the VS Code sandbox
```

Datasets: the 40-track synthetic suite (note 001), and the owner's 464-track rekordbox
library (note 005). rekordbox's BPM is treated as right, as the owner reports it usually
is.

## Question

Note 005 found 23 of 464 library tracks with a tempo that is a simple ratio of
rekordbox's (half, two thirds, four fifths). The tempo comes from rbxport's candidate
stage, which ranks autocorrelation peaks with a prior centred at 132 BPM. Can the spacing
of Beat This!'s beats, which the analyzer already computes, choose better?

## Method

`octave::judge` takes the neural beats from the activations (peaks of at least 0.5,
refined between frames), measures the median spacing and the share of intervals within
10 % of it (consistency), and compares the resulting tempo with the DSP's. If it is a
simple ratio of the DSP tempo other than 1, and consistency is high enough, the grid is
refitted at the DSP tempo times that ratio through rbxport's own fit. The kick attacks
still place the beats; only the tempo moves.

## First version: no net gain

Run on the library with every ratio and a consistency of 0.7, then replayed offline from
the recorded tempos (`ob-bench library` records the DSP tempo, neural tempo and
consistency per track):

| Rule | Fixed (DSP wrong, rule right) | Broken (DSP right, rule wrong) |
| --- | --- | --- |
| any ratio, consistency 0.5 | 13 | 18 |
| any ratio, consistency 0.7 | 11 | 12 |
| any ratio, consistency 0.9 | 4 | 11 |

On the synthetic suite it also broke a half-time dubstep track (`dubstep-140` went to 70
BPM).

## What the data says

| Neural tempo against the DSP's | Tracks | rekordbox agrees with |
| --- | --- | --- |
| same | 408 | the DSP on 403, neither on 5 |
| half | 17 | the DSP on all 17 |
| double | 10 | the neural tempo on 7, the DSP on 2, neither on 1 |
| 1.5 times | 5 | the neural tempo on all 5 |

The main cases; 24 more tracks have a neural tempo that is unrelated to the DSP's or a
less common ratio (two thirds, three quarters, 1.25, 4/3), or none at all.

When the neural tempo is faster than the DSP's by double or 1.5 times, it is usually
right (12 of 15). When it halves a tempo of 139 BPM or more, it is wrong every time: on
drum and bass and dubstep it hears half-time. No refit that landed between 90 and 180
BPM broke a tempo the DSP had right.

## Rule adopted

Refit only when the result lies between 90 and 180 BPM and consistency is at least 0.6.
Replayed offline, that fixes 13 tracks and breaks none. The 90 BPM floor is what
matters: with consistency 0.6, floors of 85 and of 80 each break 6.

## Results

Synthetic suite: identical to the run without the judge on all 40 tracks (octave 37,
beat 1 37, openbox pass 35, median error 0.07 ms).

Library, openbox-normal:

| Measure | Before | After |
| --- | --- | --- |
| BPM differs from rekordbox by more than 0.05 | 27 | 15 |
| Final BPM within 1.5 % of rekordbox's | 441 | 454 |
| Same beat as rekordbox | 308 | 311 |
| Same beat 1 as rekordbox | 256 | 260 |

All 13 corrected tracks now have rekordbox's tempo and the same kick ratio as
rekordbox's grid. rbxport and openbox without the neural model are unchanged at 27.

About 11 ratio errors remain, besides 4 fractional differences of up to 0.6 BPM. In about
half of them the neural beats are not consistent enough (consistency near or under 0.6);
in the others the neural model agrees with the DSP, or would land outside 90 to 180 BPM.

## Side effect: placement agreement

The placement stage now moves the grid only when at least two thirds of the beats
sit within 1 ms of the median offset. The earlier test, a median absolute deviation of
1 ms or less, passes two equal clusters, as a flam or a clap layered on the kick makes,
where the median is just one of them. A unit test covers it. On the synthetic suite it
changes nothing. On the library, 6 tracks crossed the 5 ms boundary that separates
"same" from "shifted" in rekordbox comparison, 4 one way and 2 the other, all by 0.2 to
2.3 ms. One track lost a kick-attack match (0.69 to 0.34 of beats within 5 ms) because
the stricter rule no longer moves its grid.

## Limits

- The library has 464 tracks; the 13 fixes come from one library of mostly 140 to 175 BPM
  music. The rule may not hold on slower genres. Re-measure on other collections.
- rekordbox's BPM is the reference, not truth.
- The judge needs the neural model. Without it, tempo is unchanged.
