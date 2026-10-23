//! Whether the tempo the DSP chose is the one a DJ counts, or a simple ratio
//! of it.
//!
//! rbxport's candidate stage ranks tempos by the onset envelope's
//! autocorrelation and a prior centred at 132 BPM. On half-time and
//! broken-beat music the strongest periodicity is often the snare's or a
//! triplet's, and it settles on half, two thirds or four fifths of the
//! tempo rekordbox shows: 23 of 464 tracks in the owner's library (research
//! note 005).
//!
//! Beat This! was trained on human beat annotations, so the spacing of the
//! beats it hears is an independent tempo estimate. When that estimate is a
//! simple ratio of the DSP's and the model is consistent about it, the grid
//! is refitted at the DSP tempo times that ratio. The fit itself stays on the
//! kick attacks; only the choice of tempo moves.

use ob_ml::Excerpt;

/// Ratios the DSP tempo can be off by; 1 means agreement.
const RATIOS: [f64; 9] = [1.0, 2.0, 0.5, 1.5, 2.0 / 3.0, 4.0 / 3.0, 0.75, 1.25, 0.8];
/// How close the neural tempo must be to a ratio of the DSP's to count as it.
const RATIO_TOLERANCE: f64 = 0.03;
/// A frame counts as a neural beat at this activation or above.
const BEAT_FLOOR: f32 = 0.5;
/// Intervals within this fraction of the median count as the beat period.
const AGREE: f64 = 0.1;
/// Fewest beat intervals for an estimate, and the share that must agree with
/// the median before the estimate may move the grid.
const MIN_INTERVALS: usize = 16;
pub const MIN_CONSISTENCY: f64 = 0.6;
/// The tempos a refit may land on. Beat This! hears half-time on drum and
/// bass and dubstep and halves a correct 140 or 170 BPM; on the owner's
/// library every such halving was wrong (research note 006), while every
/// refit that stayed in this range was right.
pub const REFIT_RANGE: (f64, f64) = (90.0, 180.0);
/// Seconds at each edge of an excerpt not used: the model hears less there.
const EDGE_SECS: f64 = 1.0;

/// What the judge measured and decided.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OctaveReport {
    /// The candidate stage's tempo.
    pub dsp_bpm: f64,
    /// The tempo implied by the neural beats' spacing.
    pub neural_bpm: Option<f64>,
    /// Share of neural beat intervals within a tenth of their median.
    pub consistency: f64,
    pub intervals: usize,
    /// The simple ratio of the DSP tempo the neural tempo matches, if any.
    pub ratio: Option<f64>,
    /// The tempo the grid was refitted at.
    pub refit_bpm: Option<f64>,
    pub applied: bool,
}

/// Times of the neural beats in one excerpt, each refined between frames.
fn neural_beats(excerpt: &Excerpt) -> Vec<f64> {
    let a = &excerpt.activations.beat;
    let mut out = Vec::new();
    for i in 1..a.len().saturating_sub(1) {
        let (prev, here, next) = (f64::from(a[i - 1]), f64::from(a[i]), f64::from(a[i + 1]));
        if a[i] < BEAT_FLOOR || here < prev || here <= next {
            continue;
        }
        let denom = prev - 2.0 * here + next;
        let offset = if denom.abs() > f64::EPSILON { (0.5 * (prev - next) / denom).clamp(-0.5, 0.5) } else { 0.0 };
        let secs = (i as f64 + offset) / ob_ml::FPS;
        let edge = excerpt.end_secs() - excerpt.start_secs - EDGE_SECS;
        if secs >= EDGE_SECS && secs <= edge {
            out.push(secs);
        }
    }
    out
}

/// The neural tempo, its consistency and the intervals it rests on.
fn neural_tempo(excerpts: &[Excerpt]) -> Option<(f64, f64, usize)> {
    let mut intervals: Vec<f64> = excerpts.iter().flat_map(|e| neural_beats(e).windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>()).collect();
    if intervals.len() < MIN_INTERVALS {
        return None;
    }
    intervals.sort_by(f64::total_cmp);
    let median = intervals[intervals.len() / 2];
    let agreeing: Vec<f64> = intervals.iter().copied().filter(|i| (i - median).abs() <= AGREE * median).collect();
    if agreeing.is_empty() {
        return None;
    }
    let period = agreeing.iter().sum::<f64>() / agreeing.len() as f64;
    Some((60.0 / period, agreeing.len() as f64 / intervals.len() as f64, intervals.len()))
}

/// Compares the DSP tempo with the neural one and says whether, and at what
/// tempo, the grid should be refitted.
pub fn judge(dsp_bpm: f64, excerpts: Option<&[Excerpt]>) -> OctaveReport {
    let mut report = OctaveReport { dsp_bpm, ..OctaveReport::default() };
    let Some((neural, consistency, intervals)) = excerpts.and_then(neural_tempo) else { return report };
    report.neural_bpm = Some(neural);
    report.consistency = consistency;
    report.intervals = intervals;
    if dsp_bpm <= 0.0 {
        return report;
    }
    let r = neural / dsp_bpm;
    report.ratio = RATIOS.iter().copied().find(|ratio| (r / ratio - 1.0).abs() <= RATIO_TOLERANCE);
    report.refit_bpm = report
        .ratio
        .filter(|&ratio| (ratio - 1.0).abs() > f64::EPSILON && consistency >= MIN_CONSISTENCY)
        .map(|ratio| dsp_bpm * ratio)
        .filter(|bpm| (REFIT_RANGE.0..=REFIT_RANGE.1).contains(bpm));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use ob_ml::Activations;

    /// Forty seconds of activations with a spike at each of `gaps`' running sums, in frames.
    fn excerpt(gaps: impl Iterator<Item = usize>) -> Excerpt {
        let frames = 2000;
        let mut beat = vec![0.05_f32; frames];
        let mut at = 10;
        for gap in gaps {
            if at >= frames - 1 {
                break;
            }
            beat[at] = 0.9;
            at += gap;
        }
        Excerpt { start_secs: 0.0, activations: Activations { beat, downbeat: vec![0.0; frames] } }
    }

    fn steady(frames: usize) -> Vec<Excerpt> {
        vec![excerpt(std::iter::repeat(frames))]
    }

    #[test]
    fn agreement_leaves_the_grid_alone() {
        let report = judge(120.0, Some(&steady(25)));
        assert!((report.neural_bpm.unwrap_or(0.0) - 120.0).abs() < 0.5, "{report:?}");
        assert_eq!(report.ratio, Some(1.0));
        assert_eq!(report.refit_bpm, None);
    }

    #[test]
    fn a_double_tempo_refits_at_the_neural_tempo() {
        let report = judge(60.0, Some(&steady(25)));
        assert_eq!(report.ratio, Some(2.0), "{report:?}");
        assert!((report.refit_bpm.unwrap_or(0.0) - 120.0).abs() < 0.01, "{report:?}");
    }

    #[test]
    fn two_thirds_of_the_neural_tempo_is_a_known_ratio() {
        let report = judge(80.0, Some(&steady(25)));
        assert_eq!(report.ratio, Some(1.5), "{report:?}");
    }

    #[test]
    fn a_half_time_reading_does_not_halve_a_fast_tempo() {
        let report = judge(120.0, Some(&steady(50)));
        assert_eq!(report.ratio, Some(0.5), "{report:?}");
        assert_eq!(report.refit_bpm, None);
    }

    #[test]
    fn beats_with_no_steady_spacing_do_not_move_the_grid() {
        let uneven = vec![excerpt([10, 40].into_iter().cycle())];
        let report = judge(60.0, Some(&uneven));
        assert!(report.consistency < MIN_CONSISTENCY, "{report:?}");
        assert_eq!(report.refit_bpm, None);
    }

    #[test]
    fn too_few_beats_say_nothing() {
        let short = vec![excerpt(std::iter::repeat_n(25, 8).chain(std::iter::once(5000)))];
        let report = judge(120.0, Some(&short));
        assert_eq!(report.neural_bpm, None);
        assert_eq!(judge(120.0, None).refit_bpm, None);
    }
}
