//! `ob-bench library`: analyzers against the grids in a real rekordbox library.
//!
//! READ-ONLY. The installed `master.db` is copied under `target/` and only
//! the copy is opened; ANLZ files and audio are only read. Per-track results,
//! which name tracks, go to `target/ob-library/` and never into the
//! repository.
//!
//! A library's grids are not ground truth: some were hand-corrected, most
//! were not, and rekordbox is exactly what is under test. So every grid,
//! rekordbox's included, is also judged against the audio itself:
//!
//! - **kick ratio:** kick-band onset energy on the grid's beats over that on
//!   its midpoints. Well above 1 is on the kick; below 1 is on the off-beat.
//! - **attack offset:** the median time from each beat to the nearest kick
//!   attack (positive: the attack comes after the grid line, so the grid is
//!   early), and the share of beats with an attack within 5 ms.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "counts and offsets convert between usize and float for reporting"
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use ob_bench::analyzers::{Analyzer, Openbox, Rbxport};
use ob_bench::metrics::Prediction;
use rbl_analysis::{attack, onset, AnalysisPreset};
use serde::Serialize;

/// How one grid sits on the music.
#[derive(Debug, Clone, Copy, Serialize)]
struct Fit {
    kick_ratio: f64,
    attack_offset_ms: f64,
    attacks_within_5ms: f64,
}

/// How an analyzer's grid compares with rekordbox's.
#[derive(Debug, Clone, Serialize)]
struct Versus {
    analyzer: &'static str,
    bpm: f64,
    bpm_diff: f64,
    /// Median of (analyzer beat − nearest rekordbox beat), in ms.
    median_offset_ms: f64,
    /// 90th percentile of the absolute offset, in ms.
    p90_abs_offset_ms: f64,
    /// `same` (within 5 ms), `off-beat` (about half a beat), `shifted`
    /// (anything else) or `tempo` (BPM differs by more than 0.05).
    relation: &'static str,
    /// Dominant (analyzer number − rekordbox number) mod 4 over beats within
    /// a quarter beat.
    bar_offset: u8,
    fit: Fit,
    runtime_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
struct Record {
    id: u64,
    title: String,
    extension: String,
    sample_rate: u32,
    duration_secs: f64,
    rekordbox_bpm: f64,
    rekordbox_fit: Fit,
    analyzers: Vec<Versus>,
    decode_ms: f64,
}

struct Job {
    id: u64,
    title: String,
    path: PathBuf,
    bpm: f64,
    grid_ms: Vec<(u32, u16)>,
}

fn fit_of(beats: &[f64], kicks: &onset::OnsetEnvelope, attacks: &attack::AttackMap) -> Fit {
    let kick_ratio = ob_analysis::phase::kick_ratio(kicks, beats);
    let mut offsets: Vec<f64> = beats.iter().filter_map(|&b| attacks.attack_within(b, 0.015).map(|a| (a.secs - b) * 1000.0)).collect();
    let within = offsets.iter().filter(|o| o.abs() <= 5.0).count() as f64 / beats.len().max(1) as f64;
    offsets.sort_by(f64::total_cmp);
    let attack_offset_ms = offsets.get(offsets.len() / 2).copied().unwrap_or(f64::NAN);
    Fit { kick_ratio, attack_offset_ms, attacks_within_5ms: within }
}

fn versus(name: &'static str, prediction: &Prediction, rekordbox: &[(u32, u16)], rb_bpm: f64, fit: Fit, runtime_ms: f64) -> Versus {
    let ours: Vec<f64> = prediction.beats.iter().map(|b| b.secs).collect();
    let period = 60.0 / rb_bpm.max(1.0);
    let mut offsets = Vec::new();
    let mut bars = [0_usize; 4];
    for &(ms, number) in rekordbox {
        let t = f64::from(ms) / 1000.0;
        let i = ours.partition_point(|x| *x < t);
        let candidates = [i.checked_sub(1), Some(i)];
        let Some(j) = candidates.iter().flatten().filter(|&&j| j < ours.len()).min_by(|&&a, &&b| (ours[a] - t).abs().total_cmp(&(ours[b] - t).abs())).copied() else {
            continue;
        };
        let d = ours[j] - t;
        offsets.push(d * 1000.0);
        if d.abs() < period / 4.0 {
            let offset = (i32::from(prediction.beats[j].number) - i32::from(number)).rem_euclid(4) as usize;
            bars[offset] += 1;
        }
    }
    let mut sorted = offsets.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or(f64::NAN);
    let mut abs: Vec<f64> = offsets.iter().map(|o| o.abs()).collect();
    abs.sort_by(f64::total_cmp);
    let p90 = abs.get(abs.len() * 9 / 10).copied().unwrap_or(f64::NAN);
    let bpm_diff = prediction.bpm - rb_bpm;
    let half = period * 500.0;
    let relation = if bpm_diff.abs() > 0.05 {
        "tempo"
    } else if median.abs() <= 5.0 {
        "same"
    } else if (median.abs() - half).abs() <= half * 0.3 {
        "off-beat"
    } else {
        "shifted"
    };
    let bar_offset = bars.iter().enumerate().max_by_key(|(_, n)| **n).map_or(0, |(o, _)| o) as u8;
    Versus { analyzer: name, bpm: prediction.bpm, bpm_diff, median_offset_ms: median, p90_abs_offset_ms: p90, relation, bar_offset, fit, runtime_ms }
}

fn copy_library(out: &Path) -> Result<rbl_db::LibraryLocation, String> {
    let mut location = rbl_db::detect().map_err(|e| format!("no rekordbox library found: {e}"))?;
    let copy_dir = out.join("master-copy");
    std::fs::create_dir_all(&copy_dir).map_err(|e| e.to_string())?;
    let copy = copy_dir.join("master.db");
    for suffix in ["", "-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{suffix}", location.master_db.display()));
        let to = PathBuf::from(format!("{}{suffix}", copy.display()));
        if from.exists() {
            std::fs::copy(&from, &to).map_err(|e| format!("copying {}: {e}", from.display()))?;
        } else if to.exists() {
            std::fs::remove_file(&to).map_err(|e| e.to_string())?;
        }
    }
    location.master_db = copy;
    location.is_real_install = false;
    Ok(location)
}

#[allow(clippy::too_many_lines, reason = "one report")]
pub fn run(limit: Option<usize>, jobs: usize, filter: Option<&str>) {
    let out = PathBuf::from("target/ob-library");
    let location = match copy_library(&out) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };
    let db = match rbl_db::Library::open(location, rbl_db::OpenMode::ReadOnly) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("cannot open the copied library: {e}");
            return;
        }
    };
    let share = db.location().share_root.clone();
    let library = match rbl_index::load(&db) {
        Ok((library, _)) => library,
        Err(e) => {
            eprintln!("cannot read the library: {e}");
            return;
        }
    };
    let mut queue = Vec::new();
    let (mut missing, mut no_grid) = (0, 0);
    for i in 0..library.ids.len() {
        let title = library.title.get(i);
        if filter.is_some_and(|f| !title.to_lowercase().contains(&f.to_lowercase())) {
            continue;
        }
        let path = PathBuf::from(library.folder_path.get(i));
        if !path.exists() {
            missing += 1;
            if std::env::var_os("OB_SHOW_MISSING").is_some() {
                // Only the first components: enough to see which drive or
                // folder the library expects, without naming tracks.
                let head: PathBuf = path.components().take(5).collect();
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                eprintln!("missing under {}\tname {name}", head.display());
            }
            continue;
        }
        let dat = rbl_anlz::resolve(&share, library.analysis_path.get(i));
        let Some(grid) = rbl_anlz::Anlz::read(&dat).ok().and_then(|d| d.beat_grid()).filter(|g| g.len() > 16) else {
            no_grid += 1;
            continue;
        };
        queue.push(Job {
            id: library.ids[i],
            title: title.to_owned(),
            path,
            bpm: f64::from(library.bpm_x100[i]) / 100.0,
            grid_ms: grid.iter().map(|b| (b.time_ms, b.beat_number)).collect(),
        });
    }
    if let Some(limit) = limit {
        queue.truncate(limit);
    }
    println!("{} tracks to analyse ({missing} files missing, {no_grid} without a rekordbox grid)", queue.len());

    let analyzers: Vec<Box<dyn Analyzer>> = vec![Box::new(Rbxport(AnalysisPreset::Rekordbox)), Box::new(Openbox { preset: AnalysisPreset::Rekordbox, neural: false }), Box::new(Openbox { preset: AnalysisPreset::Rekordbox, neural: true })];
    let next = AtomicUsize::new(0);
    let records = Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| loop {
                let n = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = queue.get(n) else { break };
                let t0 = Instant::now();
                // Thirty minutes covers any track; a long mix is cut there.
                let Ok(audio) = rbl_audio::decode_mono(&job.path, Some(1800.0)) else { continue };
                let decode_ms = t0.elapsed().as_secs_f64() * 1000.0;
                let (samples, sr) = (&audio.samples, audio.sample_rate);
                let kicks = onset::onset_envelope_band(samples, sr, onset::Band::LOW);
                let attacks = attack::AttackMap::new(samples, sr, attack::AttackOptions::default());
                let rb_beats: Vec<f64> = job.grid_ms.iter().map(|(ms, _)| f64::from(*ms) / 1000.0).collect();
                let rekordbox_fit = fit_of(&rb_beats, &kicks, &attacks);
                let mut compared = Vec::new();
                for a in &analyzers {
                    let t = Instant::now();
                    let prediction = a.analyse(samples, sr);
                    let runtime = t.elapsed().as_secs_f64() * 1000.0;
                    let beats: Vec<f64> = prediction.beats.iter().map(|b| b.secs).collect();
                    let fit = fit_of(&beats, &kicks, &attacks);
                    compared.push(versus(a.name(), &prediction, &job.grid_ms, job.bpm, fit, runtime));
                }
                let record = Record {
                    id: job.id,
                    title: job.title.clone(),
                    extension: job.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase(),
                    sample_rate: sr,
                    duration_secs: audio.duration_secs(),
                    rekordbox_bpm: job.bpm,
                    rekordbox_fit,
                    analyzers: compared,
                    decode_ms,
                };
                if let Ok(mut all) = records.lock() {
                    all.push(record);
                    let done = all.len();
                    if done % 50 == 0 {
                        eprintln!("  {done} done ({:.0} s)", started.elapsed().as_secs_f64());
                    }
                }
            });
        }
    });
    let mut records = records.into_inner().unwrap_or_default();
    records.sort_by_key(|r| r.id);
    let jsonl: String = records.iter().filter_map(|r| serde_json::to_string(r).ok()).map(|l| l + "\n").collect();
    let results = out.join("results.jsonl");
    if let Err(e) = std::fs::write(&results, jsonl) {
        eprintln!("cannot write {}: {e}", results.display());
    }
    summarize(&records, started.elapsed().as_secs_f64(), jobs);
    println!("per-track results (private, names tracks): {}", results.display());
}

fn summarize(records: &[Record], secs: f64, jobs: usize) {
    let n = records.len();
    println!("\n{n} tracks in {secs:.0} s with {jobs} jobs");
    let mut by_ext = std::collections::BTreeMap::<&str, usize>::new();
    for r in records {
        *by_ext.entry(r.extension.as_str()).or_default() += 1;
    }
    println!("formats: {by_ext:?}");

    let describe = |label: &str, fits: &[Fit]| {
        let off_beat = fits.iter().filter(|f| f.kick_ratio < 1.0).count();
        let weak = fits.iter().filter(|f| (1.0..2.0).contains(&f.kick_ratio)).count();
        let mut offsets: Vec<f64> = fits.iter().map(|f| f.attack_offset_ms).filter(|v| v.is_finite()).collect();
        offsets.sort_by(f64::total_cmp);
        let late = fits.iter().filter(|f| f.attack_offset_ms < -5.0).count();
        let early = fits.iter().filter(|f| f.attack_offset_ms > 5.0).count();
        let median = offsets.get(offsets.len() / 2).copied().unwrap_or(f64::NAN);
        println!(
            "{label:<16} kick ratio <1 (off-beat?) {off_beat:>4}   1-2 (ambiguous) {weak:>4}   median attack offset {median:+.2} ms   grid >5 ms late {late:>4}   >5 ms early {early:>4}"
        );
    };
    println!("\nhow each grid sits on the music (same detectors for all):");
    describe("rekordbox", &records.iter().map(|r| r.rekordbox_fit).collect::<Vec<_>>());
    for name in ["rbxport-normal", "openbox-dsp-normal", "openbox-normal"] {
        let fits: Vec<Fit> = records.iter().flat_map(|r| r.analyzers.iter().filter(|a| a.analyzer == name).map(|a| a.fit)).collect();
        describe(name, &fits);
    }

    println!("\nagreement with rekordbox's grid:");
    for name in ["rbxport-normal", "openbox-dsp-normal", "openbox-normal"] {
        let mine: Vec<&Versus> = records.iter().flat_map(|r| r.analyzers.iter().filter(|a| a.analyzer == name)).collect();
        let count = |rel: &str| mine.iter().filter(|v| v.relation == rel).count();
        let same_bar = mine.iter().filter(|v| v.relation == "same" && v.bar_offset == 0).count();
        let mut bars = [0_usize; 4];
        for v in mine.iter().filter(|v| v.relation == "same") {
            bars[v.bar_offset as usize] += 1;
        }
        let mean_ms = mine.iter().map(|v| v.runtime_ms).sum::<f64>() / mine.len().max(1) as f64;
        println!(
            "{name:<16} same {:>4} (same beat 1: {same_bar:>4}, bar offsets {bars:?})   off-beat {:>4}   shifted {:>4}   tempo differs {:>4}   {mean_ms:.0} ms/track",
            count("same"),
            count("off-beat"),
            count("shifted"),
            count("tempo"),
        );
    }

    // Where an analyzer and rekordbox put the grid on opposite halves of the
    // beat, which grid does the kick band side with?
    println!("\noff-beat disagreements, judged by the kick band:");
    for name in ["rbxport-normal", "openbox-dsp-normal", "openbox-normal"] {
        let (mut ours, mut theirs) = (0, 0);
        for r in records {
            for v in r.analyzers.iter().filter(|v| v.analyzer == name && v.relation == "off-beat") {
                if v.fit.kick_ratio > r.rekordbox_fit.kick_ratio {
                    ours += 1;
                } else {
                    theirs += 1;
                }
            }
        }
        println!("{name:<16} kick band sides with the analyzer {ours:>4}, with rekordbox {theirs:>4}");
    }

    // rekordbox 6 grids on MP3s were seen 1105 samples (about 25 ms) late.
    let shifted_mp3: Vec<f64> = records
        .iter()
        .filter(|r| r.extension == "mp3")
        .filter_map(|r| r.analyzers.iter().find(|a| a.analyzer == "openbox-normal"))
        .filter(|v| v.relation == "shifted" || v.relation == "same")
        .map(|v| v.median_offset_ms)
        .collect();
    let near_25 = shifted_mp3.iter().filter(|o| (-27.5..=-22.5).contains(*o)).count();
    println!("\nMP3s whose openbox grid is 22.5-27.5 ms earlier than rekordbox's (the 1105-sample offset): {near_25} of {}", shifted_mp3.len());
}
