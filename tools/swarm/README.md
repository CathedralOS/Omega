# Swarm launcher

`launch.py` fans "advance the compiler" out to Devin Cloud sessions: the
coordinator writes a wave manifest that pre-assigns one board item per session,
the launcher creates one session per entry, and each session runs the ordinary
`advance` skill restricted to that item, registering its assignment in the
shared claims registry ([tools/claims](../claims.md)) and landing through
`tools/landing.py`.

The launcher writes no board text, holds no landing claim, creates no Git refs,
and keeps receipts only under the ignored `build/swarm/<wave>/` directory.
Partitioning lives in the manifest, checked against the claims registry's live
assignments.

## Prerequisites

- Python 3.9+ and Git, run from the repository root (or pass `--repository`).
- `DEVIN_API_KEY` (a `cog_...` service-user key) and `DEVIN_ORG_ID` in the
  environment. They are read from the environment only and never printed.

Windows PowerShell:

```powershell
$env:DEVIN_API_KEY = "cog_..."
$env:DEVIN_ORG_ID = "org-..."
python tools/swarm/launch.py plan --manifest tools/swarm/waves/wave-1.json
```

macOS shell (use `python3`):

```sh
export DEVIN_API_KEY=cog_...
export DEVIN_ORG_ID=org-...
python3 tools/swarm/launch.py plan --manifest tools/swarm/waves/wave-1.json
```

`plan` needs no credentials and no network. `launch --dry-run` needs no
credentials either; real `launch`, `status`, and `report` call the Devin API
(`--base-url`, default `https://api.devin.ai/v3`).

## Workflow

1. Write a manifest (see `manifest.schema.json`; `waves/wave-1.json` is the
   worked example).
2. `plan --manifest <file>` — validates the manifest, runs `host_gates` and
   the omega-route check, probes owning-path freshness (including each path's
   crate), checks each item and its owning paths against the live claims
   registry, renders one prompt per session to `build/swarm/<wave>/prompts/`,
   and prints the exact request bodies. Each session row also carries
   `partition_hints`, advisory signals read from the item's board text:
   `dependency_language` ("depends on", "joins the preceding task") marks
   sequencing candidates for layers; `references_items` names other board
   items the text cites, and `same_layer_reference` warns when one is a
   same-layer session; `uncovered_mentions` lists paths or backticked crate
   names the item text cites that `owning_paths` do not cover — the fence
   would protect the wrong ground; `scale_hint` fires when the text spans
   many crates, marking a multi-layer decomposition rather than one slice.
   Use them before spawning: adjust owning paths to the named machinery and
   order dependency-flagged items into layers. Use
   `--skip-host-gates`, `--skip-route-check`, or `--skip-claims-check` when
   the coordinator deliberately does not run those checks. An unreachable
   registry reports `unavailable` per session rather than failing `plan`'s
   offline use; a reachable registry with a conflicting live claim fails like
   a host gate.
3. `launch --manifest <file>` — re-runs the claims check, then creates one
   session per entry and records `receipts.json`. A name with an existing
   receipt is skipped; pass `--relaunch <name>` to recreate just that session.
   `--dry-run` prints the bodies without network.
4. `status --wave <id>` — polls each session's status and ACU consumption.
5. `report --wave <id>` — merges receipts and structured output into
   `build/swarm/<wave>/report.md`, the input for a `retrospect` pass. With
   `--save` it also writes `tools/swarm/waves/<wave>.outcomes.json`: a tracked
   per-session record of `result`, `acus_consumed`, and `item_closed`
   (the assigned item's `**<item>.**` marker absent from its board at report
   time), plus a result rollup. Run it after the wave's landings settle and
   commit the outcomes file with the manifest so closure and ACU-per-closure
   stay measurable across waves.

## Local worktree status

Cloud `status`/`report` only see Devin sessions through their receipts. Local
waves run in plain git worktrees (for example `.codex/worktrees/<wave>-<task>`)
and produce no receipts, so `worktree_status.py` gives them the same
visibility: each worktree's branch, uncommitted churn, ahead/behind against
`origin/main`, whether its head already landed, the live claims-registry
assignment matched by owner name (including `item_on_board` for board claims),
and the landing queue's state — plus claims held by owners with no local
worktree (cloud sessions, other machines). It is read-only.

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

A local wave runs the same protocol from one machine without Devin sessions:
the coordinator spawns one agent per `.codex/worktrees/<wave>-<task>` worktree
and each agent claims, works, lands, and releases exactly like a cloud session.
`worktree_status.py` is the local wave's `status`/`report` equivalent. At
drain, write the per-slot tally (result, commits, `item_closed`) to
`tools/swarm/waves/<wave>.outcomes.json` so local waves stay measurable the
same way cloud waves do.

### Launching

Local sessions carry `"host": "local"` in the manifest; a manifest may mix
hosts — `plan`/`launch` handle the `linux` entries and `local` handles the
rest, each reporting the skipped side. `waves/local-example.json` is the
minimal shape.

```sh
python3 tools/swarm/launch.py local \
    --manifest tools/swarm/waves/<wave>.json --create-worktrees
```

`local` runs the same validation, host gates, route check, and claims check
as `plan`, renders one prompt per local session from
`prompt_template_local.md` into `build/swarm/<wave>/prompts/<name>.md`, and
prints the launch table: name, item, prompt path, worktree, branch, and owner
label per row. The coordinator then spawns one agent per row with that
prompt.

`--create-worktrees` pre-creates `.codex/worktrees/<wave>-<name>` on
`swarm/<wave>-<name>` from `origin/main`; without it agents create their own.
When the branch already carries unpublished commits the rendered prompt marks
the slot a continuation and instructs the agent to resume that work — the
same path as recovery step 4 below. Local claims default to a 120-minute
lease in the rendered command so interrupted slots self-release sooner than
the registry's 480-minute default.

The local template differs from the cloud one where the waves proved it
matters: conflict-to-pivot instead of stop-and-block, `mbx` instead of Cargo,
the worktree/`git stash` rules, validate-before-claim landing order, and a
per-host block generated from `platform` (Intel macOS gets the
`--target linux_x86_64` workaround).

### Layers

Sessions may carry `"layer": N` (default 0) for dependency ordering. Launch
layer N only after layer N-1 has fully landed or parked — each layer builds on
`origin/main` containing the previous layer's published work, so dependency
order replaces conflict avoidance. The owning-path overlap check applies
within a layer only: a layer-1 session may own paths a layer-0 session
touched, because the earlier work is merged by the time the later session
starts. `--layer N` on `plan`, `launch`, and `local` selects one layer, and
composes with `local --sessions` for partial relaunches.

### Coordinator runbook

The agent-side lessons from macw1–macw3 are baked into
`prompt_template_local.md` — keep it authoritative rather than re-teaching
them per prompt: conflict-to-pivot on claim conflicts, no `TASKS*.md` in
claimed paths, never `git stash` in a wave worktree, validate before claiming
the landing queue, `mbx gc` on disk exhaustion. What remains genuinely
coordinator-side:

- Every session on every machine — local subagents, cloud waves, other hosts —
  draws from one org message budget. A 20-at-once burst died within minutes;
  sustained waves at 8 stayed up while the budget was quiet and died when it
  was not. Launch a batch, let claims register, then backfill each freed slot
  instead of launching the whole wave at once.
- The `local-swarm` skill carries the coordinator procedure end to end —
  partition, launch, monitor, recover, drain — and defers here for the
  evidence behind each rule.

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

Prioritize immediate customer outcomes within the requested scope, not the ease
of producing a helper commit. Give one session the bounded end-to-end slice and
its application command; several pipeline crates can belong to that one slice.
Check live assignments with the coordinator before partitioning shared paths.
An exclusion applies to its wave, not all future work: reassess it when preparing
a new wave and retain it only for a current conflict or concrete blocker. Do not
edit a launched wave to reassign its running sessions.

Pick items that are:

- runnable on the assigned host — Linux x86-64 for cloud sessions; a local
  session takes this machine's host, including its gaps (Intel macOS has no
  host profile; see `wiki/drafts/known_baseline_failures.md`),
- non-overlapping: the launcher rejects parent/child `owning_paths` overlaps
  inside a manifest and conflicts with live claims; still review shared
  dependencies, since path checks cannot catch every semantic coupling,
- not in this wave's `exclusions` and not under another session's or machine's
  live claim (`python tools/claims.py status`),
- named in a board the launcher knows (`TASKS.md`, `TASKS_BOOTSTRAP.md`,
  `TASKS_OPTIMIZER.md`), with the item present as `**<item>.**`.
- still unimplemented: board "next slice" text can lag landed code — check
  the item's named machinery against recent history before manifesting it
  (the Sept board-history screen confirmed stale or duplicate assignments
  in 4 of 11 reviewed flags),
- ready to attempt on the assigned host: `host_gates` are environment/access and
  prerequisite checks that must pass before assignment. Keep the expected-red
  customer reproduction in the board acceptance or `suggested_first_slice`, not
  in `host_gates`. Acceptance must pass after implementation, not before selection,
- an optimizer item's owning path must be the `X-to-X` stage crate at the
  item's representation level, and that compiler-tree crate under
  `omega-rust/` must exist and be on the `omega` route; the route check does
  not reject test-harness crates outside `omega-rust/`. If the stage does not
  exist the slot is `probe_only` or dropped (wave-3 miss:
  `EXACT-MACHINE-SIMPLIFICATIONS`),
- `probe_only` is for an explicitly chosen feasibility investigation, not merely
  a cross-stage repair or a failing customer. Ordinary implementation slots must
  attempt the design-backed repair; helper tests alone do not establish closure.
- `budget_exhausted` requires active implementation of a slice the child still
  believes is landable. Diagnose scope honestly, but crossing a crate or stage
  boundary is not by itself a reason to stop at `verification_only`.

When an item returns in a later wave's manifest, the previous session's board
resume evidence should have let it skip re-witnessing the failure. At wave
review, check one returning item's session for that reuse and note the finding
beside its outcomes entry; that is where the repeated-rediscovery cost of
multi-session items would show up.

## Lead-only launch checklist

Before the first wave, a human with org access must:

1. Create a service user with `UseDevinSessions` under Settings > Service
   users, generate its `cog_` key, and note the org id.
2. Confirm Devin's GitHub integration covers both `CathedralOS/Omega` and
   `CathedralOS/Squalr-Omega` (the latter is needed for the blueprint's
   best-effort `git submodule update --init`) and can push `main` and
   `refs/coordination/*`.
3. Choose the agent mode in the manifest's `devin_mode` (wave 1 uses `fusion`).
4. Sync `.devin/blueprint.yaml` and wait for the snapshot build.

## Stated limits

- Cloud sessions expose no model choice. The only per-session knob is the
  agent mode (`normal`, `fast`, `lite`, `ultra`, `fusion`), passed as
  `devin_mode`; `null` means the org default mode. Which model backs each
  mode is not documented and cannot be pinned from the launcher.
- Snapshot warmth is relative to the snapshot's build commit. It buys
  crates.io downloads, the registry index, build scripts, and unchanged leaf
  crates; at this repository's commit velocity the first `cargo check` still
  rebuilds most of the workspace. Blueprints rebuild roughly every 24 hours.
- The claims registry is an advisory fence, not a lock. It makes live
  assignments visible across machines and waves, and `plan`/`launch` refuse a
  manifest that collides with a reachable claim — but a session that never
  claims, a `--skip-claims-check` launch, or plain direct work can still
  collide. The landing protocol serializes publication; it does not prevent
  competing implementations. Reconcile assignments before launch and on
  overlap reports; an old manifest or worktree is not a claim.
