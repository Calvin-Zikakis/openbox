//! The analyzers under test, behind one interface.

use rbl_analysis::{AnalysisPreset, TempoResult};

use crate::metrics::{PredictedBeat, Prediction};

pub trait Analyzer: Sync {
    fn name(&self) -> &'static str;
    fn analyse(&self, samples: &[f32], sample_rate: u32) -> Prediction;
}

/// rbxport's analyzer with one of its app presets.
pub struct Rbxport(pub AnalysisPreset);

impl Analyzer for Rbxport {
    fn name(&self) -> &'static str {
        match self.0 {
            AnalysisPreset::Rbxport => "rbxport",
            AnalysisPreset::Rekordbox => "rbxport-normal",
        }
    }

    fn analyse(&self, samples: &[f32], sample_rate: u32) -> Prediction {
        let analysis = rbl_analysis::analyse_with(samples, sample_rate, self.0.options());
        prediction_of(&analysis.tempo)
    }
}

/// openbox's analyzer with one of rbxport's app presets, with or without
/// the neural judge.
pub struct Openbox {
    pub preset: AnalysisPreset,
    pub neural: bool,
}

impl Openbox {
    pub fn options(&self) -> ob_analysis::Options {
        let options = ob_analysis::Options::preset(self.preset);
        if self.neural { options } else { options.without_neural() }
    }
}

impl Analyzer for Openbox {
    fn name(&self) -> &'static str {
        match (self.preset, self.neural) {
            (AnalysisPreset::Rbxport, true) => "openbox",
            (AnalysisPreset::Rekordbox, true) => "openbox-normal",
            (AnalysisPreset::Rbxport, false) => "openbox-dsp",
            (AnalysisPreset::Rekordbox, false) => "openbox-dsp-normal",
        }
    }

    fn analyse(&self, samples: &[f32], sample_rate: u32) -> Prediction {
        let result = ob_analysis::analyse(samples, sample_rate, &self.options());
        prediction_of(&result.analysis.tempo)
    }
}

/// A `TempoResult` as a prediction. Its beat list is authoritative for which
/// beats exist and their numbers, but holds whole milliseconds; each beat's
/// time is refined from the segment it lies in, whose grid is exact.
pub fn prediction_of(tempo: &TempoResult) -> Prediction {
    let beats = tempo
        .beats
        .iter()
        .map(|beat| {
            let rounded = f64::from(beat.time_ms) / 1000.0;
            let exact = tempo
                .segments
                .iter()
                .filter(|s| s.period_secs > 0.0 && rounded >= s.from_secs - 0.001 && rounded <= s.to_secs + 0.001)
                .map(|s| s.phase_secs + ((rounded - s.phase_secs) / s.period_secs).round() * s.period_secs)
                .filter(|t| (t - rounded).abs() <= 0.000_501)
                .min_by(|a, b| (a - rounded).abs().total_cmp(&(b - rounded).abs()))
                .unwrap_or(rounded);
            PredictedBeat { secs: exact, number: u8::try_from(beat.beat_number).unwrap_or(0), export_ms: beat.time_ms }
        })
        .collect();
    Prediction { bpm: tempo.bpm, beats }
}
