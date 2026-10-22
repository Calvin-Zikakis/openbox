//! `ob-bench`: run analyzers over the synthetic suite and score their grids.
//!
//! ```text
//! cargo run --release -p ob-bench -- list
//! cargo run --release -p ob-bench -- eval [--analyzer rbxport,rbxport-normal] [--filter TEXT] [--jobs N] [--json PATH] [--quiet]
//! cargo run --release -p ob-bench -- render OUT_DIR [--filter TEXT]
//! cargo run --release -p ob-bench -- probe --filter TEXT
//! cargo run --release -p ob-bench -- library [--limit N] [--filter TITLE] [--jobs N]
//! cargo run --release -p ob-bench -- files [--same-song] FILE...
//! ```
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line report")]

mod files;
mod library;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use ob_bench::analyzers::{Analyzer, Openbox, Rbxport};
use ob_bench::metrics::{score, summarize, Score};
use ob_bench::synth::{render, suite, TrackSpec};
use rbl_analysis::AnalysisPreset;

struct Args {
    command: String,
    positional: Vec<String>,
    analyzers: Vec<String>,
    filter: Option<String>,
    jobs: usize,
    json: Option<PathBuf>,
    quiet: bool,
    limit: Option<usize>,
    same_song: bool,
}

fn parse() -> Result<Args, String> {
    let mut raw = std::env::args().skip(1);
    let command = raw.next().unwrap_or_else(|| "eval".to_owned());
    let cores = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get);
    // rbxport's analysis runs about seven stages in parallel per track.
    let mut args = Args { command, positional: Vec::new(), analyzers: ["rbxport-normal", "openbox-dsp-normal", "openbox-normal"].map(str::to_owned).to_vec(), filter: None, jobs: (cores / 3).max(1), json: None, quiet: false, limit: None, same_song: false };
    while let Some(arg) = raw.next() {
        let mut value = |name: &str| raw.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--analyzer" => args.analyzers = value("--analyzer")?.split(',').map(str::to_owned).collect(),
            "--filter" => args.filter = Some(value("--filter")?),
            "--jobs" => args.jobs = value("--jobs")?.parse().map_err(|_| "--jobs takes a number".to_owned())?,
            "--json" => args.json = Some(PathBuf::from(value("--json")?)),
            "--quiet" => args.quiet = true,
            "--same-song" => args.same_song = true,
            "--limit" => args.limit = Some(value("--limit")?.parse().map_err(|_| "--limit takes a number".to_owned())?),
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => args.positional.push(other.to_owned()),
        }
    }
    Ok(args)
}

fn analyzer(name: &str) -> Option<Box<dyn Analyzer>> {
    match name {
        "rbxport" => Some(Box::new(Rbxport(AnalysisPreset::Rbxport))),
        "rbxport-normal" => Some(Box::new(Rbxport(AnalysisPreset::Rekordbox))),
        "openbox" => Some(Box::new(Openbox { preset: AnalysisPreset::Rbxport, neural: true })),
        "openbox-normal" => Some(Box::new(Openbox { preset: AnalysisPreset::Rekordbox, neural: true })),
        "openbox-dsp" => Some(Box::new(Openbox { preset: AnalysisPreset::Rbxport, neural: false })),
        "openbox-dsp-normal" => Some(Box::new(Openbox { preset: AnalysisPreset::Rekordbox, neural: false })),
        _ => None,
    }
}

fn selected(filter: Option<&str>) -> Vec<TrackSpec> {
    suite().into_iter().filter(|t| filter.is_none_or(|f| t.name.contains(f) || t.tags.iter().any(|tag| tag.contains(f)))).collect()
}

fn main() {
    if std::env::var("RUST_LOG").is_ok() {
        let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).with_target(false).without_time().try_init();
    }
    let args = match parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    match args.command.as_str() {
        "list" => list(&args),
        "eval" => eval(&args),
        "render" => render_to(&args),
        "probe" => probe(&args),
        "neural" => neural(&args),
        "neural-speed" => neural_speed(&args),
        "library" => library::run(args.limit, args.jobs, args.filter.as_deref()),
        "files" => files::run(&args.positional, args.same_song),
        other => {
            eprintln!("unknown command {other:?}; use list, eval or render");
            std::process::exit(2);
        }
    }
}

fn list(args: &Args) {
    for t in selected(args.filter.as_deref()) {
        println!("{:<24} {:<14} {:>6} Hz  {:?}  {}", t.name, t.style.name(), t.sample_rate, t.tempo, t.tags.join(","));
    }
}

fn render_to(args: &Args) {
    let Some(dir) = args.positional.first().map(PathBuf::from) else {
        eprintln!("render needs an output directory");
        std::process::exit(2);
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("cannot create {}: {e}", dir.display());
        std::process::exit(1);
    }
    for spec in selected(args.filter.as_deref()) {
        let track = render(&spec);
        let wav = dir.join(format!("{}.wav", spec.name));
        let json = dir.join(format!("{}.truth.json", spec.name));
        let written = ob_bench::wav::write_mono_16(&wav, &track.samples, track.sample_rate)
            .and_then(|()| serde_json::to_vec_pretty(&track.truth).map_err(std::io::Error::other))
            .and_then(|bytes| std::fs::write(&json, bytes));
        match written {
            Ok(()) => println!("{}", wav.display()),
            Err(e) => eprintln!("{}: {e}", spec.name),
        }
    }
}

fn eval(args: &Args) {
    let mut analyzers = Vec::new();
    for name in &args.analyzers {
        let Some(a) = analyzer(name) else {
            eprintln!("unknown analyzer {name:?}; known: rbxport, rbxport-normal, openbox, openbox-normal, openbox-dsp, openbox-dsp-normal");
            std::process::exit(2);
        };
        analyzers.push(a);
    }
    let tracks = selected(args.filter.as_deref());
    let next = AtomicUsize::new(0);
    let scores = Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..args.jobs.min(tracks.len()).max(1) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(spec) = tracks.get(i) else { break };
                let track = render(spec);
                for a in &analyzers {
                    let t0 = Instant::now();
                    let prediction = a.analyse(&track.samples, track.sample_rate);
                    let runtime = t0.elapsed().as_secs_f64() * 1000.0;
                    let s = score(&track.truth, &prediction, a.name(), runtime);
                    if let Ok(mut all) = scores.lock() {
                        all.push(s);
                    }
                }
            });
        }
    });
    let mut scores = scores.into_inner().unwrap_or_default();
    let order = |name: &str| tracks.iter().position(|t| t.name == name).unwrap_or(usize::MAX);
    scores.sort_by(|a, b| order(&a.track).cmp(&order(&b.track)).then_with(|| a.analyzer.cmp(&b.analyzer)));

    if !args.quiet {
        print_tracks(&scores);
    }
    println!();
    println!(
        "{:<16} {:>6} {:>7} {:>8} {:>9} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9}",
        "analyzer", "tracks", "octave", "bpm<.01", "downbeat", "p95(ms)", "max<=2", "max<=5", "rbx25ms", "openbox", "ms/track"
    );
    let mut summaries = Vec::new();
    for a in &analyzers {
        let mine: Vec<&Score> = scores.iter().filter(|s| s.analyzer == a.name()).collect();
        let s = summarize(a.name(), &mine);
        println!(
            "{:<16} {:>6} {:>7} {:>8} {:>9} {:>8.2} {:>8} {:>8} {:>8} {:>8} {:>9.0}",
            s.analyzer, s.tracks, s.octave_right, s.bpm_within_001, s.downbeat_right, s.median_p95_ms, s.placement_2ms, s.placement_5ms, s.pass_rbxport_25ms, s.pass_openbox, s.mean_runtime_ms
        );
        summaries.push(s);
    }
    println!("\n{} tracks in {:.1} s with {} jobs", tracks.len(), started.elapsed().as_secs_f64(), args.jobs);

    if let Some(path) = &args.json {
        let report = serde_json::json!({ "summaries": summaries, "scores": scores });
        match serde_json::to_vec_pretty(&report).map_err(std::io::Error::other).and_then(|b| std::fs::write(path, b)) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(e) => eprintln!("cannot write {}: {e}", path.display()),
        }
    }
}

fn print_tracks(scores: &[Score]) {
    println!(
        "{:<24} {:<15} {:>8} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>9} {:>6} {:>4}",
        "track", "analyzer", "bpm err", "octave", "p50 ms", "p95 ms", "max ms", "exp max", "drift", "bar off", "down", "pass"
    );
    for s in scores {
        println!(
            "{:<24} {:<15} {:>8.3} {:>7} {:>7.2} {:>7.2} {:>7.2} {:>7.2} {:>7.2} {:>4}({:>3.0}%) {:>5.0}% {:>4}",
            s.track,
            s.analyzer,
            s.bpm_error,
            format!("{:?}", s.octave).to_lowercase(),
            s.abs_error_ms.median,
            s.abs_error_ms.p95,
            s.abs_error_ms.max,
            s.export_error_ms.max,
            s.drift_ms,
            s.bar_offset,
            s.bar_offset_share * 100.0,
            s.downbeat_share * 100.0,
            if s.pass_openbox { "yes" } else { "no" }
        );
    }
}

/// Where rbxport's two phase judges stand on each track: the kick-attack fit
/// before the structure stage sees it, and the structure stage's eight bar
/// positions. Diagnostic output for the half-beat question.
fn probe(args: &Args) {
    use rbl_analysis::{attack, downbeat, onset, tempo};
    for spec in selected(args.filter.as_deref()) {
        let track = render(&spec);
        let (samples, sr) = (&track.samples, track.sample_rate);
        let options = AnalysisPreset::Rekordbox.options();
        let onsets = onset::onset_envelope(samples, sr);
        let kicks = onset::onset_envelope_band(samples, sr, onset::Band::LOW);
        let attacks = attack::AttackMap::new(samples, sr, options.attacks);
        let fitted = tempo::detect_tempo_with(&onsets, Some(&kicks), Some(&attacks), options.tempo);
        let Some(segment) = fitted.segments.first() else {
            println!("{}: no grid", spec.name);
            continue;
        };
        // Phase of the fit against the truth, as a fraction of a beat.
        let truth_first = track.truth.beats.first().map_or(0.0, |b| b.secs);
        let period = segment.period_secs;
        let frac = ((segment.phase_secs - truth_first) / period).rem_euclid(1.0);
        let fit_off_beat = (0.25..0.75).contains(&frac);
        let beat_secs: Vec<f64> = fitted.beats.iter().map(|b| f64::from(b.time_ms) / 1000.0).collect();
        let frames = downbeat::band_frames(samples, sr);
        let decision = downbeat::grid_phase_from(&frames, sr, &beat_secs);
        let mut halves = Vec::new();
        for pair in beat_secs.windows(2) {
            halves.push(pair[0]);
            halves.push(pair[0].midpoint(pair[1]));
        }
        let profiles = downbeat::beat_profiles_from(&frames, sr, &halves);
        let scores = downbeat::position_scores(&profiles, 8, &[8, 16, 32, 64]);
        // Kick-band onset energy summed on the grid's beats and on its midpoints.
        let (mut on, mut mid) = (0.0_f64, 0.0_f64);
        for &t in &halves.iter().step_by(2).copied().collect::<Vec<_>>() {
            let x = |secs: f64| (secs - kicks.origin_secs) * kicks.rate;
            on += f64::from(kicks.sample_at(x(t)));
            mid += f64::from(kicks.sample_at(x(t + period / 2.0)));
        }
        let verdict = match (fit_off_beat, decision.half_beat_off) {
            (false, false) => "fit on beat, kept",
            (false, true) => "fit on beat, structure MOVED it off",
            (true, true) => "fit off beat, structure fixed it",
            (true, false) => "fit off beat, structure KEPT it",
        };
        println!("{:<24} fit phase {:.3} beat  {verdict}", spec.name, frac);
        println!("    structure scores by half-beat position (even = grid beat): {}", scores.iter().map(|v| format!("{v:.2}")).collect::<Vec<_>>().join(" "));
        println!("    kick band: on grid beats {on:.1}, on midpoints {mid:.1}");
    }
}

/// How the neural model judges the true grid of each synthetic track: its
/// beat activation on the true beats over the midpoints, and whether its
/// downbeat activation peaks on the true bar position.
fn neural(args: &Args) {
    let size = if args.analyzers.iter().any(|a| a == "small") { ob_ml::Size::Small } else { ob_ml::Size::Full };
    if !ob_ml::available(size) {
        eprintln!("models missing: run scripts/openbox/fetch-models.sh --full");
        return;
    }
    let (mut right, mut total, mut on_beat) = (0, 0, 0);
    for spec in selected(args.filter.as_deref()) {
        let track = render(&spec);
        let started = Instant::now();
        let Some(act) = ob_ml::activations(&track.samples, track.sample_rate, size) else { continue };
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let beats: Vec<f64> = track.truth.beats.iter().map(|b| b.secs).collect();
        let (ratio, bar) = ob_bench::neural::evidence(&act, &beats);
        let truth_position = track.truth.beats.iter().position(|b| b.number == 1).unwrap_or(0) % 4;
        let best = (0..4).max_by(|a, b| bar[*a].total_cmp(&bar[*b])).unwrap_or(0);
        total += 1;
        right += usize::from(best == truth_position);
        on_beat += usize::from(ratio > 1.0);
        println!(
            "{:<24} beat ratio {:>9.2}  downbeat {:?} best {best} truth {truth_position} {}  {ms:.0} ms",
            spec.name,
            ratio,
            bar.map(|v| (v * 100.0).round() / 100.0),
            if best == truth_position { "ok" } else { "WRONG" }
        );
    }
    println!("\non the beat: {on_beat}/{total}   downbeat position right: {right}/{total}");
}

/// How long the neural model takes on stretches of the first file given.
fn neural_speed(args: &Args) {
    let Some(path) = args.positional.first() else { return };
    let Ok(audio) = rbl_audio::decode_mono(std::path::Path::new(path), None) else { return };
    let sr = audio.sample_rate as usize;
    for size in [ob_ml::Size::Full, ob_ml::Size::Small] {
        // The first call loads the model.
        let t = Instant::now();
        let _ = ob_ml::activations(&audio.samples[..sr * 5], audio.sample_rate, size);
        println!("{size:?}: load + 5 s: {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        for secs in [8, 10, 12, 15, 20, 25, 30] {
            let n = (sr * secs).min(audio.samples.len());
            let t = Instant::now();
            let _ = ob_ml::activations(&audio.samples[..n], audio.sample_rate, size);
            println!("  {secs:>4} s of audio: {:>6.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        }
        let t = Instant::now();
        let _ = ob_ml::activations(&audio.samples, audio.sample_rate, size);
        println!("  whole track ({:.0} s): {:.0} ms", audio.duration_secs(), t.elapsed().as_secs_f64() * 1000.0);
    }
}
