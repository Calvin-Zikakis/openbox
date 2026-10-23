//! openbox's track analyzer.
//!
//! It runs rbxport's analysis stages (onset envelopes, kick attacks, the
//! tempo fit, band frames, key and waveform) and makes its own decisions
//! about where the grid sits: see [`phase`]. The result is rbxport's
//! [`Analysis`] type, so everything downstream (ANLZ files, `master.db`,
//! USB export) takes it unchanged, plus a [`Report`] of the evidence.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "DSP converts freely between sample counts and float time; every such cast is bounded by the buffer length"
)]

pub mod octave;
pub mod phase;
pub mod placement;

use rbl_analysis::{attack, downbeat, key, onset, tempo, waveform, Analysis, AnalysisOptions, AnalysisPreset};

/// Everything the analyzer is tuned by.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// rbxport's stage options.
    pub analysis: AnalysisOptions,
    /// The neural judge's model, or `None` to decide on DSP evidence alone.
    pub neural: Option<ob_ml::Size>,
}

impl Options {
    /// One of rbxport's presets, with the full neural model when it is
    /// installed.
    pub fn preset(preset: AnalysisPreset) -> Self {
        let neural = ob_ml::available(ob_ml::Size::Full).then_some(ob_ml::Size::Full);
        Self { analysis: preset.options(), neural }
    }

    /// rbxport's stage options as the caller tuned them, with the full neural
    /// model when it is installed.
    pub fn with(analysis: AnalysisOptions) -> Self {
        Self { analysis, ..Self::preset(AnalysisPreset::Rekordbox) }
    }

    #[must_use]
    pub fn without_neural(self) -> Self {
        Self { neural: None, ..self }
    }
}

/// Seconds of audio in each neural excerpt. The model's cost grows faster
/// than its input: 20 s takes 0.39 s on an M3 Pro, 25 s 0.52 s, but 30 s
/// 1.26 s (research note 004), so several short excerpts beat a few long ones.
const EXCERPT_SECS: f64 = 20.0;

/// Excerpts per track, one per equal part of it.
const EXCERPTS: usize = 4;

/// Where the neural model listens: the whole of a short track; otherwise the
/// loudest 20 seconds of each quarter, which in dance music is where the
/// drums play.
fn excerpt_windows(samples: &[f32], sample_rate: u32) -> Vec<(f64, f64)> {
    let rate = sample_rate as usize;
    if rate == 0 {
        return Vec::new();
    }
    let seconds = samples.len() / rate;
    if seconds as f64 <= EXCERPT_SECS * EXCERPTS as f64 * 1.25 {
        return vec![(0.0, seconds as f64 + 1.0)];
    }
    let energy: Vec<f64> = samples.chunks(rate).map(|c| c.iter().map(|s| f64::from(*s) * f64::from(*s)).sum::<f64>()).collect();
    let mut prefix = vec![0.0; energy.len() + 1];
    for (i, e) in energy.iter().enumerate() {
        prefix[i + 1] = prefix[i] + e;
    }
    let length = EXCERPT_SECS as usize;
    let part = seconds / EXCERPTS;
    (0..EXCERPTS)
        .filter_map(|k| {
            let (from, to) = (k * part, ((k + 1) * part).min(energy.len()).saturating_sub(length));
            (from..=to.max(from)).filter(|s| s + length <= energy.len()).max_by(|a, b| (prefix[a + length] - prefix[*a]).total_cmp(&(prefix[b + length] - prefix[*b])))
        })
        .map(|start| (start as f64, EXCERPT_SECS))
        .collect()
}

/// What the analyzer saw, beside what it decided.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub octave: octave::OctaveReport,
    pub phase: phase::PhaseReport,
    pub placement: placement::PlacementReport,
}

#[derive(Debug, Clone)]
pub struct ObAnalysis {
    pub analysis: Analysis,
    pub report: Report,
}

/// Runs the full analysis.
#[allow(clippy::too_many_lines, reason = "one orchestration of independent stages")]
pub fn analyse(samples: &[f32], sample_rate: u32, options: &Options) -> ObAnalysis {
    let neural_size = options.neural;
    let options = options.analysis;
    // The same independent passes rbxport runs, each on its own thread, and
    // the neural model beside them.
    let (onsets, kicks, attacks, frames, waveform, level, found_key, excerpts) = std::thread::scope(|scope| {
        let excerpts = scope.spawn(|| neural_size.and_then(|size| ob_ml::excerpts(samples, sample_rate, size, &excerpt_windows(samples, sample_rate))));
        let onsets = scope.spawn(|| onset::onset_envelope(samples, sample_rate));
        let kicks = scope.spawn(|| onset::onset_envelope_band(samples, sample_rate, onset::Band::LOW));
        let attacks = scope.spawn(|| match options.tempo.placement {
            tempo::Placement::Attack => Some(attack::AttackMap::new(samples, sample_rate, options.attacks)),
            tempo::Placement::Envelope => None,
        });
        let frames = scope.spawn(|| downbeat::band_frames(samples, sample_rate));
        let waveform = scope.spawn(|| waveform::compute(samples, sample_rate));
        let level = scope.spawn(|| level(samples));
        let key = scope.spawn(|| {
            if key::wants_grid(key::DEFAULT_RULES) {
                return None;
            }
            let no_grid = key::KeyGrid { beats: Vec::new(), phrase_starts: Vec::new() };
            Some(key::detect_key_with(samples, sample_rate, options.key, key::DEFAULT_RULES, &no_grid))
        });
        (
            onsets.join().unwrap_or_else(|_| onset::OnsetEnvelope { values: Vec::new(), rate: 0.0, origin_secs: 0.0 }),
            kicks.join().ok(),
            attacks.join().ok().flatten(),
            frames.join().unwrap_or_default(),
            waveform.join().unwrap_or_else(|_| waveform::compute(&[], sample_rate)),
            level.join().unwrap_or((0.0, 0.0)),
            key.join().ok().flatten(),
            excerpts.join().ok().flatten(),
        )
    });

    let mut tempo = tempo::detect_tempo_with(&onsets, kicks.as_ref(), attacks.as_ref(), options.tempo);
    let mut octave = octave::judge(tempo.bpm, excerpts.as_deref());
    if let Some(bpm) = octave.refit_bpm {
        let refit = tempo::detect_tempo_at(&onsets, kicks.as_ref(), attacks.as_ref(), options.tempo, bpm);
        // The fit may wander back to the old tempo; keep it only if it stayed.
        if !refit.segments.is_empty() && (refit.bpm / bpm - 1.0).abs() <= 0.02 {
            tempo = refit;
            octave.applied = true;
        }
    }
    let (downbeat_secs, phase) = phase::resolve(&mut tempo, &frames, kicks.as_ref(), excerpts.as_deref(), sample_rate);
    tempo.beats = tempo::beats_of(&tempo.segments, rbl_analysis::phase_for(&tempo.segments, downbeat_secs));
    rbl_analysis::anchor_file_start(&mut tempo, samples, sample_rate, downbeat_secs);
    let placement = placement::refine(&mut tempo, samples, sample_rate);

    let key = found_key.unwrap_or_else(|| {
        let key_grid = key::KeyGrid {
            beats: tempo.beats.iter().map(|b| (f64::from(b.time_ms) / 1000.0, b.beat_number)).collect(),
            phrase_starts: Vec::new(),
        };
        key::detect_key_with(samples, sample_rate, options.key, key::DEFAULT_RULES, &key_grid)
    });
    let (peak, rms) = level;
    ObAnalysis {
        analysis: Analysis { tempo, key: key.map(|report| report.key), waveform, peak, rms },
        report: Report { octave, phase, placement },
    }
}

/// Peak sample magnitude and RMS over the whole track.
fn level(samples: &[f32]) -> (f32, f32) {
    let mut peak = 0.0_f32;
    let mut sum_squares = 0.0_f64;
    for &s in samples {
        peak = peak.max(s.abs());
        sum_squares += f64::from(s) * f64::from(s);
    }
    let rms = if samples.is_empty() { 0.0 } else { (sum_squares / samples.len() as f64).sqrt() as f32 };
    (peak, rms)
}
