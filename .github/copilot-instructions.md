# Copilot instructions for openbox

openbox is a fork of rbxport (Rust and Tauri 2 rekordbox library manager) that adds a
more precise beat-grid analyzer. **Read `docs/openbox/HANDOFF.md` before starting any
task,** then the newest entry in `docs/openbox/LOG.md`.

Rules that always apply:

- Never write to the owner's installed rekordbox library. Run Rust tests with
  `RB_LITE_TEST=1`.
- Never commit track titles from the owner's music, audio files, `models/`, or
  `vendor/onnxruntime/`. The repository is public.
- Put openbox code in `crates/ob-*` and `docs/openbox/`. Keep changes to rbxport-owned
  files as thin hooks, and list each in `docs/openbox/upstream-hooks.md` in the same
  commit.
- Keep `cargo clippy` clean. The workspace denies `unwrap`, `expect`, `panic` and
  `print_stdout` outside tests.
- Measure analyzer changes with `ob-bench eval` before claiming an improvement, and
  record results in `docs/openbox/research/`.
- No emojis in documentation.
- Ask the owner before committing unreviewed work, pushing, or opening pull requests
  upstream.

Before any release build, run
`export CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`. Without
it, a release build takes about 20 minutes.
