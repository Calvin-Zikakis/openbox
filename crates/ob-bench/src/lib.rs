//! openbox's beat-grid benchmark.
//!
//! Synthetic tracks give exact ground truth ([`synth`]); [`metrics`] scores a
//! predicted grid against it to the millisecond; [`analyzers`] puts every
//! analyzer under test behind one interface. The `ob-bench` binary runs them.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "DSP and scoring convert freely between sample counts, beat counts and float time"
)]

pub mod analyzers;
pub mod metrics;
pub mod neural;
pub mod synth;
pub mod truth;
pub mod wav;
