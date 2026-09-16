---
name: local-swarm
description: >-
  Coordinate a local swarm wave on this machine: partition board items into a
  wave manifest, render per-agent prompts with tools/swarm/launch.py local,
  spawn one subagent per worktree, keep the tank filled by backfilling, and
  recover or drain the wave cleanly. Use when the user asks to launch a local
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

## Launch

Run `local` once with `--create-worktrees` so every slot starts from a clean
`swarm/<wave>-<name>` branch. Then spawn one background subagent per row with
that row's prompt file contents as its task.

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

## Recover an interrupted wave

Rate-limit kills and sleeps leave orphaned claims and dirty worktrees; the
procedure is in README "Recovering an interrupted wave": status → release
live-leased orphan tickets → WIP-commit dirty worktrees → relaunch
continuations. `launch.py local` detects branches with unpublished commits
and renders a resume prompt — reuse the same manifest to continue.

## Drain

When the user says wrap up: stop backfilling, let running agents finish, then
sweep — release remaining claim tickets, WIP-commit any dirty worktree worth
keeping (never delete one with uncommitted work), remove clean worktrees and
landed branches, verify `git status` clean on the main checkout, and report
the wave tally: commits landed per slot, verified-closed items, WIP branches
retained, and friction worth feeding back to the README runbook.
