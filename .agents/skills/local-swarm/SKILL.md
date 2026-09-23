---
name: local-swarm
description: >-
  Coordinate a local swarm wave on this machine: partition board items into a
  wave manifest, render per-agent prompts with tools/swarm/launch.py local,
  spawn one in-session subagent per worktree, work your own item as the 7th
  slot, backfill automatically on every completion, and recover or drain the
  wave cleanly. Use when the user asks to launch a local
  swarm, run N concurrent subagents on the Omega boards, fill a wave to N, or
  resume an interrupted local wave. Not for cloud waves (launch.py launch),
  a single advance, or a named bug fix.
---

# Local swarm waves

A local wave is N subagents on this machine, one per Git worktree, each running
the ordinary `advance` protocol on one board item; the coordinator also works
one item itself as the last slot, matching the cloud-swarm shape — 6 subagents
plus the coordinator makes 7 workers on the machine. The shared claims registry
and `tools/landing.py` provide the fences; `tools/swarm/README.md` is the full
reference and carries the evidence behind every rule here. This skill is the
coordinator's procedure — the agents get rendered prompts, not this file.

## Partition

1. `python3 tools/claims.py status` — every live claim is off-limits.
2. Pick items from `TASKS*.md` that are unclaimed, path-disjoint from each
   other and from live claims, and suited to this host. Check each
   candidate's *validation surface* — the directories holding its focused
   tests, fixtures, and acceptance corpus — against live claims too, not
   just its owning paths: a slice whose acceptance tests sit under a
   foreign claim lands code it cannot exercise (macw7 parked an
   exact-machine leg while `tests/` was held wholesale). For e2e-visible
   work, `python3 tools/corpus_gate.py --filter <domain>` diffs the touched
   fixture domain against the recorded corpus golden in seconds-to-minutes —
   a much cheaper validation surface than chaining per-crate nextest suites.
   Same CPU discipline on gates: one touched crate's `--lib`, attribute reds
   by re-running only the failing test names on the base commit, and never a
   local `cargo build --release` for evidence — a local binary does not need
   to exist when the swarm-binaries release can be fetched.
   Prefer items whose evidence the slot can actually produce. Read
   `tools/swarm/README.md` coordinator selection rules first. Reserve one
   path-disjoint item for the coordinator's own slot — it goes in the wave
   report like every other slot and follows the same claim/land/release
   protocol in the coordinator's checkout or its own worktree.
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

## Chain legs (the small-wave shape, end to end)

At 6+1 slots the wave runs chain-ownership, not leaf handoffs: each manifest
row is a whole endgame chain (`scale_hint` items are chains, not slices),
and its subagent works the chain's leaves in order inside its own worktree.
The leaf-handoff churn machinery only pays at hundreds of workers; at this
width, amortizing orientation and build warmth across a whole endgame wins.

The operating loop mirrors the cloud small-pool mode:

- **Slice-first on fat legs.** An agent whose item turns out chunk-sized
  lands its first verifiable slice and names residuals in its report, rather
  than holding the slot for hours; the residual goes back into the manifest
  as the next assignment. Right-sized legs ran ~7-45min; multi-hour legs
  usually meant the acceptance hid a mini-endgame or a toolchain prerequisite
  that did not exist on main yet — pre-flight that before assigning.
- **Never park a slot on a `blocked` report.** The item stays claimed or is
  released for later retry; the slot immediately takes the next unblocked
  path-disjoint item. Parking the slot on a chain dep was the largest
  measured loss on the cloud side (~10 worker-hours on a 6-pool).
- **Residual treadmill.** Prefer re-dispatching a settled slot onto the
  smallest same-domain residual its own report names — reuse the same
  worktree (`launch.py local` resume prompt on the same branch keeps its
  `target/` warm); observed fastest sustained rate vs cold-domain starts.
- **Notification-driven drain.** Every completion notification is one pass:
  handle the report → release the claim ticket → `git pull` main → merge or
  land the worktree's lane → spawn the replacement into the same turn.
  A blocked coordinator silently leaves settled slots idle.
- **Merge discipline.** Local merges happen in the coordinator's checkout:
  pull fresh main first, merge the slot's branch, and resolve clerical
  conflicts (import-naming / module-move churn) by keeping BOTH intents —
  union signatures and import lists, never drop a side. Substantive
  conflicts escalate per the README conflict rules.
- **Coordinator slot.** The coordinator works its own reserved item serially
  in its checkout or a dedicated worktree — claim, edits, scoped gates,
  land through `tools/landing.py` like every other slot. Between drains,
  keep the next micro-step named so no turn ends in "waiting".

## Launch

Pull the coordinator's main checkout (`git pull origin main`) before running
`local` and before every spawn that creates a worktree — the launcher fetches
`origin/main`, but an up-to-date local checkout is also what the coordinator
reads for board text, claims scripts, and its own slot's base. Never spawn a
worktree while main is behind.

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

Verify each spawn before counting it: a spawn call returning is only a
request, not a running slot. Rate-limit kills routinely arrive ~30–120 s after
spawn as a completion notification carrying the limiter error — an agent can
be dead before you finish spawning its siblings. A spawn call that errors or
looks malformed can still have created a live handle: before reissuing, check
whether a handle was returned — a duplicated spawn runs a second agent in the
same worktree under the same owner string, which the claims registry cannot
fence (two `advancer14` instances once shared one worktree and ticket; the
work landed once, but only by luck of complementary edits). Before announcing a slot as
running, wait ~90 s and read its subagent handle non-blockingly
(Devin: `read_subagent` with `block=false`; Claude Code / Codex / Pi: the
equivalent status/read on the task handle). A returned result or a completion
notification inside that window is a spawn-death, not a completed task —
retry it after a delay, and if several slots die in a burst spawn ONE probe
slot first and backfill the rest only after it survives the window. Announce
the real running count, not the spawn count.

Concurrency is bounded by the org-wide message budget shared with cloud waves
and other machines, not by this host. A burst of ~20 died in minutes; 8 held
while the budget was quiet. When the user gives no count, run 6 subagents in
parallel plus the coordinator's own item — 7 workers total on the machine —
and keep the tank there constantly: every completion notification (landed,
blocked, superseded, or death) immediately frees a slot, and the coordinator
spawns the next unclaimed path-disjoint item into it on the same turn, without
waiting for the user. Treat a slot showing zero filesystem output for ~30
minutes (clean worktree, no commits, no new files) as dead — relaunch it in a
fresh worktree rather than waiting. Keep the tank full only while the user
asked the wave to keep running.

Host capacity bounds the wave separately from the message budget. On an
interactive desktop — this Windows machine is one — run at most 3-4 slots and
cap each agent's build parallelism in the rendered prompt (`CARGO_BUILD_JOBS=8`;
nextest `-j 4..8` per the global rules' 28-thread test cap, which documents
desktop freeze under full oversubscription). Uncapped, N slots × 32-core
builds plus multi-GB links starve dwm/explorer; explorer.exe AppHang clusters
were witnessed during an 8-slot wave window. Create worktrees from the
top-level checkout only — never inside another worktree (a nested `jev-live`
wave left six sub-worktrees carrying their own `target/` dirs). On Windows,
keep `.codex/worktrees` and `target/` out of Defender real-time scanning and
file indexers.

## Monitor

`python3 tools/swarm/worktree_status.py` is the local `status`/`report`:
worktree dirt, ahead/behind, landed state, claim↔owner matching, queue state.
When a slot lands, report the commit and item; when it reports `superseded` or
`blocked`, release its claim ticket before backfilling. Every completion
notification is a refill trigger: handle the report, release the ticket, pull
main, and spawn the replacement subagent in that same turn — the wave does not
wait for a user message to backfill.

**Keep a handle ledger; let `fill.py` turn notifications into a plan.** Handle
IDs die with the context that saw them, so `tools/swarm/fill.py` keeps a
durable `build/swarm/<wave>/slots.json` mapping slot → `{agent_id, state,
note}` (`running`/`dead`/`done`/`parked`/`recovered`). Update it on every
lifecycle event — spawn, completion, death, recovery:

```bash
python3 tools/swarm/fill.py --manifest tools/swarm/waves/<wave>.json \
    note --slot <name> --agent-id <id> --state running
python3 tools/swarm/fill.py --manifest tools/swarm/waves/<wave>.json \
    note --slot <name> --state dead --death "<why>"
python3 tools/swarm/fill.py --manifest tools/swarm/waves/<wave>.json \
    note --slot <name> --state done
```

At every wake run `fill.py --manifest <file>` (`plan`): it joins the manifest,
worktree state, live claims, and the ledger into a per-slot state and an
ordered action list (`recover` / `land` / `resume <id>` / `spawn <prompt>` /
`pick-item` / `check-handle` / `wait`). Act on the plan's actions in order —
do not re-derive slot state by hand. `fill.py recover --execute` performs the
WIP-commit + orphan-ticket-release half of recovery for every ledger-dead
slot in one step; `fill.py sweep --execute` removes clean landed worktrees
before `.codex/worktrees` grows unboundedly again.

**Pace the refill; do not burst it.** A resume replays the agent's whole
transcript and a fresh spawn builds a prompt — five or six at once re-trips
the shared model rate limiter and kills the refill wave itself. Execute the
plan's actions one per block, ~30-60 s apart while the limiter is hot, and
resume existing handles before spawning replacements: a resume keeps the
agent's context and its claim, while a spawn burns both setup cost and a
fresh slot in the manifest. Fresh items are for genuinely gone handles and
newly added slots. Manifest rows are append-only: a replacement item goes
in as a NEW session name, never by renaming or reusing a dead slot's row —
`slots.json` keys slot → handle, so a reused name aliases the dead
handle's record with the new agent's.

Silent deaths produce no completion notification. At every checkpoint —
completion, backfill, user ping — verify the liveness of EVERY running slot
through its subagent handle, not just the one that reported. Liveness is
checked in this order:

1. **Handle read** (authoritative): a non-blocking status/read on the
   subagent handle (Devin: `read_subagent` `block=false`). A returned result
   means the agent finished or died — read the output to tell which. Still
   pending means alive.
2. **Claim renewal**: a live agent renews its board claim before expiry.
   `claims.py status` showing the slot's claim with a lease extending into
   the future is corroborating evidence; an expired or absent claim is not.
3. **Live processes**: `ps` for processes working inside the slot's worktree
   (mbx/cargo/nextest/run.sh children with the worktree path) is stronger
   than mtimes — orphaned gate scripts and detached builds keep writing
   files long after their parent agent is dead, and shared build caches give
   every worktree fresh mtimes it did not earn.

Worktree mtimes alone are NOT evidence either way: dead agents leave orphans
that write for hours (false positive), and a thinking agent can read for
minutes without touching a file (false negative). A slot whose handle is
gone, whose claim is absent or expired, and whose worktree has no new
commits, no dirty files, and no live child processes is dead: run the
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
continuations. With the ledger kept current, `fill.py recover --execute`
performs the first three steps mechanically; `launch.py local` detects
branches with unpublished commits and renders a resume prompt — reuse the
same manifest to continue.

## Drain

Local subagents are in-session agents, not Devin sessions — there is nothing to
archive; their state is the worktree, branch, and claims ticket, which this
drain handles. Cloud-sibling sessions are different: archive finished ones via
`devin_session_interact archive` (see the cloud-swarm skill).

When the user says wrap up: stop backfilling, let running agents finish, then
sweep — collect pending worker evidence first (`python3 tools/claims.py
notes`; workers attach findings to their claim tickets instead of committing
board files, and `landing.py` refuses their board-only or empty publishes),
fold what the notes justify into one board sweep commit landed with
`--board-update`, then `python3 tools/claims.py sweep --owner <wave>` marks
this wave's notes consumed without eating sibling waves' pending evidence —
release remaining claim tickets, WIP-commit any dirty worktree worth
keeping (never delete one with uncommitted work), record each parked WIP
branch and its covered slice as a compact resume line in the item's board
evidence when the item stays open (replacing the frontier it supersedes, not
appended to it), collapse any accreted landing ledger in touched items back
to its current frontier, remove clean worktrees and landed branches, verify `git status`
clean on the main checkout — including untracked files under agent-owned
paths, since agents occasionally write probe/scratch files into the
coordinator checkout instead of their worktree — run `fill.py --manifest
<file> close` so the ledger stops proposing refills, and save the wave tally to
`tools/swarm/waves/<wave>.outcomes.json` (result, commits, `item_closed` per
slot): commits landed per slot, verified-closed items, WIP branches
retained, and friction worth feeding back to the README runbook.
