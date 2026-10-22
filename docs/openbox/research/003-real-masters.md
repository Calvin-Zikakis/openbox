# 003: Real masters without reference grids

Date: 2026-10-21.

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo build --release -p ob-bench
target/release/ob-bench files [--same-song] FILE...
```

## Why not the rekordbox library

The local rekordbox 6 library (`ob-bench library`, which reads only a copy of
`master.db`) has 947 analyzed tracks. 929 of their audio files are no longer at their
recorded paths, and no file with the same name exists anywhere in the home folder. The
other 18 have no grid. The comparison against rekordbox's grids needs the drive that
holds the DJ collection, or a rekordbox USB export.

## Dataset

Eleven mastered electronic tracks found on this machine (124 to 142 BPM; progressive,
psytrance, house, UKG), plus six renders of one of them: pre-master at 96 kHz, three
masters, a trimmed master and an MP3. There are no reference grids. Each grid is judged
on three things:

- the **kick ratio**: low-band onset energy on beats over midpoints
- the **attack offset**
- **agreement** between analyzers, and between renders of the same song once they are
  aligned by cross-correlation

Track titles stay out of this note.

## Results

| Check | Result |
| --- | --- |
| BPM | All four analyzers agree on every track; every tempo is a whole number |
| Normal presets, openbox against rbxport | Identical grids and downbeats on all 11 tracks |
| Same song, 5 renders at 44.1 kHz | Beat 1 lands on the same musical moment in every render, for both Normal presets |
| Tempo-following presets | Split 2 of 11 constant-tempo masters into 3 and 5 segments. The kick ratio of the 5-segment grid fell from 2.19 to 1.12, and on the 3-segment MP3 openbox's beat 1 moved by a beat |
| Half beat fixed by the judges | 2 tracks began on the off-beat (fit kick ratio 0.39 and 0.46); both openbox and rbxport moved them on (final 2.56 and 2.19) |
| Ambiguous | 1 track: kick ratio about 1 either way (a bass line as strong as the kick in the low band), structure 1.96 against 2.04 |

Kick ratios of grids that are plainly on the kick ranged from 1.3 (UKG, broken kick) to
5.2. So a ratio of 2 is decisive on four-to-the-floor masters, but lower ratios are
common, and the structure fallback carries real weight.

## Conclusions

1. **Default to one constant grid.** Tempo following creates more errors than it fixes
   on a library of quantized music. rbxport counted 32 tempo changes among 35,262 grids
   in one rekordbox library. openbox should fit one grid and *flag* a possible tempo
   change for review, unless the change is overwhelming.
2. **Flag ambiguity instead of guessing.** A kick ratio near 1 together with a structure
   margin near 1 is a coin flip. The track must show up in the review queue.
3. **A better kick-versus-bass judge** would settle the ambiguous track. Candidates are
   the click band's attack heights, sub-bass against kick-body spectra, and neural beat
   activations.
4. The MP3 render lines up 52 ms after the WAV it came from. That is too coarse to test
   the 1105-sample offset: the envelope resolution is 5.8 ms, and the MP3 may come from
   a different bounce.
