# 002: Why house grids land on the off-beat, and the fix

Date: 2026-10-21.

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo run --release -p ob-bench -- probe --filter house
cargo run --release -p ob-bench -- eval --quiet
```

## Cause

rbxport fits its grid on kick attacks, then a structure stage decides two things with
one vote: whether the grid is on the off-beat, and which beat is 1. It scores the eight
half-beat positions in a bar by where the arrangement changes most. If an odd position
(a midpoint) wins, it moves every segment half a beat.

`ob-bench probe` shows, on all three failing tracks:

- **The kick-attack fit was right:** phase 0.998 of a beat, which is the 1 ms early
  bias.
- **The structure stage moved it.** Its scores come in pairs, the downbeat and the half
  beat after it, because the change straddles them: the off-beat bass enters on the
  "and" of beat 1. The midpoint won by small margins: 1.62 vs 1.24, 1.48 vs 0.98, and
  1.42 vs 1.25.
- **The kick band was decisive:** about 3.8x more low-band onset energy on the grid's
  beats than on the midpoints (364 vs 95, 375 vs 96, 419 vs 111).

## Fix (in `ob-analysis::phase`)

The two questions are answered separately:

1. **On the beat or off it.** The kick band decides when its ratio is at least 2 in
   either direction. Otherwise structure decides, by comparing the total change on
   beats with the total between them, not the single best position. If neither has
   evidence, the fitted grid is kept.
2. **Which beat is 1.** Each beat scores its own position plus half of each
   neighbouring half beat. A change straddling the downbeat then votes for the
   downbeat.

## Results (40 synthetic tracks)

| Measure (of 40) | rbxport | openbox | rbxport-normal | openbox-normal |
| --- | --- | --- | --- | --- |
| Beat 1 right | 33 | **36** | 34 | **37** |
| Every exported beat within 2 ms | 31 | **34** | 31 | **34** |
| openbox gate | 31 | **34** | 31 | **34** |
| Mean time per track | 265 ms | 266 ms | 255 ms | 221 ms |

All three off-beat house grids are fixed. No track got worse. The remaining failures
are the hip-hop tracks, the 140 to 174 cut, the ramp, and two placements just over 2 ms.

## Open questions

- rbxport's notes say that on 155 real tracks, structure told beat from midpoint on
  153 and its *attack* detector on 150 (the misses were off-beat claps with a sharper
  transient than the kick). The judge here uses the kick-*band* onset envelope, which
  claps barely reach, but that has to be shown on real music. Next step: research note
  003, the rekordbox library comparison.
- The threshold of 2 is a first guess. The real-library run should record the kick
  ratio on every track to calibrate it.
