---
name: local-swarm
description: >-
  Coordinate a local swarm wave on this machine: partition board items into a
  wave manifest, render per-agent prompts with tools/swarm/launch.py local,
  spawn one in-session subagent per worktree, keep the tank filled by
  backfilling, and recover or drain the wave cleanly. Use when the user asks to launch a local
  swarm, run N concurrent subagents on the Omega boards, fill a wave to N, or
  resume an interrupted local wave. Not for cloud waves (launch.py launch),
  a single advance, or a named bug fix.
---

# Local swarm waves

A local wave is N subagents on this machine, one per Git worktree, each running
the ordinary `advance` protocol on one board item. The shared claims registry
and `tools/landing.py` provide the fences; `tools/swarm/README.md` is the full
reference and carries the evidence behind every rule here. This skill is the
coordinator's procedure — the agents get rendered prompts, not this file.

## Partition

1. `python3 tools/claims.py status` — every live claim is off-limits.
2. Pick items from `TASKS*.md` that are unclaimed, path-disjoint from each
   other and from live claims, and suited to this host. Read
   `tools/swarm/README.md` coordinator selection rules first.
3. Write `tools/swarm/waves/<wave>.json` — copy `local-example.json`; each
   session gets `"host": "local"`, its board item, and owning paths. Keep
   `TASKS*.md` out of owning paths.
4. `python3 tools/swarm/launch.py local --manifest <file>` — it validates,
   runs host gates/route/claims checks, renders prompts to
   `build/swarm/<wave>/prompts/<name>.md`, and prints the launch table.
   Fix what it rejects; do not `--skip-*` without a reason you can state.
5. Read each row's `partition_hints` before spawning: `dependency_language`
   and `same_layer_reference` mark items that belong in separate layers;
   `uncovered_mentions` means `owning_paths` don't cover the machinery the
   board text names — fix the paths so the fence protects the real surface;
   `scale_hint` means the item is a multi-layer decomposition, not a slice.

## Launch

Run `local` once with `--create-worktrees` so every slot starts from a clean
`swarm/<wave>-<name>` branch. It fetches `origin/main` first and reports the
resolved base (or the fetch failure) as `base` in the launch table — every
spawned slot branches from current main, and each backfill relaunch re-fetches,
so a long wave does not drift onto a stale base. If `base` reports a fetch
failure, say so in the wave report; the slots are on the cached ref.
`launch.py local` is only the validation and
prompt renderer — it must never be the spawn mechanism.

Spawn each row as a subagent **inside the coordinator session**, using the
session's own subagent tool (Devin: `run_subagent` with `subagent_general`,
`is_background=true`; Claude Code / Codex / Pi: their equivalent), with that
row's prompt file contents as the task. One in-session background subagent per
row — never a detached shell, `nohup`, `tmux`, SSH, spawned terminal, or
`launch.py launch` cloud session the coordinator cannot see. In-session
subagents keep the wave visible, let the coordinator read reports and
notification fences directly, and make death detection reliable instead of
guessing at orphaned shells. Only when the coordinator genuinely lacks a
subagent tool is a detached local session an acceptable fallback — state that
choice explicitly in the wave report, because it loses in-session visibility
and clean death detection.

Verify each spawn before counting it: a slot is not "running" until its
subagent handle confirms live (read/status on the handle, or first observable
activity in the worktree). Announce the real running count, not the spawn
count.

Concurrency is bounded by the org-wide message budget shared with cloud waves
and other machines, not by this host. A burst of ~20 died in minutes; 8 held
while the budget was quiet. When the user gives no count, run 6 subagents in
parallel constantly: backfill a freed slot immediately on landing, report, or
confirmed death, and treat a slot showing zero filesystem output for ~30
minutes (clean worktree, no commits, no new files) as dead — relaunch it in a
fresh worktree rather than waiting. Keep the tank at 6 only while the user
asked the wave to keep running.

## Monitor

`python3 tools/swarm/worktree_status.py` is the local `status`/`report`:
worktree dirt, ahead/behind, landed state, claim↔owner matching, queue state.
When a slot lands, report the commit and item; when it reports `superseded` or
`blocked`, release its claim ticket before backfilling.

Silent deaths produce no completion notification. At every checkpoint —
completion, backfill, user ping — verify the liveness of EVERY running slot
through its subagent handle, not just the one that reported. A slot whose
handle is gone, whose claim is absent or expired, and whose worktree has no
new commits, no dirty files, and no recent file mtimes is dead: run the
recovery procedure on it. Do not count dead slots as running and do not
report a tank level you have not just verified.

On repeated "Canceled by user" exits or a mass die-off, STOP backfilling:
preserve dirty work (WIP commits), release orphaned live-leased tickets, tell
the user the wave is being canceled externally, and wait — respawning into a
canceller burns the shared budget and loses nothing by waiting.

A shared external blocker that prevents refilling — a repo-wide claim
(`omega-rust`, `source`, `tests` held wholesale), a frozen landing queue, a
budget outage — pauses spawns but does not suspend the wave. Keep ticketed
agents working, re-check the blocker at every wake, and refill to target the
moment it clears. Never report the tank below target without naming the active
blocker; "still running" is a claim about live handles plus a reason for any
empty slot, not a count of sessions spawned earlier.

## Recover an interrupted wave

Rate-limit kills and sleeps leave orphaned claims and dirty worktrees; the
procedure is in README "Recovering an interrupted wave": status → release
live-leased orphan tickets → WIP-commit dirty worktrees → relaunch
continuations. `launch.py local` detects branches with unpublished commits
and renders a resume prompt — reuse the same manifest to continue.

## Drain

When the user says wrap up: stop backfilling, let running agents finish, then
sweep — release remaining claim tickets, WIP-commit any dirty worktree worth
keeping (never delete one with uncommitted work), record each parked WIP
branch and its covered slice in the item's board evidence when the item
stays open, remove clean worktrees and landed branches, verify `git status`
clean on the main checkout, and save the wave tally to
`tools/swarm/waves/<wave>.outcomes.json` (result, commits, `item_closed` per
slot): commits landed per slot, verified-closed items, WIP branches
retained, and friction worth feeding back to the README runbook.
