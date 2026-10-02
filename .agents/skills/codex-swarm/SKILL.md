---
name: codex-swarm
description: Coordinate compiler advancement through reusable Codex cloud sessions, one independent executor per worker. Use for a Codex swarm or a pool of Codex sessions. Use dot-swarm for explicitly requested native agents sharing one executor, and devin-swarm for Devin sessions.
---

# Codex session swarm

Run the requested compiler workload through a persistent pool of Codex sessions.
Each worker keeps its own cloud executor, checkout, build cache and outcome chain.
Reuse a settled session for its next dependency-ready assignment. The coordinator
selects programs, reconciles ownership, collects evidence and integrates results.

Read [AGENTS.md](../../../AGENTS.md), [advance](../advance/SKILL.md),
[testing](../../../tools/testing.md), [claims](../../../tools/claims.md), and
[landing](../../../tools/landing.md). Follow the existing
[cloud launch preflight](../dot-swarm/references/cloud-launch.md) on each actual
executor, and the [cloud coordination contract](../../../tools/swarm/cloud.md)
for durable assignments, bounded reports, dependencies and acceptance.

Creating this skill alone does not launch sessions or advance a compiler.
Honor the user's requested worker count and stopping condition. When the user
requests thirty sessions, record thirty as the target; report the real running
count and actual missing prerequisites instead of silently substituting six
native workers or thirty queued assignments.

## Inspect the available session tools

Discover the current Codex session schemas before dispatch. Depending on the
host, session operations may be exposed as Codex thread tools or cloud task
tools. Use only tools actually returned by discovery; these names describe
capabilities, not guaranteed endpoints:

| Capability | Possible Codex app tool |
| --- | --- |
| Inspect existing sessions | `list_threads`, `read_thread` |
| Select an accessible project | `list_projects` |
| Create a requested session | `create_thread` |
| Resume the same session | `send_message_to_thread` |
| Observe completion or required input | `wait_threads` |

Retain each returned thread/task ID, host ID, environment ID, cursor and artifact
locator. Confirm the actual model and reasoning metadata when reported. Use the
repository's model defaults through supported launch fields; a prompt does not
set runtime configuration. Never guess a project, environment or session ID.

A thread ID does not prove an independent machine. Verify the returned executor
identity and the worker's preflight report before counting it as an independent
worker. Sessions sharing an executor share its resource and build budget. The
native `collaboration.spawn_agent` tree limit describes shared-executor agents;
it does not describe the capacity of independent Codex sessions. Do not apply
that limit to a cloud pool or multiply it with nested native agents.

Read-only session tools do not establish launch or resume capability. If create
or follow-up tools are unavailable, report the exact missing operation and keep
the manifest pending. Do not replace independent sessions with native agents,
Devin, detached shell agents, guessed HTTP routes or credential workarounds.
An explicitly requested native swarm uses [dot-swarm](../dot-swarm/SKILL.md).

## Select work and record the pool

1. Preserve the starting checkout and fetch current main. Record the exact
   revision; an unavailable fetch leaves a named local snapshot, not fresh main.
2. Inspect live claims and the coordinator's in-flight assignments. Select real
   failing programs from the requested boards under `advance`. Recheck board
   evidence against current code before dispatching an already-landed repair.
3. Partition implementation and validation paths. Each assignment carries the
   program, authored exit/output, board acceptance, governing designs, exact
   ownership, dependencies and acceptance commands. Never reserve a whole board.
4. Keep `build/swarm/<wave>/pool.json` and the cloud recovery journal. Record each
   logical slot's actual session/host/executor IDs, generation, assignment, source
   base, claim ticket, status, genuine progress time, result cursor, artifacts,
   pending operations and next dependency. Record intended ownership before
   dispatch, then attach the returned handle. A pending create is not running.
5. Inspect actual service capacity errors and executor CPU/memory/disk limits.
   Honor capacity responses without treating the requested count as a receipt.
   Keep one Rust build active per executor; independent machines have separate
   build slots. Do not launch compiler builds on the coordinator to warm every
   worker's machine.

Use the executable cloud controller's supported admission sizes and dependency
gates when using that controller. A larger requested pool does not authorize
inventing an unsupported schema or bypassing its gates. Record the remaining
requested sessions as pending and report the particular admission prerequisite.

## Launch and reuse sessions

Create each new worker with a bounded preflight assignment. Carry publication
authorization already given by the user, model/effort, exact repository and
program, ownership, known blockers, report destination and stopping condition.
The worker verifies its own environment, source revision, identity, custody,
toolchain, resources and actual smoke run before expensive implementation.

Creating Codex chats requires an explicit user request for sessions/chats; a
request for a Codex session swarm supplies that scope. Apply current app rules
for returned thread receipts and emit required created-thread directives using
actual returned IDs. Never claim a session was launched without its receipt.

Reconcile an existing session before reuse. Follow-up goes to that same returned
session and host, retaining its warm checkout and executor when the runtime
supports this. A new assignment message states that the previous assignment is
complete, names the next program, updates ownership/dependencies and carries the
new acceptance. Synchronize its lane with current main before editing. Do not
create another session merely because a worker ended its turn or became idle.

When create or follow-up has an uncertain result, preserve the operation identity
and read the existing task before retrying. Do not duplicate a worker under the
same logical slot. A capacity error is not permission for a resume-cap loophole.
Keep blocked source checkpoints and route the session onto independent ready
work only within the user's wave scope.

## Observe, integrate and drain

Use compact `wait_threads` snapshots when available, with returned host IDs and
cursors. Bound each wait to preserve user communication; batch at most the
tool's supported target count. Read detailed results when a session completes,
needs action, or returns a meaningful checkpoint. Do not send keep-alive nudges
or re-derive a verdict a worker already posted.

Workers report actual revision, host, program before/after, commands, exits,
claim, commits, artifacts, outstanding processes and smallest residual. Collect
confirmed artifacts or published lanes through the supported channel; a path on
another machine is not accessible evidence. Verify artifact content and commit
ancestry before integration. Keep independent proof, custody and trust checks.

Drain each result promptly: inspect evidence and diff, reconcile claims, record
the outcome, integrate a verified candidate through `landing.py`, and follow up
with the next ready leg on the same session. Do not hold an idle session behind
a coordinator gate unless its next program truly depends on that result.
Update only board items justified by actual acceptance. A program that still
fails has a named dependency; diagnostic reports do not close compiler tasks.

On cancellation or a change of execution mode, stop dispatching, request safe
checkpoints, preserve dirty source and reconcile actual processes before cleanup.
Do not terminate or archive reusable sessions unless requested. Release only
owned claims; remove only clean, verified-landed worktrees. Report confirmed
running/settled sessions separately from requested capacity, native/interpreter
acceptance, publication state and remaining blockers. Never describe a saved
workflow or an agent-count check as a compiler improvement.
