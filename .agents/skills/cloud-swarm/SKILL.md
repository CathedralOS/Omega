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
  `running` or `new`. Notifications can still be lost to `403` or prompt
  drops — the search count stays the backstop — but every dispatch carries
  its own alarm, so a settle you caused always reaches you.
- **Arm `notify_on_response` on every dispatch.** `devin_session_interact
  message` to a worker MUST pass `notify_on_response=true`. It is one-shot:
  consumed when that session next settles (finishes, blocks, or exits), so
  re-arm it on every subsequent message too. Without it the coordinator
  never learns the worker finished and the slot idles invisibly.
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

## Small-pool chain mode (the ≤ ~8-slot loop)

When the org cap leaves only a handful of slots (~7 total, coordinator
included), drop the mass-wave machinery below: no mine/fuzz/garden churn, no
Overlord tier, no batch merges. Each worker owns ONE endgame chain end-to-end
on a persistent lane; the coordinator works the last slot as a worker itself.
Chain-ownership amortizes clone+orientation+build warmth across the whole
endgame — the leaf-handoff churn machinery only pays at hundreds of workers.

**Dispatch contract.** Every worker message carries, in order:

1. `# NEW ASSIGNMENT — your previous task is complete` when recycling a
   settled worker (or the `You are zergling z<N> of the <wave> cloud swarm`
   role line on first dispatch).
2. The named leaf: title, the board's acceptance text verbatim, the
   witness/repro lane or file, and the gates
   (`cargo check -p <touched-crates> --all-targets` +
   `cargo nextest run -p <touched-crates>`; `mbx` if present).
3. The lane: `git push -f origin HEAD:leaf/<kebab-item>` — one lane per chain
   link. Workers NEVER merge, NEVER touch board files, NEVER run landing.py.
4. "Reuse your existing clone/worktree when present" — suspended workers
   resume on the same VM; a re-clone plus cold build is measured leg-time tax.
5. Final message = bare JSON verdict, nothing fenced:
   `{"verdict":"lane_pushed|blocked|landed","lane":"leaf/<item>",
   "commits":[<sha>...],"gate":"<commands + counts>","witness":"<evidence>",
   "notes":"<smallest next same-domain residual>","remaining_legs":[...]}`.

And ALWAYS pass `notify_on_response=true` on every
`devin_session_interact message` dispatch — it is one-shot (consumed on that
settle), so re-arm it on every subsequent leg. Session auto-sleep ~30min is
what silently starves the pool when dispatches lack the alarm.

**Drain = verdict + merge + redispatch, one pass.** On every settle:
`get_messages` tail → record the outcome → if `lane_pushed`, run the merge
cycle below immediately → message the next leaf with the notify re-armed.
One settle = one full drain; never batch deferred merges.

**Coordinator merge cycle (per landed lane).**

1. `git fetch origin`; `git checkout -B merge-check origin/main` in the
   dedicated merge worktree (never the coordinator's leaf worktree).
2. `git merge --no-ff <lane-tip>`. Clerical conflicts — the import-naming /
   module-move churn (globbing→named imports, `X` → `types::X`,
   `terminal_unit::Y` → `calls::Y`) — resolve by keeping BOTH intents:
   union the signatures and import lists, never drop a side. Substantive
   conflicts follow the conflict rules (escalate ambiguous intent).
3. Scoped gate: `cargo check -p <touched crates>` +
   `cargo nextest run -p <touched> --lib`, plus `python tools/fmt.py`.
4. `git push origin HEAD:main`. A rejection means main moved under you —
   re-fetch, re-checkout the fresh tip, re-merge; never rebase the merge
   checkout and never carry a wedged tree forward.
5. Append the ledger row (ISO-UTC, lane, sha, verdict summary, gate counts)
   to `.swarm/merge-ledger.tsv` or the wave outcomes file.
6. Prune the remote ref only after the push lands.

**Never park on a `blocked` verdict.** The LANE stays pushed for later
resume; the WORKER immediately takes the next unblocked leaf. Parking the
worker on a chain dep was the largest measured loss (~10 worker-hours on a
6-pool — a `blocked` verdict idled a slot ~6h).

**Slice-first on fat legs.** A leg that turns out chunk-sized lands its
first verifiable slice and names residuals in the verdict, rather than
holding the lane for hours. Right-sized legs ran ~7-45min; the 2-5h+ fat
tails clustered where "leaf" acceptance hid a mini-endgame or a toolchain
prerequisite that did not exist on main yet — pre-flight every dispatch:
verify the acceptance's dependencies are already merged before spending a
leg that can only end `blocked`.

**Coordinator slot.** The coordinator claims and works a leaf like any
worker (`claims.py claim` → edits → scoped gates → own `leaf/<item>` lane →
merge through the same cycle). Between drains every turn has a named
micro-step; long gates run in background shells polled between cycles —
a blocked coordinator silently leaves settled workers occupying cap slots.

**Toolchain refresh is coordinator-owned.** When workers verify against a
pinned packaged binary (e.g. Squalr's `swarm-binaries` omega build), the
coordinator rebuilds and re-pins it in a dedicated worktree after merges
that touch the toolchain path — never per-leg "build your own binary"
instructions, which burn leg time and split evidence across mismatched
binaries.

**Residual treadmill.** Prefer re-dispatching the settled worker onto the
smallest same-domain residual its own verdict names; warm clone + warm
build + warm context produced the fastest sustained rate observed (10+
lands/session vs cold-domain starts).

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

To build N resumable slots under the cap, repeat strict rounds of 6:

1. Track `next_zergling_number` in the pool map json — the real max title
   number seen in `devin_session_search` (titles collide if you restart
   numbering; there is no rename API).
2. Create probes **sequentially, one at a time** (not parallel — parallel
   batches race the org counter and half the batch 429s). Prompt: "Do not run
   anything. Reply with exactly one word (READY) and end your turn
   immediately." — a no-op prompt minimizes the session's running window.
3. Issue `sleep` **immediately after each create returns** — do NOT wait for
   the READY reply first. The point is to shrink the window the session
   counts against the cap.
4. After the batch, confirm each session actually reached `suspended` via
   `interact get` (re-issue `sleep` while it still shows running/working/
   waiting). `sleep` is asynchronous — spawning the next round before the
   suspend lands just 429s the next round.
5. Only then create the next round of 6 into the freed slots. Sleep ALL
   leftover `running` children before every stockpile session — a straggler
   `waiting_for_user` probe still eats a slot. Verified to 140+ pool
   members; rejected rounds retry on a 30s timer.

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
- **Never park a worker on a `blocked` verdict.** A blocked lane stays pushed
  for later resume; the worker itself immediately gets the next *unblocked*
  leaf — chain-dep parking wastes slot-hours (measured: ~6h idle on a small
  pool) and a different unblocked leaf always exists or mining does.
- **Slice-first landing rule.** Dispatches that turn out chunk-sized (a leg
  spanning a chain of sub-discoveries) should land the first verifiable slice
  and report the rest as named residuals rather than holding the lane for
  hours. Right-sized leaves ran ~7-45min; the fat tails (2-5h+) clustered on
  legs whose leaves were really mini-endgames. Pre-flight every dispatch:
  check the acceptance's compiler/toolchain prerequisites actually exist on
  main before spending a leg that can only end `blocked`.
- **Reuse the worker's checkout.** Suspended workers resume on the same VM —
  dispatch templates say "reuse your existing clone/worktree when present"
  rather than re-cloning every leg (cold clone + cold build is leg-time tax).
- **Coordinator-owned toolchain refresh.** When workers verify against a
  pinned packaged binary (Squalr's `swarm-binaries` omega), the coordinator —
  not each leg — rebuilds and re-pins it after every merge that touches the
  toolchain path; per-leg "build your own binary" instructions burn leg-time
  and split evidence across mismatched binaries.
- **Residual treadmill > fresh domains.** Verdicts should name "the smallest
  next residual in this domain"; re-dispatching the same worker same-domain is
  the fastest sustained rate observed (warm clone, warm build, warm context).
- **Verify main is green before reverting a lane.** A merge-failure diagnosis
  that skips the main-red check can cost a revert + repair + re-land cycle on
  an innocent lane (observed: the "clobbered registrations" revert; the real
  failure was preexisting main red).
- **Expect churn at width.** Beyond ~20 workers on this board, most verdicts
  are `blocked` on foreign claims (other waves hold renewable leases; nothing
  guarantees expiry). A blocked worker costs ~1-5 min and produces a structured
  verdict — cheap, but the real width limiter is unfenced path supply, not
  session count. TASKS.md regenerates legs as landings append board notes.
- **Mass-fires settle in waves.** After firing ~200 suspended workers at once,
  ~half return verdicts within ~10-15 min (blocked churn + fast mine/fuzz
  legs). The coordinator must run drain→record→refire cycles continuously —
  never wait for notifications. Refire `waiting_for_user` AND `suspended`
  every cycle, indiscriminately; alternate retry prompts with mine legs so
  blocked churn still manufactures board supply.
- **High `blocked` rate = supply shortage, not worker shortage.** When most
  settles come back `blocked`, stop blind-retrying and route settled workers
  to mine legs — each `mine_report` carries candidate item names the
  coordinator dedupes and commits to TASKS.md (authorized), converting churn
  into new unfenced supply. When retrying anyway, shard big items into
  per-file legs so claims hit narrower (freer) fences.
- **Batch-merge beats the landing queue at width.** The serialized
  landing.py queue saturates around ~30 deep with 100+ workers (each enqueue
  re-runs validation on a serial lane). Faster path: workers commit on their
  branch and push, then report `branch_ready` with shas; the coordinator
  fetches `refs/heads/zergling/*` and merges batches onto main each cycle.
  Claims-disjoint pathsets mean merges apply clean (~0 conflicts observed;
  ~17 branches/cycle vs ~9 queue-landed/cycle). Caveat: merges skip
  landing.py validation — watch for main breakage, and treat a batch-merge
  that breaks main as a coordinator priority fix.
- **Bound branch refs — one lane per zergling.** Per-item branches
  (`zergling/z<N>-<item>`) explode to 400+ refs at width. Have each worker
  force-push to a single persistent lane: `git push -f origin
  HEAD:zergling/z<N>`, verdict `{"result":"branch_ready","branch":
  "zergling/z<N>"}`. The coordinator merges each lane then deletes the ref —
  remote ref count stays bounded by in-flight work, never grows per task.
  Workers on separate VMs cannot land any other way (their commits aren't
  reachable until pushed); direct `HEAD:main` pushes race non-FF at width.
- **Prune stale lanes by ancestry, not text diff.** A lane is stale iff its
  tip is already an ancestor of main: `git merge-base --is-ancestor <tip>
  origin/main`. A two-dot `git diff main ref` ALWAYS differs on old-base
  lanes (main moved under them) even when their commits were merged — the
  text-diff check prunes nothing and conflicted duplicates accumulate.
  Three-dot (`main...ref`) is closer but ancestry is exact.
- **Never `pull --rebase` the merge checkout — merge instead, and detect the
  wedge.** A rebased batch replays every lane-merge as a pick (100+ stale
  picks on conflict) and a non-checked returncode leaves the coordinator
  merging on a half-rebased tree forever (observed: 228 merged commits
  stranded local-only while "the loop ran fine"). Per cycle, pre-flight:
  `rebase-merge/ || rebase-apply/ || MERGE_HEAD` present → `rebase --abort`
  + `merge --abort` + `reset --hard origin/main` (lane content lives on
  origin — reset loses nothing). Then `pull --no-rebase --no-edit` (one
  merge commit, trivially resolvable) — and if that fails, abort and skip
  the batch, never carry a wedged tree forward. Check push returncode:
  only delete lane refs when push succeeds.
- **`waiting_for_user` is a settled state — drain and refire it.** A worker
  that finished its turn sits `running (waiting_for_user)`, NOT `suspended`.
  Treating only `suspended` as settle-able leaves ~85% of the pool parked and
  undrained (observed: 176/203 idle while "working"). Classify
  `suspended | blocked | waiting_for_user` as settled; `running (working)`
  is the only true working state.
- **`get_messages` pages OLDEST-first — verdicts live on the LAST page.**
  `first: 80` returns the first 80 messages ever, so verdicts posted later
  are invisible. Page with `after=` until no cursor (cap ~10 pages), then
  scan the last page's devin messages newest-first. Verdicts may also carry
  `candidate`/`landing_ticket` (landing-queue style) instead of `commits` —
  extract both.
- **Dead sessions hold claims for the whole lease (8h).** `claims.py
  release --ticket <t>` every claim whose owner maps to an `exit`ed session —
  zombie fences block real assignments until expiry.
- **Never replay a long rebase chain — abort and re-merge.** If a mid-merge
  `pull --rebase` wedges on conflicts with dozens of steps left (100+ stale
  picks), `git rebase --abort`, `git reset --hard origin/main`, and re-merge
  the branches fresh — the source refs still exist on origin and re-merging
  against current main is cheaper than resolving each stale pick.
  TASKS.md-only conflicts resolve by union-merge (keep both sides' unique
  lines — board notes accumulate); a scripted 3-way marker pass handles them.
- **Union-merge board-only conflicts in the coordinator, not in a resolver.**
  When `git merge <lane>` fails and every unmerged path matches
  `TASKS*.md` / `samples/apps/*/TASKS*.md`, run `git merge-file --union -p
  :2: :1: :3:` per file, `git add`, `git commit --no-edit` and keep going.
  Observed: 26 of 40 conflicted lanes were board-only — this alone took a
  cycle from 3 merged lanes to 26. Anything else unmerged → `merge --abort`
  and queue the lane for an Overlord resolve leg (below).
- **Gate every batch with a scoped check before pushing main.** `mbx check -p
  <affected crates>` (Cargo only if `mbx` is missing); red → `reset --hard`
  to the pre-batch SHA, lanes stay on origin and retry next cycle. Main
  never goes red from a batch; absence of a failure log is NOT evidence the
  gate ran — log every gate result.

### Overlord tier (Fusion planners / reviewers / resolvers)

Cursor's split — smart planners, fast workers, neutral merge resolver, stacked
reviewers — maps onto this loop as five persistent **Overlord** sessions
(`devin_mode:"fusion"` on create; org default SWE-2 Max is the fallback if the
create 400s). Creating them needs the whole pool asleep first (see
stockpiling). Persist their ids (`overlords.json`); they are refired like
zerglings — event-driven, one leg at a time, never stacked. Zerglings stay
SWE-2 Max and never plan.

- **Plan leg** (region → leaves). Input: one TASKS.md region + `claims.py
  status`. Output: 15-40 DISJOINT leaves, each `{name (UPPER-KEBAB), paths,
  acceptance, parent}`, plus `retire: [...]` for rows that are resolved,
  duplicated, or unactionable. Session messages truncate past ~4KB, so
  planners emit `plan_chunk` messages of ≤5 leaves and a short final `plan`
  summary; the coordinator aggregates every chunk before folding. Fold leaves
  into the item pool with their path sets (`item-paths.json`) so assignment
  is path-partitioned, drop retired rows, and reject placeholder names
  (`ITEM-NAME`, `NEW-UPPER-KEBAB`, …) — a fallback regex will happily
  extract the prompt's own example. Planner leaves replace blind mine output
  as the primary supply.
- **Review leg** (green batch → field notes). After each batch pushes, hand
  one Overlord `pre..post` with the lane list. Lens: Psi/Omega firewall and
  crate placement, test evidence, board noise, duplicated work. No reverts.
  Verdict `{"result":"review","verdict":"ok|concerns","followups":[leaves],
  "notes"}`; followups enter the pool like planner leaves.
- **Field notes (stigmergy).** Agents walk the paths the codebase gives them,
  so a finding only sticks if it lives where the next agent will step: a
  terse code comment at the exact site, or a row in the nearest README /
  wiki draft — not in the verdict JSON alone. Reviewers commit `field notes:
  <range>` and push (pull `--no-rebase` and retry on reject). This is the
  same mechanism as AGENTS.md, board rows, and claim notes: durable traces in
  the environment, not chat.
- **Resolve leg** (conflicted lane → repaired lane). Queue every non-board
  conflict (`resolve-queue.json`); keep ≤2 resolve legs active. Prompt: fetch,
  `checkout -B resolve origin/<lane>`, `merge origin/main`, preserve BOTH
  intents (main wins on board/claims files), scoped `mbx check -p` on touched
  crates, `push -f origin HEAD:<lane>`; report `resolved` or `unresolvable`
  (pure duplicate of main → the coordinator prunes the ref). Resolved lanes
  merge on the next cycle instead of being lost to `merge --abort`.
- **Resolve at the source, not in the queue.** Two resolvers clear ~1 lane per
  cycle; a 200-worker pool can conflict 80 lanes in an hour, so the queue only
  grows. The worker prompt therefore requires `git fetch && git rebase
  origin/main` immediately before `push -f`, with the worker resolving its own
  conflicts (it has the intent context) and re-running its scoped check. The
  Overlord queue is the fallback for lanes that still conflict when merged.
- **Route a queued lane back to its owner first.** Lanes are one-per-zergling
  (`zergling/z<N>`), so when z<N> settles and its own lane is in the resolve
  queue, prepend to its next assignment: rebase that lane onto `origin/main`,
  resolve, scoped check, `push -f`, post `resolved`, then start the new item on
  top. Mark the lane busy under that zergling; count only Overlord-held lanes
  against the ≤2 resolver cap. Drop queue/busy entries for lanes that merge or
  prune. First run: 27 lanes routed in one cycle, 15 `resolved` over the next
  two, conflicted 75→56 — vs ~1/cycle from Overlords alone.
- **Fold board-only lanes into one squash commit per cycle.** Audit of 60
  merges on main: 32 touched only `TASKS.md`/`build/swarm/` (fence stamps,
  "alias stub — covered", re-verify notes from `blocked`/`superseded` legs).
  They are real stigmergy, not fake pushes, but one merge commit each is
  history noise. Classify each lane with `git diff --name-only
  origin/main...origin/<lane>`: all-board → `merge --squash` it after the code
  lanes (TASKS.md conflicts union-merge, anything else → resolve queue), then
  `reset --soft` to the pre-fold mark and commit once as
  `board: fold N lane notes (z…)`. Code lanes still merge individually so
  attribution and revertability survive.
- **Persist a message-page cursor per session.** `get_messages` pages
  oldest-first at ≤80/page; a zergling with 100+ legs has 1000+ messages, so a
  blind tail costs ~10 reads per session per cycle (~2,200 API reads/cycle for
  187 sessions). Store the `after` token of the last page you fetched
  (`tail-cursor.json`) and resume from it — the newest verdict is always on or
  after that page. Cycle read volume dropped 1,273 → 388 calls. Skip the
  cursor for Overlords: planner legs need the whole leg's chunk history.
- **Reviewer `duplicate_lanes` are advisory, never authoritative.** Reviewers
  get the resolve queue and may list lanes whose content "already landed via a
  sibling". Every one checked so far still differed from main in non-board
  files (e.g. 11 of 26 files), so the coordinator MUST run its own guard before
  deleting: `git diff --name-only origin/main...origin/<lane>`, drop board paths
  (`TASKS.md`, `build/swarm/`, `tools/claims`), then `git diff --quiet
  origin/main origin/<lane> -- <rest>`; prune only when that is clean or the
  lane is board-only. A guard refusal keeps the lane queued.
- **Verdict JSON must survive truncation.** Overlords often post the JSON
  unfenced after a long prose tail, so the parser must accept a fenced block OR
  a raw `{"result":"review"...}` object via `raw_decode`, with a regex fallback
  for `leaves` / `retire` / `duplicate_lanes`. Ask for <3KB JSON with
  `duplicate_lanes` first and ≤3 followups.
- **Print `merge/ready` every cycle** (`merged / branch_ready`, plus
  `dup_pruned` and queue depth). Observed 12–40%; the gap is almost entirely the
  conflict queue. It is operational telemetry, not a quality measure — a lane
  that merged green is not a lane that was reviewed.
- **Overlord legs are refire-gated like workers**: only send the next leg
  after the session produced output past the last fire timestamp. Dedupe
  verdicts by `(session, result, item/lane, commit, timestamp)` — tails
  re-read the same last page every cycle.
- **Size surplus legs big — small legs churn.** A one-doc mine leg finishes in
  ~2-5 min, so ~half the pool settles every cycle and the coordinator drowns
  in message volume. Give each miner a whole directory/tree
  (`ALL of wiki/drafts/`, `every crate under omega-rust/psi/pipeline/`) and
  require 20+ candidates per report; mix in fuzz legs (~15 cases each). Deep
  legs take 15-30+ min, which damps the settle wave AND yields ~650
  candidates per cycle vs ~165 for shallow sweeps.

### Coordinator pre-partitioning (the fix for churn)

The coordinator owns the task graph — workers never choose work, so they never
conflict. **Assignment dedup is the coordinator's job too**: claims.py only
covers items a worker has *already claimed* — there is a 60-90s window between
your assign and the worker's `claim` where the item looks free, so a bare
cursor re-hands the same item to siblings. Observed: ~2/3 of pruned lanes were
sibling-duplicated diffs before this fix. Keep a persistent **in-flight
ledger** (`item → session_id, assigned_utc`): on every assign, skip items that
are fenced AND items already in-flight; release the entry when the holder's
verdict drains or it goes `exit`. Never assign an item that's already in
flight. When unfenced items run out, do NOT park the pool:

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
  (`wiki/drafts/measurements/known_baseline_failures.md`, `rust_compiler_completion.md`,
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

## Naming

Sibling titles are org-wide visible, so they must be org-wide unique — the
session list does not disambiguate same-named children, and a restarted
per-wave counter is how four sessions once all answered to the same number.

- Title each spawn `Zergling: <n>` continuing the shared sequence: before the
  first spawn of a wave, list existing session titles (not only your own
  children — other coordinators' zerglings share the namespace), take the
  highest `Zergling: <m>` match, and start at `<m>+1`. Start at 1 only when no
  prior zergling exists at all.
- If you cannot enumerate existing sessions, do not guess a number: suffix the
  wave and manifest name instead (`Zergling: w9l-frame-layout`), which is
  unique by construction.
- Renaming a settled duplicate to fill a gap is fine; renaming a live or
  pending sibling risks the coordinator losing track of which handle is which.

## Bookkeeping

- Keep `tools/swarm/waves/wave-N.outcomes.json` append-only: each settle adds
  {name, item, board, session_id, devin_mode, result, item_closed, commits,
  notes, recorded_utc}. Verify `git merge-base --is-ancestor <sha> origin/main`
  for every reported commit before recording `landed`.
- Drain claim notes with the verdicts: `python3 tools/claims.py notes` lists
  findings workers attached to their tickets ("already resolved upstream,
  verified at <sha>"); fold what they justify into the coordinator's next
  board sweep commit (landed with `--board-update`), then `python3
  tools/claims.py sweep` marks them consumed. Workers never commit board
  files — `landing.py` refuses board-only and empty candidates, and a lane
  whose only diff is `TASKS.md` gets deleted as stale, not merged.
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
