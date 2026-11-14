# Swarm coordination

Automatic assignment starts with the common [dispatch reservation procedure](dispatch.md):
current-main gap evidence, atomic existing-claims outcome/session reservation,
one durable send attempt, completed-session reuse and honest cache continuity.
Codex, Cursor, Devin, and local agents use that reserve/arm/send/receipt procedure.
No new queue or shared acceptance system is introduced.

For dot cloud tasks with parent-owned create/read/follow-up tools, use the
[cloud coordination adapter](cloud.md). It extends the recovery journal with
durable dispatch results, dependency handoffs, final acceptance and bounded
2/5/10 admission.

Apply the [law of completion](../../AGENTS.md#law-of-completion): each
implementation session retains context through its complete assigned acceptance,
necessary cross-layer repairs, required review and authorized landing verified
on remote main. Checkpoints, bounded report turns and transferred artifacts
preserve evidence; they are not finish lines or implementation handoffs.

## Local worktree status

For restartable lane state, bounded commands, verified backups and conservative
cleanup inventory, use [the recovery procedure](../../.agents/skills/swarm/references/recovery.md)
and `python3 tools/swarm/recovery.py --help`. It consumes explicit current adapter
observations, complements these helpers, and never executes a proposed recovery
or cleanup action. Reconcile first; a stale `slots.json` flag is not live-process
evidence. The existing fill ledger now uses atomic file replacement as well.

Local waves run in plain git worktrees (for example `.agents/worktree/<wave>-<task>`)
and produce no receipts. `worktree_status.py` reports each worktree's branch,
uncommitted churn, ahead/behind against `origin/main`, whether its head already
landed, the live claims-registry assignment matched by owner name (including
`item_on_board` for board claims), and the landing queue's state — plus claims
held by owners with no local worktree (cloud sessions, other machines). It is
read-only.

```sh
python3 tools/swarm/worktree_status.py            # one JSON object
python3 tools/swarm/worktree_status.py --markdown # report table
python3 tools/swarm/worktree_status.py --offline  # local git state only
```

```powershell
python tools/swarm/worktree_status.py
```

`--base <ref>` changes the ahead/behind and landed baseline (default
`origin/main`); `--remote` and `--repository` match `claims.py`. Unreachable
registries degrade to `unavailable` in the record rather than failing the
run; `--offline` skips them deliberately.

A live claim is not a live worker. Leases outlive dead agents — a claim
stays `live` until its expiry even when the process holding it is gone, and
long leases give no renewal heartbeat. Verify liveness through the agent
handle itself or through worktree activity (a `git status` fingerprint that
keeps changing); treat claim presence alone as "assigned", not "running".

## Local waves

A local wave runs on one machine without Devin sessions: the coordinator spawns
one agent per `.agents/worktree/<wave>-<task>` worktree and each authorized
end-to-end owner claims, works, lands, and releases. Contributor and read-only
assignments retain their narrower scope: the integration DRI lands contributor
work, and review/scouting does not authorize publication. Declare publication
and required review responsibility before dispatch under
[AGENTS.md](../../AGENTS.md#delegation).
`worktree_status.py` reports the local wave's branch, claim, and landing state.
At drain, collect worker evidence notes first: `python3 tools/claims.py notes`
lists findings workers attached to their claim tickets; fold what they
justify into one board sweep PR through the native merge queue, then
`python3 tools/claims.py sweep --owner <wave>` marks this wave's notes
consumed while leaving sibling waves' pending evidence alone. Finish with
`python3 tools/swarm/fill.py --manifest tools/swarm/waves/<wave>.json close`
so the ledger stops proposing refills, then write the per-slot
tally (result, commits, `item_closed`) to
`tools/swarm/waves/<wave>.outcomes.json` so local waves stay measurable the
same way cloud waves do.

### Coordinator runbook

The agent-side lessons from macw1–macw3 are baked into
`prompt_template_local.md` — keep it authoritative rather than re-teaching
them per prompt: outcome handoffs and advisory path overlaps, no `TASKS*.md` in
claimed paths, never `git stash` in a wave worktree, validate before PR submission,
release build slots before queue waiting, `mbx gc` on disk exhaustion. What remains genuinely
coordinator-side:

- Every session on every machine — local subagents, cloud waves, other hosts —
  draws from one org message budget. A 20-at-once burst died within minutes;
  sustained waves at 8 stayed up while the budget was quiet and died when it
  was not. Launch a batch, let claims register, then backfill each freed slot
  instead of launching the whole wave at once.
- linw2 evidence: in-session background subagents can be canceled en masse
  without any completion notification — nine linw2 slots exited "Canceled by
  user" across two rounds while their coordinator believed they were running;
  only a manual liveness check exposed it. A spawned slot is not a running
  slot: verify each handle is alive before counting it, verify every slot's
  handle at every checkpoint, and correlate quiet worktrees (no commits, no
  dirt, no recent mtimes, no live claim) against the handle before assuming
  progress. On repeated cancellations stop backfilling, WIP-commit dirty
  worktrees, release orphaned tickets, and report rather than respawn into a
  canceller. Spawn only through the session's own subagent mechanism — never
  detached shells, tmux, or terminals the user cannot observe.
- The `swarm` skill (`.agents/skills/swarm/references/local.md`) carries the
  coordinator procedure end to end — partition, launch, monitor, recover, drain
  — and defers here for the evidence behind each rule.
- Cursor cloud subagents started from a Cursor chat follow
  `.agents/skills/swarm/references/cursor.md`. Devin sessions follow
  `.agents/skills/swarm/references/devin.md`.

### Recovering an interrupted wave

1. `python3 tools/swarm/worktree_status.py` matches each worktree to its live
   claim and lists claims whose owner has no worktree.
2. For each dead agent, release its ticket with
   `python3 tools/claims.py release --ticket <ticket>` from the status row.
   Confirm the agent is actually dead first (agent handle gone, worktree
   quiet) — a live claim on a quiet worktree can still be a long-running
   build. Expired claims reap themselves on the next registry action; only
   live-leased orphans need the explicit release.
3. Preserve dirty worktrees before removing them:
   `git -C <wt> add -A && git -C <wt> commit -m "wip(...): interrupted"` keeps
   the work on the branch. Clean worktrees and landed branches can be removed
   outright. When the item stays open, record the parked branch ref and the
   slice it covers in the item's board evidence so the coordinator can merge
   it to main through the landing queue — a successor on another machine
   cannot see an unmerged local branch.
4. Relaunch continuations onto the same branches (fresh worktree per branch)
   and have them reclaim the item; the wave loses no work.

## Coordinator selection rules

Follow [capability selection](../../AGENTS.md#the-sample-loop): retain regressions
and explicit user scope, then select by existing outcome priority and release
dependencies, not sample size. Give one session the parent task, its remaining
acceptance and next executable witness; all necessary pipeline crates belong
to that owner. After each witness lands, continue the same capability until its
acceptance closes or the canonical switching/stopping conditions apply.
Do not fill slots with invented failures when a first witness already passes.
Send changed instructions explicitly to resumed workers at safe checkpoints.
Check live outcome assignments; shared paths do not require partitioning.
An exclusion applies to its wave, not all future work: reassess it when preparing
a new wave and retain it only for a current conflict or concrete blocker. Do not
edit a launched wave to reassign its running sessions.

Pick items that are:

- runnable on the assigned host — Linux x86-64 for cloud sessions; a local
  session takes this machine's host, including its gaps,
- distinct outcomes: one session owns an outcome, while shared files are
  allowed. Review semantic dependencies and reconcile overlapping
  implementations at landing,
- not in this wave's `exclusions` or already owned as an outcome
  (`python tools/claims.py status`); paths are advisory,
- named in `TASKS.md`, `TASKS_BOOTSTRAP.md`, or `TASKS_OPTIMIZER.md`, with
  the item present as `**<item>.**`.
- still unimplemented: board "next slice" text can lag landed code — check
  the item's named machinery against recent history before manifesting it
  (the Sept board-history screen confirmed stale or duplicate assignments
  in 4 of 11 reviewed flags).

When an item returns in a later wave's manifest, the previous session's board
resume evidence should have let it skip re-witnessing the failure. At wave
review, check one returning item's session for that reuse and note the finding
beside its outcomes entry; that is where the repeated-rediscovery cost of
multi-session items would show up.

## Claims

Claims record outcome DRIs. Path overlaps appear in reports, not rejection.
Existing manifests and tickets remain valid. Refresh old clients before new
assignments; landing still serializes publication and requires reconciling
incoming changes. An old manifest is not a current ownership claim.
