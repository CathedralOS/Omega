---
name: dot-swarm
description: Coordinate an Omega cloud swarm with dot's native collaboration agents, using devin-swarm's small-pool chain workflow, isolated worktrees, immediate result drains, and verified outcomes. Use for a dot swarm, a native-agent cloud wave, or continuing such a wave. Use devin-swarm for Devin sessions, local-swarm for other local launchers, and cursor-swarm for Cursor cloud subagents on separate VMs.
---

# dot swarm

Adapt [devin-swarm](../devin-swarm/SKILL.md)'s outcome workflow to dot's
cloud computer. Keep its task selection, warm chain ownership, slice-first
progress, immediate result drains, and same-domain continuations. Replace its
Devin transport and mass-wave mechanics with the actual native agent tools.
[AGENTS.md](../../../AGENTS.md), [advance](../advance/SKILL.md), and the current
[testing](../../../tools/testing.md), [claims](../../../tools/claims.md), and
[landing](../../../tools/landing.md) contracts govern repository work.

Apply the [law of completion](../../../AGENTS.md#law-of-completion).
Each implementation owner keeps context through complete acceptance, required
review and authorized verified main landing. It finishes necessary prerequisite
repairs itself rather than waiting for another agent or ending at a handoff.
Read-only diagnostic assignments remain read-only.
Follow the concrete path through completion; let working programs reveal design.

## Install and invoke

Keep this whole directory at `.agents/skills/dot-swarm/` in an Omega checkout.
The repository discovers `.agents/skills` directly; do not maintain a second
copy in a vendor-specific skills directory. To share the adaptation, include
this directory in a repository patch. From the checkout root, run
`git apply --check dot-swarm.patch`, then `git apply dot-swarm.patch`; inspect an
existing skill instead of overwriting it on a conflict. This is an Omega workflow,
not a standalone compiler or a provisioner for arbitrary cloud machines.

Invoke with a concrete scope, for example:

- `Use $dot-swarm for two diagnostic workers on SAMPLE-CORPUS and CANARY-CORPUS. Stop after both reports; no edits or publication.`
- `Use $dot-swarm to advance <program or board item>, with up to four workers. Keep going until <acceptance or stopping condition>.`
- `Resume the dot-swarm wave <wave> from its saved ledger.`

A broad compiler-advancement request uses the existing boards and the
`advance` selection rules; it does not need an invented milestone. A request
to create, review, or test this skill alone does not authorize compiler changes.
If scope is genuinely missing, ask for it or complete only an authorized
read-only diagnostic wave. State the wave's completion condition before launch.

Prerequisites: an Omega checkout, Python 3, Git, a native collaboration runtime,
and the host/toolchain needed by the chosen acceptance. Examples use `python3`;
substitute the available Python 3 executable, such as `python` on Windows. Use the repository's
pinned Rust toolchain and `mbx` plus cargo-nextest when compilation is needed;
Cargo is the documented fallback if `mbx` is unavailable. Follow the current
testing policy rather than installing tools or building a release unnecessarily.
Read [the verified diagnostic-wave record](references/verified-wave.md) for
what has actually been exercised and what remains unverified.

Before dispatch, after interruption, and at completion, follow
[recover interrupted lanes](references/recovery.md): checkpoint exact work,
reconcile uncertain outcomes, verify backups, and inventory owned resources.
Use its executor-scoped build slot and next-action plan; disconnected is not
dead, a local archive is not a confirmed backup, and cleanup is never implicit.

For coordinated cloud batches and staged rollout gates, follow the executable
[cloud coordination contract](references/cloud-coordination.md). It adds durable
assignment/result ingestion and final acceptance to the recovery journal; it
does not authorize extra sessions or replace outcome ownership.

## Detect the runtime and budget

For a new cloud session or an existing session's native workers, first apply
the [cloud launch preflight and model defaults](references/cloud-launch.md).
It also covers a single cloud advance without invoking a swarm. The cloud
session launch tool selects the environment; `collaboration.spawn_agent`
starts a worker on the current shared executor. Do not describe a native
worker as a newly provisioned machine or silently inherit a different model.

Inspect available tools and current handles before starting. The native mapping
is:

| Operation | Native tool |
| --- | --- |
| Start a bounded assignment | `collaboration.spawn_agent` |
| Send information to a running worker | `collaboration.send_message` |
| Continue the same assignment after its turn ended | `collaboration.followup_task` |
| Reconcile current handles and statuses | `collaboration.list_agents` |
| Wait when no useful independent step remains | `collaboration.wait_agent` |
| Stop an active turn when necessary | `collaboration.interrupt_agent` |

Tool schemas and returned statuses are authoritative. A native message has no
`notify_on_response` option; completion reports arrive through the runtime.
`send_message` does not restart an idle worker; use `followup_task` for a
continuation. Interruption ends a turn, not necessarily a background process.

The current runtime declares seven active-agent slots **across the entire root tree**,
including the root and every nested coordinator/reviewer. This is not six new
workers per coordinator. Enumerate all agents, reserve capacity for existing
work, and use the current runtime's limit if it differs. Treat capacity errors
as a smaller available pool; do not probe repeatedly or create a hierarchy to
multiply slots. Report confirmed running workers separately from the coordinator.

Native agents share one filesystem. Give each code-editing worker its own
worktree and never run two owners in one checkout. Read-only workers may share
a pinned source checkout with separate scratch output paths. Do not use Devin
sessions, detached agent shells, the resume-cap loophole, stockpiled pools, an
Overlord tier, or cloud-specific archive operations. If the native tools are
absent, report the missing runtime rather than pretending a shell is an agent.

## Select and partition

1. Complete the [mandatory main preflight](../../../AGENTS.md#sessions-and-subagents)
   before dispatch. Record the starting revision, branch, worktree status, host,
   scope and publication authorization. Fetch successfully, preserve unrelated
   work and reread updated instructions. A failed fetch stops the new launch;
   cached main is not a fallback.
2. Read the relevant board sections and named code, then check live assignments
   with `python3 tools/claims.py status`. These name outcome DRIs, not file fences. Read the [coordinator selection rules](../../../tools/swarm/README.md#coordinator-selection-rules).
3. Follow [capability selection](../../../AGENTS.md#the-sample-loop). Choose a
   required compiler capability with an executable acceptance witness, or a
   measured development-health outcome with a bounded target. Keep a continuation
   on its outcome through acceptance. Verify stale board claims
   against the code and recorded evidence. Do not fill spare slots with invented
   mining, fuzzing, or unrelated cleanup.
4. Assign distinct outcomes with advisory implementation and validation paths. Include the board's
   acceptance verbatim, the next requirement to discharge, concrete witness path,
   expected result, governing design,
   path estimates, known dependencies, and other workers' outcomes. Check required
   prerequisites before spending a slot on an impossible leg. Reading a shared
   dependency is allowed; necessary edits proceed with overlap reported.
5. Reserve a distinct coordinator outcome when scope and capacity justify it.
   Keep drains ahead of that slice. At this width, assign one persistent outcome
   chain per worker, then implement its smallest verifiable leaves in order.
6. Record assignments **before dispatch** in `build/swarm/<wave>/inflight.json`:
   item, worker, paths, assigned time, dependencies, mode, and stopping condition.
   Skip already-owned outcomes and in-flight assignments; claims alone leave a race
   before a new worker claims. Record the returned handle after spawn succeeds.

For implementation, create worktrees from the top-level checkout under
`.codex/worktrees/<wave>-<name>` and use local `swarm/<wave>-<name>` branches.
Start fresh work from fetched `origin/main`; keep a continuation's worktree and
warm build state. Never switch, reset, auto-stash, or clean another session's tree.
Git's stash is shared across worktrees; use a separate baseline worktree instead.

Claim before editing, retain the ticket, renew before expiry, and report evidence
through `claims.py note`. Another claim on the same outcome or on overlapping
paths does not block work or require an override; reconcile at rebase. Shared claims, notes, releases, and landing are remote writes:
if the assignment forbids remote writes, do not silently invoke them. A read-only
wave needs no edit claim; keep its assignments locally and label them read-only.
A local-only edit exception needs an explicit scope decision and is not a shared
claim. For an unavailable registry, follow AGENTS.md and tools/claims.md: record
the intended scope and outage, then continue authorized development in an
isolated worktree. Reconcile uncertain mutations before retrying. Notify peers
of path overlap and preserve both outcomes during integration.

## Reuse the repository helpers carefully

For an implementation wave, use `tools/swarm/waves/local-example.json` as the
manifest shape, with `host: "local"`: local means this shared executor, even
though the computer is in the cloud. The selected boards, item markers, host
gates, route checks, and owning paths still apply.

`python3 tools/swarm/launch.py local --manifest <manifest>` validates and renders;
`--create-worktrees` additionally prepares isolated checkouts. It does **not**
spawn native agents. Read each rendered prompt and its `partition_hints` before
passing it to a worker. The existing template invokes `advance`, claims, and
publication; do not use it unchanged for diagnostics or local-only work. Give a
self-contained prompt matching the actual authorization instead. Keep a draft
manifest and all runtime ledgers under ignored `build/swarm/<wave>/`.

Cloud sessions reach the Jev advisories only when the manifest injects
`"secret_ids": ["TYPESAFE_API_KEY"]`
([tools/swarm/README.md](../../../tools/swarm/README.md)) AND the session
network policy allows api.typesafe.ai — the ccr2 wave ran with neither and
recorded Jev unavailable (tools/swarm/waves/ccr2.outcomes.json).

`launch.py launch` is the Devin path and is not used here. Host gates are
prerequisites, not the expected-red customer acceptance. Do not bypass checks to
make an invalid implementation wave appear launchable.

`python3 tools/swarm/worktree_status.py --offline --base <wave-base>` gives a
local-only worktree view. `tools/swarm/fill.py` can retain slot bookkeeping with
`note` and `close`; its `plan` performs remote reads and has no offline flag.
Inspect their current help and proposed plan;
`recover --execute`, `sweep --execute`, and claims-related actions mutate state.
A helper's recommendation is not authorization. Native handle state is the source
of truth for agent liveness; these helpers do not manage native agents for you.

## Dispatch contract

Give each worker one self-contained assignment containing:

- Wave and slot name; exact checkout, branch, and base revision
- Mode: diagnostic, local implementation, or authorized publication
- One program/outcome chain, current board acceptance, witness and expected result
- Read-only restrictions where applicable, advisory edit paths, validation,
  design links and the current outcome/session roster
- Exact useful checks, build ownership, relevant toolchain setup, and stop rules
- Reporting destination and the verdict fields below

Workers read `AGENTS.md` and the relevant skill in their checkout. For diagnostics,
request source-backed findings and actual checks they can safely execute; missing
binaries or dependencies are named blockers, not permission to fabricate a run.
For code work, validate affected behavior per `tools/testing.md`: focused
regression, affected crate checks/tests, and corpus/native checks when applicable.
Use `mbx` when present and record revision, command, host, exit, and counts.
Run one host build at a time; the shared computer is not six independent VMs.

Authorized end-to-end owners finish and publish their complete paths through
the normal landing protocol. Do not downgrade them into contributor-only roles
without the user's instruction. Keep board evidence scoped to the assigned
outcome; coordinator ledger work does not replace acceptance or landing.
Retain the assignment's required independent review; report turns add no approval
gate. Keep commits semantically whole and follow the
repository's current commit conventions. Format changed code before committing;
report superseded changes instead of landing duplicates. Intermediate status
does not end the assignment. The same owner finishes all required gates before
claiming completion or publishing its path.

Return a compact verdict (JSON or equally explicit prose) with:
`worker`, `item`, `mode`, `result`, `base`, `branch`, `commits`, `claim_ticket`,
`checks`, `witness`, `notes`, `remaining_legs`, `background_jobs`, `item_closed`.
Use `verification_only`, `candidate_ready`, `blocked`, `superseded`, or `landed`
accurately. A diagnostic report leaves the compiler board item open. Name a
background job's actual process/session handle and log, not just "still running".

## Coordinate, drain, and continue

Reconcile handles after each completion, user message, long tool return, and
before returning to the coordinator's own slice. Process new reports immediately:
inspect the result and diff, append an outcome, reconcile claims and in-flight
ownership, then give the next dependency-ready leg to available capacity.
Do not await a long build while finished reports remain undrained. Yield long
commands through the execution tool and retain their session IDs; handle other
work between checks. When nothing useful remains runnable, wait for a report.

Prefer the next requirement of the same parent capability, not merely nearby
work in the same domain; a landed witness does not close broader acceptance.
Resume that assignment with `followup_task`. Send changed selection instructions
explicitly to existing workers at safe checkpoints, preserving their work and
user limits. Use a new worker for a new
independent assignment. Reuse the session: a new executor spends a machine
and the tokens required to rebuild the investigation. After a landed repair,
compact when the next assignment is a different outcome. When the next run
branches from that repair, do not compact. Do not silently broaden a completed chain to any board
item. Redispatch only while the user-requested wave scope and stopping condition
remain open. A missing dependency blocks the item, not unrelated permitted work;
when no eligible work exists, leave the slot empty and name the blocker.

Record actual statuses, not merely spawn calls or filesystem mtimes. Confirm the
worker acknowledged its assignment before claiming useful work began. On an
uncertain spawn, reconcile handles before retrying. If a worker settles without
a verdict, inspect its last report, worktree, and processes; continue the same
leg if appropriate. Do not repeatedly nudge a posted verdict into rerunning its
completed audit. Preserve dirty work before replacing a failed worker, and do
not release a claim until its owner is confirmed stopped. Stop refill on a user
cancellation; interruption alone does not prove child processes stopped.

## Integrate only within authorization

Keep native workers' candidates in local branches; their commits are already
available on the shared filesystem. No remote worker branch or force-push is
needed. Review and integrate one verified candidate at a time in an isolated
integration worktree, preserving both intents in conflicts. Resolve substantive
ambiguities with the owner rather than guessing.

For authorized publication, use the current `tools/landing.py` workflow: prepare
and validate, obtain the required exact-commit receipt, enqueue/claim, rebase onto
the reserved base, and revalidate changed inputs. If rebasing changes HEAD, run
`python3 tools/landing.py verify --base <reserved-base>` again for that exact candidate SHA
even when ordinary test evidence is reusable, then publish. Respect its lease and
clean-tree requirements. Never use merge commits, direct main pushes or a batch-merge bypass. A lane is
`landed` only after its candidate is verified as an ancestor of fetched
`origin/main`; local integration is `candidate_ready`.

Coordinator-owned board updates follow actual evidence: remove an item only when
its program acceptance passes, otherwise replace its frontier rather than append
history. Scope claim-note sweeps to this wave. Do not republish toolchain release
assets or advance external pins without authorization; if a shared packaged
binary is used, verify its provenance once and communicate its revision to every
worker. Never assume another service's credentials or release bundles exist.

## Finish and preserve the evidence

Append each settled leg once to `build/swarm/<wave>/outcomes.json`, keyed by worker,
item, revision and assignment. Keep the base, mode, commands/results, candidate or
landed commits, blockers, residuals, and retained worktree paths. For compiler
implementation, lead the report with
`python3 tools/corpus_scoreboard.py --base <wave-base> --json` and explain its
recorded-evidence scope; it is not a substitute for running the changed program.
For diagnostics, mark native acceptance and scoreboard execution as not run when
they were not run. Do not claim compiler progress from an agent-count test.

At the stopping condition, stop backfilling and drain every running assignment
and relevant background check. Report named blockers requiring authorization or
user input. Release only this wave's claims when permitted. Retain dirty or
unpublished candidates; remove only clean, verified-landed worktrees/branches
owned by this wave. Native agents have no Devin archive step. Verify the starting
checkout remains intact and report useful findings, actual validation, publication
state, and any remaining dependency. A successful diagnostic wave proves dispatch,
coordination, and evidence collection; it does not prove compiler fixes or landing.
