//! The reference grid for a track.

use serde::{Deserialize, Serialize};

/// One beat of the reference grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TruthBeat {
    /// Seconds from the start of the file, on the timeline the analyzer sees.
    pub secs: f64,
    /// 1..=4, where 1 is the downbeat.
    pub number: u8,
}

/// What a track's grid is, exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Truth {
    pub name: String,
    pub style: String,
    pub sample_rate: u32,
    pub duration_secs: f64,
    /// Tempo at the first downbeat.
    pub bpm: f64,
    pub constant_tempo: bool,
    /// Timing jitter of the performance, in milliseconds; the grid itself is
    /// the nominal one.
    pub humanize_ms: f64,
    /// Every grid beat inside the file, including those extended back over
    /// an intro or silence.
    pub beats: Vec<TruthBeat>,
    /// Downbeats where the arrangement changes.
    pub phrase_starts_secs: Vec<f64>,
    pub tags: Vec<String>,
}

impl Truth {
    /// The same grid seen `secs` later, for a decoder whose timeline starts
    /// earlier than the source (an MP3 decoded without trimming its delay).
    #[must_use]
    pub fn shifted(&self, secs: f64) -> Self {
        let mut shifted = self.clone();
        for beat in &mut shifted.beats {
            beat.secs += secs;
        }
        for start in &mut shifted.phrase_starts_secs {
            *start += secs;
        }
        shifted.duration_secs += secs;
        shifted
    }
}
