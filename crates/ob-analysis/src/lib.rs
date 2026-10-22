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

pub mod phase;

use rbl_analysis::{attack, downbeat, key, onset, tempo, waveform, Analysis, AnalysisOptions};

/// What the analyzer saw, beside what it decided.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub phase: phase::PhaseReport,
}

#[derive(Debug, Clone)]
pub struct ObAnalysis {
    pub analysis: Analysis,
    pub report: Report,
}

/// Runs the full analysis.
#[allow(clippy::needless_pass_by_value, reason = "a Copy options struct")]
pub fn analyse(samples: &[f32], sample_rate: u32, options: AnalysisOptions) -> ObAnalysis {
    // The same independent passes rbxport runs, each on its own thread.
    let (onsets, kicks, attacks, frames, waveform, level, found_key) = std::thread::scope(|scope| {
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
        )
    });

    let mut tempo = tempo::detect_tempo_with(&onsets, kicks.as_ref(), attacks.as_ref(), options.tempo);
    let (downbeat_secs, phase) = phase::resolve(&mut tempo, &frames, kicks.as_ref(), sample_rate);
    tempo.beats = tempo::beats_of(&tempo.segments, rbl_analysis::phase_for(&tempo.segments, downbeat_secs));
    rbl_analysis::anchor_file_start(&mut tempo, samples, sample_rate, downbeat_secs);

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
        report: Report { phase },
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
