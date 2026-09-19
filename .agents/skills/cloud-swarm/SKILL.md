---
name: cloud-swarm
description: Coordinate a cloud swarm wave — spawn sibling Devin sessions on board items, work your own item as the 7th slot, drain settled sessions into outcomes.json, and backfill freed slots. Use when the user asks for a cloud wave, a Devin session swarm, or to keep N cloud sessions working. Not for local in-session subagents (see local-swarm) or a single advance.
---

# Cloud swarm coordination

One session coordinates; sibling cloud sessions each run `advance` on one board
item; the coordinator also works items serially as the last slot.

## The loop

Never sit still. The wave fails when the coordinator waits on one thing:

- **Notifications drive drains.** When a settle notification arrives, handle it
  immediately: `devin_session_interact get` → verify commits are ancestors of
  `origin/main` → append a record to the wave outcomes file → `terminate` →
  spawn the next unconflicted queue item into the freed slot. One settle =
  one drain+backfill, in a single pass.
- **Never block.** Do not `get_output` with a long timeout or `wait` while a
  local check runs. Launch it in a background shell, return to coordination
  work, and poll it for ~seconds between other steps. A blocked coordinator
  silently leaves settled sessions occupying cap slots.
- **Your item fills spare time.** Between drains, work the coordinator item:
  read code, apply edits, launch checks. Always have the next micro-step
  identified so no turn ends in "waiting".
- **Poll opportunistically.** Every ~10 minutes without a notification, batch a
  status `get` over all live sessions — sessions sometimes settle without a
  visible notification or land `waiting_for_user` with structured output set.

## Slots

- Respect the org concurrency cap (the API refuses creation past it; on the
  free SWE-2 promotion it is ~7 total including the coordinator).
- `terminate` is asynchronous — wait ~30-60s after terminating before spawning
  a replacement, or the create may 429.
- Probe slots (`probe_only` in the manifest) are real work: they report
  `verification_only`/`blocked` with the next acceptance — spawn them like any
  other item when the queue is otherwise empty.
- A `blocked` result naming a live claim is a *retry later*, not a dead item —
  note the expiry and respawn when it lapses.

## Bookkeeping

- Keep `tools/swarm/waves/wave-N.outcomes.json` append-only: each settle adds
  {name, item, board, session_id, devin_mode, result, item_closed, commits,
  notes, recorded_utc}. Verify `git merge-base --is-ancestor <sha> origin/main`
  for every reported commit before recording `landed`.
- A child that reports a main-break it caused or witnessed (build failure on
  `origin/main`) is a coordinator priority item: reproduce, claim, fix, land
  via `tools/landing.py` — do not wait for the offender to return.
- `landing.py` needs a clean checkout; coordinator scratch files (manifests,
  outcomes, queue) must live under a gitignored dir such as `build/swarm/w9/`
  or be stashed during enqueue/publish.
- Claims: `python3 tools/claims.py claim --board B --item I --owner O --path P
  [--path P2 ...] --lease-minutes N --note "..."`. Exit 2 = live conflict,
  never retry with `--allow-overlap`. Re-claiming your own item with more paths
  requires `release --ticket` first (same-item conflict).

## Ending

The wave drains when the queue holds only: running items, items blocked behind
named live claims (record expiry times), and probe slots already spawned. Say
so in outcomes; keep polling until the last running session settles, then
report the wave summary to the user.
