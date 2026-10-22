# 0003: Use Beat This! activations as a phase judge, not as a beat tracker

Date: 2026-10-21. Status: accepted.

## Context

The DSP judges cannot tell a kick from a bass line of equal low-band weight, and
structure can be fooled by a fill one beat before a phrase. Beat This! (MIT) put the
true synthetic grids on the beat 40 times out of 40 and found the downbeat 40 times out
of 40 (research note [004](../research/004-neural-judge.md)).

## Decision

- `ob-ml` wraps the `beat-this` crate (`=1.1.0`, pure-Rust `rten` backend) and returns
  per-frame beat and downbeat probabilities. The models are not committed;
  `scripts/openbox/fetch-models.sh` downloads them at pinned versions and checks their
  SHA-256 digests.
- The neural model judges only the questions the DSP finds hardest: the half beat
  (decisive at a ratio of 2) and beat 1 (blended 0.6 against structure, flagged when
  they disagree). Grid placement stays on kick attacks.
- It runs on four 20-second excerpts per track, so it costs about 1.5 s on the CPU.
- Without the models, analysis falls back to the DSP judges.

## Consequences

- Analysis now depends on 83 MB of model weights for the best result. The app will need
  to ship or download them.
- The CPU cost is about 6 times the DSP's; GPU inference is the next performance task.
- The neural downbeat can be off by half a bar on real electronic music, so beat-1
  conflicts go to the review queue instead of being trusted blindly.
