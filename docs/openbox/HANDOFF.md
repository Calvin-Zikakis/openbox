# Handoff for coding agents

State as of 2026-10-21. This file is for an AI coding agent (GitHub Copilot or similar)
picking up openbox work. Read it before changing anything, then read the most recent
entry in [LOG.md](LOG.md).

## 1. What this repository is

openbox is a fork of [rbxport](https://github.com/chrisle/rbxport), a Rust and Tauri 2
rekordbox library manager that writes CDJ-ready USB sticks. The fork adds:

1. **A more precise beat-grid analyzer.** This is the current focus.
2. **Lexicon-class library features** (smart tagging, automatic cue points, import).
   These are not started.

The owner's actual complaint about rekordbox: **the grid lines miss the beat, and beat 1
lands on the wrong beat. BPM is usually right.** Prioritize phase placement and downbeat
selection over tempo estimation.

- Fork: github.com/Calvin-Zikakis/rbxport-beat-analysis (renamed from
  Calvin-Zikakis/openbox on 2026-10-22; the project is still called openbox). Local
  checkout: `~/repositories/rbxport-beat-analysis`. Remotes: `origin` is the fork,
  `upstream` is chrisle/rbxport.
- License: GPL-2.0-or-later, inherited. The fork is public.

## 2. Read these first

| Document | Why |
| --- | --- |
| [PLAN.md](PLAN.md) | Goals, phases, architecture of the fork, open decisions |
| [LOG.md](LOG.md) | Dated progress log; the newest entry is the current state |
| [upstream-hooks.md](upstream-hooks.md) | Every change to an rbxport-owned file |
| [research/001](research/001-synthetic-baseline.md) to [004](research/004-neural-judge.md) | Measured results behind every analyzer decision |
| [decisions/](decisions/) | Decision records 0001 to 0003 |
| `crates/rbl-analysis/docs/overview.md` | rbxport's own analyzer, which openbox reuses stage by stage |
| `docs/development/conventions.md` | rbxport's code conventions, which apply here too |

## 3. Rules

1. **Never write to the owner's rekordbox library.** `ob-bench library` copies
   `master.db` into `target/ob-library/` and opens only the copy. Rust tests that touch a
   library must run with `RB_LITE_TEST=1` and use `rbl_db::fixture::build`.
2. **Never commit private or downloaded material.** That means track titles or
   per-track results from the owner's music, audio files, the model files (`models/`), or
   the ONNX Runtime library (`vendor/onnxruntime/`). Summaries in `docs/openbox/` must
   not name tracks, except the owner's own productions already named in research notes
   003 and 004.
3. **Keep openbox code in `crates/ob-*` and `docs/openbox/`.** A change to an
   rbxport-owned file must be a thin hook. List it in `upstream-hooks.md` in the same
   commit.
4. **Keep the lints clean.** The workspace denies `unwrap`, `expect`, `panic` and
   `print_stdout` outside tests, and warns on clippy's pedantic group. CI treats warnings
   as errors. Binaries opt out of `print_stdout` at the top of the file, as
   `crates/ob-bench/src/main.rs` does. Cast lints are allowed per module, with a `reason`.
5. **Measure before claiming.** Every change to `ob-analysis` must be run on
   `ob-bench eval`. A research note must name its command, commit and dataset.
6. **Document as you go.** Add a dated entry at the top of `LOG.md` for each step, a
   research note for each measurement, and a decision record for each choice of approach.
7. **Documentation style:** no emojis. Use plain status words such as done, planned or
   later.
8. **Do not open pull requests against upstream** (chrisle/rbxport) without the owner's
   go-ahead. Ask before committing or pushing work the owner has not reviewed (see
   section 6).

## 4. Environment on the owner's Mac

- **Machine:** Apple M3 Pro, 12 cores, 18-core GPU, 36 GB RAM.
- **Rust:** rustup's global default host is x86_64 and Rosetta is not installed, so x86
  toolchains fail with "Bad CPU type". This folder has a directory override,
  `rustup override set 1.99.0-aarch64-apple-darwin`, matching `rust-toolchain.toml`. If
  upstream bumps the toolchain, install the matching `-aarch64-apple-darwin` toolchain
  and update the override.
- **Fast builds:** the release profile uses fat LTO and a single codegen unit, so a clean
  release build takes about 20 minutes. For bench work, run this first:

  ```sh
  export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
  ```

- **Models:** `scripts/openbox/fetch-models.sh --full` downloads the Beat This! ONNX
  files into `models/`, checked against pinned SHA-256 digests. Without them, analysis
  runs on DSP evidence only.
- **ONNX Runtime (optional):** `scripts/openbox/fetch-onnxruntime.sh` downloads
  Microsoft's 1.24.2 build into `vendor/onnxruntime/`. It is only needed for
  `OPENBOX_NEURAL_BACKEND=coreml`.
- **Frontend:** pnpm is not installed globally. Use `npx --yes pnpm@10.17.1 <command>`.
  Node 26 is installed, but the project pins Node 24. Under Node 26, 15 vitest tests in
  5 files fail on a clean tree too (`localStorage` and React `act`):
  - `src/lib/analysisChoice.test.ts`
  - `src/lib/deckQuantize.test.ts`
  - `src/lib/rekordboxBrowse.test.ts`
  - `src/lib/rekordboxBrowseStartup.test.ts`
  - `src/views/analysis/AnalysisDialog.test.tsx`

  Do not "fix" them by changing code; use Node 24.
- **Shell:** zsh. Quote globs, as in `grep --include='*.rs'`.

## 5. Architecture

### Crates openbox added

| Crate | Purpose | Entry points |
| --- | --- | --- |
| `crates/ob-bench` | Benchmark: synthetic tracks with exact ground truth, millisecond scoring, real-file and library comparisons | `src/synth.rs` (suite), `src/metrics.rs` (scoring), `src/analyzers.rs`, `src/main.rs` (CLI), `src/files.rs`, `src/library.rs` |
| `crates/ob-analysis` | The analyzer. It runs rbxport's stages and makes openbox's own phase, bar and placement decisions. It returns rbxport's `Analysis` type, so the ANLZ, `master.db` and USB writers take it unchanged | `src/lib.rs` (`analyse`, `Options`), `src/phase.rs` (half-beat and beat-1 judges), `src/placement.rs` (sub-millisecond shift) |
| `crates/ob-ml` | Beat This! frame activations (50 fps), per-thread model cache, excerpts, opt-in CoreML backend | `src/lib.rs` (`activations`, `excerpts`, `backend`) |

### Analysis pipeline (`ob_analysis::analyse`)

1. Run rbxport's independent stages in parallel scoped threads: onset envelope,
   kick-band envelope, attack map, band frames, waveform, level, key. Neural excerpts
   run beside them: the loudest 20 s of each quarter of the track.
2. `rbl_analysis::tempo::detect_tempo_with` fits the grid on kick attacks.
   `octave::judge` then compares the tempo with the spacing of Beat This!'s beats and,
   when it is a simple ratio that lands in 90 to 180 BPM with steady beats, refits at it
   through `tempo::detect_tempo_at` (note 006).
3. `phase::resolve` settles the half beat: the neural ratio decides when it is at least
   2 or at most 0.5; otherwise the kick band when its ratio is at least 2 or at most 0.5;
   otherwise structure, comparing total change on beats against between them. It then
   picks beat 1: structure scores including half of each neighbouring half beat's,
   blended 0.4 / 0.6 with the neural downbeat. The report carries the evidence and the
   `half_beat_conflict` and `bar_conflict()` flags.
4. `tempo::beats_of` and `rbl_analysis::phase_for` number the beats.
5. `rbl_analysis::anchor_file_start` applies rbxport's file-start adjustment.
6. `placement::refine` measures the median offset to the transient onsets and shifts
   the whole grid when the measurement is trustworthy.

### How the app reaches it

The app's analysis mode `"openbox"` (TypeScript `AnalysisMode`) is sent as `mode` to
the `analyse_track` command in `src-tauri/src/analysis.rs`. It maps to
`Engine::Openbox` with rbxport's Normal (constant-grid) preset options.
`analyse_file_with` then calls `ob_analysis::analyse(..., &ob_analysis::Options::with(options))`.

### Upstream hooks so far

See [upstream-hooks.md](upstream-hooks.md):

- `rbl-analysis`: six items made `pub`.
- `.gitignore`: two folders added.
- `README.md`: a banner at the top.
- `.github/copilot-instructions.md`: rules for coding agents.
- `src-tauri` and five frontend files, for the app mode.

## 6. Current state

**Committed and pushed to `origin/main`** (see `git log`): the plan and docs, `ob-bench`,
`ob-analysis` (phase judges and the placement stage), `ob-ml` (neural judge, opt-in
CoreML), the fetch scripts, and the app mode "openbox (Precise Grid)", the fork's
default.

| Change | Status |
| --- | --- |
| App mode: `src-tauri/Cargo.toml`, `src-tauri/src/analysis.rs`, `src/lib/preferences.ts`, `src/lib/analysisChoice.ts`, `src/ipc/types.ts`, `src/views/analysis/AnalysisDialog.tsx`, `src/views/settings/AnalysisPane.tsx` | `tsc` passes, `cargo clippy -p rbxport` is clean, and `RB_LITE_TEST=1 cargo test -p rbxport --lib analysis::` passes 8 of 8, including `the_openbox_engine_grids_the_fixture_track_as_rbxport_does` |
| `crates/ob-analysis/src/placement.rs` | Measured on the synthetic suite and the owner's library (note 005) |

Ask the owner before committing or pushing new work they have not reviewed. On
2026-10-22 the owner asked the agent to commit, push and keep improving the analyzer.

### Measured results to keep in mind

| Result | Source |
| --- | --- |
| rbxport places grids about 1 ms early on every quantized synthetic track | note 001 |
| rbxport's structure stage moved correct house grids onto the off-beat on 3 of 10 synthetic house tracks; openbox fixes all 3. Beat 1 is right on 37 of 40 (openbox, Normal preset) against 34 of 40 (rbxport) | note 002 |
| The tempo-following preset splits constant real masters into spurious segments (2 of 11) | note 003 |
| Beat This! (full model) is right on the half beat 40 of 40 and on the downbeat 40 of 40 on the synthetic truth. On real music its downbeat can flip by half a bar | note 004 |
| Neural cost: 20 s takes 0.39 s, 25 s 0.52 s, but 30 s 1.26 s on the CPU. Four 20 s excerpts take about 1.5 s per track | note 004 |
| CoreML with default ONNX Runtime settings is 2.4 times slower than the CPU, with outputs within 0.013 | note 004 |

## 7. Next tasks, in order

1. **Measure the placement stage.** Done on the synthetic suite and the owner's library
   (note 005): bias 0.97 to 0.07 ms on synthetic tracks, and on real music openbox's
   grids sit on far more kick attacks than rekordbox's where they differ. Remaining:
   run `ob-bench files` on the masters of note 003. The original criteria follow.

   ```sh
   cargo build --release -p ob-bench
   target/release/ob-bench eval --quiet --jobs 3 --json target/ob-bench-v4.json
   ```

   Done when, against `target/ob-bench-v3.json` (or a fresh run with placement disabled):
   - the median signed error (`bias_ms` per track in the JSON) moves from about -1 ms to
     within plus or minus 0.3 ms on quantized tracks
   - "max<=2" rises
   - the downbeat and octave counts do not fall

   Write research note 005. If it regresses any track, find out why before tuning
   thresholds (`THRESHOLD`, `CLEAR`, `MIN_BEATS`, `MAX_SPREAD_MS` in `placement.rs`).
   Also run `ob-bench files` on the real masters listed in note 003 (local only; do not
   commit names beyond the owner's own tracks) to check that the shift stays small and
   stable on real kicks.

2. **Commit the app mode and the placement stage**, once the owner approves. Use
   separate commits with descriptive messages.

   **Tempo ratio errors: first round done (note 006).** The neural tempo judge cut the
   library's tempo disagreements with rekordbox from 27 to 15 of 464 tracks. About 11
   ratio errors remain, half of them with unsteady neural beats. Next ideas: score each
   candidate tempo's grid by its neural beat activation, and measure on other genres.
   Re-run `ob-bench eval` (octave must not fall) and the library for any change.

3. **One constant grid by default, with tempo changes flagged.** Add a flag to
   `ob_analysis::Report` when rbxport's tempo-change detection, run as evidence only,
   finds a settled second tempo. Keep the grid constant unless the evidence is
   overwhelming.

   Done when the synthetic edits (`edit-*` tracks) are flagged and no constant track is.

4. **Persist the evidence and show a review queue.** Store `Report` in an openbox
   sidecar SQLite database keyed by an audio hash plus analyzer version, never in
   `master.db`. The fields that matter: `phase.kick_ratio`, `phase.neural_ratio`,
   `phase.judge`, `phase.bar_margin`, `phase.half_beat_conflict`, `phase.bar_conflict()`,
   `phase.structure_choice`, `phase.neural_choice`, and `placement.*`.

   Add a review list in the UI sorted by lowest confidence. A one-click "beat 1 here" uses
   the two candidates. Follow rbxport's UI conventions (`src/views`, `src/ipc`, `src/i18n`)
   and add mock-backend behavior for every new IPC call.

5. **Tune CoreML.** The `beat-this` crate's `OrtRuntime` hard-codes
   `CoreML::default()`. Implement `beat_this::Model` over your own `ort::session::Session`.
   The mel front end can stay on `rten`, behind an either-type implementing `Model`. Try
   the MLProgram model format, compute units (All, CPUAndGPU, CPUAndNeuralEngine), static
   input shapes (the excerpts are a constant 20 s), and a compiled-model cache directory.

   Done when it beats the CPU's 0.39 s per 20 s excerpt with outputs within 0.02.

6. **Ship the models with the app.** `ob_ml::models_dir()` currently relies on
   `OPENBOX_MODELS`, `./models`, or the build-time workspace path. A packaged app needs
   an app-data location and a first-run download with checksum checks.

**Blocked on the owner:**

- Where beat 1 is on the owner's tracks "ROUZ - The One" and "ROUZ & Wyllie - Perceive"
  (note 004). Use the answer to validate the 0.6 neural weight for beat 1.

The library blocker is resolved: on 2026-10-22 the owner removed the dead entries and
imported tracks from USB, leaving 464 tracks with audio and a rekordbox grid.
`ob-bench library` reads `~/Library/Pioneer`, which the VS Code terminal sandbox hides,
so run it unsandboxed.

## 8. Commands

```sh
# Fast release profile for bench work
export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16

# Lint the openbox crates and rbxport's analyzer
cargo clippy -p ob-ml -p ob-analysis -p ob-bench -p rbl-analysis --release --all-targets

# rbxport's analyzer tests (must stay green: the hooks change no behavior)
RB_LITE_TEST=1 cargo test -p rbl-analysis --release

# App-side analysis tests
RB_LITE_TEST=1 cargo test -p rbxport --lib analysis::

# Benchmark. `clippy` does not rebuild the binary; always build first.
cargo build --release -p ob-bench
target/release/ob-bench list
target/release/ob-bench eval --quiet --jobs 3 [--analyzer rbxport-normal,openbox-dsp-normal,openbox-normal] [--filter TEXT] [--json PATH]
target/release/ob-bench probe --filter house           # rbxport's two phase judges on a track
target/release/ob-bench neural                         # Beat This! against the synthetic truth
target/release/ob-bench neural-speed FILE              # inference time by input length
target/release/ob-bench files [--same-song] FILE...    # real files, no reference grid
target/release/ob-bench library [--limit N]            # against a COPY of the installed rekordbox library

# Frontend
npx --yes pnpm@10.17.1 install --frozen-lockfile
npx --yes pnpm@10.17.1 exec tsc --noEmit -p tsconfig.json
npx --yes pnpm@10.17.1 exec vitest run src/lib src/views/analysis
```

Analyzer names in `ob-bench`:

| Name | What it runs |
| --- | --- |
| `rbxport` | rbxport's analyzer, tempo-following preset |
| `rbxport-normal` | rbxport's analyzer, constant-grid preset |
| `openbox-dsp` | openbox without the neural judge, tempo-following preset |
| `openbox-dsp-normal` | openbox without the neural judge, constant-grid preset |
| `openbox` | openbox with the neural judge, tempo-following preset |
| `openbox-normal` | openbox with the neural judge, constant-grid preset (what the app's "openbox" mode runs) |

## 9. Gotchas

- `Beat::time_ms` in `rbl_analysis` holds whole milliseconds. Sub-millisecond grid
  times come from the segments (`phase_secs`, `period_secs`). See `prediction_of` in
  `crates/ob-bench/src/analyzers.rs`.
- Neural inference already uses every core through rayon. `ob-bench eval --jobs` above
  about 3 oversubscribes the CPU and inflates the per-track times.
- Keep neural excerpts at 25 s or less. A 30 s input crosses a model chunk boundary and
  costs 2.4 times as much.
- `ob-bench files --same-song` cannot align files of different sample rates, because the
  envelope rate differs. The 96 kHz render of Perceive is skipped for that reason.
- The tempo-following preset (`AnalysisPreset::Rbxport`) creates spurious tempo segments
  on constant tracks. The app's openbox mode deliberately uses the Normal preset.
- `rbl_db::detect()` finds the installed library (rekordbox 6 here, last used May 2024).
  The library comparison resolves ANLZ files from the real `share/` tree, read-only.
- The structure judge tends to pick the beat *before* the true downbeat when a fill or
  riser leads into a phrase. That is why the neural downbeat is weighted 0.6.
- Synthetic results alone do not justify a rule change. Confirm on real music
  (`ob-bench files`, and `ob-bench library` once the library is available).
