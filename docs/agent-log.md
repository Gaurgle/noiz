# Relay log

Finished tickets, newest first, moved here from `docs/agent-handoff.md` when
they are Done. Not required reading on takeover. Each record ends with the
metrics line defined in `docs/agent-workflow.md`, so a retrospective is a
search rather than a tally.

## NOIZ-1: Seamless rain looper (Done 2026-10-30)

Issue #1. Base 614cc37, branch `feat/noiz-1-seamless-looper`, commit
50ae257, merge ea909bc. Worker noiz-worker (opus, medium), reviewer
noiz-reviewer (opus, medium), lead Fable 5.1.

Loops are baked at load time (trim baked fade by RMS vs median, linear
resample to the device rate, equal-power crossfade of a tenth of the sample
clamped to 1-5 s, attenuate-only normalisation to -30 dBFS RMS) into
`Arc<LoopSample>`; the callback holds three `LoopVoice`s with equal-power
switch envelopes and never allocates. Hidden `--render-rain` prints seam and
level metrics. `samples-rain/light-rain.wav` replaced by the owner's flat
15 s export; the old file failed the pump metric (0.75) on its gusts.

Review 1 (diff d3f72f05bece): needs change, tests only; the seam test was
blind on white noise and the envelope test checked its own formula, proven
by mutants. Rework round 1 fixed both plus a zero-rate panic, and folded in
two owner directions (proportional crossfade, normalisation). Round 2 made
normalisation attenuate-only on owner direction. Review 2 (fe7452e683bd):
accepted, 10 mutants each fail a test. Owner accepted the listen test.
Real-file RMS min/median: light 0.84, calm 0.83, heavy 0.78 (source
dynamics outside the crossfade, accepted). Follow-ups noted on the issue:
voice stealing hard-cuts on 3+ presses in 500 ms; `--render-loops 0`
prints `x 0`; per-sample volume trims (NOIZ-8, NOIZ-13); default rain
volume may want raising now that calm and heavy are 9 dB quieter.

NOIZ-1: review changes 1; fix cycles 2; owner decisions 3; worker 14; review 6; usage worker 136k total, reviewer 89k total (harness showed totals, not in/out)
