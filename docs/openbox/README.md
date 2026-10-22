# openbox documentation

openbox is a fork of rbxport that adds a more precise song analyzer and Lexicon-class
library features. Everything specific to the fork lives in this folder. The rest of
`docs/` is rbxport's own documentation, kept as upstream maintains it.

| Document | What it holds |
| --- | --- |
| [HANDOFF.md](HANDOFF.md) | Handoff for coding agents: rules, environment, architecture, state, next tasks. |
| [PLAN.md](PLAN.md) | Goals, phases, architecture of the fork, risks and open decisions. |
| [LOG.md](LOG.md) | Dated progress log: what was done, what was measured, what is next. |
| [decisions/](decisions/) | Decision records: one file per decision, with the evidence behind it. |
| [research/](research/) | Research notes and benchmark results. |

## Rules for this folder

- Results measured on someone's private music library are summarized without track
  titles. Per-track output stays in `target/` or another ignored location, never in the
  repository, because the fork is public.
- Every benchmark number names the command that produced it, the commit, and the
  dataset, so it can be reproduced.
