//! Scoring a predicted grid against the truth, to the millisecond.
//!
//! Two questions are kept apart. **Placement**: is each grid line on the
//! beat, measured as the signed time from the true beat to the nearest
//! predicted one. **Numbering**: is beat 1 on the bar's first beat, measured
//! as the offset between the predicted and the true beat number at each
//! matched beat. A grid can be perfectly placed and still start the bar on
//! beat 2, which is the failure DJs notice first.

use serde::Serialize;

use crate::truth::Truth;

/// A predicted beat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictedBeat {
    pub secs: f64,
    /// 1..=4.
    pub number: u8,
    /// The time as exported, in whole milliseconds (`PQTZ` precision).
    pub export_ms: u32,
}

/// What an analyzer said about a track.
#[derive(Debug, Clone, Default)]
pub struct Prediction {
    pub bpm: f64,
    /// Sorted by time.
    pub beats: Vec<PredictedBeat>,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Spread {
    pub median: f64,
    pub p95: f64,
    pub max: f64,
}

fn spread(mut values: Vec<f64>) -> Spread {
    if values.is_empty() {
        return Spread { median: f64::NAN, p95: f64::NAN, max: f64::NAN };
    }
    values.sort_by(f64::total_cmp);
    let at = |q: f64| values[((values.len() - 1) as f64 * q).round() as usize];
    Spread { median: at(0.5), p95: at(0.95), max: values[values.len() - 1] }
}

/// How a predicted tempo relates to the true one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Octave {
    Same,
    Double,
    Half,
    ThreeHalves,
    TwoThirds,
    Other,
    None,
}

fn octave(predicted: f64, truth: f64) -> Octave {
    if predicted <= 0.0 {
        return Octave::None;
    }
    let ratio = predicted / truth;
    let near = |r: f64| (ratio / r - 1.0).abs() < 0.02;
    if near(1.0) {
        Octave::Same
    } else if near(2.0) {
        Octave::Double
    } else if near(0.5) {
        Octave::Half
    } else if near(1.5) {
        Octave::ThreeHalves
    } else if near(2.0 / 3.0) {
        Octave::TwoThirds
    } else {
        Octave::Other
    }
}

/// One analyzer on one track.
#[derive(Debug, Clone, Serialize)]
pub struct Score {
    pub track: String,
    pub analyzer: String,
    pub tags: Vec<String>,
    pub bpm_true: f64,
    pub bpm_predicted: f64,
    pub bpm_error: f64,
    pub octave: Octave,
    /// True beats compared.
    pub beats: usize,
    /// True beats with a predicted beat within a quarter beat.
    pub matched: usize,
    /// |predicted − true| to the nearest predicted beat, in ms, at full precision.
    pub abs_error_ms: Spread,
    /// The same after rounding to whole milliseconds, as a USB stick holds it.
    pub export_error_ms: Spread,
    /// Mean signed error in ms: positive is a grid late against the music.
    pub bias_ms: f64,
    /// Error at the end of the track minus at the start, from a line fit, in ms.
    pub drift_ms: f64,
    pub within_1ms: f64,
    pub within_2ms: f64,
    pub within_5ms: f64,
    pub within_25ms: f64,
    /// The predicted number minus the true one, mod 4, that most matched
    /// beats agree on: 0 is right, 1 means the grid calls beat 2 "1", and so on.
    pub bar_offset: u8,
    /// Share of matched beats carrying the dominant offset.
    pub bar_offset_share: f64,
    /// Share of true downbeats whose matched predicted beat is numbered 1.
    pub downbeat_share: f64,
    /// rbxport's own gate: at least 98 % of true beats have a predicted beat
    /// within 25 ms with the same number.
    pub pass_rbxport_25ms: bool,
    /// openbox's gate: right octave, every exported beat within 2 ms, and at
    /// least 98 % of downbeats numbered 1.
    pub pass_openbox: bool,
    pub runtime_ms: f64,
}

/// Seconds at the end of a file that are not scored: the last beat of a
/// grid extended to the end is not evidence of anything.
const TAIL_SECS: f64 = 0.25;

pub fn score(truth: &Truth, prediction: &Prediction, analyzer: &str, runtime_ms: f64) -> Score {
    let predicted: Vec<f64> = prediction.beats.iter().map(|b| b.secs).collect();
    let mut errors = Vec::new();
    let mut export_errors = Vec::new();
    let mut signed = Vec::new();
    let mut offsets = [0_usize; 4];
    let mut downbeats = 0_usize;
    let mut downbeats_right = 0_usize;
    let mut same_number_25ms = 0_usize;
    let mut compared = 0_usize;
    let mut matched = 0_usize;

    for (i, beat) in truth.beats.iter().enumerate() {
        if beat.secs > truth.duration_secs - TAIL_SECS {
            continue;
        }
        compared += 1;
        let period = local_period(truth, i);
        if beat.number == 1 {
            downbeats += 1;
        }
        let Some(j) = nearest(&predicted, beat.secs) else { continue };
        let p = prediction.beats[j];
        let error = p.secs - beat.secs;
        let ms = error * 1000.0;
        errors.push(ms.abs());
        signed.push((beat.secs, ms));
        export_errors.push((f64::from(p.export_ms) - beat.secs * 1000.0).abs());
        // Further than a quarter beat is not this beat at all (an off-beat
        // grid sits half a beat away), so it says nothing about numbering.
        if error.abs() > period / 4.0 {
            continue;
        }
        matched += 1;
        let offset = (i32::from(p.number) - i32::from(beat.number)).rem_euclid(4) as usize;
        offsets[offset] += 1;
        if beat.number == 1 && p.number == 1 {
            downbeats_right += 1;
        }
        if ms.abs() <= 25.0 && p.number == beat.number {
            same_number_25ms += 1;
        }
    }

    let share =|limit: f64| if compared == 0 { 0.0 } else { errors.iter().filter(|e| **e <= limit).count() as f64 / compared as f64 };
    let (bar_offset, top) = offsets.iter().enumerate().max_by_key(|(_, n)| **n).map_or((0, 0), |(o, n)| (o, *n));
    let bias_ms = if signed.is_empty() { f64::NAN } else { signed.iter().map(|(_, e)| e).sum::<f64>() / signed.len() as f64 };
    let octave = octave(prediction.bpm, truth.bpm);
    let downbeat_share = if downbeats == 0 { 0.0 } else { downbeats_right as f64 / downbeats as f64 };
    let export = spread(export_errors);
    let pass_openbox = octave == Octave::Same && matched == compared && export.max <= 2.0 && downbeat_share >= 0.98;
    Score {
        track: truth.name.clone(),
        analyzer: analyzer.to_owned(),
        tags: truth.tags.clone(),
        bpm_true: truth.bpm,
        bpm_predicted: prediction.bpm,
        bpm_error: prediction.bpm - truth.bpm,
        octave,
        beats: compared,
        matched,
        abs_error_ms: spread(errors.clone()),
        export_error_ms: export,
        bias_ms,
        drift_ms: drift(&signed),
        within_1ms: share(1.0),
        within_2ms: share(2.0),
        within_5ms: share(5.0),
        within_25ms: share(25.0),
        bar_offset: bar_offset as u8,
        bar_offset_share: if matched == 0 { 0.0 } else { top as f64 / matched as f64 },
        downbeat_share,
        pass_rbxport_25ms: compared > 0 && same_number_25ms as f64 / compared as f64 >= 0.98,
        pass_openbox,
        runtime_ms,
    }
}

/// The true beat period around beat `i`.
fn local_period(truth: &Truth, i: usize) -> f64 {
    let beats = &truth.beats;
    if beats.len() < 2 {
        return 60.0 / truth.bpm;
    }
    if i + 1 < beats.len() {
        beats[i + 1].secs - beats[i].secs
    } else {
        beats[i].secs - beats[i - 1].secs
    }
}

/// Index of the value nearest `t` in a sorted slice.
fn nearest(sorted: &[f64], t: f64) -> Option<usize> {
    if sorted.is_empty() {
        return None;
    }
    let i = sorted.partition_point(|x| *x < t);
    if i == 0 {
        return Some(0);
    }
    if i == sorted.len() {
        return Some(i - 1);
    }
    Some(if (sorted[i] - t).abs() < (t - sorted[i - 1]).abs() { i } else { i - 1 })
}

/// The change in signed error across the track, from a least-squares line.
fn drift(points: &[(f64, f64)]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let n = points.len() as f64;
    let mean_t = points.iter().map(|(t, _)| t).sum::<f64>() / n;
    let mean_e = points.iter().map(|(_, e)| e).sum::<f64>() / n;
    let (mut num, mut den) = (0.0, 0.0);
    for (t, e) in points {
        num += (t - mean_t) * (e - mean_e);
        den += (t - mean_t) * (t - mean_t);
    }
    if den == 0.0 {
        return 0.0;
    }
    let span = points[points.len() - 1].0 - points[0].0;
    num / den * span
}

/// Totals over many tracks for one analyzer.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub analyzer: String,
    pub tracks: usize,
    pub octave_right: usize,
    /// |BPM error| under 0.01 among tracks with the right octave.
    pub bpm_within_001: usize,
    pub downbeat_right: usize,
    /// Median over tracks of each track's 95th-percentile placement error.
    pub median_p95_ms: f64,
    /// Tracks whose every exported beat is within 2 ms.
    pub placement_2ms: usize,
    pub placement_5ms: usize,
    pub pass_rbxport_25ms: usize,
    pub pass_openbox: usize,
    pub mean_runtime_ms: f64,
}

pub fn summarize(analyzer: &str, scores: &[&Score]) -> Summary {
    let count = |f: &dyn Fn(&Score) -> bool| scores.iter().filter(|s| f(s)).count();
    let mut p95s: Vec<f64> = scores.iter().map(|s| s.abs_error_ms.p95).filter(|v| v.is_finite()).collect();
    p95s.sort_by(f64::total_cmp);
    let median_p95_ms = if p95s.is_empty() { f64::NAN } else { p95s[p95s.len() / 2] };
    Summary {
        analyzer: analyzer.to_owned(),
        tracks: scores.len(),
        octave_right: count(&|s| s.octave == Octave::Same),
        bpm_within_001: count(&|s| s.octave == Octave::Same && s.bpm_error.abs() < 0.01),
        downbeat_right: count(&|s| s.downbeat_share >= 0.98),
        median_p95_ms,
        placement_2ms: count(&|s| s.matched == s.beats && s.export_error_ms.max <= 2.0),
        placement_5ms: count(&|s| s.matched == s.beats && s.export_error_ms.max <= 5.0),
        pass_rbxport_25ms: count(&|s| s.pass_rbxport_25ms),
        pass_openbox: count(&|s| s.pass_openbox),
        mean_runtime_ms: if scores.is_empty() { 0.0 } else { scores.iter().map(|s| s.runtime_ms).sum::<f64>() / scores.len() as f64 },
    }
}
