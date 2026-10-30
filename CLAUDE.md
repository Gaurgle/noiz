# noiz

Terminal noise generator in Rust: real-time white, pink and brown noise,
binaural tones and looped rain samples through `cpal`, with a `ratatui` TUI.
Single crate, no workspace: `src/audio.rs` (engine and cpal callback),
`src/noise.rs` (generators), `src/samples.rs` (wav loading and loop player),
`src/tui.rs` (UI and keys), `src/main.rs` (clap entry). Samples live in
`samples-rain/`, resolved next to the executable or the cwd.

Agent work runs on Relay: `docs/agent-workflow.md` (roles, checks, safe-merge),
`docs/agent-handoff.md` (the baton), `docs/agent-log.md` (finished tickets),
`.claude/agents/noiz-*.md`, and `scripts/relay-check` / `scripts/relay-sweep` /
`scripts/relay-status` / `scripts/relay-board` / `scripts/relay-scope`.
Board: https://github.com/users/Gaurgle/projects/6, tickets `NOIZ-<n>`.
Analysis and upgrade candidates: `docs/analysis-2026-10-30.md`.

<!-- house-rules:start v1 -->
## House rules

These mirror the global config at `~/claude-config`, which is machine-local and
therefore invisible to cloud and mobile sessions. They apply here regardless of
where the session runs.

- Avoid em dashes (U+2014) and en dashes (U+2013) in new prose. Preserve exact
  quotations, literal data, code, and unrelated existing text.
- **Conventional Commits** (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`,
  `cleanup:`), first line under 72 characters. Body is for the why, not the what.
- **No `Co-Authored-By` lines.** Ever.
- By default, present the full `git commit -m "..."` command for the human to
  run. An explicit commit/push request authorizes that action. **"Ship it"**
  authorizes committing and following this repo's branch/PR and **Ship policy**
  instructions. If that flow is missing or unclear, ask before committing or
  pushing. Never include unrelated changes or secrets, or ship failing checks.
- Merging, tagging, releasing, deploying, force-pushing, rewriting published
  history, deleting remote refs, and infrastructure changes need a separate
  explicit request. Check for deployment side effects before pushing.
- Never weaken tests just to hide a failure. Update tests for authorized
  behavior changes while preserving meaningful coverage. In spec-driven/ATDD
  mode, acceptance tests stay frozen; raise a suspected test defect first.
- Ask before dependency, schema/API, CI, or file-deletion changes unless
  already explicitly authorized. Do not request the same permission twice.
<!-- house-rules:end -->

## Ship policy

Direct commits to `main`, no PR step, no CI yet. A push to `main` deploys
nothing. Relay leads integrate under the safe-merge rule in
`docs/agent-workflow.md` (owner delegation recorded there and in
`docs/agent-handoff.md`); ordinary sessions present commit commands.
Only Andreas runs `cargo install`, tags or releases.

## Checks

```bash
cargo build && cargo clippy --all-targets && cargo test
```

Clippy warnings are reported, not gating, until the cleanup ticket lands; do
not add new ones. `cargo fmt --check` fails on pre-existing drift, so do not
reformat untouched files in an unrelated change.

## Gotchas

- The cpal callback is real-time: no allocation, locking or blocking inside
  it. Share state through atomics or lock-free structures.
- Rain samples are 22 s stereo 44.1 kHz PCM16 wav. The device may run at
  48 kHz; the loop player must not assume the file rate.
- `samples_dir()` looks next to the binary first, then `./samples-rain`, so
  `cargo install` without the samples silently disables rain.
- Timer and pause state once desynced and froze the timer (commit ff7b34d);
  keep timer logic in one place.
