//! Which half of the beat the grid sits on, and which beat is 1.
//!
//! rbxport answers both with one vote: the half-beat position in the bar
//! where the arrangement changes most. That works when a change lands
//! squarely on the downbeat, but a change usually straddles the downbeat and
//! the half beat after it (an off-beat bass line entering, a lead's first
//! note on the "and"), and the two scores then sit close together. When the
//! half after wins, rbxport moves a correctly placed grid onto the off-beat.
//! On ob-bench's synthetic house tracks that is three in ten.
//!
//! Here the two questions are answered apart:
//!
//! 1. **On the beat or off it.** The kick band decides when it is decisive:
//!    kicks land on beats, and in four-to-the-floor music the kick band
//!    carries several times more onset energy on the beat than between.
//!    Structure decides only when the kick band cannot (no kick, or an
//!    off-beat bass as strong as the kick), by comparing the total change on
//!    beats with the total between them.
//! 2. **Which beat is 1.** Each beat's score is its own structural score
//!    plus half of each neighbouring half beat's, so a change that straddles
//!    the downbeat still votes for the downbeat.

use rbl_analysis::downbeat::{self, BANDS};
use rbl_analysis::onset::OnsetEnvelope;
use rbl_analysis::tempo::{self, TempoResult};

/// Kick-band onset energy on beats over that between them, beyond which the
/// kick band alone decides the half-beat question (and its inverse below).
pub const KICK_DECISIVE: f64 = 2.0;

/// Who settled the half-beat question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judge {
    Kick,
    Structure,
    /// No usable evidence either way: the fitted grid is kept.
    Neither,
}

/// What the phase stage saw and decided, for confidence and diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseReport {
    /// Kick-band onset strength on the fitted grid's beats over that on its
    /// midpoints. Above [`KICK_DECISIVE`] the fit is on the beat.
    pub kick_ratio: f64,
    /// Structural change summed over the grid's beats and over its midpoints.
    pub structure_on: f64,
    pub structure_off: f64,
    pub moved_half_beat: bool,
    pub judge: Judge,
    /// Bar-position scores for the four beats of the final grid, counted
    /// from its first beat.
    pub bar_scores: [f64; 4],
    pub bar: usize,
    /// Best bar score over the second best: near 1 is a guess.
    pub bar_margin: f64,
    /// Whether the arrangement changes enough to say anything about bars.
    pub structured: bool,
}

/// Unnumbered beat times of a grid, in seconds.
fn beat_secs(segments: &[tempo::Segment]) -> Vec<f64> {
    tempo::beats_of(segments, 0).iter().map(|b| f64::from(b.time_ms) / 1000.0).collect()
}

/// Every beat and the midpoint after it.
fn halves_of(beats: &[f64]) -> Vec<f64> {
    let mut halves = Vec::with_capacity(beats.len() * 2);
    for pair in beats.windows(2) {
        halves.push(pair[0]);
        halves.push(pair[0].midpoint(pair[1]));
    }
    halves
}

/// The strongest envelope value within `reach` seconds of `secs`.
fn peak_near(envelope: &OnsetEnvelope, secs: f64, reach: f64) -> f64 {
    if envelope.rate <= 0.0 || envelope.values.is_empty() {
        return 0.0;
    }
    let index = |t: f64| ((t - envelope.origin_secs) * envelope.rate).round();
    let lo = index(secs - reach).max(0.0) as usize;
    let hi = (index(secs + reach).max(0.0) as usize).min(envelope.values.len() - 1);
    envelope.values.get(lo..=hi).map_or(0.0, |w| f64::from(w.iter().copied().fold(0.0_f32, f32::max)))
}

/// Kick-band strength on beats over that on midpoints.
pub fn kick_ratio(kicks: &OnsetEnvelope, beats: &[f64]) -> f64 {
    let (mut on, mut off) = (0.0, 0.0);
    for pair in beats.windows(2) {
        let period = pair[1] - pair[0];
        // An eighth of a beat either way: the envelope's frame timing and a
        // millisecond-level grid error both fit inside it, the other half does not.
        let reach = period / 8.0;
        on += peak_near(kicks, pair[0], reach);
        off += peak_near(kicks, pair[0] + period / 2.0, reach);
    }
    if off <= 0.0 {
        return if on > 0.0 { f64::INFINITY } else { 1.0 };
    }
    on / off
}

/// Structural scores for the eight half-beat positions from the first beat,
/// and whether the track changes enough for them to mean anything.
fn structure(frames: &[[f64; BANDS]], sample_rate: u32, beats: &[f64]) -> (Vec<f64>, bool) {
    let halves = halves_of(beats);
    let profiles = downbeat::beat_profiles_from(frames, sample_rate, &halves);
    if profiles.len() < downbeat::POSITIONS * 2 {
        return (vec![0.0; downbeat::POSITIONS], false);
    }
    // rbxport's own test for "nothing changes over four bars".
    let structured = downbeat::novelty_peaks(&profiles, 32).iter().any(|&(_, v)| v >= downbeat::MIN_STRUCTURE_NOVELTY);
    if !structured {
        return (vec![0.0; downbeat::POSITIONS], false);
    }
    (downbeat::position_scores(&profiles, downbeat::POSITIONS, &downbeat::SCALES), true)
}

/// Decides the grid's half beat (moving its segments if they are on the
/// off-beat) and its downbeat. Returns the downbeat's time.
pub fn resolve(tempo: &mut TempoResult, frames: &[[f64; BANDS]], kicks: Option<&OnsetEnvelope>, sample_rate: u32) -> (f64, PhaseReport) {
    let beats = beat_secs(&tempo.segments);
    let ratio = kicks.map_or(1.0, |k| kick_ratio(k, &beats));
    let (scores, structured) = structure(frames, sample_rate, &beats);
    let on: f64 = scores.iter().step_by(2).sum();
    let off: f64 = scores.iter().skip(1).step_by(2).sum();
    let (moved, judge) = if ratio >= KICK_DECISIVE {
        (false, Judge::Kick)
    } else if ratio <= 1.0 / KICK_DECISIVE {
        (true, Judge::Kick)
    } else if structured {
        (off > on, Judge::Structure)
    } else {
        (false, Judge::Neither)
    };
    if moved {
        for segment in &mut tempo.segments {
            *segment = segment.shifted_half_beat();
        }
        tempo.first_beat_secs = tempo.segments.first().map_or(0.0, tempo::Segment::start_secs);
    }

    // Beat 1 from the first tempo's own beats when it is long enough to
    // have phrases, as rbxport does, so later tempos cannot outvote it.
    let own = match tempo.segments.first() {
        Some(first) if tempo.segments.len() > 1 && first.beats() >= downbeat::MIN_BEATS_FOR_OWN_PHASE => beat_secs(&tempo.segments[..1]),
        _ => beat_secs(&tempo.segments),
    };
    let (scores, bar_structured) = structure(frames, sample_rate, &own);
    let mut bar_scores = [0.0_f64; 4];
    for (p, score) in bar_scores.iter_mut().enumerate() {
        let at = 2 * p;
        *score = scores[at] + f64::midpoint(scores[(at + 7) % 8], scores[at + 1]);
    }
    let mut ranked: Vec<usize> = (0..4).collect();
    ranked.sort_by(|a, b| bar_scores[*b].total_cmp(&bar_scores[*a]));
    let bar = if bar_structured { ranked[0] } else { 0 };
    let second = bar_scores[ranked[1]];
    let bar_margin = if second > 0.0 { bar_scores[ranked[0]] / second } else if bar_structured { f64::INFINITY } else { 1.0 };
    let downbeat_secs = own.get(bar).or_else(|| own.first()).copied().unwrap_or(0.0);
    let report = PhaseReport {
        kick_ratio: ratio,
        structure_on: on,
        structure_off: off,
        moved_half_beat: moved,
        judge,
        bar_scores,
        bar,
        bar_margin,
        structured: structured || bar_structured,
    };
    (downbeat_secs, report)
}
