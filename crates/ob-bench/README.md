# ob-bench

openbox's beat-grid benchmark. It renders synthetic electronic tracks whose grid is known
exactly, runs analyzers over them, and scores each grid to the millisecond.

## Why synthetic tracks

Hand annotations are accurate to about 10 ms, and the grid target is 1 to 2 ms. So the
reference has to come from construction. Every hit is rendered from a continuous-time
formula starting at its exact, fractional-sample onset.

The tracks carry what makes real grids hard: breakdowns without a kick, beatless
intros, pickups, off-beat bass and hats, broken and half-time kick patterns,
sidechain pumping, clipped masters, tempo cuts and ramps, and humanized timing. The
`subtle-bars`, `fills`, `anacrusis`, `drop-gap` and `long-intro` tags hide the bar
the way real tracks do. See `src/synth.rs` for the suite.

## Commands

From the repository root. The release profile uses fat LTO and takes about 20 minutes
to build, so override it for bench work:

```sh
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo run --release -p ob-bench -- list
cargo run --release -p ob-bench -- eval [--analyzer rbxport,openbox] [--filter TEXT] [--json PATH] [--quiet]
cargo run --release -p ob-bench -- probe --filter TEXT
cargo run --release -p ob-bench -- render OUT_DIR [--filter TEXT]
```

- `eval` scores each analyzer on each track and prints a summary.
- `probe` shows how rbxport's two phase judges (kick-attack fit and structure) stand
  on a track.
- `render` writes 16-bit WAV files and their truth as JSON, for listening or for
  feeding encoders.

`--filter` matches a track name or tag. `RUST_LOG=rbl_analysis=debug` prints the
analyzer's tempo decisions.

## What the scores mean

Placement and numbering are scored separately:

- **Placement:** for each true beat, the signed time to the nearest predicted beat. It
  is reported at full precision and after rounding to whole milliseconds (`exp max`),
  as a USB stick stores it. `bias` is the mean signed error; negative means the grid is
  early. `drift` is the change in error from start to end.
- **Numbering:** at each matched beat (within a quarter beat), the predicted number
  minus the true one, mod 4. `bar off` is the dominant offset; 0 is right. `down` is
  the share of true downbeats numbered 1.

Pass criteria:

- **openbox:** right tempo octave, every exported beat within 2 ms, and at least 98% of
  downbeats numbered 1.
- **rbx25ms:** rbxport's own gate. At least 98% of beats are within 25 ms with the same
  number.
