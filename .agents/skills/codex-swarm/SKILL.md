---
name: codex-swarm
description: Coordinate compiler advancement through reusable Codex cloud sessions, one independent executor per worker, including compact source uploads to a single batch-integration session. Use for a Codex swarm or a pool of Codex sessions. Use dot-swarm for explicitly requested native agents sharing one executor, devin-swarm for Devin sessions, and cursor-swarm for Cursor cloud subagents.
---

# Codex session swarm

Before automatic implementation assignment, follow the common
[reserve-before-dispatch procedure](../../../tools/swarm/dispatch.md). Review a
current-main spec/fact gap, reserve outcome and chosen session atomically through
existing claims, and arm one durable send attempt before invoking the actual
runtime. Shared compiler paths remain advisory. Never auto-recover an expired
Zac/Devin claim: inspect actual liveness/progress; record confirmed stop evidence
or named user takeover authority. Adopt the coordinator's reserved ticket rather
than registering a second claim. Unknown sends retain their ticket and require
runtime reconciliation before retrying.

Prefer available completed sessions in the same saved Omega environment.
Compare actual runtime/workspace/cache probes, preserve existing target/MBX
caches and branch progress, and report unknown or reprovisioned continuity
honestly. Reuse still fetches current main, creates an isolated lane, refreshes
spec context and validates affected inputs before native PR/queue submission.
The repository emits/reserves requests; the parent must execute the discovered
session API once and preserve its actual receipt. It cannot enforce arbitrary
external tool calls. Do not start a fresh session solely because a turn ended.

Run the requested compiler workload through a persistent pool of Codex sessions.
Each worker keeps its own cloud executor, checkout, build cache and outcome chain.
Reuse a settled session for its next dependency-ready assignment. The coordinator
selects fact witnesses under `samples/cli/facts`, reconciles ownership and verifies complete results.

Each session qualifies coherently on one checkpoint with the executable
[qualification runner](../../../tools/qualification.md). When actual conflicts
or changed assumptions require integration, assess changed dependencies, reuse
justified unchanged observations and rerun affected source checks. Preserve exact
producer artifacts and required source review; GitHub owns integration admission.

Compiler work dispatch requires the focused canonical passages by default.
Configure `semantic_contracts` and a checkout at the recorded source through
the controller's `spec-context` command before `prepare-step`. Dispatch returns
the validated packet inside `request.request.prompt`; forward that generated
prompt to the actual runtime. Resume reinjects passages unless an exact child
receipt and fresh read establish retained context. Genuine non-semantic work
requires an explicit classification and reason; status reads remain usable.
Follow [required work context](../../../tools/swarm/cloud.md#required-work-context).
External direct message APIs are outside repository enforcement.
When an existing owner has no historical controller journal, locate the parent's
journal first. If unavailable, use the explicit
[legacy continuation bootstrap](../../../tools/swarm/cloud.md#existing-owner-without-historical-controller-state).
It binds the existing thread and freshly validated packet without inventing
create provenance, managed credit or prior delivery; preserve uncommitted work.

Apply the [law of completion](../../../AGENTS.md#law-of-completion).
Each implementation owner retains context through full acceptance, required
review and authorized verified main landing, including its necessary compiler,
library and prerequisite repairs. Do not split these stages into dependencies
on other workers or substitute artifact handoffs for completion. When the user
selects compact batch integration below, the named integration session owns final
integration and landing; workers retain semantic context for findings and report
`source_ready`, not completion or landed acceptance.
Follow the concrete path through completion; let working programs reveal design.

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

When the parent has only a recorded thread-read schema and no managed ledger,
use the controller's separate `observational-only` bootstrap to inspect an
actual existing thread. Bind it through `observe-existing`, then prepare and
reserve a read using the durable adapter. Preserve the raw `structuredContent`
receipt. Leave unreported executor/environment/generation unknown, and do not
invent compiler acceptance or preflight to fit a managed assignment. This mode
cannot dispatch work or acquire managed rollout/completion credit. See
[observational bootstrap](../../../tools/swarm/cloud.md#observational-bootstrap-without-a-managed-pool).

## Select work and record the pool

1. Complete the [mandatory main preflight](../../../AGENTS.md#sessions-and-subagents)
   before dispatch. Fetch successfully, preserve the starting checkout, record
   the exact main commit and reread updated instructions. A failed fetch stops
   the new launch; cached main is not a fallback. Each independent executor
   repeats this preflight before implementation.
2. Inspect live claims and the coordinator's in-flight assignments. Follow
   [capability selection](../../../AGENTS.md#the-sample-loop) under `advance`.
   Compiler advancement witnesses are fact folders under `samples/cli/facts`.
   One fact is one folder. The nine places are one row, not a matrix.
   `samples/cli/facts/length/` is the filled row that met this bar. The next
   witness fills another fact across those places or takes the next uncovered
   fact. Do not assign a program from the rest of `samples/`. Copy the worker
   goal below into every assignment. Use `python3 tools/sample_queue.py
   --facts` for fact failures.
   Record the parent task, remaining acceptance and next required witness in
   the existing assignment context; a controller's program-level success does
   not establish parent-task closure. Recheck board
   evidence against current code before dispatching an already-landed repair.
3. Assign distinct outcomes, allowing shared implementation paths. Each assignment carries the
   program, authored exit/output, board acceptance, governing designs, exact
   session, advisory paths, dependencies and acceptance commands. Never reserve a whole board.
4. Keep `build/swarm/<wave>/pool.json` and the cloud recovery journal. Record each
   logical slot's actual session/host/executor IDs, generation, assignment, source
   base, claim ticket, status, genuine progress time, result cursor, artifacts,
   pending operations and next dependency. Record intended ownership before
   dispatch, then attach the returned handle. A pending create is not running.
5. Inspect actual service capacity errors and executor CPU/memory/disk limits.
   Honor capacity responses without treating the requested count as a receipt.
   Keep one Rust build active per executor; independent machines have separate
   build slots. Do not launch compiler builds on the coordinator to warm every
   worker's machine. Each session batches its own machine's chain — the whole
   local build/test sequence inside one hold, releasing it before GitHub waits
   ([build slots](../../../tools/build_slot.md#one-hold-for-a-whole-chain));
   build pools never span sessions and a session cannot upload a build to
   another's pool, so cross-machine coordination stays with the claims
   registry and the serialized landing queue. In compact batch mode, build the
   combined candidate on the integration executor; transfer source, never worker
   build pools or cached producer identities.

The executable cloud controller currently checks compiler-program acceptance;
use ordinary advance and measured acceptance for development-health assignments,
not fabricated native/interpreter results to fit that schema.
Use the executable cloud controller's supported admission sizes and dependency
gates when using that controller. A larger requested pool does not authorize
inventing an unsupported schema or bypassing its gates. Record the remaining
requested sessions as pending and report the particular admission prerequisite.

## Compact source intake and batch integration

Use this mode when the user requests workers to upload source into one existing
integration session. Record the integration owner and batch stopping condition
before dispatch or changing execution mode. Workers do not independently publish
overlapping candidates; reconcile existing claims, leases and uncertain writes at
safe checkpoints first. Preserve original worker worktrees and dirty state.
This mode changes orchestration ownership, not language or acceptance rules.

1. **Export source, not a machine.** Each worker inventories every authored
   worktree, including older dirty ones. Package `committed.patch` (obtainable
   BASE to HEAD), `staged.patch` (HEAD to index), `unstaged.patch` (index to
   working tree), including empty layers, with binary/full-index diff and
   external diff/text conversion disabled. Include authored untracked and
   explicitly identified ignored source needed to continue, retaining paths,
   deletions, executable modes and symlinks without following external links.
   A manifest binds session/environment/worktree/branch, BASE/HEAD provenance,
   layers, file sizes/hashes, landed status and omissions. Verify the base is
   obtainable from repository history or an identified shared base package.
   Verify restoration in isolation against captured bytes, index and modes;
   detect concurrent changes. No full repository/history, linked `.git`, build
   cache, binaries, nested bulk archives or qualification-state dumps. Preserve
   unique evidence separately and reference its confirmed identity. Prefer a
   small source ZIP below 50 MiB; explain genuine large source members.
2. **Receive locally.** Use the currently documented artifact service workflow
   to materialize into the integration executor. Confirm readable local bytes,
   version and hashes; a worker's cloud path is not a local path. Keep original
   packages immutable. Reconcile uncertain transfers before retrying; never
   bypass access errors or change authentication. Record missing source rather
   than replacing it with an old review snapshot or assuming empty dirty layers.
3. **Inventory before selecting.** Keep every worktree's committed, staged,
   unstaged and new-file mappings distinct. Filter committed work by fetched-main
   ancestry and reviewed source equivalence for rebased commits. An ancestor HEAD
   does not clear dirty/new-file layers; a missing commit object does not prove
   source absent. Deduplicate identical payload bytes by hash while retaining all
   provenance and worktree mappings. Restore on the exact obtainable base in an
   isolated location. Restoration reconstructs source/index state, not original
   commit identity, compiler correctness or permission to reuse captured locks.
4. **Bound the batch.** Select compatible, dependency-related candidates and
   record their exact layers, producer/consumer dependencies, witnesses, oracles,
   affected closure and exclusions. Intake may collect many lanes; it never
   overlays every patch blindly. Integrate candidates sequentially in a separate
   checkout from freshly fetched main. Resolve textual and semantic conflicts
   deliberately, preserving both intents; ask the owner about real ambiguities.
   Shared paths alone are neither incompatibility nor proof of independence.
5. **Validate the combined closure.** Build the optimized producer for the
   combined candidate on this executor once, then run the union of required
   affected checks and fixed positive/negative engine oracles. Use
   [change-scoped validation](../../../tools/testing.md#validation-scope) and
   [qualification](../../../tools/qualification.md); retain exact producer,
   inputs, environment, logs and applicable historical observations. Worker
   acceptance or a successful patch application does not accept the batch.
   Required independent review covers the exact integrated candidate. Do not
   replay redundant per-worker integration pipelines or claim foreign execution.
6. **Isolate failures without losing work.** Keep a candidate inclusion ledger
   and checkpoints. On a failure, use focused comparisons or smaller candidate
   subsets to locate the changed dependency/interaction; preserve all originals
   and report deferred slices. Do not suppress checks or rewrite fixed oracles.
   After an actual conflict resolution, prerequisite or inclusion/source change, reassess
   inputs and rerun affected/final checks, reusing only justified evidence.
7. **Land only when authorized.** Batch intake and combined validation are not
   a queue bypass. Submit the validated batch as a PR through tools/landing.md;
   GitHub checks integration and squash-merges one PR into one main commit.
   Do not repeat author review/build merely for a new queue SHA. Resolve actual
   conflicts or failures and retain required source review. Confirm MERGED,
   the submitted head, and the published commit in fetched main before `landed`, and
   record acceptance per outcome without closing broader tasks. Source-only or
   planning authorization stops before integration/publication as requested.

## The worker's goal

Copy this into every worker prompt. In compact batch mode, explicitly replace
the worker-publication instructions with source export and `source_ready`; name
the integration owner, keep workers available for findings, and assign final
acceptance/publication to that owner. An upload is not a landed outcome:

> Read the assigned capability's full acceptance and identify the requirement
> this witness establishes. Apply [requirements and evidence](../../../AGENTS.md#requirements-and-evidence):
> name the owning requirement, link the positive witnesses and fact-sensitive
> reject twins in the fact README, and state the acceptance demonstrated and
> what remains open. The nine places test transport; account for establishment,
> identity and invalidation, composition and soundness where applicable. A filled
> row closes only the acceptance its evidence establishes.
> Follow [execution evidence](../../../tools/testing.md#execution-evidence-and-replay-inputs),
> [main-change validation](../../../tools/testing.md#main-change-validation) and any
> required [independent review](../../../tools/testing.md#independent-semantic-review);
> preserve historical identities and finish review fixes in the same assignment.
> Compiler advancement witnesses are fact folders
> under `samples/cli/facts`. One fact is one folder. The nine places are one
> row, not a matrix: local, field, parameter, call result, match join, const
> name, another state, another machine, and another target body. Do not select
> a program from the rest of `samples/`. `samples/cli/facts/length/` is the
> filled row that met this bar. A fact folder is done only when the same fact
> survives all nine places. One sentence names the fact and the operation, and
> every place copies that sentence. Changing the fact, the bound, or the
> operation so a place passes is a different fact and does not count. When the
> failure is a requirement the producing layer dropped, the sentence names the
> requirements that layer records on the value it emits, and the consuming
> operation uses that recorded list. A repair that recognizes one production
> and rebuilds one requirement leaves the fact open. The next production of
> that layer stays in this folder. A separate fact folder for that production
> does not count. A red
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

## Launch and reuse sessions

Create each new worker with a bounded preflight assignment. Carry publication
authorization already given by the user, model/effort, exact repository and
program, ownership, known blockers, report destination and stopping condition.
The worker verifies its own environment, source revision, identity, custody,
toolchain, resources and actual smoke run before expensive implementation.
Cloud sessions reach the Jev advisories only when the manifest injects
`"secret_ids": ["TYPESAFE_API_KEY"]`
([tools/swarm/README.md](../../../tools/swarm/README.md)) AND the session
network policy allows api.typesafe.ai — the ccr2 wave ran with neither and
recorded Jev unavailable (tools/swarm/waves/ccr2.outcomes.json).

Creating Codex chats requires an explicit user request for sessions/chats; a
request for a Codex session swarm supplies that scope. Apply current app rules
for returned thread receipts and emit required created-thread directives using
actual returned IDs. Never claim a session was launched without its receipt.

Reconcile an existing session before reuse. Follow-up goes to that same returned
session and host, retaining its warm checkout and executor when the runtime
supports this. Reuse is the default: a new session spends a machine and the
tokens required to rebuild the investigation. A landed witness is not
completion while the parent acceptance still fails; return to that outcome
in the same session. Compact after that acceptance, when the next assignment
is a different outcome, and send that session the claim note rather than
the investigation trace. When the next failure is still that outcome's, do
not compact. A new assignment message states that the
previous assignment is complete, names the next program, updates
ownership/dependencies and carries the new acceptance. Keep a continuation's fixed base and warm producers. A new independent assignment
gets one initial main preflight; main movement alone never refreshes an active lane. Do not create another session merely
because a worker ended its turn or became idle.

When create or follow-up has an uncertain result, preserve the operation identity
and read the existing task before retrying. Do not duplicate a worker under the
same logical slot. A capacity error is not permission for a resume-cap loophole.
A blocked session keeps its source checkpoint and continues every feasible
part of its outcome. A genuine external gate leaves it unfinished; do not move
the session away from its outcome.

Use the controller's durable step adapter for continuation: `prepare-step`
records the exact operation and checkpoint, `dispatch-step` reserves its one
supported tool invocation, `save-step-result` commits the actual envelope, and
`complete-step` ingests it. On restart, consume step status; never repeat a
reserved invocation when its effects are uncertain. A saved stale read calls for
another authoritative read, with its original evidence retained. See the
[adapter contract](../../../tools/swarm/cloud.md#durable-adapter-steps).

Once a bound slot has managed landed acceptance and completed cleanup, use
`successor` with a reviewed one-outcome manifest and a fresh verified binding
packet for the same pool/slot. This lets that slot continue while a peer remains
blocked, retaining the peer's checkpoint. It does not increase capacity or satisfy
staged rollout gates. Preserve original denial and protected-path history.

When an existing thread's completed-turn receipt is available but its executor
or process evidence is missing, explicitly authorize one `admit-preflight`
inspection assignment. Reserve its one actual supported follow-up through the
durable step adapter. The read-only preflight obtains those facts before any
implementation admission; do not demand implementation readiness to send that
inspection. Its generated prompt selects a prospective program and stops before
builds, source edits, claims or publication. An enqueue receipt grants neither
child acknowledgment nor readiness; collect the actual preflight report, then
admit the selected contract under ordinary advance gates.

The controller emits requests; actual execution still requires the supported
runtime capabilities discovered by the coordinator.

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

In compact batch mode, drain results into the intake ledger above; do not start
an independent worker landing loop. Otherwise drain each result promptly:
inspect evidence and diff, reconcile claims, record
the outcome, verify the owner's complete path through `landing.py`, and follow up
with the next necessary step of the owned outcome on the same session.
Continue the same parent task through successive complete landed witnesses;
use the controller's normal successor protocol for program-level assignments.
Choose another capability only under the canonical selection rules and user
scope. Send changed selection instructions explicitly to existing workers at
safe checkpoints without discarding current work. Path overlap is
not a blocker; reconcile changes rather than pivoting to peripheral work. Do not hold an idle session behind
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
