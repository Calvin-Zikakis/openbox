# 001: rbxport on synthetic tracks with exact ground truth

Date: 2026-10-21. Commit: the first `ob-bench` commit. Machine: Apple M3 Pro (12 cores),
36 GB RAM.

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo run --release -p ob-bench -- eval --analyzer rbxport,rbxport-normal
```

## Question

How close does rbxport's analyzer come to the exact grid, and where does it fail, on
tracks where the grid is known to the sample?

## Dataset

The 40-track `ob-bench` suite (see `crates/ob-bench/src/synth.rs`):

- house, tech house, techno, trance, drum and bass, dubstep, breaks, hip-hop and afro
  house, at 87 to 175 BPM
- 44.1 and 48 kHz, starting at sample 0, after silence, or with pickups
- 3 tempo-change edits (two cuts and a ramp) and 2 humanized tracks (2 ms and 6 ms
  jitter)
- 8 tracks that hide the bar: no crashes, chords every four bars, fills and reverse
  cymbals into sections, melodic pickups on beat 4, a silent beat before the drop,
  and 32-bar kick-only intros

## Results

| Measure (of 40) | rbxport | rbxport-normal |
| --- | --- | --- |
| Tempo octave right | 39 | 37 |
| BPM within 0.01 | 38 | 37 |
| Beat 1 right (at least 98% of downbeats) | 33 | 34 |
| Every exported beat within 2 ms | 31 | 31 |
| rbxport's 25 ms gate | 33 | 33 |
| openbox gate | 31 | 31 |
| Mean analysis time per track | 265 ms | 255 ms |

## Findings

1. **Placement is about 1 ms early, consistently.** On quantized tracks the signed
   error is between -0.8 and -1.1 ms on almost every track, with p50 equal to max. That
   makes it a fixed bias, not noise. rbxport's docs already note that its RMS attack
   detector, stepping in 1 ms, runs about a millisecond early. It is inaudible and
   invisible on a CDJ's 150-column-per-second waveform, but it is a correctable bias.
2. **The main failure is an off-beat grid on house music.** On 3 of 10 house tracks,
   both presets put every grid line half a beat late (on the off-beat), with downbeats
   0% right: `house-118`, `house-124-subtle`, `house-122-everything`. These tracks have
   the house off-beat bass and open hat. Research note 002 traces the cause.
3. **BPM is right** on every constant four-to-the-floor track. This matches the
   experience with rekordbox: the tempo is right, while grid placement and beat 1 are
   not.
4. **Hip-hop with a soft 808 kick:** grids 17 ms late (Normal preset), or a wandering
   tempo (RBXport preset). At 87 BPM the tempo comes out doubled (the 70 to 180 range
   and a prior near 132 favor 174).
5. **Tempo changes:** the RBXport preset handles the 128 to 140 cut exactly. It fails
   the 140 to 174 cut (55% of downbeats) and the 124 to 128 ramp (75%).
6. **One spurious cut:** on `techhouse-125.25` (constant tempo) the tempo-following
   preset drops one beat (a 478 ms hole) where it opens a new segment.

## Caveats

- These tracks are synthetic. They are built from the failure modes of real music,
  but rbxport is tuned on real tracks. Each finding needs confirming on real music
  before any rule changes.
- The truth onset is the first sample of the kick's attack. rekordbox may use a
  different convention; rbxport matched rekordbox 7 to 0.0 ms median on its own
  playlist.
