# Relay handoff

The baton between leads. Read this file, `CLAUDE.md` and
`docs/agent-workflow.md` before acting. The board (https://github.com/users/Gaurgle/projects/6) is authoritative
for status; this file holds what the board can't: in-flight work, decisions
and the next authorized step. Keep it short: finished tickets go to
`docs/agent-log.md`.

## Current lead

None. Baton released 2026-10-30 about 12:40 CET by the Claude Code
`noiz-coordinator` session (Fable 5.1) that took it at 11:58 CET. No
workers or reviewers running; both agents finished and reported.

## Authorized by the owner

2026-10-30, Andreas, in the setup session: integration is delegated to the
lead under the safe-merge rule (`docs/agent-workflow.md`, "Owner-approved
integration policy"): commit reviewed diffs, push, `git merge --no-ff` into
`main`, push, and commit coordination docs directly. No CI exists, so local
checks replace the CI conditions. Andreas keeps `cargo install`, tags and
releases.

2026-10-30, Andreas, same session, on the plan that named NOIZ-1 and NOIZ-4
as the first pair: "sounds good. i like the plan, lets do it. ship it and
start". Authorized: NOIZ-1 (done) and NOIZ-4 (clippy cleanup), in that
order. No standing scope beyond those two; the lead asks before dispatching
anything else. Stop conditions: none recorded beyond the safe-merge rule.

2026-10-30, Andreas, in the lead session: approved the `rustfft` dependency
and basing on `main` for the spectroscope spike only. "lets wrap this up
and ship it" closed the session after NOIZ-1; NOIZ-4 was not started.

## In flight

Nothing dispatched. One worktree kept by the sweeper:

- `.claude/worktrees/SPIKE-scope`, branch `spike/spectroscope` (pushed,
  commit e616749, base 614cc37). Owner-requested demo of a real FFT
  spectroscope of the mixed output, toggled with `v`: lock-free ring tap in
  the callback, rustfft 2048-point Hann, braille canvas. Not reviewed, not
  for merge as is; it conflicts with NOIZ-1 on `src/audio.rs`. Owner: "we
  can work more on that later". A proper ticket (layout, stereo tap for a
  vectorscope, modes, settings) belongs after NOIZ-2 and NOIZ-3; ask before
  creating it. Removing the worktree or remote branch is the owner's call.

## Decisions and open questions

- Checks: `cargo build`, `cargo clippy --all-targets`, `cargo test`. Clippy
  is reported, not gating, until NOIZ-4 lands (12 warnings on `main`). fmt
  is not a gate.
- Rain levels: loops are attenuated to -30 dBFS RMS, so calm and heavy are
  about 9 dB quieter than before at the default `rain_vol` 0.35. Owner may
  want a higher default; per-sample trims are noted on NOIZ-8 and NOIZ-13.
- AirPods noise control: no public macOS API. Candidate route is shelling out
  to `pods-control` (raulgg/tap, private AVRouting and CoreAudio HAL
  properties) as an optional integration; see the analysis and its ticket.
- Open: whether to add CI (NOIZ-5) early so the safe-merge rule regains its
  CI condition.

## Next step

NOIZ-4 (clippy cleanup) is authorized and Ready; base it on current `main`
(ea909bc or later). When it lands, make clippy gating in
`docs/agent-workflow.md` and `CLAUDE.md`. Anything else needs the owner.

## Start the lead

From the repo root: `claude --agent noiz-coordinator`. A Codex lead
starts from `AGENTS.md`.
