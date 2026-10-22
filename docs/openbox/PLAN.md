# openbox: project plan

openbox is a fork of [rbxport](https://github.com/chrisle/rbxport), a fast, open-source
rekordbox library manager written in Rust and Tauri. rbxport already reads and edits the
rekordbox library and writes USB sticks that CDJs load. openbox adds two things:

1. **A much better song analyzer.** Beat grid first, measured to the millisecond, using
   every CPU core and the GPU.
2. **A Lexicon-class feature set** that rbxport deliberately leaves out: smart tagging,
   automatic cue points, broader import, and library cleanup tools.

rbxport's README states its vision is "fewer features" and invites forks for anything
beyond that. openbox is that fork.

Status: Phase 0 under way. The fork is github.com/Calvin-Zikakis/rbxport-beat-analysis
(created 2026-10-21 as Calvin-Zikakis/openbox, renamed 2026-10-22). Progress is
recorded in [LOG.md](LOG.md).

Contents

1. Goals
2. Why fork rbxport
3. Phases
4. The rekordbox device format (what rbxport covers, what we verify)
5. Analysis research
6. The analysis engine
7. Library and UI
8. Smart tagging and automatic cue points
9. Architecture of the fork
10. Safety rules
11. Risks
12. Open decisions
13. References

---

## 1. Goals

1. **Analyze better than rekordbox and rbxport.** Beat grid, BPM, downbeat, phrases,
   key, vocals and energy, with the beat grid as the top priority. The specific
   complaint with rekordbox is that **the grid lines miss the beat and beat 1 lands in
   the wrong place**; its BPM is usually right. So phase placement and downbeat
   selection come first, ahead of tempo estimation.
2. **Use the hardware it runs on.** Use every CPU core, the GPU when there is one, and as
   much RAM as helps, while the UI stays responsive.
3. **Manage the rekordbox library** with Lexicon's feature set on top of what rbxport
   already does.
4. **Keep rbxport's strengths:** compatibility with current rekordbox and its hardware,
   and speed (rbxport opens a 38,000-track library in 1.5 s using under 200 MB).

### What a "perfect beat grid" means

A grid is correct when every grid line sits on the attack of the beat, on the same sample
timeline the player's decoder produces, and beat 1 falls on the first beat of each bar
(ideally the first bar of each phrase).

No analyzer can be perfect on every recording. Live drummers, tempo ramps, swing and
beatless intros have no single correct constant grid. So the plan has two parts:

- **Exact on quantized music.** Tracks produced on a DAW grid have a fixed tempo, and
  that tempo can be recovered almost exactly. This covers most of a typical electronic
  DJ library.
- **Self-aware everywhere else.** Each track gets a grid confidence score and specific
  flags (tempo change, ambiguous downbeat, half/double-time ambiguity, drift), so you
  only review the tracks that need it.

### Targets, and where rbxport stands

rbxport's figures come from its own docs. It measures against hand-checked rekordbox
grids on 155 purchased electronic tracks (123 to 178 BPM) with a 25 ms tolerance. Our
targets use a 1 to 2 ms tolerance and exact ground truth (section 5.6), so the two
columns are not directly comparable. The first job in Phase 2 is to measure rbxport with
our harness.

| Measure | openbox target (constant-tempo electronic music) | rbxport today (its docs) |
|---|---|---|
| Beat phase error, across every beat | 95th percentile at or under 1 ms, max at or under 2 ms (after export rounding) | 91.6% of tracks have at least 98% of beats within 25 ms |
| Tempo | within 0.001 BPM; drift over the track under 1 ms | 100% within 0.05 BPM |
| Downbeat (beat 1 on the true bar start) | at least 99% of tracks | 92.3% within 25 ms (141 of 142 on tracks rekordbox 7 analyzed) |
| Phrase alignment (bar 1 starts a 16/32-bar phrase) | at least 95% of tracks | not measured |
| Tempo changes (DJ edits) | measured per segment | 70% within 3 ms of the hand grid |
| Key | beat rbxport on the same set | 14 misses on its reference playlist |
| Failure detection | at least 95% of tracks with any grid error get flagged | no confidence score |
| Speed, standard analysis | under 1 s of wall-clock time per track on an Apple Silicon laptop | about 460 ms per track (mean), a few tracks at a time |

---

## 2. Why fork rbxport

### What it already gives us

Most of what the first version of this plan scheduled as Phases 1, 4 and 5 already
exists, much of it tested on hardware:

- **Device formats:** writers for `export.pdb` (`rbl-pdb`), `exportLibrary.db` /
  OneLibrary (`rbl-onelibrary`) and ANLZ `.DAT`/`.EXT`/`.2EX` (`rbl-anlz`), plus
  orchestrated USB export and sync to several drives at once (`rbl-export`). It also has
  a "Max Compatibility" mode that converts FLAC and M4A to WAV for older CDJs.
- **LINK Export:** serves the library to CDJs over Ethernet as rekordbox does
  (`rbl-link`, `rbl-prolink`, `rbl-dbserver`, `rbl-nfs`).
- **rekordbox library access:** opens the SQLCipher `master.db` with guarded writes
  (`rbl-db`). It also has a diff tool that records what rekordbox itself changes, so its
  writes copy rekordbox instead of guessing (`rbl-difftool`).
- **UI:** Tauri 2, React and TypeScript, with virtualized track tables, playlists,
  intelligent playlists, My Tag, cue and loop editing, a beat grid editor, two-deck
  preview with Rubber Band key lock, undo and redo, backups and restore, and AppleScript.
- **Analysis already solved:** rbxport established that rekordbox grids sit on the
  *untrimmed* MP3 timeline (encoder plus decoder delay, 1105 samples). Symphonia with
  gapless off matches it. That was the riskiest research item in the old plan.
- **Analyzer seam:** analysis is one call, `rbl_analysis::analyse_with(samples,
  sample_rate, options)`. It is invoked from one place, `src-tauri/src/analysis.rs`. A
  new analyzer can plug in there.
- The stack matches the one this plan would have chosen (Rust core, Tauri 2 UI).

### What it lacks (our work)

- **Analysis:** classical DSP only, with no ML and no GPU. It assumes 4/4 and is tuned for
  electronic music. Attack search uses RMS in 1 ms steps. Phrase labels and vocal
  detection are not implemented. There is no confidence score. Open issues report grid
  problems: #535 (every grid slightly right of the beat), #635 (the tempo-following mode
  missed the beat where Normal mode hit it) and #410 (beatgrid mismatch).
- **Features outside its scope:** smart tagging, automatic cue points, import from other
  DJ software, duplicate detection, metadata cleanup, watched folders, similarity search,
  and a grid review workflow.

### What forking costs

- **License.** rbxport is GPL-2.0-or-later, so openbox is too. That is fine for personal
  use. If openbox is ever distributed, its source must be shipped with it, and it cannot
  become closed-source.
- **Upstream speed.** About 1,500 commits since September 25, 2026, almost all by one
  author. To stay mergeable, our changes live in our own crates with thin hooks into
  upstream files, and we merge upstream weekly (section 9).
- **Maturity.** The project is under a month old and has about 130 open issues.
- **Source of truth.** rbxport edits rekordbox's `master.db` directly, and rekordbox must
  be closed while it writes. That decides the old plan's open question: your rekordbox
  library stays the main store. openbox keeps its extra data (analysis evidence,
  confidence, embeddings, tag suggestions) in a separate sidecar database.

---

## 3. Phases

| Phase | Focus | Done when |
|---|---|---|
| 0 | Fork, build, learn the codebase, set up upstream sync, run it against a *copy* of your library | the fork builds, its tests pass, and it opens your library copy |
| 1 | Verify the device format on your players; read rbxport's format code and docs | the hardware checklist (4.3) passes on every player you use |
| 2 | Analysis research: harness, datasets, baselines (rbxport included), prototype | a chosen pipeline beats rbxport and rekordbox on our metrics, with measured numbers |
| 3 | The analysis engine in the fork: new crates, GPU batching, phrase and vocal output | the openbox analyzer is selectable in the app and meets its targets |
| 4 | Grid review workflow: confidence, grid health view, review queue | you review only flagged tracks |
| 5 | Smart tagging and automatic cue points | cue templates and tag suggestions work on real playlists |
| 6 | Broader import, cleanup tools, duplicates, watched folders | as needed |
| ongoing | Merge upstream weekly; offer in-scope fixes back to rbxport | |

---

## 4. The rekordbox device format

rbxport implements this. Its references are in `docs/reference/` (for example
`usb-export-db.md` and `usb-export-pipeline.md`). This section keeps only what the
analyzer has to produce and what we must verify ourselves.

### 4.1 What the analyzer writes into

| Tag | File | Contents |
|---|---|---|
| PQTZ | DAT | **beat grid**: one entry per beat holding beat-in-bar (1 to 4), tempo (BPM x 100) and time (integer milliseconds). rbxport notes this is the grid players use, and `.EXT` files can carry an empty PQT2 |
| PCOB, PCO2 | DAT, EXT | cues: legacy, and extended with colors and comments |
| PWAV, PWV2 to PWV7 | DAT, EXT, 2EX | preview, detail, color and 3-band waveforms (detail at 150 entries per second) |
| PSSI | EXT | song structure: phrases and mood (XOR-masked in newer exports). Needed for phrase display on CDJ-3000-era players; rbxport does not generate labels yet |

**Precision on the device.** PQTZ stores every beat explicitly, so error does not build
up over the track. Each beat is rounded to the nearest millisecond (plus or minus
0.5 ms). openbox keeps sub-sample precision internally and rounds only at export.

**rekordbox XML** (if we export it) stores grids as tempo markers with two-decimal BPM.
An error of 0.005 BPM at 128 BPM adds up to about 16 ms over 900 beats, so the exporter
must re-anchor with extra markers.

### 4.2 Which players read what

| Format | Players |
|---|---|
| Device Library (`export.pdb` and ANLZ) | essentially every rekordbox-compatible player |
| OneLibrary (`exportLibrary.db`, encrypted SQLite, also references ANLZ files) | CDJ-3000 (firmware 3.30 or later), CDJ-3000X, OPUS-QUAD, XDJ-AZ, OMNIS-DUO, per AlphaTheta (October 2025) |

### 4.3 Hardware checklist (Phase 1)

Run on every player you use, with a stick exported by the fork:

- library loads; playlists and intelligent playlists; artwork
- grid lines on the kick at full zoom; beat 1 correct; BPM display; beat sync
- hot cues, memory cues and loops, with colors and names
- waveforms (preview, detail, 3-band on CDJ-3000)
- phrases (once we write PSSI); My Tag filtering
- history written back after a set
- LINK Export, if you use it

---

## 5. Analysis research

This is the main work of the project. The output is measured evidence and a chosen
pipeline, written up in `docs/openbox/research/`.

### 5.1 Metrics and the evaluation harness

A bench tool runs any analyzer (rbxport's Normal and RBXport presets, rekordbox's stored
grids, and ours) over a dataset. It reports per-beat phase error (median, 95th
percentile, max), drift, BPM error, octave errors, downbeat and phrase accuracy, how well
confidence predicts failure, and runtime. rbxport has its own private evaluation
"golden" rig (`crates/rbl-analysis/examples/golden.rs`). We extend it rather than start
over. Standard research metrics (F-measure at 70 ms, CMLt, AMLt) are reported only to
compare with published results.

### 5.2 The timeline

- **MP3: solved by rbxport.** rekordbox 7 grids sit on the untrimmed MP3 timeline (1105
  samples of encoder plus decoder delay). rekordbox 7 bundles `libmpg123`, and Symphonia
  with gapless off matches it.
- **rekordbox 6 versus 7:** rbxport found 11 MP3s with rekordbox 6 grids exactly 1105
  samples (25 ms) late. openbox should detect this in imported grids and offer a fix.
- **Still to verify:** AAC/M4A (2112-sample priming), ALAC, FLAC, WAV and AIFF, each
  against rekordbox's stored grids and on a CDJ. Use click tracks with known positions,
  encoded each way.

### 5.3 rbxport's analyzer (the baseline to beat)

Summary of `crates/rbl-analysis/docs/overview.md`:

- **Tempo:** a spectral-flux onset envelope, with candidates from autocorrelation
  combined with Fourier rhythm strength and a mild prior. Range 70 to 200 BPM.
- **Placement:** for each predicted beat, find the kick attack. The track is band-passed
  to 900 to 9,000 Hz, and RMS is measured in 1 ms steps within plus or minus 15 ms. A
  weighted line is fitted through the attacks, outliers removed, then refitted. A tempo
  within 0.1 BPM of a whole number snaps to it when that grid fits nearly as well.
- **Tempo changes:** 16-second windows. Gradual changes are walked beat by beat; sudden
  ones get a cut at a supported beat. Breakdowns are held through when the phase returns.
- **Beat 1:** structural novelty at 1, 2, 4 and 8 bar scales over 8 positions per bar
  (beats and half-beats). This also corrects off-beat grids.
- **Key:** Faraldo's edmkey (HPCP with EDMA profiles), preferring minor on ties.
- **Limits:** 4/4 only; tuned for electronic music; 1 ms attack resolution; no ML, GPU,
  confidence, phrase labels or vocals.

This is strong, careful work, and its kick-attack line fit is the same core idea the old
plan proposed. openbox builds on it rather than replacing it outright.

### 5.4 State of the art to bring in

| Method | Year | Use in openbox |
|---|---|---|
| Beat This! (Foscarin, Schlüter, Widmer), MIT code and weights; a Rust port exists (`beat-this` crate, rten or ort backends) | 2024 | frame-level beat and downbeat activations; robust where kicks are absent or misleading |
| All-In-One (Kim and Nam), MIT code; check the weights' license | 2023 | downbeats, segment boundaries and section labels, which feed PSSI phrases and cues |
| Beat Transformer (Zhao et al.) | 2022 | reference for using separated stems |
| Böck and Davies multi-task TCN; Böck, Krebs, Widmer (madmom; models CC BY-NC-SA) | 2016 to 2020 | baselines |
| Schreiber and Müller tempo CNN | 2018 | octave-error handling |
| Key CNN (Korzeniowski and Widmer) | 2018 | compare with edmkey |

On Traktor: its algorithm is proprietary. Observed behavior: it commits to one constant
tempo unless told otherwise and anchors the grid on transient attacks. Traktor Pro 4
added Flexible Beatgrids. Both rbxport and this plan already take the same approach. The
gains left are in hard cases, downbeats, phrases and sub-millisecond phase.

### 5.5 Proposed pipeline (a hypothesis for Phase 2 to test)

1. **Decode** on the calibrated timeline (rbxport's `rbl-audio`).
2. **Evidence, computed in parallel:** rbxport's onset and kick-attack envelopes, plus
   neural beat and downbeat activations (GPU, batched across tracks).
3. **Tempo:** rbxport's candidates, with neural activations breaking octave ties and
   carrying beatless or kick-less sections.
4. **Placement:** rbxport's weighted line fit, refined to sub-sample attack positions.
   With 2 ms of jitter per onset, one line through 900 beats gives roughly
   2/sqrt(900), about 0.07 ms, of phase error.
5. **BPM snapping:** keep rbxport's whole-number rule, extended to x.5 and x.25, and snap
   only when the predicted drift at the end of the track stays under 1 ms.
6. **Beat 1 and phrases:** fuse rbxport's structural novelty with neural downbeat
   activations. Choose the phrase start that puts segment boundaries on multiples of 8
   bars.
7. **Tempo changes:** rbxport's walk and cut logic, checked against neural activations.
8. **Structure, vocals and energy:** segment labels for PSSI, vocal presence (from stems
   in the deep tier) and a 1 to 10 energy rating.
9. **Confidence and flags:** from fit residuals, disagreement between evidence paths, and
   octave and downbeat ambiguity.
10. **Determinism:** rbxport guarantees that the same audio gives the same result. GPU
    inference can differ slightly across hardware, so activations are quantized before
    any decision is made, and a CPU reference path produces identical grids.

### 5.6 Datasets and ground truth

- **Your library.** Grids and cues you have hand-corrected in rekordbox are the most
  valuable labels we have (section 12).
- **Synthetic tracks** rendered with exactly known grids, at many tempos, sample rates and
  encoders. These give exact ground truth for millisecond precision, which hand
  annotations cannot.
- **Public:** Harmonix Set (beats, downbeats, sections), GiantSteps Tempo and Key (EDM),
  Ballroom, SMC, Hainsworth. These cover tempo, downbeats and key; they are too coarse for
  millisecond phase.
- **Baselines** on the same set: rekordbox (Normal and Dynamic), rbxport (both presets),
  Traktor, Mixxx, Beat This!, All-In-One.

### 5.7 Language for research

Use Python for quick experiments and model work (PyTorch, then ONNX export), kept in
`research/`. Production code is Rust in the fork, with parity tests.

---

## 6. The analysis engine

### 6.1 How rbxport schedules analysis today

Within a track, rbxport runs its independent stages on parallel threads
(`std::thread::scope`). Across tracks, the frontend queue runs a few tracks at a time
(the `concurrentTracks` preference).

### 6.2 What openbox adds

```
decode --> DSP evidence --------------------------------> fuse and fit --> write ANLZ
(CPU,      (CPU, rbxport stages)                           (CPU)            and master.db
parallel)                                                                   (rbxport)
       \-> model input --> batched inference (GPU; CPU fallback) --/
```

- **A Rust-side scheduler** replaces the frontend's few-at-a-time queue for bulk jobs. It
  sizes workers, GPU batches and in-flight tracks from detected hardware (core counts,
  including performance and efficiency cores on Apple Silicon; GPU; free RAM), with
  bounded channels for backpressure.
- **GPU batching:** collect model input windows from several tracks into one batch.
- **Throttle modes:** "full speed" for overnight runs, and "background" while you use the
  app.
- **Tiers:** standard (grid, key, waveforms, structure, energy) and an optional slower
  deep tier with stem separation for vocals, drums and better cues.
- **Inference:** `ort` (ONNX Runtime: CoreML on macOS, CUDA and TensorRT on NVIDIA,
  DirectML on Windows) for GPU, and `rten` (pure Rust) as the CPU fallback. These are the
  `beat-this` crate's two backends.

### 6.3 Sidecar data

Analysis evidence, confidence, flags, embeddings and tag suggestions go in an openbox
SQLite sidecar, keyed by a hash of the audio stream plus the analyzer version. Tag edits
then do not trigger re-analysis, moved files are recognized, and an algorithm upgrade
re-runs only the stages that changed. Final grids, cues, keys and tags go to rekordbox's
`master.db` and ANLZ files through rbxport's existing writers.

---

## 7. Library and UI

### 7.1 Already in rbxport

Library browser, playlists and folders, intelligent playlists, My Tag, cue and loop
editing, a beat grid editor, preview decks, USB export and sync, LINK Export, backups,
undo and redo, AppleScript.

### 7.2 openbox additions (Lexicon-class)

Priority: v1 comes first after the analyzer; v2 follows soon after; later means after
that.

| Feature | Priority |
|---|---|
| openbox analyzer selectable beside rbxport's presets | v1 |
| Grid health view: plot of per-beat residuals (onset minus grid) across the track. Drift shows as a slope, a phase offset as a constant shift, and a tempo change as a kink | v1 |
| Review queue: tracks sorted from lowest confidence up, keyboard-driven accept or fix | v1 |
| Grid compare: openbox versus the existing rekordbox grid, difference in ms | v1 |
| Grid editor additions: nudge by a sample or 0.1 ms, snap anchor to nearest transient, metronome click with latency compensation | v1 |
| Detect and fix rekordbox 6 MP3 grids that sit 25 ms late | v1 |
| Automatic cue points with templates (section 8) | v2 |
| Smart tag suggestions (section 8) | v2 |
| Import from folders, with analysis on import (check what rbxport already supports) | v2 |
| Watched "incoming" folder: auto-import and auto-analyze | v2 |
| Metadata tools: batch edit, find and replace, cleanup rules, write tags to files | v2 |
| Duplicate detection by audio fingerprint; relocate missing files | v2 |
| "More like this" similarity search | v2 |
| Import from Traktor (NML), Serato, Engine DJ, VirtualDJ, Apple Music XML | later |
| Artwork and metadata lookup from online services | later |

UI work follows rbxport's conventions (React views in `src/views`, IPC in `src/ipc`,
strings in `src/i18n`) and its performance budget (`perf-budgets.json`).

---

## 8. Smart tagging and automatic cue points

### 8.1 Smart tagging, in layers

1. **Tags derived from analysis:** BPM band, key, energy, vocal or instrumental, intro
   length in bars, has a breakdown.
2. **Audio embeddings** (CLAP, MERT, Discogs-EffNet or similar; check licenses) for
   similarity search, clustering, and text-prompted tags ("dark", "acid", "vocal").
3. **Personal tag learning:** small classifiers trained on embeddings and *your own* My
   Tags suggest tags for untagged tracks, with confidence scores. Accepting or rejecting
   a suggestion improves the classifier.
4. **Rule-based auto-tags:** user-defined rules that apply tags.

Suggestions live in the sidecar. Accepted tags are written as rekordbox My Tags through
rbxport's writer, so they reach USB exports and the players.

### 8.2 Automatic cue points

- **Inputs:** beat grid and downbeats, phrase segments, energy curve, and stem activity
  (drums, bass and vocals coming in and out) when the deep tier is on.
- **Events:** first downbeat, mix-in point, first drop, breakdown starts, second drop,
  outro start, mix-out point, vocal entries.
- **Templates:** you map events to hot cue slots, colors and names, plus memory cues and
  loops. Example: A = first downbeat (green), B = 32 bars before drop 1, C = drop 1 (red),
  D = breakdown, E = outro.
- Every cue snaps to a bar or phrase boundary. Cues you set yourself are never
  overwritten unless you ask. You preview before applying, and can batch-apply to a
  playlist.
- **Evaluation:** compare against the cues you have already placed by hand.
- rbxport already adds a first-beat memory cue after analysis. Templates generalize that.

---

## 9. Architecture of the fork

Rule: **keep the diff in upstream-owned files small.** New work goes in new crates and
directories; upstream files get only thin hooks. That keeps weekly merges cheap.

```
openbox (fork of rbxport)
  crates/rbl-*               upstream crates, changed only through small hooks
  crates/ob-analysis/        the openbox analyzer: fuses rbxport's DSP stages with model
                             evidence and returns rbl_analysis's result types
  crates/ob-ml/              inference backends (ort, rten), model registry, batching
  crates/ob-engine/          hardware detection, bulk-job scheduler, throttle modes
  crates/ob-sidecar/         sidecar database: evidence, confidence, embeddings, suggestions
  crates/ob-bench/           evaluation harness and metrics (extends rbxport's golden rig)
  crates/ob-tags/            smart tagging
  crates/ob-cues/            automatic cue templates
  src-tauri/src/openbox/     Tauri commands for the above, registered with one hook
  src/views/openbox/         new React views (review queue, grid health, templates)
  research/                  Python: experiments, model training, ONNX export
  docs/openbox/              this plan, research notes, decision records
```

Upstream workflow:

- `main` tracks our work, and `upstream` is chrisle/rbxport. Merge upstream weekly, and
  run rbxport's checks plus ours on every merge.
- Bug fixes to upstream crates go to rbxport as pull requests first, so we don't carry
  them.
- If rbxport wants analyzer accuracy improvements (analysis is within its scope), offer
  the parts that fit its "fewer features" and performance rules.

---

## 10. Safety rules

- Follow rbxport's rules: develop against a temporary fixture library
  (`rbl_db::fixture::build`), run tests with `RB_LITE_TEST=1` so writes to the installed
  library are refused, and never use the installed library as a write fixture.
- Run the fork against a *copy* of your rekordbox library until Phase 1 passes, and back
  up before every bulk analysis or cue job.
- rekordbox must be closed while openbox writes; rbxport already enforces this.
- Audio files are never modified unless you turn on tag writing.

---

## 11. Risks

| Risk | Mitigation |
|---|---|
| Upstream moves very fast and could restructure the crates we hook into | thin hooks, weekly merges, upstreaming fixes |
| rbxport has one main maintainer and is under a month old | the fork owns everything it depends on; we can continue alone if needed |
| GPL-2.0-or-later | fine for personal use; any distribution must ship source |
| Model licenses (some popular models are non-commercial only) | track the license per model; prefer MIT models such as Beat This! |
| GPU results differ across hardware | quantize activations before decisions; CPU reference path (5.5, step 10) |
| ML makes analysis slower than rbxport's 460 ms per track | GPU batching, a standard and a deep tier, measured against the throughput target |
| AlphaTheta changes formats or encryption | inherited from rbxport, which tracks it; our hardware checklist |
| "Perfect every time" is impossible on some music | confidence scores, flags and the review queue |
| Naming: "openbox" is also a Linux window manager, and "openboxxx" is a Mixxx export project | fine for a personal fork; settle before any public release |

---

## 12. Open decisions

1. **Create the fork.** Done 2026-10-21: github.com/Calvin-Zikakis/openbox, checked out
   in this folder, with `upstream` pointing to chrisle/rbxport.
2. **Players.** Which players do you play on, yours and at venues? This defines the
   hardware checklist.
3. **Personal tool or product?** The code is GPL either way; this decides which models we
   can use.
4. **Ground truth.** Which of your rekordbox grids and cues are hand-corrected? (For
   example, locked grids, or a playlist of tracks you have fixed.)
5. **Your library.** How many tracks, which formats, and which genres? This sets tempo
   priors and which formats to verify first.
6. **Deep analysis tier.** Is an optional slower stem-separation pass acceptable for
   better cues and vocal detection?
7. **Upstream.** Offer analyzer improvements back to rbxport, or keep them in the fork?

---

## 13. References

rbxport

- Repository: https://github.com/chrisle/rbxport
- Site: https://rbxport.com/
- Analyzer docs: `crates/rbl-analysis/docs/` (overview, pipeline, algorithms, validation)
- Format references: `docs/reference/`

Formats and interoperability

- Deep Symmetry, DJ Link ecosystem analysis (export.pdb, ANLZ): https://djl-analysis.deepsymmetry.org/
- pyrekordbox: https://github.com/dylanljones/pyrekordbox
- rekordcrate: https://github.com/Holzhaus/rekordcrate
- OneLibrary / Device Library Plus notes: https://gist.github.com/0xdevalias/b803476793b56f7c45e6361799168eb0
- AlphaTheta, OneLibrary hardware support: https://support.alphatheta.com/en-US/articles/51298635657881
- AlphaTheta, USB notice for Device Library on newer players: https://alphatheta.com/en/information/important-notice-for-customers-using-usb-devices-with-our-dj-equipment/

Analysis

- Foscarin, Schlüter, Widmer. "Beat This! Accurate Beat Tracking Without DBN Postprocessing." ISMIR 2024. https://github.com/CPJKU/beat_this
- beat-this (Rust port): https://github.com/danigb/beat-this-rs
- Kim, Nam. "All-In-One Metrical and Functional Structure Analysis with Neighborhood Attentions on Demixed Audio." WASPAA 2023, arXiv:2307.16425. https://github.com/mir-aidj/all-in-one
- Zhao, Xia, Wang. "Beat Transformer: Demixed Beat and Downbeat Tracking with Dilated Self-Attention." ISMIR 2022.
- Böck, Davies. "Deconstruct, Analyse, Reconstruct: How to Improve Tempo, Beat, and Downbeat Estimation." ISMIR 2020.
- Schreiber, Müller. "A Single-Step Approach to Musical Tempo Estimation Using a Convolutional Neural Network." ISMIR 2018.
- Böck, Krebs, Widmer. "Joint Beat and Downbeat Tracking with Recurrent Neural Networks." ISMIR 2016. https://github.com/CPJKU/madmom
- Faraldo et al. "Key Estimation in Electronic Dance Music." ECIR 2016.
- Korzeniowski, Widmer. "Genre-Agnostic Key Classification with Convolutional Neural Networks." ISMIR 2018.
- Harmonix Set: https://github.com/urinieto/harmonixset
- GiantSteps Tempo: https://github.com/GiantSteps/giantsteps-tempo-dataset
- GiantSteps Key: https://github.com/GiantSteps/giantsteps-key-dataset

Rust ecosystem

- ort (ONNX Runtime): https://github.com/pykeio/ort
- Symphonia: https://github.com/pdeljanov/Symphonia
- Tauri: https://tauri.app
