---
name: local-swarm
description: Coordinate a requested local swarm using one isolated worktree per agent, persistent outcome ownership, focused checks and verified landing. Use for local concurrent agents or resuming a local wave. Use cursor-swarm for Cursor cloud sessions on separate VMs, and advance for a single sample.
---

# Local outcome coordination

Before automatic implementation assignment, follow the common
[reserve-before-dispatch procedure](../../../tools/swarm/dispatch.md). Review a
current-main spec/fact gap, reserve outcome and chosen session atomically through
existing claims, and arm one durable send attempt before invoking the actual
runtime. Shared compiler paths remain advisory. Never auto-recover an expired
Zac/Devin claim: inspect actual liveness/progress; record confirmed stop evidence
or named user takeover authority. Adopt the coordinator's reserved ticket rather
than registering a second claim. Unknown sends retain their ticket and require
runtime reconciliation before retrying.

Follow [AGENTS.md](../../../AGENTS.md), [advance](../advance/SKILL.md),
[claims](../../../tools/claims.md), [testing](../../../tools/testing.md) and
[landing](../../../tools/landing.md). The user controls scope and duration.
Skill maintenance does not launch agents.

Apply the [law of completion](../../../AGENTS.md#law-of-completion).
Each implementation subagent retains context and owns the full assigned path,
including necessary prerequisite repairs, acceptance, required review and
authorized verified main landing. Do not divide its stages among agents or
replace completion with an artifact handoff.
Follow the concrete path through completion; let working programs reveal design.

Use [host setup and build admission](host-setup.md) before dispatch. Local
subagents share one machine, not just one conversation: discover its actual
OS/architecture, CPU quota, memory, disk, native tools and retained cache state.
Reuse valid measurements. Ask only for unresolved scope, model/effort or authorized
width; do not make the user inventory their machine.

## Prepare and dispatch

Complete the [mandatory main preflight](../../../AGENTS.md#sessions-and-subagents)
before preparing dispatch: fetch successfully, update a clean main checkout or
use an isolated checkout at the fetched commit, and reread current instructions.
Stop a new launch if the fetch fails. Inspect host capacity, live claims and
in-flight assignments. Assign
distinct outcomes, not disjoint files. Compiler capabilities and measured
iteration-performance/test-environment incidents are eligible. Preserve explicit
parked scope; idle rather than mine tasks or invent audits.

Follow [capability selection](../../../AGENTS.md#the-sample-loop). Compiler
advancement assigns fact folders under `samples/cli/facts` only. One fact is
one folder. The nine places are one row, not a matrix.
`samples/cli/facts/length/` is the filled row that met this bar. The next
witness fills another fact across those places or takes the next uncovered
fact. Do not assign a program from the rest of `samples/`. Copy the worker
goal below into every assignment. Use `python3 tools/sample_queue.py --facts`
when a failure list is needed; the unfiltered queue is ordinary program
discovery.

## The worker's goal

Copy this into every worker prompt:

> Read the assigned capability's full acceptance and identify the requirement
> this witness establishes. Compiler advancement witnesses are fact folders
> under `samples/cli/facts`. One fact is one folder. The nine places are one
> row, not a matrix: local, field, parameter, call result, match join, const
> name, another state, another machine, and another target body. Do not select
> a program from the rest of `samples/`. `samples/cli/facts/length/` is the
> filled row that met this bar. A fact folder is done only when the same fact
> survives all nine places. One sentence names the fact and the operation, and
> every place copies that sentence. Changing the fact, the bound, or the
> operation so a place passes is a different fact and does not count. A red
> place is a checking repair: keep the program the same shape, and record the
> diagnostic before editing the compiler. Each place needs a reject twin that
> breaks only that fact and still rejects. A place with no twin does not count.
> A landing that only rewrites the failing place is not acceptance. Exact
> subtraction, alias transfer, and dependent subtract are examples of that
> repair path. Run its public acceptance command. Repair failures
> generally, preserving valid compositions and invalid controls, and land each
> complete repair through `landing.py`. After landing, or if this witness
> already works, reconcile only what the evidence establishes and continue the
> same capability's next remaining requirement. Do not manufacture a failing
> sample or a compiler edit. Full task acceptance, not the first passing
> program, closes the capability. Return to the remaining acceptance in this
> same session. A session that lands the witness and stops has not finished.
> The claim note names what of the customer now runs and what still fails,
> in words a person can read without the diff. Respect explicit one-sample
> scope, user stops and budgets; preserve remaining acceptance when interrupted.

For automatic
work, use the launcher's existing `board`/`item` manifest form and put the next
witness, oracle and requirement in `suggested_first_slice`. The `sample` form
in `tools/swarm/waves/local-example.json` remains for explicitly sample-scoped
work; do not combine those mutually exclusive forms. Keep active IDs stable.
`owning_paths` is a
compatibility name for advisory estimates. Layers represent real dependencies,
not a requirement to serialize file overlaps.

`python tools/swarm/launch.py local --manifest <file> --create-worktrees`
validates and renders prompts under ignored `build/swarm/<wave>/`; it does not
spawn agents. Inspect prompts and host prerequisites. Each editing agent gets
a distinct worktree from fetched main. Never reset/stash another owner's work.

Use actual in-session agent tools and returned handles, not detached shell agents.
Respect runtime capacity and the shared CPU/memory budget; workers on one host
are not independent machines. Schedule builds explicitly using actual capacity.
Agent slots and expensive-build slots are separate. Begin with one expensive
build at a time when shared-host contention is unmeasured; keep other agents
editing their own outcomes. Increase build overlap only after a same-host
comparison improves useful throughput without memory/disk pressure or worse
E2E latency. Do not infer a universal agent count or Cargo jobs cap from CPU count.

Keep worktree targets independent and reuse the host's supported MBX action
cache. Never force a common `CARGO_TARGET_DIR` across editing agents. Prefer a
stable worktree for an ongoing outcome; a same-checkout no-op does not prove a
new worktree's cache hits. Check pressure before builds and close delegated stdin
using the native host's supported entrypoint.

Dispatch parent outcome, remaining acceptance, next witness and the requirement
it discharges, failure, next action, dependencies, design,
exact worktree/base and publication permission. Agents reread the short assignment
after compaction. Reuse the session that already has the checkout. A landed
witness is not completion while the parent acceptance still fails; return to
that outcome in the same session. Compact after that acceptance, when the
next assignment is a different outcome. When the next failure is still that
outcome's, do not compact. A checkpoint is not
completion; a missing next stage is more work on the same assignment.

## Coordinate and integrate

Report overlaps, continue necessary work and reconcile one implementation at
rebase. Two sessions on the same outcome is an overlap to reconcile, not a
permission question. Never narrow path estimates merely to evade a collision.

Keep returned handles and real states in `build/swarm/<wave>/slots.json`;
`tools/swarm/fill.py note` and `plan` support this. On completion inspect
evidence/diff and verify the owner's complete acceptance and authorized remote
landing. If either remains incomplete, resume the same owner on that outcome.
After each witness lands, continue the parent's remaining acceptance; do not
count a witness landing as full capability closure. Switch only under the
canonical selection rules and user scope. Send updated selection instructions
explicitly to existing workers at safe checkpoints without discarding their work.

Use local branches and landing.py, not merge commits/direct pushes. Rebase
incoming changes, preserve behavior and validate the combined result. Existing
unrelated failures do not require another repair assignment. Reconcile board
requirements; never union-merge accumulated notes. Two advisories order that
verification: `tools/jev_attribute.py` ranks a commit window for a failing
signature before a bisect, and `corpus_gate.py --jev` gives the record-safety
verdict before any `--record` (tools/testing.md).

Inspect actual agent handles/processes, not mtimes. An idle turn or missing
notification does not prove death. Do not duplicate a worker or release its
claim because a peer needs the same files. Follow
[recovery](../dot-swarm/references/recovery.md) for interruptions and inspect
helper actions before executing them.

## Finish

At the stopping condition stop refilling, request safe checkpoints and drain
reports. Preserve dirty/unpublished work and remaining acceptance; release owned
claims and remove only verified-landed worktrees/branches. Report outcome closure
and observed behavior, not helper-test, commit or occupied-slot counts.
