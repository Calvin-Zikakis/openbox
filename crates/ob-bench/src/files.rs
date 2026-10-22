//! `ob-bench files`: every analyzer on audio files that have no reference grid.
//!
//! Without a reference, a grid is judged by how it sits on the music (the
//! kick ratio and attack offset of `library.rs`) and by agreement: between
//! analyzers on one file, and, for several renders of one song (`--same-song`),
//! across files once they are aligned by cross-correlation, since the same
//! song must get its downbeat on the same musical moment in every version.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "envelope indices and lags convert between usize, isize and float time"
)]

use std::path::{Path, PathBuf};

use ob_bench::analyzers::{Analyzer, Openbox, Rbxport};
use ob_bench::metrics::Prediction;
use rbl_analysis::{attack, onset, AnalysisPreset};

struct Analysed {
    path: PathBuf,
    kicks: onset::OnsetEnvelope,
    grids: Vec<(&'static str, Prediction)>,
}

fn downbeat_of(p: &Prediction) -> Option<f64> {
    p.beats.iter().find(|b| b.number == 1).map(|b| b.secs)
}

/// Lag, in seconds, that best lines `b` up with `a` (positive: `b`'s audio
/// starts later than `a`'s), from the cross-correlation of their kick-band
/// envelopes over lags up to `max_secs`.
fn lag_secs(reference: &onset::OnsetEnvelope, other: &onset::OnsetEnvelope, max_secs: f64) -> Option<f64> {
    if reference.rate <= 0.0 || (reference.rate - other.rate).abs() > 1e-6 {
        return None;
    }
    let max = (max_secs * reference.rate) as isize;
    let mut best = (0_isize, f64::NEG_INFINITY);
    for lag in -max..=max {
        let mut sum = 0.0_f64;
        for (i, &value) in reference.values.iter().enumerate() {
            let at = i as isize + lag;
            if at >= 0 && (at as usize) < other.values.len() {
                sum += f64::from(value) * f64::from(other.values[at as usize]);
            }
        }
        if sum > best.1 {
            best = (lag, sum);
        }
    }
    Some(best.0 as f64 / reference.rate + (other.origin_secs - reference.origin_secs))
}

pub fn run(paths: &[String], same_song: bool) {
    let analyzers: Vec<Box<dyn Analyzer>> = vec![
        Box::new(Rbxport(AnalysisPreset::Rekordbox)),
        Box::new(Openbox { preset: AnalysisPreset::Rekordbox, neural: false }),
        Box::new(Openbox { preset: AnalysisPreset::Rekordbox, neural: true }),
    ];
    let mut done = Vec::new();
    for path in paths.iter().map(PathBuf::from) {
        let Ok(audio) = rbl_audio::decode_mono(&path, Some(1800.0)) else {
            eprintln!("cannot decode {}", path.display());
            continue;
        };
        let (samples, sr) = (&audio.samples, audio.sample_rate);
        let kicks = onset::onset_envelope_band(samples, sr, onset::Band::LOW);
        let attacks = attack::AttackMap::new(samples, sr, attack::AttackOptions::default());
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        println!("\n{name}  ({sr} Hz, {:.0} s)", audio.duration_secs());
        let mut grids = Vec::new();
        for a in &analyzers {
            let prediction = a.analyse(samples, sr);
            let beats: Vec<f64> = prediction.beats.iter().map(|b| b.secs).collect();
            let kick_ratio = ob_analysis::phase::kick_ratio(&kicks, &beats);
            let mut offsets: Vec<f64> = beats.iter().filter_map(|&b| attacks.attack_within(b, 0.015).map(|x| (x.secs - b) * 1000.0)).collect();
            let within = offsets.iter().filter(|o| o.abs() <= 5.0).count() as f64 / beats.len().max(1) as f64;
            offsets.sort_by(f64::total_cmp);
            let median = offsets.get(offsets.len() / 2).copied().unwrap_or(f64::NAN);
            println!(
                "  {:<16} {:>8.3} BPM  first downbeat {:>8.3} s  kick ratio {:>5.2}  attack offset {:+6.2} ms  attacks within 5 ms {:>4.0}%  segments {}",
                a.name(),
                prediction.bpm,
                downbeat_of(&prediction).unwrap_or(f64::NAN),
                kick_ratio,
                median,
                within * 100.0,
                count_segments(&prediction),
            );
            grids.push((a.name(), prediction));
        }
        let full = ob_analysis::analyse(samples, sr, &ob_analysis::Options::preset(AnalysisPreset::Rekordbox)).report;
        let report = full.phase;
        let round = |a: [f64; 4]| a.map(|v| (v * 100.0).round() / 100.0);
        println!(
            "  openbox phase: judge {:?}, kick ratio {:.2}, neural ratio {:.2}, structure on/off {:.2}/{:.2}, moved {}, bar scores {:?}, neural bar {:?}, margin {:.2}{}",
            report.judge,
            report.kick_ratio,
            report.neural_ratio.unwrap_or(f64::NAN),
            report.structure_on,
            report.structure_off,
            report.moved_half_beat,
            round(report.bar_scores),
            report.neural_bar.map(round),
            report.bar_margin,
            if report.half_beat_conflict { "  HALF-BEAT CONFLICT" } else { "" }
        );
        if report.bar_conflict() {
            println!("  BEAT-1 CONFLICT: structure says position {:?}, neural says {:?}", report.structure_choice, report.neural_choice);
        }
        let p = full.placement;
        println!(
            "  openbox placement: measured {:+.3} ms over {} beats, spread {:.3} ms, {}",
            p.measured_ms,
            p.beats_used,
            p.spread_ms,
            if p.applied { format!("moved {:+.3} ms", p.shift_ms) } else { "not moved".to_owned() }
        );
        done.push(Analysed { path, kicks, grids });
    }
    if same_song && done.len() > 1 {
        compare_versions(&done);
    }
}

/// Distinct tempos in a prediction, as a stand-in for its segment count.
fn count_segments(p: &Prediction) -> usize {
    let mut runs = 0;
    let mut last = None;
    for pair in p.beats.windows(2) {
        let period = ((pair[1].secs - pair[0].secs) * 1000.0).round() as i64;
        if last.is_none_or(|l: i64| (l - period).abs() > 2) {
            runs += 1;
        }
        last = Some(period);
    }
    runs
}

/// For renders of one song, whether each analyzer puts beat 1 on the same
/// musical moment in every file.
fn compare_versions(done: &[Analysed]) {
    let reference = &done[0];
    println!("\nversions against {} (lag from kick-band cross-correlation):", file_name(&reference.path));
    for other in &done[1..] {
        let Some(lag) = lag_secs(&reference.kicks, &other.kicks, 20.0) else { continue };
        print!("  {:<60} lag {lag:+8.3} s", file_name(&other.path));
        for ((name, a), (_, b)) in reference.grids.iter().zip(&other.grids) {
            let (Some(da), Some(db)) = (downbeat_of(a), downbeat_of(b)) else { continue };
            let bar = 4.0 * 60.0 / a.bpm.max(1.0);
            // Where the other file's downbeat falls in the reference's bar.
            let shift = (db - lag - da).rem_euclid(bar);
            let beats = shift / (bar / 4.0);
            print!("  {name}: {beats:.2} beats");
        }
        println!();
    }
    println!("  (0.00 or 4.00 beats: the same downbeat; whole numbers: beat 1 on another beat; .50: the off-beat)");
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}
