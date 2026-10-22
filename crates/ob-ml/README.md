# ob-ml

Neural evidence for openbox's analyzer: per-frame beat and downbeat probabilities from
[Beat This!](https://github.com/CPJKU/beat_this), run through the
[`beat-this`](https://github.com/danigb/beat-this-rs) crate on its pure-Rust `rten`
backend.

```sh
scripts/openbox/fetch-models.sh          # mel front end and the small model
scripts/openbox/fetch-models.sh --full   # plus the full model (83 MB), which analysis prefers
```

Models go in `models/` (git-ignored), or wherever `OPENBOX_MODELS` points. Without
them, `ob_ml::activations` returns `None` and analysis uses DSP evidence only.

```rust
let windows = [(30.0, 20.0), (120.0, 20.0)]; // (start, length) in seconds
let excerpts = ob_ml::excerpts(&mono, sample_rate, ob_ml::Size::Full, &windows);
```

Each thread loads its own copy of a model on first use: the model needs `&mut` to run.
Timing and accuracy are in research note
[004](../../docs/openbox/research/004-neural-judge.md).
