//! Neural beat and downbeat evidence.
//!
//! [Beat This!](https://github.com/CPJKU/beat_this) (Foscarin, Schlüter and
//! Widmer, ISMIR 2024) predicts, for every 20 ms frame, how likely a beat and
//! a downbeat are there. openbox does not use its picked beats: the grid comes
//! from kick attacks, which are far more precise than 20 ms frames. It uses
//! the activations as one more judge of the questions the DSP finds hardest —
//! which half of the beat the kicks are on, and which beat is 1.
//!
//! Models are ONNX files fetched by `scripts/openbox/fetch-models.sh` into
//! `models/`, or wherever `OPENBOX_MODELS` points. Without them,
//! [`activations`] returns `None` and analysis runs on DSP evidence alone.

#![allow(clippy::cast_precision_loss, reason = "frame and sample counts to seconds")]

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use beat_this::{BeatThis, RtenRuntime, Runtime};

/// The loaded model type; the crate does not export it by name.
type Loaded = BeatThis<<RtenRuntime as Runtime>::Model>;

/// Frames per second of the activations.
pub const FPS: f64 = 50.0;

/// Per-frame probabilities, frame `i` centred at `i / FPS` seconds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Activations {
    pub beat: Vec<f32>,
    pub downbeat: Vec<f32>,
}

impl Activations {
    /// The strongest beat probability within `reach` seconds of `secs`.
    pub fn beat_near(&self, secs: f64, reach: f64) -> f32 {
        peak(&self.beat, secs, reach)
    }

    /// The strongest downbeat probability within `reach` seconds of `secs`.
    pub fn downbeat_near(&self, secs: f64, reach: f64) -> f32 {
        peak(&self.downbeat, secs, reach)
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "frame indices from non-negative times")]
fn peak(values: &[f32], secs: f64, reach: f64) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let lo = ((secs - reach) * FPS).round().max(0.0) as usize;
    let hi = (((secs + reach) * FPS).round().max(0.0) as usize).min(values.len() - 1);
    values.get(lo..=hi).map_or(0.0, |w| w.iter().copied().fold(0.0, f32::max))
}

/// Which beat model to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    /// `beat_this.onnx`, about 83 MB: the published accuracy.
    #[default]
    Full,
    /// `beat_this_small.onnx`, about 10 MB.
    Small,
}

/// Where models are looked for: `OPENBOX_MODELS`, else `models/` under the
/// current directory, else under the workspace this crate was built in.
pub fn models_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("OPENBOX_MODELS") {
        return PathBuf::from(dir);
    }
    let here = PathBuf::from("models");
    if here.join("mel_spectrogram.onnx").is_file() {
        return here;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models")
}

fn model_paths(dir: &Path, size: Size) -> (PathBuf, PathBuf) {
    let beat = match size {
        Size::Full => "beat_this.onnx",
        Size::Small => "beat_this_small.onnx",
    };
    (dir.join("mel_spectrogram.onnx"), dir.join(beat))
}

/// Whether the models for `size` are installed.
pub fn available(size: Size) -> bool {
    let (mel, beat) = model_paths(&models_dir(), size);
    mel.is_file() && beat.is_file()
}

thread_local! {
    // One loaded model per thread and size: loading parses tens of megabytes,
    // and the model needs `&mut` to run, so threads cannot share one.
    static LOADED: RefCell<Vec<(Size, Loaded)>> = const { RefCell::new(Vec::new()) };
}

/// Activations over one stretch of a track: frame `i` is centred at
/// `start_secs + i / FPS` seconds into the track.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Excerpt {
    pub start_secs: f64,
    pub activations: Activations,
}

impl Excerpt {
    pub fn end_secs(&self) -> f64 {
        self.start_secs + self.activations.beat.len() as f64 / FPS
    }

    /// Whether `secs` lies inside the excerpt, at least `margin` seconds from
    /// either edge (the model sees less context near an edge).
    pub fn covers(&self, secs: f64, margin: f64) -> bool {
        secs >= self.start_secs + margin && secs <= self.end_secs() - margin
    }

    pub fn beat_near(&self, secs: f64, reach: f64) -> f32 {
        self.activations.beat_near(secs - self.start_secs, reach)
    }

    pub fn downbeat_near(&self, secs: f64, reach: f64) -> f32 {
        self.activations.downbeat_near(secs - self.start_secs, reach)
    }
}

/// Activations for stretches of a track, each `(start, length)` in seconds,
/// or `None` when the models are missing or inference fails. Running the
/// model on a few excerpts costs a fraction of the whole track.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "sample indices from non-negative times")]
pub fn excerpts(samples: &[f32], sample_rate: u32, size: Size, windows: &[(f64, f64)]) -> Option<Vec<Excerpt>> {
    let rate = f64::from(sample_rate);
    let mut out = Vec::with_capacity(windows.len());
    for &(start, length) in windows {
        let from = ((start * rate) as usize).min(samples.len());
        let to = (((start + length) * rate) as usize).min(samples.len());
        if to <= from {
            continue;
        }
        let activations = activations(&samples[from..to], sample_rate, size)?;
        out.push(Excerpt { start_secs: from as f64 / rate, activations });
    }
    Some(out)
}

/// Beat and downbeat probabilities for mono audio at any sample rate, or
/// `None` when the models are missing or inference fails.
pub fn activations(samples: &[f32], sample_rate: u32, size: Size) -> Option<Activations> {
    LOADED.with(|loaded| {
        let mut loaded = loaded.borrow_mut();
        if !loaded.iter().any(|(s, _)| *s == size) {
            let (mel, beat) = model_paths(&models_dir(), size);
            let model = BeatThis::new(&RtenRuntime, &mel, &beat).ok()?;
            loaded.push((size, model));
        }
        let (_, model) = loaded.iter_mut().find(|(s, _)| *s == size)?;
        let analysis = model.analyze_audio(samples, sample_rate).ok()?;
        Some(Activations { beat: analysis.beat_logits.iter().map(|&x| sigmoid(x)).collect(), downbeat: analysis.downbeat_logits.iter().map(|&x| sigmoid(x)).collect() })
    })
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
