# 0002: Decide "on the beat" and "which beat is 1" separately

Date: 2026-10-21. Status: accepted for `ob-analysis`; rbxport's analyzer is unchanged.

## Context

rbxport answers both questions with one structural vote over eight half-beat
positions. On synthetic house tracks the vote moved a correctly fitted grid onto the
off-beat 3 times in 10. The kick band's evidence was decisive each time (research note
[002](../research/002-half-beat-judge.md)).

## Decision

`ob-analysis::phase` decides the half beat from the kick band when it is decisive (a
ratio of at least 2), and from total on-beat against off-beat structure otherwise. It
decides beat 1 from per-beat structure scores that include half of each neighbouring
half beat.

## Consequences

- On the synthetic suite, beat 1 is right on 36 of 40 tracks instead of 33, with no
  regressions.
- This must be validated on real music before openbox's analyzer becomes the app's
  default (research note 003).
- It is a candidate fix to offer rbxport upstream once real-music evidence exists.
