---
name: local-swarm
description: Coordinate a requested local swarm using one isolated worktree per agent, persistent outcome ownership, focused checks and verified landing. Use for local concurrent agents or resuming a local wave; not cloud sessions or a single advance.
---

# Local outcome coordination

Follow [AGENTS.md](../../../AGENTS.md), [advance](../advance/SKILL.md),
[claims](../../../tools/claims.md), [testing](../../../tools/testing.md) and
[landing](../../../tools/landing.md). The user controls scope and duration.
Skill maintenance does not launch agents.

Apply the [law of completion](../../../AGENTS.md#law-of-completion).
Each implementation subagent retains context and owns the full assigned path,
including necessary prerequisite repairs, acceptance, required review and
authorized verified main landing. Do not divide its stages among agents or
replace completion with an artifact handoff.
For a user-authorized shared publication wave, follow
[shared integration](../../../tools/integration.md): owners keep responsibility
for complete repairs through remote acceptance, and the designated integrator
freezes a ready selection, validates it and publishes through the normal queue.
An archive is not completion. Independent publication remains the default.
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

Use `tools/swarm/waves/local-example.json`: one session name, sample
(`"sample": "samples/..."`, whose authored oracle is the acceptance) and path
estimate per outcome; name a board item instead only for work outside any
sample. Keep active IDs stable. `owning_paths` is a
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

Dispatch outcome, acceptance, failure, next action, dependencies, design,
exact worktree/base and publication permission. Agents reread the short assignment
after compaction. A checkpoint is not completion; a missing next stage is more
work on the same assignment.

## Coordinate and integrate

Report overlaps, continue necessary work and reconcile one implementation at
rebase. Two sessions on the same outcome is an overlap to reconcile, not a
permission question. Never narrow path estimates merely to evade a collision.

Keep returned handles and real states in `build/swarm/<wave>/slots.json`;
`tools/swarm/fill.py note` and `plan` support this. On completion inspect
evidence/diff and verify the owner's complete acceptance and authorized remote
landing. If either remains incomplete, resume the same owner on that outcome.
Only after completion choose new work within user scope.

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
