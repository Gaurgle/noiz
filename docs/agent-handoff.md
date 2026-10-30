# Relay handoff

The baton between leads. Read this file, `CLAUDE.md` and
`docs/agent-workflow.md` before acting. The board (https://github.com/users/Gaurgle/projects/6) is authoritative
for status; this file holds what the board can't: in-flight work, decisions
and the next authorized step. Keep it short: finished tickets go to
`docs/agent-log.md`.

## Current lead

None. Relay was set up on 2026-10-30 by an ordinary Claude session (Fable
5.1), which did not take the lead. Baton released, no workers running.

## Authorized by the owner

2026-10-30, Andreas, in the setup session: integration is delegated to the
lead under the safe-merge rule (`docs/agent-workflow.md`, "Owner-approved
integration policy"): commit reviewed diffs, push, `git merge --no-ff` into
`main`, push, and commit coordination docs directly. No CI exists, so local
checks replace the CI conditions. Andreas keeps `cargo install`, tags and
releases.

2026-10-30, Andreas, same session, on the plan that named NOIZ-1 and NOIZ-4
as the first pair: "sounds good. i like the plan, lets do it. ship it and
start". Authorized: NOIZ-1 (seamless looper) and NOIZ-4 (clippy cleanup),
in that order or in parallel on disjoint files. No standing scope beyond
those two; the lead asks before dispatching anything else. Stop conditions:
none recorded beyond the safe-merge rule.

## In flight

Nothing dispatched yet.

Input for NOIZ-1: Andreas added `new-light-rain-loop.wav` at the repo root
(15 s, stereo 44.1 kHz PCM16, level flat from head to tail at about -30 dB
mean, no baked fade, unlike the three files in `samples-rain/`). The worker
should treat it as the reference loop source and decide with the lead
whether it replaces `samples-rain/light-rain.wav`.

## Decisions and open questions

- Checks: `cargo build`, `cargo clippy --all-targets`, `cargo test`. Clippy
  is reported, not gating, until the cleanup ticket lands. fmt is not a gate.
- AirPods noise control: no public macOS API. Candidate route is shelling out
  to `pods-control` (raulgg/tap, private AVRouting and CoreAudio HAL
  properties) as an optional integration; see the analysis and its ticket.
- Open: whether to add CI (GitHub Actions, macOS + ubuntu) as an early
  ticket so the safe-merge rule regains its CI condition.

## Next step

The next lead (started with `relay`) takes the baton, runs the start
checks, and dispatches NOIZ-1 and NOIZ-4. Stop condition: both integrated
under the safe-merge rule, or a condition fails.

## Start the lead

From the repo root: `claude --agent noiz-coordinator`. A Codex lead
starts from `AGENTS.md`.
