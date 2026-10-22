//! Synthetic electronic tracks whose beat grid is known exactly.
//!
//! Hand annotations are good to about 10 ms, and the grid target is 1–2 ms,
//! so the reference has to come from construction. Every hit is rendered from
//! a continuous-time formula evaluated from its exact, fractional-sample
//! onset, so the truth carries no rounding of its own.
//!
//! The tracks are not meant to sound like records. They are meant to carry
//! the things that make real grids hard: breakdowns without a kick, beatless
//! intros, pickups before the first downbeat, off-beat bass and hats,
//! half-time and broken kick patterns, sidechain pumping, clipped masters,
//! tempo cuts and ramps, and (for the humanized tracks) timing jitter.

use crate::truth::{Truth, TruthBeat};

/// splitmix64: deterministic and seedable, with no dependency.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x2545_F491_4F6C_DD1D)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// Uniform in `[-1, 1)`.
    pub fn signed(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// Roughly normal with unit variance (Irwin–Hall of four).
    pub fn normal(&mut self) -> f64 {
        (self.unit() + self.unit() + self.unit() + self.unit() - 2.0) * 3.0_f64.sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    House,
    TechHouse,
    Techno,
    Trance,
    DrumAndBass,
    Dubstep,
    Breaks,
    HipHop,
    AfroHouse,
}

impl Style {
    pub fn name(self) -> &'static str {
        match self {
            Self::House => "house",
            Self::TechHouse => "tech-house",
            Self::Techno => "techno",
            Self::Trance => "trance",
            Self::DrumAndBass => "drum-and-bass",
            Self::Dubstep => "dubstep",
            Self::Breaks => "breaks",
            Self::HipHop => "hip-hop",
            Self::AfroHouse => "afro-house",
        }
    }
}

/// How the tempo runs, counted in bars from the first downbeat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TempoPlan {
    Constant(f64),
    /// `bpm_a` until the downbeat of bar `at_bar` (0-based), `bpm_b` from it.
    Cut { bpm_a: f64, at_bar: u32, bpm_b: f64 },
    /// `bpm_a` until bar `from_bar`, then the tempo moves linearly per beat
    /// over `bars` bars, then `bpm_b`.
    Ramp { bpm_a: f64, from_bar: u32, bars: u32, bpm_b: f64 },
}

impl TempoPlan {
    pub fn start_bpm(self) -> f64 {
        match self {
            Self::Constant(bpm) | Self::Cut { bpm_a: bpm, .. } | Self::Ramp { bpm_a: bpm, .. } => bpm,
        }
    }

    /// The tempo between musical beat `k` and `k + 1`.
    fn bpm_after(self, k: i64) -> f64 {
        match self {
            Self::Constant(bpm) => bpm,
            Self::Cut { bpm_a, at_bar, bpm_b } => {
                if k < i64::from(at_bar) * 4 {
                    bpm_a
                } else {
                    bpm_b
                }
            }
            Self::Ramp { bpm_a, from_bar, bars, bpm_b } => {
                let from = i64::from(from_bar) * 4;
                let to = i64::from(from_bar + bars) * 4;
                if k < from {
                    bpm_a
                } else if k >= to {
                    bpm_b
                } else {
                    bpm_a + (bpm_b - bpm_a) * (k - from) as f64 / (to - from) as f64
                }
            }
        }
    }
}

/// One synthetic track.
#[derive(Debug, Clone)]
pub struct TrackSpec {
    pub name: String,
    pub style: Style,
    pub tempo: TempoPlan,
    pub sample_rate: u32,
    pub seed: u64,
    /// Seconds from the start of the file to the first downbeat.
    pub lead_in_secs: f64,
    /// Beats of drums before the first downbeat (a pickup), 0–3.
    pub pickup_beats: u32,
    /// Bars at the start with no drums at all.
    pub beatless_intro_bars: u32,
    /// Standard deviation of every hit's timing, in milliseconds. Zero for a
    /// quantized track; the truth is always the nominal grid.
    pub humanize_ms: f64,
    /// Labels for reports. Some also switch arrangement features that hide
    /// the bar: `subtle-bars` (no crashes, chords change every four bars),
    /// `fills` (a snare fill and reverse cymbal into each section),
    /// `anacrusis` (a melodic pickup on beat 4 before each section),
    /// `drop-gap` (silence on the beat before each drop) and `long-intro`
    /// (32 bars of kick and hats before anything changes).
    pub tags: Vec<&'static str>,
}

impl TrackSpec {
    pub fn has(&self, tag: &str) -> bool {
        self.tags.contains(&tag)
    }
}

/// Musical beat times: index `origin` is the first downbeat, bar 1 beat 1.
struct Timeline {
    times: Vec<f64>,
    origin: usize,
}

impl Timeline {
    fn build(plan: TempoPlan, lead_in: f64, musical_beats: usize) -> Self {
        // Backwards from the first downbeat at the opening tempo, to one beat
        // before the file starts, so pickups and the extended grid exist.
        let first_period = 60.0 / plan.start_bpm();
        let mut before = Vec::new();
        let mut t = lead_in;
        while t > -first_period {
            t -= first_period;
            before.push(t);
        }
        before.reverse();
        let origin = before.len();
        let mut times = before;
        let mut t = lead_in;
        // A few beats past the end, for the tail and for interpolation.
        for k in 0..(musical_beats + 16) as i64 {
            times.push(t);
            t += 60.0 / plan.bpm_after(k);
        }
        Self { times, origin }
    }

    /// The time of a musical position in beats from the first downbeat.
    fn at(&self, beat: f64) -> f64 {
        let index = self.origin as f64 + beat;
        let last = self.times.len() - 1;
        if index <= 0.0 {
            return self.times[0] + index * (self.times[1] - self.times[0]);
        }
        let i = index.floor() as usize;
        if i >= last {
            return self.times[last] + (index - last as f64) * (self.times[last] - self.times[last - 1]);
        }
        let frac = index - i as f64;
        self.times[i] + frac * (self.times[i + 1] - self.times[i])
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools, reason = "one switch per instrument layer reads best here")]
struct Layers {
    kick: bool,
    hats: bool,
    clap: bool,
    bass: bool,
    pad: bool,
    lead: bool,
    perc: bool,
    roll: bool,
    riser: bool,
    crash: bool,
}

#[derive(Debug, Clone, Copy)]
struct Section {
    bars: u32,
    layers: Layers,
}

const DRUMS: Layers = Layers { kick: true, hats: true, clap: false, bass: false, pad: false, lead: false, perc: true, roll: false, riser: false, crash: false };
const GROOVE: Layers = Layers { kick: true, hats: true, clap: true, bass: true, pad: false, lead: false, perc: true, roll: false, riser: false, crash: false };
const BREAKDOWN: Layers = Layers { kick: false, hats: false, clap: false, bass: false, pad: true, lead: true, perc: false, roll: false, riser: false, crash: true };
const BUILD: Layers = Layers { kick: false, hats: true, clap: false, bass: false, pad: true, lead: true, perc: false, roll: true, riser: true, crash: false };
const DROP: Layers = Layers { kick: true, hats: true, clap: true, bass: true, pad: true, lead: true, perc: true, roll: false, riser: false, crash: true };
const BASS_BREAK: Layers = Layers { kick: false, hats: true, clap: false, bass: true, pad: true, lead: false, perc: false, roll: false, riser: false, crash: true };
const PAD_ONLY: Layers = Layers { kick: false, hats: false, clap: false, bass: false, pad: true, lead: false, perc: false, roll: false, riser: false, crash: false };

fn arrangement(style: Style, beatless_bars: u32, long_intro: bool) -> Vec<Section> {
    let mut sections = Vec::new();
    if beatless_bars > 0 {
        sections.push(Section { bars: beatless_bars, layers: PAD_ONLY });
    }
    if style == Style::HipHop {
        let verse = Layers { lead: false, pad: true, perc: false, ..GROOVE };
        let hook = Layers { lead: true, crash: true, ..verse };
        for (bars, layers) in [(4, PAD_ONLY), (16, verse), (8, hook), (16, verse), (8, hook), (4, Layers { pad: true, ..DRUMS })] {
            sections.push(Section { bars, layers });
        }
        return sections;
    }
    let intro = if long_intro { (32, Layers { perc: false, ..DRUMS }) } else { (16, DRUMS) };
    for (bars, layers) in [
        intro,
        (16, GROOVE),
        (12, BREAKDOWN),
        (4, BUILD),
        (32, DROP),
        (8, BASS_BREAK),
        (16, DROP),
        (16, DRUMS),
    ] {
        sections.push(Section { bars, layers });
    }
    sections
}

/// Where each instrument plays within a bar, in beats.
struct Patterns {
    kick: &'static [f64],
    clap: &'static [f64],
    hats: &'static [f64],
    open_hats: &'static [f64],
    /// (position, length) in beats.
    bass: &'static [(f64, f64)],
    perc: &'static [f64],
    /// Delay of the off-beat sixteenths (x.25 and x.75), as a fraction of a sixteenth.
    swing: f64,
}

const FOUR: &[f64] = &[0.0, 1.0, 2.0, 3.0];
const BACKBEAT: &[f64] = &[1.0, 3.0];
const OFFBEATS: &[f64] = &[0.5, 1.5, 2.5, 3.5];
const EIGHTHS: &[f64] = &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5];
const SIXTEENTHS: &[f64] = &[0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0, 3.25, 3.5, 3.75];
const NONE: &[f64] = &[];

fn patterns(style: Style) -> Patterns {
    match style {
        Style::House => Patterns {
            kick: FOUR,
            clap: BACKBEAT,
            hats: EIGHTHS,
            open_hats: OFFBEATS,
            bass: &[(0.5, 0.35), (1.5, 0.35), (2.5, 0.35), (3.5, 0.35)],
            perc: NONE,
            swing: 0.0,
        },
        Style::TechHouse => Patterns {
            kick: FOUR,
            clap: BACKBEAT,
            hats: SIXTEENTHS,
            open_hats: OFFBEATS,
            bass: &[(0.25, 0.2), (0.75, 0.2), (1.25, 0.2), (1.75, 0.2), (2.25, 0.2), (2.75, 0.2), (3.25, 0.2), (3.75, 0.2)],
            perc: &[0.75, 2.25, 3.5],
            swing: 0.12,
        },
        Style::Techno => Patterns {
            kick: FOUR,
            clap: NONE,
            hats: SIXTEENTHS,
            open_hats: OFFBEATS,
            bass: &[
                (0.25, 0.2), (0.5, 0.2), (0.75, 0.2), (1.25, 0.2), (1.5, 0.2), (1.75, 0.2),
                (2.25, 0.2), (2.5, 0.2), (2.75, 0.2), (3.25, 0.2), (3.5, 0.2), (3.75, 0.2),
            ],
            perc: &[1.5, 3.75],
            swing: 0.0,
        },
        Style::Trance => Patterns {
            kick: FOUR,
            clap: BACKBEAT,
            hats: OFFBEATS,
            open_hats: OFFBEATS,
            bass: &[
                (0.25, 0.2), (0.5, 0.2), (0.75, 0.2), (1.25, 0.2), (1.5, 0.2), (1.75, 0.2),
                (2.25, 0.2), (2.5, 0.2), (2.75, 0.2), (3.25, 0.2), (3.5, 0.2), (3.75, 0.2),
            ],
            perc: NONE,
            swing: 0.0,
        },
        Style::DrumAndBass => Patterns {
            kick: &[0.0, 2.5],
            clap: BACKBEAT,
            hats: EIGHTHS,
            open_hats: NONE,
            bass: &[(0.0, 2.4), (2.5, 1.4)],
            perc: &[1.75, 3.25],
            swing: 0.0,
        },
        Style::Dubstep => Patterns {
            kick: &[0.0],
            clap: &[2.0],
            hats: EIGHTHS,
            open_hats: NONE,
            bass: &[(0.0, 1.5), (2.0, 0.75), (3.0, 0.75)],
            perc: NONE,
            swing: 0.0,
        },
        Style::Breaks => Patterns {
            kick: &[0.0, 1.75, 2.5],
            clap: BACKBEAT,
            hats: EIGHTHS,
            open_hats: &[3.5],
            bass: &[(0.0, 0.5), (1.75, 0.5), (2.5, 0.75)],
            perc: NONE,
            swing: 0.0,
        },
        Style::HipHop => Patterns {
            kick: &[0.0, 1.75, 2.5],
            clap: BACKBEAT,
            hats: SIXTEENTHS,
            open_hats: NONE,
            bass: &[(0.0, 1.0), (2.5, 0.75)],
            perc: NONE,
            swing: 0.45,
        },
        Style::AfroHouse => Patterns {
            kick: FOUR,
            clap: BACKBEAT,
            hats: SIXTEENTHS,
            open_hats: NONE,
            bass: &[(0.5, 0.3), (1.75, 0.25), (2.5, 0.3), (3.5, 0.3)],
            perc: &[0.75, 1.5, 2.25, 3.0, 3.75],
            swing: 0.08,
        },
    }
}

/// A kick drum's sound: a falling sine body with an optional click.
#[derive(Debug, Clone, Copy)]
struct Kick {
    start_hz: f64,
    end_hz: f64,
    sweep_secs: f64,
    decay_secs: f64,
    click: f64,
}

impl Kick {
    fn for_style(style: Style, rng: &mut Rng) -> Self {
        let (start, decay, click) = match style {
            Style::HipHop => ((100.0, 140.0), (0.35, 0.55), (0.0, 0.15)),
            Style::DrumAndBass => ((170.0, 240.0), (0.10, 0.18), (0.3, 0.8)),
            Style::Techno => ((110.0, 160.0), (0.20, 0.40), (0.1, 0.6)),
            Style::Dubstep => ((140.0, 200.0), (0.25, 0.4), (0.3, 0.7)),
            _ => ((150.0, 220.0), (0.15, 0.32), (0.15, 0.7)),
        };
        Self {
            start_hz: rng.range(start.0, start.1),
            end_hz: rng.range(44.0, 58.0),
            sweep_secs: rng.range(0.02, 0.05),
            decay_secs: rng.range(decay.0, decay.1),
            click: rng.range(click.0, click.1),
        }
    }
}

/// A two-pole filter in direct form I.
#[derive(Debug, Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn bandpass(sample_rate: u32, hz: f64, q: f64) -> Self {
        let w = 2.0 * std::f64::consts::PI * hz / f64::from(sample_rate);
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b: [alpha / a0, 0.0, -alpha / a0],
            a: [-2.0 * w.cos() / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn run(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// First sample at or after `at` seconds, and the offset of that sample from
/// the onset in seconds (in `[0, 1/sr)`).
fn first_sample(at: f64, sample_rate: u32) -> (usize, f64) {
    if at < 0.0 {
        // Starts before the file: render from sample 0 with a later offset.
        return (0, -at);
    }
    let position = at * f64::from(sample_rate);
    let n = position.ceil();
    (n as usize, (n - position) / f64::from(sample_rate))
}

fn render_kick(buf: &mut [f32], sample_rate: u32, at: f64, kick: &Kick, gain: f64, rng: &mut Rng) {
    let (start, offset) = first_sample(at, sample_rate);
    let dt = 1.0 / f64::from(sample_rate);
    let length = (kick.decay_secs * 6.0 / dt) as usize;
    let tau = kick.sweep_secs;
    let span = kick.start_hz - kick.end_hz;
    for (i, out) in buf.iter_mut().skip(start).take(length).enumerate() {
        let t = offset + i as f64 * dt;
        // Phase of a sine whose frequency falls exponentially from start to end.
        let phase = 2.0 * std::f64::consts::PI * (kick.end_hz * t + span * tau * (1.0 - (-t / tau).exp()));
        let body = phase.sin() * (-t / kick.decay_secs).exp();
        let click = kick.click * rng.signed() * (-t / 0.0015).exp();
        *out += (gain * (body + click)) as f32;
    }
}

/// Noise through first differences (a crude high-pass), exponentially decaying.
fn render_noise(buf: &mut [f32], sample_rate: u32, at: f64, decay_secs: f64, differences: u32, gain: f64, rng: &mut Rng) {
    let (start, offset) = first_sample(at, sample_rate);
    let dt = 1.0 / f64::from(sample_rate);
    let length = (decay_secs * 7.0 / dt) as usize;
    let mut previous = [0.0_f64; 3];
    for (i, out) in buf.iter_mut().skip(start).take(length).enumerate() {
        let t = offset + i as f64 * dt;
        let mut x = rng.signed();
        for stage in previous.iter_mut().take(differences as usize) {
            let d = (x - *stage) * 0.5;
            *stage = x;
            x = d;
        }
        *out += (gain * x * (-t / decay_secs).exp()) as f32;
    }
}

fn render_clap(buf: &mut [f32], sample_rate: u32, at: f64, gain: f64, rng: &mut Rng) {
    let (start, offset) = first_sample(at, sample_rate);
    let dt = 1.0 / f64::from(sample_rate);
    let length = (0.45 / dt) as usize;
    let mut filter = Biquad::bandpass(sample_rate, rng.range(1000.0, 1600.0), 1.2);
    for (i, out) in buf.iter_mut().skip(start).take(length).enumerate() {
        let t = offset + i as f64 * dt;
        // Three quick bursts, then a tail.
        let bursts = [0.0, 0.009, 0.018].iter().map(|b| if t >= *b { (-(t - b) / 0.004).exp() } else { 0.0 }).sum::<f64>();
        let tail = if t >= 0.018 { 0.6 * (-(t - 0.018) / 0.07).exp() } else { 0.0 };
        *out += (gain * 3.0 * filter.run(rng.signed()) * (bursts + tail)) as f32;
    }
}

/// A note made of harmonics, with linear attack and release.
#[allow(clippy::too_many_arguments, reason = "a synth voice's parameters, all scalar")]
fn render_tone(buf: &mut [f32], sample_rate: u32, at: f64, length_secs: f64, hz: f64, harmonics: &[f64], attack: f64, release: f64, gain: f64) {
    let (start, offset) = first_sample(at, sample_rate);
    let dt = 1.0 / f64::from(sample_rate);
    let total = length_secs + release;
    let samples = (total / dt) as usize;
    let nyquist = f64::from(sample_rate) * 0.45;
    // One rotating phasor per harmonic: cheaper than a sine per sample.
    let mut phasors: Vec<(f64, f64, f64, f64, f64)> = harmonics
        .iter()
        .enumerate()
        .filter(|(h, _)| hz * (*h as f64 + 1.0) < nyquist)
        .map(|(h, amp)| {
            let w = 2.0 * std::f64::consts::PI * hz * (h as f64 + 1.0) * dt;
            let start_phase = 2.0 * std::f64::consts::PI * hz * (h as f64 + 1.0) * offset;
            (start_phase.cos(), start_phase.sin(), w.cos(), w.sin(), *amp)
        })
        .collect();
    for (i, out) in buf.iter_mut().skip(start).take(samples).enumerate() {
        let t = offset + i as f64 * dt;
        let envelope = if t < attack {
            t / attack
        } else if t < length_secs {
            1.0
        } else {
            (1.0 - (t - length_secs) / release).max(0.0)
        };
        let mut sum = 0.0;
        for p in &mut phasors {
            sum += p.1 * p.4;
            let (c, s) = (p.0 * p.2 - p.1 * p.3, p.1 * p.2 + p.0 * p.3);
            p.0 = c;
            p.1 = s;
        }
        *out += (gain * envelope * sum) as f32;
    }
}

fn midi_hz(note: f64) -> f64 {
    440.0 * 2.0_f64.powf((note - 69.0) / 12.0)
}

/// A rendered track and its exact grid.
pub struct Rendered {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub truth: Truth,
}

struct Buses {
    kick: Vec<f32>,
    drums: Vec<f32>,
    music: Vec<f32>,
}

/// Renders one track.
#[allow(clippy::too_many_lines, reason = "one arrangement loop placing every instrument")]
pub fn render(spec: &TrackSpec) -> Rendered {
    let mut rng = Rng::new(spec.seed);
    let sr = spec.sample_rate;
    let sections = arrangement(spec.style, spec.beatless_intro_bars, spec.has("long-intro"));
    let subtle = spec.has("subtle-bars");
    let chord_bars = if subtle { 4 } else { 2 };
    let total_bars: u32 = sections.iter().map(|s| s.bars).sum();
    let timeline = Timeline::build(spec.tempo, spec.lead_in_secs, total_bars as usize * 4);
    let end_secs = timeline.at(f64::from(total_bars) * 4.0);
    let duration_secs = end_secs + 2.0;
    let length = (duration_secs * f64::from(sr)) as usize;
    let mut buses = Buses { kick: vec![0.0; length], drums: vec![0.0; length], music: vec![0.0; length] };

    let kick = Kick::for_style(spec.style, &mut rng);
    let pattern = patterns(spec.style);
    let root = 45.0 + (rng.next_u64() % 8) as f64;
    // i, VI, III, VII of a minor key, as semitone offsets of each chord's root.
    let progression = [0.0, 8.0, 3.0, 10.0];
    let minor_like = [true, false, false, false];
    let jitter = spec.humanize_ms / 1000.0;
    let mut kick_times = Vec::new();
    let mut phrase_starts = Vec::new();
    let mut mutes = Vec::new();

    let mut bar = 0_u32;
    for (index, section) in sections.iter().enumerate() {
        let is_drop = section.layers.kick && section.layers.lead;
        if is_drop && bar > 0 && spec.has("drop-gap") {
            mutes.push((timeline.at(f64::from(bar) * 4.0 - 1.0), timeline.at(f64::from(bar) * 4.0)));
        }
        phrase_starts.push(timeline.at(f64::from(bar) * 4.0));
        for local in 0..section.bars {
            let b = f64::from(bar + local) * 4.0;
            let layers = section.layers;
            let absolute_bar = bar + local;
            let chord = (absolute_bar / chord_bars % 4) as usize;
            let chord_root = root + progression[chord];
            let third = if minor_like[chord] { 3.0 } else { 4.0 };
            let hit = |beat: f64, rng: &mut Rng| timeline.at(b + beat) + jitter * rng.normal();
            let swung = |beat: f64| {
                let sixteenth = (beat * 4.0).round() as i64;
                if sixteenth % 2 == 1 { beat + pattern.swing * 0.25 } else { beat }
            };

            if layers.kick {
                for &beat in pattern.kick {
                    let at = hit(beat, &mut rng);
                    kick_times.push(at);
                    render_kick(&mut buses.kick, sr, at, &kick, 0.9, &mut rng);
                }
            }
            if layers.clap {
                for &beat in pattern.clap {
                    let at = hit(beat, &mut rng);
                    render_clap(&mut buses.drums, sr, at, 0.35, &mut rng);
                }
            }
            if layers.hats {
                for &beat in pattern.hats {
                    let accent = if (beat * 2.0).fract() == 0.0 { 0.10 } else { 0.07 };
                    let at = hit(swung(beat), &mut rng);
                    render_noise(&mut buses.drums, sr, at, 0.018, 2, accent, &mut rng);
                }
                for &beat in pattern.open_hats {
                    let at = hit(beat, &mut rng);
                    render_noise(&mut buses.drums, sr, at, 0.09, 2, 0.08, &mut rng);
                }
            }
            if layers.perc {
                for &beat in pattern.perc {
                    let at = hit(swung(beat), &mut rng);
                    let hz = midi_hz(root + 36.0 + [0.0, 7.0, 12.0][(rng.next_u64() % 3) as usize]);
                    render_tone(&mut buses.drums, sr, at, 0.03, hz, &[1.0, 0.3], 0.001, 0.12, 0.12);
                }
            }
            if layers.roll && local + 1 == section.bars {
                // A snare roll in the bar before the drop: sixteenths, rising.
                for step in 0..16 {
                    let at = hit(f64::from(step) * 0.25, &mut rng);
                    render_clap(&mut buses.drums, sr, at, 0.08 + 0.015 * f64::from(step), &mut rng);
                }
            }
            if layers.bass {
                for &(beat, len) in pattern.bass {
                    let at = hit(beat, &mut rng);
                    let secs = timeline.at(b + beat + len) - timeline.at(b + beat);
                    let saw = [1.0, 0.5, 0.33, 0.25, 0.2, 0.16, 0.14, 0.12];
                    render_tone(&mut buses.music, sr, at, secs, midi_hz(chord_root - 12.0), &saw, 0.003, 0.02, 0.3);
                }
            }
            if layers.pad && local % chord_bars == 0 {
                let at = timeline.at(b);
                let secs = timeline.at(b + f64::from(chord_bars) * 4.0) - at;
                for interval in [0.0, third, 7.0, 12.0] {
                    render_tone(&mut buses.music, sr, at, secs, midi_hz(chord_root + 12.0 + interval), &[1.0, 0.4, 0.2], 0.25, 0.4, 0.035);
                }
            }
            if layers.lead {
                for beat in [0.5, 1.75, 2.5, 3.25] {
                    let at = hit(beat, &mut rng);
                    let note = chord_root + 24.0 + [0.0, third, 7.0][(rng.next_u64() % 3) as usize];
                    render_tone(&mut buses.music, sr, at, 0.12, midi_hz(note), &[1.0, 0.0, 0.33, 0.0, 0.2], 0.002, 0.08, 0.05);
                }
            }
            if layers.crash && !subtle && local % 16 == 0 {
                let at = hit(0.0, &mut rng);
                render_noise(&mut buses.music, sr, at, 1.1, 2, 0.12, &mut rng);
            }
            if layers.riser {
                // Noise swelling over the section to the next downbeat.
                let from = timeline.at(b);
                let to = timeline.at(b + 4.0);
                let start = (from * f64::from(sr)) as usize;
                let stop = ((to * f64::from(sr)) as usize).min(length);
                let section_start = timeline.at(f64::from(bar) * 4.0);
                let section_end = timeline.at(f64::from(bar + section.bars) * 4.0);
                for (n, out) in buses.music.iter_mut().enumerate().take(stop).skip(start) {
                    let t = n as f64 / f64::from(sr);
                    let rise = ((t - section_start) / (section_end - section_start)).clamp(0.0, 1.0);
                    *out += (0.06 * rise * rise * rng.signed()) as f32;
                }
            }
        }
        let next_bar = bar + section.bars;
        if index + 1 < sections.len() {
            let last = f64::from(next_bar - 1) * 4.0;
            if spec.has("fills") {
                for step in [3.0, 3.25, 3.5, 3.75] {
                    let at = timeline.at(last + step) + jitter * rng.normal();
                    render_clap(&mut buses.drums, sr, at, 0.22, &mut rng);
                }
                // A reverse cymbal: noise swelling over the last beat, cut
                // dead on the next downbeat.
                let from = timeline.at(last + 3.0);
                let to = timeline.at(last + 4.0);
                let start = (from * f64::from(sr)) as usize;
                let stop = ((to * f64::from(sr)) as usize).min(length);
                for (n, out) in buses.music.iter_mut().enumerate().take(stop).skip(start) {
                    let x = (n as f64 / f64::from(sr) - from) / (to - from);
                    *out += (0.14 * x * x * x * rng.signed()) as f32;
                }
            }
            if spec.has("anacrusis") {
                let next_chord = (next_bar / chord_bars % 4) as usize;
                for (step, interval) in [(3.0, 7.0), (3.5, 12.0)] {
                    let at = timeline.at(last + step) + jitter * rng.normal();
                    let hz = midi_hz(root + progression[next_chord] + 24.0 + interval);
                    render_tone(&mut buses.music, sr, at, 0.2, hz, &[1.0, 0.0, 0.33, 0.0, 0.2], 0.002, 0.1, 0.09);
                }
            }
        }
        bar = next_bar;
    }

    // Pickup hits before the first downbeat: kick and clap on the last beats
    // of the bar before bar 1.
    for p in 0..spec.pickup_beats.min(3) {
        let beat = -f64::from(p + 1);
        let at = timeline.at(beat) + jitter * rng.normal();
        if at >= 0.0 {
            render_clap(&mut buses.drums, sr, at, 0.25, &mut rng);
            render_kick(&mut buses.kick, sr, at, &kick, 0.7, &mut rng);
            kick_times.push(at);
        }
    }

    let samples = mix(&mut buses, &kick_times, &mutes, sr, &mut rng);
    let truth = truth_of(spec, &timeline, duration_secs, phrase_starts);
    Rendered { samples, sample_rate: sr, truth }
}

/// Sidechain ducking on the music bus, summing, muting, and a soft-clipped master.
fn mix(buses: &mut Buses, kick_times: &[f64], mutes: &[(f64, f64)], sample_rate: u32, rng: &mut Rng) -> Vec<f32> {
    let mut kicks = kick_times.to_vec();
    kicks.sort_by(f64::total_cmp);
    let depth = rng.range(0.0, 0.55);
    let mut next = 0;
    let mut last_kick = f64::NEG_INFINITY;
    let n = buses.kick.len();
    let mut out = vec![0.0_f32; n];
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / f64::from(sample_rate);
        while next < kicks.len() && kicks[next] <= t {
            last_kick = kicks[next];
            next += 1;
        }
        let duck = 1.0 - depth * (-(t - last_kick) / 0.12).exp();
        let drums_duck = 1.0 - 0.3 * depth * (-(t - last_kick) / 0.08).exp();
        *sample = buses.kick[i] + buses.drums[i] * drums_duck as f32 + buses.music[i] * duck as f32;
    }
    // Silences with 3 ms fades, so the cut itself is not a click.
    let fade = 0.003;
    for &(from, to) in mutes {
        let start = ((from - fade) * f64::from(sample_rate)).max(0.0) as usize;
        let stop = ((to * f64::from(sample_rate)) as usize).min(n);
        for (i, sample) in out.iter_mut().enumerate().take(stop).skip(start) {
            let t = i as f64 / f64::from(sample_rate);
            let gain = if t < from { (from - t) / fade } else if t > to - fade { (t - (to - fade)) / fade } else { 0.0 };
            *sample *= gain.clamp(0.0, 1.0) as f32;
        }
    }
    let peak = out.iter().fold(0.0_f32, |m, s| m.max(s.abs())).max(1e-9);
    let drive = rng.range(1.0, 2.6);
    let norm = (drive).tanh();
    for s in &mut out {
        *s = ((f64::from(*s / peak) * drive).tanh() / norm * 0.95) as f32;
    }
    out
}

fn truth_of(spec: &TrackSpec, timeline: &Timeline, duration_secs: f64, phrase_starts: Vec<f64>) -> Truth {
    let beats = timeline
        .times
        .iter()
        .enumerate()
        .filter(|(_, t)| **t >= 0.0 && **t < duration_secs)
        .map(|(i, t)| {
            let musical = i as i64 - timeline.origin as i64;
            TruthBeat { secs: *t, number: (musical.rem_euclid(4) + 1) as u8 }
        })
        .collect();
    Truth {
        name: spec.name.clone(),
        style: spec.style.name().to_owned(),
        sample_rate: spec.sample_rate,
        duration_secs,
        bpm: spec.tempo.start_bpm(),
        constant_tempo: matches!(spec.tempo, TempoPlan::Constant(_)),
        humanize_ms: spec.humanize_ms,
        beats,
        phrase_starts_secs: phrase_starts,
        tags: spec.tags.iter().map(|t| (*t).to_owned()).collect(),
    }
}

/// The default suite: weighted toward what a DJ library holds (house and
/// techno around 120–135 BPM), with the hard cases each represented.
#[allow(clippy::too_many_lines, reason = "a table of tracks")]
pub fn suite() -> Vec<TrackSpec> {
    use Style::{AfroHouse, Breaks, DrumAndBass, Dubstep, HipHop, House, TechHouse, Techno, Trance};
    use TempoPlan::{Constant, Cut, Ramp};
    struct Row(&'static str, Style, TempoPlan, u32, f64, u32, u32, f64, &'static [&'static str]);
    let rows = [
        Row("house-124-zero", House, Constant(124.0), 44_100, 0.0, 0, 0, 0.0, &["starts-at-zero"]),
        Row("house-126.5", House, Constant(126.5), 48_000, 0.3127, 0, 0, 0.0, &["fractional-bpm"]),
        Row("house-120-beatless", House, Constant(120.0), 44_100, 0.1213, 0, 8, 0.0, &["beatless-intro"]),
        Row("house-128-pickup", House, Constant(128.0), 44_100, 1.4061, 3, 0, 0.0, &["pickup"]),
        Row("house-125", House, Constant(125.0), 48_000, 0.0507, 0, 0, 0.0, &[]),
        Row("house-118", House, Constant(118.0), 44_100, 0.7779, 0, 0, 0.0, &[]),
        Row("house-123-zero", House, Constant(123.0), 48_000, 0.0, 0, 0, 0.0, &["starts-at-zero"]),
        Row("techhouse-127-zero", TechHouse, Constant(127.0), 44_100, 0.0, 0, 0, 0.0, &["starts-at-zero", "swing"]),
        Row("techhouse-125.25", TechHouse, Constant(125.25), 48_000, 0.4833, 0, 0, 0.0, &["fractional-bpm", "swing"]),
        Row("techhouse-126", TechHouse, Constant(126.0), 44_100, 0.2001, 1, 0, 0.0, &["pickup", "swing"]),
        Row("techno-130", Techno, Constant(130.0), 44_100, 0.0213, 0, 0, 0.0, &[]),
        Row("techno-135-beatless", Techno, Constant(135.0), 48_000, 0.7518, 0, 16, 0.0, &["beatless-intro"]),
        Row("techno-140-zero", Techno, Constant(140.0), 44_100, 0.0, 0, 0, 0.0, &["starts-at-zero"]),
        Row("techno-132", Techno, Constant(132.0), 48_000, 0.3333, 0, 0, 0.0, &[]),
        Row("techno-145", Techno, Constant(145.0), 44_100, 0.1, 0, 0, 0.0, &[]),
        Row("techno-133-live", Techno, Constant(133.0), 44_100, 0.0, 0, 0, 2.0, &["humanized"]),
        Row("trance-138", Trance, Constant(138.0), 44_100, 0.2049, 0, 0, 0.0, &[]),
        Row("trance-136.6-zero", Trance, Constant(136.6), 48_000, 0.0, 0, 0, 0.0, &["fractional-bpm", "starts-at-zero"]),
        Row("trance-142", Trance, Constant(142.0), 44_100, 0.0901, 0, 8, 0.0, &["beatless-intro"]),
        Row("trance-122-melodic", Trance, Constant(122.0), 48_000, 0.1577, 0, 0, 0.0, &[]),
        Row("dnb-174-zero", DrumAndBass, Constant(174.0), 44_100, 0.0, 0, 0, 0.0, &["broken-kick", "starts-at-zero"]),
        Row("dnb-172-pickup", DrumAndBass, Constant(172.0), 48_000, 0.4011, 2, 0, 0.0, &["broken-kick", "pickup"]),
        Row("dnb-175", DrumAndBass, Constant(175.0), 44_100, 0.0377, 0, 0, 0.0, &["broken-kick"]),
        Row("dubstep-140", Dubstep, Constant(140.0), 44_100, 0.05, 0, 0, 0.0, &["half-time"]),
        Row("breaks-132-zero", Breaks, Constant(132.0), 44_100, 0.0, 0, 0, 0.0, &["broken-kick", "starts-at-zero"]),
        Row("hiphop-92", HipHop, Constant(92.0), 44_100, 0.25, 0, 0, 0.0, &["broken-kick", "swing"]),
        Row("hiphop-87.3-live", HipHop, Constant(87.3), 48_000, 0.1, 0, 0, 6.0, &["broken-kick", "swing", "humanized", "fractional-bpm"]),
        Row("afro-122-zero", AfroHouse, Constant(122.0), 44_100, 0.0, 0, 0, 0.0, &["starts-at-zero", "percussion"]),
        Row("afro-121.7-pickup", AfroHouse, Constant(121.7), 48_000, 0.6602, 1, 0, 0.0, &["fractional-bpm", "pickup", "percussion"]),
        Row("edit-128-cut-140", House, Cut { bpm_a: 128.0, at_bar: 48, bpm_b: 140.0 }, 44_100, 0.0, 0, 0, 0.0, &["tempo-change"]),
        Row("edit-140-cut-174", Trance, Cut { bpm_a: 140.0, at_bar: 64, bpm_b: 174.0 }, 48_000, 0.1503, 0, 0, 0.0, &["tempo-change"]),
        Row("edit-124-ramp-128", TechHouse, Ramp { bpm_a: 124.0, from_bar: 48, bars: 8, bpm_b: 128.0 }, 48_000, 0.1, 0, 0, 0.0, &["tempo-ramp"]),
        // Bar cues hidden the way real tracks hide them.
        Row("house-124-subtle", House, Constant(124.0), 44_100, 0.2071, 0, 0, 0.0, &["subtle-bars", "fills", "anacrusis"]),
        Row("techhouse-126-dropgap", TechHouse, Constant(126.0), 48_000, 0.0503, 0, 0, 0.0, &["subtle-bars", "drop-gap", "fills", "swing"]),
        Row("techno-128-longintro", Techno, Constant(128.0), 44_100, 0.0, 0, 0, 0.0, &["long-intro", "subtle-bars", "starts-at-zero"]),
        Row("trance-138-anacrusis", Trance, Constant(138.0), 44_100, 0.3019, 0, 0, 0.0, &["anacrusis", "fills"]),
        Row("afro-123-subtle", AfroHouse, Constant(123.0), 48_000, 0.4107, 0, 0, 0.0, &["subtle-bars", "anacrusis", "long-intro", "percussion"]),
        Row("house-122-everything", House, Constant(122.0), 44_100, 0.9013, 2, 0, 0.0, &["subtle-bars", "fills", "anacrusis", "drop-gap", "long-intro", "pickup"]),
        Row("techno-131-dropgap", Techno, Constant(131.0), 44_100, 0.1307, 0, 0, 0.0, &["drop-gap", "long-intro", "subtle-bars"]),
        Row("melodic-124-subtle", Trance, Constant(124.0), 48_000, 0.0, 0, 0, 0.0, &["subtle-bars", "anacrusis", "long-intro", "starts-at-zero"]),
    ];
    rows.iter()
        .enumerate()
        .map(|(i, Row(name, style, tempo, sample_rate, lead_in, pickup, beatless, humanize, tags))| TrackSpec {
            name: (*name).to_owned(),
            style: *style,
            tempo: *tempo,
            sample_rate: *sample_rate,
            seed: 0x0B0E_0000 + i as u64,
            lead_in_secs: *lead_in,
            pickup_beats: *pickup,
            beatless_intro_bars: *beatless,
            humanize_ms: *humanize,
            tags: tags.to_vec(),
        })
        .collect()
}
