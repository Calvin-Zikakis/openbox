//! Moving the finished grid onto the start of each transient, to a fraction
//! of a millisecond.
//!
//! rbxport finds kick attacks on an RMS envelope in 1 ms steps, and its grid
//! lands about a millisecond early on every track (research note 001). That
//! is a fixed bias, not noise, so it is removed as one: measure where each
//! beat's transient really starts, take the median offset over every beat
//! with a clear transient, and move the whole grid by it. Averaging over
//! hundreds of beats is what makes the estimate precise. Moving beats one by
//! one would turn each beat's measurement noise into jitter.
//!
//! The onset of one transient is the first moment its energy, in a
//! high-frequency emphasis of the signal smoothed over a quarter of a
//! millisecond, rises a tenth of the way from the level before it to its
//! peak, interpolated between samples.

use rbl_analysis::tempo::{self, TempoResult};

/// What the stage measured and did.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlacementReport {
    /// The move applied to the grid, in ms; positive is later.
    pub shift_ms: f64,
    /// The median offset measured, in ms, whether or not it was applied.
    pub measured_ms: f64,
    /// Beats with a clear transient near their grid line.
    pub beats_used: usize,
    /// Median absolute deviation of the offsets, in ms.
    pub spread_ms: f64,
    pub applied: bool,
}

/// Search either side of a grid line for its transient.
const REACH_SECS: f64 = 0.006;
/// The quiet before a transient, measured just before the search window.
const BEFORE_SECS: f64 = 0.006;
/// Energy smoothing window.
const SMOOTH_SECS: f64 = 0.000_25;
/// A transient's peak energy must exceed the level before it by this factor.
const CLEAR: f64 = 8.0;
/// Share of the rise from the level before to the peak that marks the onset.
const THRESHOLD: f64 = 0.1;
/// Fewest clear transients, and the widest spread, for a move to be trusted.
const MIN_BEATS: usize = 16;
const MAX_SPREAD_MS: f64 = 1.0;
/// Largest move the stage makes: past this the grid is wrong in a way a
/// constant shift should not paper over.
const MAX_SHIFT_MS: f64 = 4.0;

/// The onset of the transient nearest `secs`, if there is a clear one.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss, reason = "sample indices from bounded, non-negative times")]
fn onset_near(samples: &[f32], sample_rate: u32, secs: f64) -> Option<f64> {
    let rate = f64::from(sample_rate);
    let smooth = ((SMOOTH_SECS * rate).round() as usize).max(1);
    let lo = ((secs - REACH_SECS - BEFORE_SECS) * rate).floor();
    let hi = ((secs + REACH_SECS) * rate).ceil() as usize + smooth + 2;
    if lo < 2.0 || hi >= samples.len() {
        return None;
    }
    let lo = lo as usize;
    // Second difference: a high-frequency emphasis that leaves a transient's
    // front and flattens the low notes and kick bodies already sounding.
    let emphasis: Vec<f64> = (lo..hi).map(|n| f64::from(samples[n]) - 2.0 * f64::from(samples[n - 1]) + f64::from(samples[n - 2])).collect();
    // Energy, smoothed over a centred window so the smoothing adds no delay.
    let half = smooth / 2;
    let energy: Vec<f64> = (0..emphasis.len())
        .map(|i| {
            let a = i.saturating_sub(half);
            let b = (i + half + 1).min(emphasis.len());
            emphasis[a..b].iter().map(|x| x * x).sum::<f64>() / (b - a) as f64
        })
        .collect();
    let before_end = (BEFORE_SECS * rate) as usize;
    let mut before: Vec<f64> = energy[..before_end.min(energy.len())].to_vec();
    before.sort_by(f64::total_cmp);
    let level = before.get(before.len() / 2).copied().unwrap_or(0.0);
    let window = &energy[before_end.min(energy.len())..];
    let peak = window.iter().copied().fold(0.0_f64, f64::max);
    if peak <= level * CLEAR || peak <= 0.0 {
        return None;
    }
    let mark = level + THRESHOLD * (peak - level);
    let at = window.iter().position(|&e| e >= mark)?;
    // Interpolate between the sample below the mark and the one at it.
    let index = before_end + at;
    let fraction = if index > 0 && energy[index] > energy[index - 1] {
        (mark - energy[index - 1]) / (energy[index] - energy[index - 1])
    } else {
        1.0
    };
    Some((lo + index) as f64 / rate - (1.0 - fraction) / rate)
}

/// Measures the grid's offset from the transients and, when the measurement
/// is trustworthy, moves the grid by it, keeping beat 1 where it was.
pub fn refine(tempo: &mut TempoResult, samples: &[f32], sample_rate: u32) -> PlacementReport {
    if sample_rate == 0 || tempo.beats.is_empty() {
        return PlacementReport::default();
    }
    let mut offsets: Vec<f64> = tempo
        .segments
        .iter()
        .flat_map(|segment| {
            let start = segment.start_secs();
            (0..segment.beats()).map(move |i| start + i as f64 * segment.period_secs)
        })
        .filter_map(|beat| onset_near(samples, sample_rate, beat).map(|onset| (onset - beat) * 1000.0))
        .collect();
    if offsets.is_empty() {
        return PlacementReport::default();
    }
    offsets.sort_by(f64::total_cmp);
    let median = offsets[offsets.len() / 2];
    let mut deviations: Vec<f64> = offsets.iter().map(|o| (o - median).abs()).collect();
    deviations.sort_by(f64::total_cmp);
    let spread = deviations[deviations.len() / 2];
    let applied = offsets.len() >= MIN_BEATS && spread <= MAX_SPREAD_MS && median.abs() <= MAX_SHIFT_MS;
    let report = PlacementReport { shift_ms: if applied { median } else { 0.0 }, measured_ms: median, beats_used: offsets.len(), spread_ms: spread, applied };
    if !applied || median == 0.0 {
        return report;
    }
    let shift = median / 1000.0;
    // Beat 1 by time, so a beat crossing the file's start cannot renumber the count.
    let downbeat = tempo.beats.iter().find(|b| b.beat_number == 1).map(|b| f64::from(b.time_ms) / 1000.0);
    // Every boundary moves with the beats, except the file's start and end,
    // so no segment's first beat slips across its own start.
    let last = tempo.segments.len().saturating_sub(1);
    for (i, segment) in tempo.segments.iter_mut().enumerate() {
        // The one-beat opening segment `anchor_file_start` puts at the file's
        // first sample stays there: no transient before it can be measured.
        let pinned = i == 0 && i < last && segment.from_secs == 0.0 && segment.phase_secs == 0.0;
        if !pinned {
            segment.phase_secs += shift;
        }
        if i > 0 {
            segment.from_secs += shift;
        }
        if i < last {
            segment.to_secs += shift;
        }
    }
    tempo.first_beat_secs = tempo.segments.first().map_or(0.0, tempo::Segment::start_secs);
    let phase = downbeat.map_or(0, |d| rbl_analysis::phase_for(&tempo.segments, d + shift));
    tempo.beats = tempo::beats_of(&tempo.segments, phase);
    report
}
