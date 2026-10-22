# 005: Sub-millisecond grid placement

Date: 2026-10-22. Code: `09c8436b` plus the uncommitted `crates/ob-analysis/src/placement.rs`
and its wiring in `crates/ob-analysis/src/lib.rs`.

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo build --release -p ob-bench
target/release/ob-bench eval --quiet --jobs 3 --json target/ob-bench-v5.json
```

Baseline: `target/ob-bench-v3.json`, the same suite before the placement stage. Dataset:
the 40-track synthetic suite (note 001).

## Question

rbxport's grids land about 1 ms early on every quantized synthetic track (note 001).
`placement::refine` measures each beat's transient onset to a fraction of a millisecond,
takes the median offset, and moves the whole grid by it. The move happens only when at
least 16 beats have a clear transient, their spread (median absolute deviation) is at
most 1 ms, and the move is at most 4 ms. Does it remove the bias without disturbing
anything else?

## Results

openbox-normal and openbox-dsp-normal give identical figures:

| Measure | Before (v3) | After (v5) |
| --- | --- | --- |
| Median absolute signed error per track | 0.97 ms | 0.07 ms |
| Median p95 absolute error | 0.97 ms | 0.07 ms |
| Every exported beat within 2 ms ("max<=2") | 34 / 40 | 35 / 40 |
| openbox pass | 34 / 40 | 35 / 40 |
| Beat 1 right | 37 / 40 | 37 / 40 |
| Octave right | 37 / 40 | 37 / 40 |

The extra pass is a techno track whose grid was 1.8 ms late; it is now within 0.09 ms.

## A regression found and fixed

The first run (v4) left one beat about 0.9 ms late on each track that starts on a beat
at the file's first sample (the 8 `starts-at-zero` tracks). `anchor_file_start` puts that
beat in a one-beat opening segment at exactly 0 s, where it is already right, and the
stage moved it with the rest of the grid. The stage now leaves that segment alone. With
the fix, the worst beat on those tracks is 0.03 to 0.25 ms off, where it had been 0.74 to
1.00 ms. Nothing else changed between v4 and v5.

## Not changed

- **Hip-hop (2 tracks):** 17 ms and 4.4 ms bias before and after. These are tempo and
  phase errors that the stage does not try to fix: the spread rule keeps it from moving
  them.
- **Tempo edits (3 tracks):** the stage shifted these by about 0.9 ms, but their errors
  (212 to 428 ms worst case) come from wrong tempo segments. Within-2 ms on the ramp moved
  from 54.3 % to 54.1 %, one beat. This is task 3 in the handoff: one constant grid with
  tempo changes flagged.

## Still to check

The synthetic transients are clean. Real kicks have slower attacks and layered
transients. `ob-bench files` now prints the placement measurement (measured offset,
beats used, spread, and whether it moved), but has not yet been run on the masters of
note 003.

## The owner's rekordbox library

```sh
target/release/ob-bench library --jobs 4   # run outside the VS Code terminal sandbox
```

Dataset: the owner's rekordbox library on 2026-10-22 after the dead entries were
removed: 464 tracks with audio and a rekordbox grid (397 AIF, 41 AIFF, 20 MP3, 6 WAV).
39 tracks without a grid were skipped. Many tracks and their grids were imported from a
USB export that day. 748 s with 4 jobs.

rekordbox's grid is not ground truth here. Each grid is also judged against the audio
with the same detectors for all: the kick ratio, and the offset to the nearest kick
attack (rbxport's attack map, which itself reads about 1 ms early on the synthetic
truth, note 001).

### Placement

| Grid | Median attack offset | Grids more than 5 ms late | More than 5 ms early |
| --- | --- | --- | --- |
| rekordbox | -1.29 ms (late) | 28 | 5 |
| rbxport-normal | +0.05 ms | 1 | 3 |
| openbox-dsp-normal | -0.68 ms | 1 | 4 |
| openbox-normal | -0.66 ms | 1 | 4 |

- On 114 tracks openbox and rekordbox put the grid on the same beat at the same tempo,
  but 5 ms or more apart ("shifted"). On 105 of them the gap is under an eighth of a beat:
  the median gap is about 9 ms, with openbox earlier on most. On those tracks a median of
  64 % of openbox's beats have a kick attack within 5 ms, against 34 % of rekordbox's.
  This is the owner's complaint that rekordbox's lines miss the beat, measured.
- On the 308 tracks where the two agree within 5 ms, the gap is within plus or minus
  1.3 ms on the middle 60 %.
- openbox sits 0.7 ms after rbxport's attack map. On the synthetic truth the attack map
  is about 1 ms early, so this is consistent with openbox being right. Without a
  reference it does not prove it.

### Half beat

| Analyzer | Off-beat against rekordbox | Kick band sides with the analyzer | With rekordbox |
| --- | --- | --- | --- |
| rbxport-normal | 43 | 5 | 38 |
| openbox-dsp-normal | 26 | 13 | 13 |
| openbox-normal | 15 | 7 | 8 |

The neural judge removes most of rbxport's off-beat grids. The 15 that remain are split
evenly by the kick band, so they are candidates for the review queue, not clear errors.

### Beat 1

| Analyzer | Same beat as rekordbox | Same beat 1 too | Off by half a bar |
| --- | --- | --- | --- |
| rbxport-normal | 281 | 238 (85 %) | 17 |
| openbox-dsp-normal | 300 | 245 (82 %) | 19 |
| openbox-normal | 308 | 256 (83 %) | 32 |

Where the neural and DSP openbox grids are on the same beat but choose a different beat
1 (65 tracks), rekordbox agrees with the neural choice 28 times and with the DSP choice
26 times. The neural judge more than doubles the half-bar disagreements, the same flip
seen in note 004. rekordbox is not truth for beat 1 either, so this needs the owner's
ground truth on a sample.

### Tempo

All three analyzers differ from rekordbox's BPM on the same 27 tracks; this comes from
rbxport's tempo stage, which they share. 4 are fractional differences of up to 0.6 BPM.
The other 23 are ratio errors: half (9), double (2), two thirds (6), four fifths (3), and
one each of 3/4, 4/3 and 3/2. Most of these tracks are between 135 and 175 BPM by
rekordbox. On 13 of the 23 rekordbox's grid has the higher kick ratio. The
owner reports rekordbox's BPM as usually right, so these are openbox errors until shown
otherwise: 5 % of this library.

### MP3 offset

No MP3 shows the 1105-sample (about 25 ms) rekordbox 6 offset (0 of 16).

## Conclusions

1. The placement stage behaves on real music as on the synthetic suite: it does not move
   grids by more than a millisecond or so, and openbox's grids sit on far more kick
   attacks than rekordbox's on the tracks where the two differ.
2. **Tempo ratio errors are now the largest class of real failure** (23 of 464), larger
   than the remaining off-beat grids (15). They belong ahead of the CoreML work.
3. Beat 1 needs ground truth from the owner on a sample before the 0.6 neural weight
   changes.
