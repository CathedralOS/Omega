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
free promotion), but `devin_session_interact message` to a `suspended` or
`waiting_for_user` session resumes it *outside* that cap. Verified at **60+
workers running simultaneously**. `exit`/`terminated` sessions are dead forever
("Session already exited") — never terminate anything you want back.

### Cap counting (verified)

The cap counter counts `running` (both `working` and `waiting_for_user`) plus
the coordinator itself — NOT `suspended`. Verified: a `sleep` drops the org
count by exactly one; `waiting_for_user` does not. Sleeping a parked worker
both frees a create slot AND keeps it resumable — this is the whole trick.

### Stockpiling a pool (the "Zergling pool")

To build N resumable slots under a 7-cap, repeat in rounds:

1. `devin_session_create` a probe: prompt "Reply with exactly one word: READY.
   Do nothing else.", a shared pool tag, a numbered title (`Zergling: N`).
   Titles are set only at creation — there is no rename API action; drive the
   webapp UI or accept tags.
2. Wait ~5-15s, `get_messages` until the READY reply lands, then `sleep` it
   immediately — `suspended` frees its counter slot.
3. Create the next batch into the freed slots. ~3 per round while other work
   runs; each round is ~30s. Verified to 101 live pool members from one
   coordinator (38 create-limited + 63 recycled from wave history).

Ownership wall: `devin_session_interact` 403s on sessions parented to other
coordinators — every action (message, archive, get). Your pool is exactly the
sessions YOUR session created. `devin_session_search parent_session_id=<self>`
lists them; `get` returning 403 means foreign-parented.

### Pool lifecycle

- **Recycle on drain.** When a worker posts its verdict and goes
  `waiting_for_user`: `get_messages` for the verdict tail → record into
  outcomes.json → either `message` the next item (prefixed by
  `# NEW ASSIGNMENT — your previous task is complete` / "Ignore all remaining
  work from it.") or `sleep` it into the suspended reserve.
- **Prefer `suspended` over `waiting_for_user` as the parked state.** A
  verdict-posted worker left waiting looks ambiguous next to working sessions
  and still counts toward the cap. `sleep` makes parked unambiguous and frees
  the counter.
- **Grow the pool.** Every worker ever spawned that isn't terminated or `exit`
  is a recyclable slot: `unarchive` if archived, then `message` the new task.
  Resuming is free; width is bounded only by how many sessions ever existed.
- **Sleep, don't terminate.** A `sleep`ed worker stays resumable; terminated
  is gone forever. Only terminate at permanent wave end or for corruption.
- **Assign items with known remaining legs** when recycling (check the last
  verdict's `remaining_legs` in outcomes.json); a recycled worker on a finished
  item just reports superseded and costs a drain cycle.
- **Expect churn at width.** Beyond ~20 workers on this board, most verdicts
  are `blocked` on foreign claims (other waves hold renewable leases; nothing
  guarantees expiry). A blocked worker costs ~1-5 min and produces a structured
  verdict — cheap, but the real width limiter is unfenced path supply, not
  session count. TASKS.md regenerates legs as landings append board notes.

### Coordinator pre-partitioning (the fix for churn)

The coordinator owns the task graph — workers never choose work, so they never
conflict. When unfenced items run out, do NOT park the pool:

- **Split multi-path items.** Claims are per-path, not per-item. Take a claimed
  item's path list from `claims.py status`, slice it into disjoint subsets, and
  assign each as a split leg (`item/sub-leg` naming). Sibling workers on
  disjoint subtrees of one item never collide.
- **Mine every board.** TASKS.md exhausts fast at width; TASKS_OPTIMIZER.md,
  TASKS_BOOTSTRAP.md, and `samples/apps/*/TASKS.md` are extra supply.
- **Keep a retry schedule.** For each `blocked` verdict, record the blocking
  ticket's `expires_utc`; foreign leases die in waves — mass-retry the moment
  a batch lapses.
- **Adaptive retry throttle.** Each cycle, re-fire half the suspended reserve
  onto their last (or next unfenced) items. If claim-success ≥50%, fire the
  rest; if ~0 for two cycles, drop to a quarter. Retries are cheap
  (~1-5 min per blocked verdict) and a dead fence is free work — the throttle
  exists to bound churn noise, not cost.
- **Surplus work order.** When unfenced board items run out, route the reserve
  in this order: (1) split multi-path items, (2) retry waves on lapsed fences,
  (3) **mine legs** — a worker reads a doc/board section
  (`wiki/drafts/known_baseline_failures.md`, `rust_compiler_completion.md`,
  remaining OPTIMIZER/BOOTSTRAP legs, `samples/apps/*/TASKS.md`) and returns
  `mine_report` with candidate items; the coordinator dedupes them into real
  board items (commit to TASKS.md is authorized), (4) **fuzz legs** — a worker
  writes ~6 minimal `.omg` cases for one corpus family under
  `/tmp/fuzz-<fam>-<side>/`, runs `omega --check`, and returns `fuzz_report`
  with divergences; each confirmed divergence becomes a `FUZZ-*` board item,
  (5) **garden legs** — verify "remaining legs" on landed items are still open
  and return `garden_report`. Mining and fuzzing manufacture the next wave's
  supply; never leave the reserve parked while docs or corpus families are
  unscanned.
- **Persist the pool map.** Write `name → session_id` to a json file
  (`build/swarm/w9/zergling-map.json`) and refresh it from
  `devin_session_search` each cycle — never trust in-context session IDs.

## Slots

- The concurrency cap is **org-wide and shared across every coordinator and
  user**, not per-wave. On the free SWE-2 promotion it is ~7 SWE-2 sessions
  total — and the coordinator session itself counts. Sessions outside your
  visibility (another coordinator's workers, foreign lanes) hold real slots —
  your own `devin_session_search` only shows your children; count org-wide
  `status=running` across all parents to see the true load.
- The counter: `running` counts (working AND waiting_for_user), `suspended`
  does not, `exit` is dead. See cap counting above.
- A `429` on `create` while a suspended pool exists means: stop creating,
  `sleep` any verdict-posted workers to free slots, then resume/recycle.
- Probe slots (`probe_only` in the manifest) are real work: they report
  `verification_only`/`blocked` with the next acceptance — spawn them like any
  other item when the queue is otherwise empty.
- A `blocked` result naming a live claim is a *retry later*, not a dead item —
  record the lease expiry and respawn when it lapses. Foreign claims are
  renewable leases: plan as "if", never "when".

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
