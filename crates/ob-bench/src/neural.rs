//! What the neural model says about a grid, for experiments.

use ob_ml::Activations;

/// Beat activation on a grid's beats over that on its midpoints, and the
/// mean downbeat activation on each of the four bar positions counted from
/// the grid's first beat.
pub fn evidence(activations: &Activations, beats: &[f64]) -> (f64, [f64; 4]) {
    let (mut on, mut off) = (0.0_f64, 0.0_f64);
    let mut bar = [0.0_f64; 4];
    let mut counts = [0_usize; 4];
    for (i, pair) in beats.windows(2).enumerate() {
        let period = pair[1] - pair[0];
        let reach = (period / 8.0).max(0.021);
        on += f64::from(activations.beat_near(pair[0], reach));
        off += f64::from(activations.beat_near(pair[0] + period / 2.0, reach));
        bar[i % 4] += f64::from(activations.downbeat_near(pair[0], reach));
        counts[i % 4] += 1;
    }
    for (b, n) in bar.iter_mut().zip(counts) {
        if n > 0 {
            *b /= n as f64;
        }
    }
    (if off > 0.0 { on / off } else { f64::INFINITY }, bar)
}
