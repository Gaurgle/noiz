# Relay handoff

The baton between leads: at most 60 lines, read on every start. The board
(https://github.com/users/Gaurgle/projects/6) holds status, each ticket's
issue holds its record and rulings, `docs/agent-log.md` holds finished work
and the owner's dated words (the pre-0.8.0 baton is archived there, moved
2026-10-30).

## Current lead

None. Baton released 2026-10-30 about 12:40 CET by the Claude
`noiz-coordinator` session (Fable 5.1) that took it at 11:58 CET. No
workers or reviewers running. Migrated to Relay 0.8.0 on 2026-10-30 by
the advisor session, no lead active.

## Standing authority

| What | Granted | Scope | Until |
|---|---|---|---|
| Integrate under safe-merge: commit reviewed diffs, push, `merge --no-ff`, commit coordination docs | 2026-10-30, setup session | all Relay tickets; no CI, local checks stand in | revoked by word |
| Run NOIZ-4 (clippy cleanup) | 2026-10-30, "ship it and start" | NOIZ-4 only, after NOIZ-1 (Done) | NOIZ-4 Done |
| `rustfft` dependency | 2026-10-30, lead session | the spectroscope spike only | spike decided |

Andreas keeps `cargo install`, tags and releases. No standing scope beyond
NOIZ-4; the lead asks before dispatching anything else.

## In flight

- NOIZ-4 (#?): Ready, not dispatched; gate none. Base on current `main`.
- Spike worktree `.claude/worktrees/SPIKE-scope`, branch
  `spike/spectroscope` (e616749, base 614cc37): owner demo, not for
  merge, conflicts with NOIZ-1 in `src/audio.rs`; removal is his call.

## Open questions

- Add CI early (NOIZ-5) so the safe-merge rule regains its CI condition?
- Rain default level: loops now -30 dBFS RMS, about 9 dB quieter at
  `rain_vol` 0.35; per-sample trims noted on NOIZ-8 and NOIZ-13.
- AirPods noise control: `pods-control` route, see its ticket.
- A proper spectroscope ticket after NOIZ-2 and NOIZ-3: ask before creating.

## Next step

Dispatch NOIZ-4 on a worker; when it lands, make clippy gating in
`docs/agent-workflow.md` and `CLAUDE.md`. Checks: `cargo build`,
`cargo clippy --all-targets`, `cargo test`; fmt is not a gate.

## Start the lead

From the repo root: `claude --agent noiz-coordinator`. A Codex lead
starts from `AGENTS.md`.
