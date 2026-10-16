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
Inspect live claims plus in-flight assignments. Choose existing
release capabilities or measured iteration-performance/test-environment outcomes.
Applications are witnesses, not separate priorities. Do not manufacture mining,
fuzzing or gardening assignments to fill slots.

Before dispatch record outcome, acceptance, session, revision, current failure,
next action, dependencies and ticket in ignored `build/swarm/<wave>/`.
Consult in-flight assignments too: claims can lag dispatch. Distinct outcomes
may edit the same files in isolated worktrees. Notify peers, reuse repairs where
practical and reconcile at publication. No file fences or waiting for path leases.

A new obstacle stays in the assignment. Do not split its necessary stages or
prerequisite repairs into dependencies on other workers. Keep the worker on its
outcome through checkpoints and complete acceptance and authorized landing.
For a genuine external prerequisite, retain the owner and unfinished status.
Do not turn unfinished work into a list of speculative residuals.

## Dispatch and resume

Inspect actual Devin tools and session states. Honor service capacity rather
than assuming historical limits or bypassing caps through resume behavior.
Reuse warm checkouts safely; never run two agents in one checkout.

Each dispatch includes the outcome/acceptance, governing design, expected behavior,
exact worktree/base, publication authority, peers, next action, dependencies,
focused validation, reporting destination and stopping condition. Include the
short assignment to reread after compaction. Reuse the session that already
has the checkout. Compact after a landed repair when the next assignment is
a different outcome. When the next run branches from that repair, do not
compact. Cloud sessions reach the Jev
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

After completion choose another ready outcome within user scope, including a
measured development-health incident. If nothing useful is available, idle.
Missing implementation and path overlap are not external blockers.

For interruption follow [recovery](../dot-swarm/references/recovery.md):
distinguish lost connection from dead processes, preserve unpublished work and
verify remote artifacts before cleanup. Release only owned or confirmed-abandoned
claims. Respect user stops and budgets; reuse/archive sessions only as authorized.
Finish with observed acceptance/publication, parked outcomes, retained work and
actual live processes.
