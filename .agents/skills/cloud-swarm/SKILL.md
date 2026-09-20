---
name: cloud-swarm
description: Coordinate a cloud swarm wave — spawn sibling Devin sessions on board items, work your own item as the last slot, drain settled sessions into outcomes.json, backfill by recycling suspended sessions (resume bypasses the create cap), and keep a resumable pool. Use when the user asks for a cloud wave, a Devin session swarm, or to keep N cloud sessions working. Not for local in-session subagents (see local-swarm) or a single advance.
---

# Cloud swarm coordination

One session coordinates; sibling cloud sessions each run `advance` on one board
item; the coordinator also works items serially as the last slot.

## The loop

Never sit still. The wave fails when the coordinator waits on one thing:

- **Reconcile slots every turn.** At every turn boundary — after any tool batch,
  after any user message, and before returning to your own item — run
  `devin_session_search parent_session_id=<self>` and count children that are
  `running` or `new`. Settle notifications are unreliable: sessions die
  silently as `suspended (user_request)`, `suspended (inactivity)`, or vanish
  to `403` — only the search count tells the truth. Notifications are a
  bonus, never the trigger.
- **Notifications drive drains.** When a settle notification arrives, handle it
  immediately: `devin_session_interact get` → verify commits are ancestors of
  `origin/main` → append a record to the wave outcomes file → **recycle or
  pool** the session (see below) → give it the next unconflicted queue item.
  One settle = one drain+recycle, in a single pass.
- **Never block.** Do not `get_output` with a long timeout or `wait` while a
  local check runs. Launch it in a background shell, return to coordination
  work, and poll it for ~seconds between other steps. A blocked coordinator
  silently leaves settled sessions occupying cap slots.
- **Your item fills spare time.** Between drains, work the coordinator item:
  read code, apply edits, launch checks. Always have the next micro-step
  identified so no turn ends in "waiting".

## The resume loophole (scaling past the cap)

`devin_session_create` is capped (~7 concurrent SWE-2 sessions org-wide on the
free promotion), but `devin_session_interact message` to a **suspended**
session resumes it *outside* that cap. Verified: 18 workers + coordinator
running simultaneously. `exit` sessions are dead forever
("Session already exited"); only `suspended` resumes.

- **Recycle on drain.** When a worker posts its verdict and goes
  `waiting_for_user`, send `message` with the next item's prompt prefixed by:
  `# NEW ASSIGNMENT — your previous task is complete` / "Ignore all remaining
  work from it. Below is your new board item." The session keeps its VM,
  worktree, and context — the prompt must claim paths for the NEW item.
- **Grow the pool.** Every worker ever spawned that isn't terminated or `exit`
  is a recyclable slot. `devin_session_search` for the wave tag, filter
  `status=suspended`: `unarchive` if archived, then `message` the new task.
  Resuming is free; width is bounded only by how many sessions exist.
- **Sleep, don't terminate, during an active wave.** A `sleep`ed worker stays
  resumable; a terminated or exited one is gone. Only `archive`/`terminate`
  when the wave is permanently ending or the session is corrupt.
- **Assign items with known remaining legs** when recycling (check the last
  verdict's `remaining_legs` in outcomes.json); a recycled worker on a finished
  item just reports blocked/blocked-free and costs a drain cycle.

## Slots

- The concurrency cap is **org-wide and shared across every coordinator and
  user**, not per-wave. On the free SWE-2 promotion it is ~7 SWE-2 sessions
  total. Sessions outside your visibility (another coordinator's workers,
  zombie sessions that answer `403`) hold real slots — your own search will
  never show them.
- Suspended and archived sessions count toward the **create** cap only; resume
  via `message` ignores it. Once the recycle pool exists, the cap is a
  bootstrap constraint, not a width constraint.
- A `429` on `create` while a suspended pool exists means: stop creating,
  start recycling. Recycled resumes have no known cap — grow gradually and
  verify (`get` each) rather than blasting the whole pool at once.
- `terminate` is asynchronous and permanent — during a live wave prefer
  `sleep`, which keeps the session resumable.
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

## Housekeeping

Sessions never auto-archive — a finished session sits as `exit`/`suspended`
until someone archives it. During an active wave, drained workers go to the
suspended pool (they are recyclable slots — see the resume loophole), so sweep
only sessions you will NOT reuse: `devin_session_search` with the wave tag,
then for each `devin_session_events list` (last outgoing message) to confirm a
verdict was posted — recording any uncaptured verdict into the outcomes file
first — then `archive`. Never archive a session that hasn't posted its
verdict. At wave end, archive the whole pool.
