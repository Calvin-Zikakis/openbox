# Changes to upstream files

openbox keeps its own code in `ob-*` crates and `docs/openbox/`. This file lists every
change made to a file that rbxport owns, so a merge from upstream knows what to keep.
Keep each hook as small as possible, and add a line here in the same commit.

| File | Change | Why |
| --- | --- | --- |
| `crates/rbl-analysis/src/lib.rs` | `phase_for` and `anchor_file_start` made `pub` | `ob-analysis` runs rbxport's stages with its own phase decision, then numbers and anchors the grid exactly as rbxport does |
| `.gitignore` | `/models/` and `/vendor/onnxruntime/` appended | neural models and the ONNX Runtime library are downloaded by `scripts/openbox/fetch-*.sh`, never committed |
| `.github/copilot-instructions.md` | new file: rules for coding agents, pointing to `docs/openbox/HANDOFF.md` | Copilot loads it automatically |
| `README.md` | a four-line banner at the top pointing to `docs/openbox/` | so the fork says what it is on GitHub |
| `src-tauri/Cargo.toml` | depends on `ob-analysis` | the app's "openbox" analysis mode |
| `src-tauri/src/analysis.rs` | `Engine` enum; `AnalysisSettings.engine` (never sent by the frontend); mode `"openbox"`; `analyse_file_with`, with `analyse_file` kept for tests; one new test | runs `ob-analysis` for the "openbox" mode without changing `analyse_and_save`'s signature |
| `src/lib/preferences.ts`, `src/lib/analysisChoice.ts`, `src/ipc/types.ts`, `src/views/analysis/AnalysisDialog.tsx`, `src/views/settings/AnalysisPane.tsx` | `"openbox"` analysis mode, its option, its explanation, and the default | choosing the openbox analyzer in the app |
| `crates/rbl-analysis/src/downbeat.rs` | `novelty_peaks`, `SCALES`, `MIN_STRUCTURE_NOVELTY` and `POSITIONS` made `pub` | `ob-analysis::phase` reuses rbxport's structural scoring and its "nothing changes" test instead of copying them |

No behavior of rbxport's own analyzer changes. Its tests pass unchanged
(`RB_LITE_TEST=1 cargo test -p rbl-analysis`).

## Merging upstream

```sh
git fetch upstream
git merge upstream/main
```

If a hook conflicts, keep upstream's version and reapply the visibility change. Then
run the checks in [LOG.md](LOG.md)'s most recent entry.
