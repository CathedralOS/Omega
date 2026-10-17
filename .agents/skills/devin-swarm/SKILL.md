---
name: devin-swarm
description: Coordinate a requested Devin swarm with complete end-to-end outcome ownership, isolated worktrees and verified main landing. Use for Devin cloud sessions or a Devin swarm. Use cursor-swarm for Cursor cloud subagents, local-swarm for agents on one machine, and advance for a single sample.
---

# Devin outcome coordination

Each session carries its outcome through acceptance and integration. Follow
[AGENTS.md](../../../AGENTS.md), [advance](../advance/SKILL.md),
[claims](../../../tools/claims.md), [testing](../../../tools/testing.md) and
[landing](../../../tools/landing.md). The user sets scope, width and stopping
condition. Editing this skill does not launch a swarm.

Apply the [law of completion](../../../AGENTS.md#law-of-completion).
Each implementation owner retains context and finishes its complete assigned
path through acceptance, required review and authorized verified main landing.
The coordinator coordinates; it does not replace an owner's unfinished
engineering or introduce an artifact-only finish line.
Follow the concrete path through completion; let working programs reveal design.

## Assign outcomes, not territory

Complete the [mandatory main preflight](../../../AGENTS.md#sessions-and-subagents)
on the coordinator before dispatch and on each independent executor before
implementation. A failed fetch stops a new launch; cached main is not a fallback.
Inspect live claims plus in-flight assignments. Follow
[capability selection](../../../AGENTS.md#the-sample-loop): choose existing
release capabilities or measured iteration-performance/test-environment outcomes.
Compiler advancement witnesses are fact folders under `samples/cli/facts`.
One fact is one folder. The nine places are one row, not a matrix.
`samples/cli/facts/length/` is the filled row that met this bar. The next
witness fills another fact across those places or takes the next uncovered
fact. Do not assign a program from the rest of `samples/`. Copy the worker
goal below into every assignment. Use `python3 tools/sample_queue.py --facts`
for fact failures.
Applications are witnesses, not separate priorities. Do not manufacture mining,
fuzzing or gardening assignments to fill slots.

Use the launcher's `board`/`item` form for capability assignments, with the next
witness and requirement in `suggested_first_slice`; reserve `sample` assignments
for explicit sample scope. Before dispatch record outcome, remaining acceptance,
next requirement and witness, session, revision, current failure,
next action, dependencies and ticket in ignored `build/swarm/<wave>/`.
Consult in-flight assignments too: claims can lag dispatch. Distinct outcomes
may edit the same files in isolated worktrees. Notify peers, reuse repairs where
practical and reconcile at publication. No file fences or waiting for path leases.

A new obstacle stays in the assignment. Do not split its necessary stages or
prerequisite repairs into dependencies on other workers. Keep the worker on its
outcome through checkpoints and complete acceptance and authorized landing.
For a genuine external prerequisite, retain the owner and unfinished status.
Do not turn unfinished work into a list of speculative residuals.

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

## Dispatch and resume

Inspect actual Devin tools and session states. Honor service capacity rather
than assuming historical limits or bypassing caps through resume behavior.
Reuse warm checkouts safely; never run two agents in one checkout.

Each dispatch includes the outcome/acceptance, governing design, expected behavior,
exact worktree/base, publication authority, peers, next action, dependencies,
focused validation, reporting destination and stopping condition. Include the
short assignment to reread after compaction. Reuse the session that already
has the checkout. A landed witness is not completion while the parent
acceptance still fails; return to that outcome in the same session. Compact
after that acceptance, when the next assignment is a different outcome.
When the next failure is still that outcome's, do not compact. Cloud sessions reach the Jev
advisories only when the manifest injects `"secret_ids":
["TYPESAFE_API_KEY"]` ([tools/swarm/README.md](../../../tools/swarm/README.md))
AND the session network policy allows api.typesafe.ai — the ccr2 wave ran
with neither and recorded Jev unavailable
(tools/swarm/waves/ccr2.outcomes.json).

Use `notify_on_response=true` where supported; re-arm on follow-ups because
notifications are one-shot. Inspect existing events before retrying an uncertain
create/resume. Retain actual returned IDs. A settled turn is not a completed
outcome; new work requires completion of the existing assignment.

Retain page cursors; get_messages can return oldest-first. Reconcile handles
after notifications, long commands and before ending a coordinator turn. Drain
posted verdicts rather than asking workers to repeat completed investigation.

## Validate and publish

Workers use focused checks from tools/testing.md. Existing unrelated failures
may be recorded; affected regressions must be resolved. A packaged binary
validates only its pinned source, not subsequent compiler edits. Validate
compiler changes using the changed compiler, not a previously published binary.

Reports name outcome, revision, host, commands/results, ticket, commits, remaining
acceptance and background processes. A source checkpoint is not a passing gate.
Count completed outcomes, not commits or occupied slots.

Use landing.py and linear history. Rebase current main, preserve incoming behavior
and revalidate affected inputs. No direct main push or batch-merge bypass.
Two advisories order that verification: `tools/jev_attribute.py` ranks a
commit window for a failing signature before a bisect, and
`corpus_gate.py --jev` gives the record-safety verdict before any `--record`
(tools/testing.md).
Shared executors use local worktrees. Authorized end-to-end owners on independent
executors publish completed paths through the same queue; do not
invent remote worker branches or force-push workflows.

Artifact transport supports review and recovery, not completion or transfer of
unfinished implementation. The outcome owner retains its context until the
accepted path is landed and verified on remote main.
Board edits replace the remaining frontier, link blockers once and delete
completed work. Never union-merge histories or create one task per diagnostic.

## Continue and drain

After a witness lands, continue the same parent capability's remaining acceptance.
A passing first witness is evidence to reconcile, not a reason to invent a new
failure. Send changed selection policy explicitly to existing workers at safe
checkpoints, retaining current work and user limits.
After capability completion choose another ready outcome within user scope, including a
measured development-health incident. If nothing useful is available, idle.
Missing implementation and path overlap are not external blockers.

For interruption follow [recovery](../dot-swarm/references/recovery.md):
distinguish lost connection from dead processes, preserve unpublished work and
verify remote artifacts before cleanup. Release only owned or confirmed-abandoned
claims. Respect user stops and budgets; reuse/archive sessions only as authorized.
Finish with observed acceptance/publication, parked outcomes, retained work and
actual live processes.
