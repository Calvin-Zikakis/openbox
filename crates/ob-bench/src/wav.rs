//! A minimal WAV writer, for listening to synthetic tracks and feeding encoders.

use std::io::Write;
use std::path::Path;

/// Writes mono 16-bit PCM.
pub fn write_mono_16(path: &Path, samples: &[f32], sample_rate: u32) -> std::io::Result<()> {
    let data_len = u32::try_from(samples.len() * 2).map_err(|_| std::io::Error::other("audio too long for a WAV file"))?;
    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + data_len).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16_u32.to_le_bytes())?;
    out.write_all(&1_u16.to_le_bytes())?;
    out.write_all(&1_u16.to_le_bytes())?;
    out.write_all(&sample_rate.to_le_bytes())?;
    out.write_all(&(sample_rate * 2).to_le_bytes())?;
    out.write_all(&2_u16.to_le_bytes())?;
    out.write_all(&16_u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&data_len.to_le_bytes())?;
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.write_all(&v.to_le_bytes())?;
    }
    out.flush()
}
