# Cloud coordination adapter

Before automatic implementation dispatch, apply [common outcome/session reservation](dispatch.md).
The parent reserves through existing claims before admitting/sending the chosen
assignment, retains the exact ticket in this controller's ownership binding,
and uses the actual durable adapter once. Prefer completed reusable Codex
sessions; verify executor/workspace/cache continuity and refresh main/spec/input
context. This controller does not replace cross-runtime outcome reservation.

`cloud.py` adds cloud assignment records to the existing schema-2 recovery
journal. It uses the same `Store`, process locks, atomic writes and revision
checks as `recovery.py`. It has no network client, daemon, provider endpoint,
credentials, permission bypass, worker launcher or resource deleter. The coordinator executes the emitted tool requests through the runtime it actually
has. An authorized advance runs through required tests, commit/push, verified
ancestry and cleanup. Bounded report turns continue automatically under that
authority; bounded reports require no routine parent sign-off or separate reviewer.
Independent review required by the user or assignment remains mandatory.
Apply the [law of completion](../../AGENTS.md#law-of-completion). Durable
checkpoints and bounded tool/report turns preserve the same owner's work;
they never complete an unfinished implementation or transfer its necessary
compiler/library repairs to another agent. Completion requires the entire
assigned acceptance and authorized landing verified on remote main.
`launch.py`'s legacy Devin transport and manifests are unchanged; this controller
is the portable dot/cloud tool adapter, not an alternate claims registry.

Python 3.9+ and Git work on Windows, macOS and Linux. Use `python` on Windows or
`python3` on macOS/Linux. Run the examples from the repository root. Every JSON
input is a reviewed file under ignored `build/swarm/`. Replace identifiers and
SHAs with actual returned values. Reads/results are attestations from the trusted
adapter, not authentication of peers. Evidence hashes and Git ancestry are
machine checked; complete dependency/input closure and honest observation still
require the assigned owners' review.

## Placement and visibility

Keep one recovery ledger per actual executor and generation. On the coordinator
executor initialize it with `recovery.py`; add cloud state with `cloud.py init`.
Each independent cloud computer uses its own recovery journal and build slot.
Use the cloud logical assignment ID as the child lane’s exact `assignment` so
operation receipts and local recovery fences correlate.
The cloud controller maps logical assignments to actual threads, environments
and generations; it never interprets the native seven-agent tree limit as cloud
provisioning capacity. The parent bounds cloud sessions to the batch size and
honors actual capacity errors. A failed/uncertain create is not permission to
launch a replacement. Shared publication still uses the single landing queue.

The implementation executor inspected its exposed tools: native collaboration
and local command tools exist, but **cloud_threads is not exposed here**. The
parent reports `cloud_threads.read` provides recorded task messages/results,
and no direct filesystem access to child computers. Inspect the parent's actual
create/read/follow-up schemas before filling capabilities; do not substitute
native `spawn_agent` for cloud provisioning or guess a lifecycle endpoint.

A local status file is not parent-visible evidence. Use **bounded report turns**:

1. Create a bounded preflight turn, carrying the request's exact assignment
   contract, model/effort, preflight guidance and report deadline. The child ends
   with a structured task result and a confirmed patch/evidence artifact.
2. Parent reads that existing thread's recorded result. Preserve the message or
   result locator, sequence, child progress timestamp and artifact identity.
   Ingest it through a correlated `read` result. A runtime-completed preflight
   is still unfinished work.
3. Parent issues one implementation continuation only after the preflight and
   dependency gates. Bound the turn by a useful checkpoint, not by fake activity.
   Before a long command, child records phase, command, actual process/session
   handle, start/progress timestamps, reason and next gate in its local journal.
   End the turn at a safe checkpoint with a task result when mid-turn messages
   cannot be read reliably. A running build keeps its executor slot; a turn end
   never proves children stopped.
4. Repeat same-assignment continuations only after fresh read/process evidence
   and within retry budget. Deliver acceptance logs/manifests through confirmed
   artifacts or recorded task results. Parent materializes and hashes artifacts
   on its own supported executor; a child path is never presumed accessible.

If read shows no new report, preserve the last genuine progress timestamp and
observe the existing task. Do not invent progress, send keep-alive loops, or
create another task. No mid-turn visibility guarantee is claimed. A runtime
report contains `assignment`, `thread`, `environment`, increasing integer
`sequence`, `evidence` (actual message/result locator), and `checkpoint`.
A read envelopes it as `result.report`. Parent observation time is distinct from
child `checkpoint.progress_utc`; preserve the latter. The schema rejects stale
sequence numbers and mismatched assignment/thread/environment identities.

## Observational bootstrap without a managed pool

A parent that has only recorded thread reads and no managed controller ledger
can initialize a separate `observational-only` controller. Capabilities contain
only the actual `read` tool, schema evidence, and
`reporting:{"mode":"recorded-thread-items","evidence":"actual schema"}`.
It requires no create/follow-up capability, compiler acceptance, claims,
preflight, smoke result, or independent executor identity. If executor/generation
are unreported, use literal `unreported` for those Store envelope fields and
retain that unknown status; this observation journal owns no host or build slot.
It cannot be promoted into a managed pool by adding capabilities or certificates.

`observe-existing` input is exactly `{"id":"observation-ID",
"thread":"ACTUAL_EXISTING_THREAD","evidence":"actual parent mapping source"}`.
This is a binding to an existing reported thread, not session creation, task
admission, or verified host mapping. Environment/generation remain null. Only
read intents are permitted on this observational assignment. A separate explicitly
authorized preflight assignment is described below. Prepare/reserve/save/complete
use the existing
durable step protocol. Read parameters are optional `limit` (1..100, default20)
and `cursor` (actual returned string). For the supplied parent tool contract,
the emitted `runtime_arguments` are `threadId`, `limit`, and optional `cursor`.
The adapter must verify this against its actual exposed schema before execution.

For `cloud_threads.read`, the read envelope's `result` is the unmodified
`structuredContent` object: `threadId`, `parentThreadId`, `latestTurn`
(`id`,`status`,`error`, or null), `items`, and `nextCursor`. Correlation,
receipt evidence and observation time belong in the outer envelope. The
controller checks the exact thread ID and retains the raw object. It does not
parse task text into readiness, process clearance, acknowledgments, pending
mutation proofs, claims, landing credit, or new assignments. A completed
`latestTurn` never establishes settled processes. Status keeps runtime unknown,
executor readiness false, and acceptance null. Ordinary managed controller
admission and final acceptance gates are unchanged.

## One preflight before implementation admission

An explicit parent request may authorize a bounded read-only preflight on an
existing completed thread to establish its unknown executor and process evidence.
Do not require implementation readiness before that inspection. `admit-preflight`
uses the observational controller but creates a separate `preflight-only`
assignment; it never promotes an observation into managed compiler acceptance.

Input fields are `id`, `observed_assignment`, `observed_operation`,
`expected_turn`, `tool`, `schema_evidence`, `authorization_evidence`, and `lane`.
The first supported adapter is the parent-supplied
`cloud_threads.send_message(threadId,model,thinking,prompt)` for
`lane:"frontend resolution"`. The source must be the exact applied read receipt
for the reported completed turn. A completed turn is only a scheduling receipt;
processes, environment and generation stay unknown. Parent authorization is
explicit and preserved. The inspection itself establishes current host/process
facts; this admission does not repurpose an old turn as fresh host readiness.
Known approval denials remain blockers.

Prepare a single follow-up with parameters
`{"model":"gpt-6.1-sol","reasoning":"medium","phase":"preflight"}`,
then reserve it through `dispatch-step`. Its `runtime_arguments` map to actual
`threadId`, `model`, `thinking`, and a generated concise prompt. The prompt binds
operation/execution IDs, reads current main/claims and executor/process/recovery
state, retains an already-authorized outcome or scouts a prospective frontend
defect within the authorized lane and asks for its authored oracle,
controls and next gate. This turn performs no builds, source edits, claim
mutations or publication, and preserves held work/approval fences. Implementation
retains the assignment's review and publication scope; preflight adds no authority
or separate reviewer/sign-off requirement. Historical claim IDs are not live fences.
Implementation is the separately authorized next objective after actual readiness and selected
contract admission. Do not silently expand preflight into a full advance cycle.

One thread receives one preflight assignment and one message intent. Failed or
unknown effects do not permit another message under a new operation or contract;
use supported reads and reconcile instead. A successful enqueue envelope retains
its actual raw result and exact thread, but provides neither child acknowledgment
nor executor readiness. Status waits for the preflight report. Read results still
retain unmodified structured thread items; they never synthesize a preflight
certificate from task prose. Store resources remain empty, identities unknown,
and acceptance null. Ordinary managed admission/acceptance gates are unchanged.

## Controller procedure

```sh
python3 tools/swarm/recovery.py --state build/swarm/recovery/controller.json init --executor ACTUAL_EXECUTOR --generation ACTUAL_GENERATION
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision 0 init --input build/swarm/capabilities.json
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json status
```

`capabilities.json` contains `evidence` for inspected tool schemas, `tools` with
actual names for `create`, `read`, `follow_up`, and optionally `lifecycle` only
when exposed; and `reporting: {"mode":"bounded-turn-results",
"max_turn_seconds":900,"evidence":"actual supported task-result read route"}`.
The interval is a bound in the adapter's assignment, **not a platform timer**.
The helper does not enforce timeouts on remote tools. Model and effort are
explicit `parameters.model: "gpt-6.1-sol"` and `parameters.reasoning: "medium"`;
translate to the exposed schema fields and verify returned metadata. The helper
records the intended selection, never claims it changed an existing session.

Admission input contains `manifest` and `claims`. Obtain claims through actual
`claims.py --timeout-seconds 120 status`; add adapter `observed_utc` and
`evidence` locator to its sanitized JSON. Unavailable/old snapshots are refused.
The status list already contains only live claims; do not supply expired entries.
Record exact existing tickets, owner labels and item/board identities; known
live same-outcome claims are rejected; path overlaps are advisory.

```sh
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION admit --input build/swarm/admission.json
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION checkpoint --input build/swarm/checkpoint.json
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION intent --input build/swarm/create-intent.json
```

Each successful write returns the new revision. `checkpoint.json` is
`{"assignment":"A","checkpoint":{...}}`. Checkpoint fields are `phase`,
`active_command` (null when none), `session` (null when none), `started_utc`,
`observed_utc`, `progress_utc`, `evidence`, `next_gate`, and `reason` for a wait,
blocker or required approval. Phase is one of `preflight`, `implementation`,
`validation`, `publication`, `waiting_dependency`, `required_approval`, `blocked`,
`candidate_ready`, `landed`. Before source work follow cloud-launch.md: exact
checkout/fetched base, authorized repository-local identity, actual execution
approval route, toolchain/doctor/resources, compiler provenance, package-free
native/interpreter smoke, claims and confirmed source backup. No skip-permissions,
new credentials or package authority blanket approvals.

`create-intent.json` is `{"assignment":"A","operation":"create-A-1",
"action":"create","parameters":{"model":"gpt-6.1-sol",
"reasoning":"medium","phase":"preflight"}}`. Normalized parameters accept only `model`, `reasoning`, `phase`, `message`,
`environment`, `lifecycle_action`, `role` and `purpose`; credentials and permission bypass fields
are rejected. The emitted `request` carries the immutable contract,
dependencies, reporting bound, tool name and execution ID. **Only after intent
is durable**, execute the actual tool once. For managed create/follow-up work,
use the generated `request.prompt` verbatim as the runtime prompt; it already
contains the assignment and validated focused spec packet. Translate
parameters to supported tool fields; never forward this envelope as an invented
API schema. Save the sanitized result file before attempting ledger ingestion.

### Required work context

Managed work defaults to semantic. Assignments carry `semantic_contracts`,
`context_repository` (a local checkout at the exact recorded assignment source),
and optionally `canonical_revision` (defaults to the admitted base). Contract
files must exist unchanged between that canonical revision and the source;
the canonical revision must derive from the admitted base or a recorded accepted
main authority. Advancing beyond that authority through `spec-context` performs
one bounded read/fetch of main at the checkout's actual publication destination
and verifies ancestry. Dispatch reuses that accepted snapshot; it does not
poll main or requalify unrelated work. Candidate spec edits cannot nominate
themselves as authority. Select focused passages
using the [qualification mapping](../qualification.md). Genuine non-semantic
work instead records `work_classification:{"kind":"non_semantic","reason":"concrete scope"}`.
No language/task classifier can prove that declaration true; independent review
judges scope and semantic compliance.

For an already admitted assignment, supply `spec-context` with
`{"assignment":"A","context":{"context_repository":"ABSOLUTE_CHECKOUT",
"semantic_contracts":[MAPPINGS],"canonical_revision":"FULL_CANONICAL_SHA"}}`.
This records definitions, not a dispatch. It can also record the explicit
non-semantic classification. Existing journals without either declaration
remain readable, but work preparation refuses until context is configured.

```text
python tools/swarm/cloud.py --state CONTROLLER_JSON --revision CURRENT_REVISION spec-context --input CONTEXT_JSON
python tools/swarm/cloud.py --state CONTROLLER_JSON --revision CURRENT_REVISION prepare-step --input NEXT_TURN_JSON
python tools/swarm/cloud.py --state CONTROLLER_JSON --revision CURRENT_REVISION dispatch-step --input STEP_JSON
```

`NEXT_TURN_JSON` uses the ordinary stable assignment/step/operation/action and
parameters. `role` defaults to implementation for create and continuation for
follow-up; `role:"review"` produces read-only independent-review instructions.
Dispatch returns the exact saved request at `request.request`, including
`work_context` and **`prompt`**. The adapter's runtime caller forwards this
generated prompt to the exposed tool's prompt field rather than reconstructing
or manually pasting a spec packet. Prepare and dispatch both validate source
identity and focused contracts; a changed context refuses dispatch before its
reservation. These steps run no builds or test suites.

Mappings/classification enter the readiness contract; selected contract files
enter its reviewed input closure. Changing a mapping invalidates the relevant
readiness scope. Unrelated main movement does not change pinned observations.
Resume includes the actual passages unless an exact completed acknowledgment
names `context_fingerprint` and `context_retained:true`, and the fresh runtime
read also reports that same fingerprint and retention. A generic task receipt
is insufficient. Missing retention proof reinjects passages; changed excerpts
still fail until the focused mapping is refreshed.

Ordinary reads, observational/preflight-only inspection and status do not need
spec packaging. A settled managed session can receive a fixed status-only turn
using follow-up parameters `{"phase":"preflight","purpose":"status"}`.
It permits no custom `message` or `role`, requests only status, and forbids edits,
builds and publication. Existing lifecycle/recovery gates still apply.
Repository tooling cannot intercept the external direct `send_message` API;
calls that bypass this adapter are outside its enforcement coverage.

### Existing owner without historical controller state

Locate the parent journal first. An owner-side context packet is not a managed
dispatch receipt. `bind-existing` requires actual original create/pool and prior
settlement evidence; do not fabricate those fields to adopt an ongoing legacy
outcome. If that history is unavailable, use the separate one-continuation
bootstrap `legacy_context.py`. It creates no session and grants no managed
rollout credit or final-candidate acceptance.

Prepare input contains `thread`, `binding_evidence` (the parent's actual existing
owner mapping), `authorization_evidence` (continue that same authorized outcome),
`outcome`, `repository`, `owner_packet`, `owner_packet_sha256`, and reported
`owner_worktree_status` (`uncommitted`, `clean`, or `unknown`). Paths identify a
context checkout at the packet's recorded HEAD and the exact materialized owner
packet. That checkout may be the owner worktree or an explicitly identified
parent snapshot; its clean status does not certify the owner's working changes.
The supplied packet must be an implementation/continuation semantic handoff.
Preparation revalidates its passages/blobs/fingerprint and accepted main authority,
then emits the full context with `prior_context_retained:false`. `changed_paths`
still describes committed HEAD only. Preserve uncommitted work and pending owner
decisions; this is context adoption, never new semantic authority.

```text
python tools/swarm/legacy_context.py prepare --state LEGACY_JOURNAL_JSON --input ADOPTION_JSON
python tools/swarm/legacy_context.py reserve --state LEGACY_JOURNAL_JSON --revision 0 --input FRESH_READ_JSON
python tools/swarm/legacy_context.py record-result --state LEGACY_JOURNAL_JSON --revision 1 --input ACTUAL_RESULT_JSON
```

`prepare` returns `inspect_only` plus **`request.prompt`** and
`request.runtime_arguments`; save that output. It does not authorize invocation.
Reserve input is `{"tool":"cloud_threads.read","observed_utc":"ACTUAL_UTC",
"evidence":"ACTUAL_RECEIPT_LOCATOR","structuredContent":ACTUAL_RAW_READ}`.
The raw read must identify the same thread and an error-free completed latest
turn. A fresh read is not process-clearance evidence; the generated prompt tells
the same owner to reconcile its processes/recovery before commands. A parent
without an actual read preserves the prepared journal rather than guessing.
Reserve rechecks source/spec context and durably reserves the sole invocation,
then returns `action:"invoke_once"` and the exact same `request.runtime_arguments`.
For the supplied runtime, forward those arguments to the exposed
`cloud_threads.send_message` once. Verify the actual tool schema first. External
direct calls remain outside repository enforcement.

Record the actual result with `operation`, `execution_id`, `effect`
(`applied`, `not_applied`, `unknown`), `evidence`, and `result`. An applied enqueue
result must report the exact `threadId`. Success means enqueue recorded, not
compiler completion or spec compliance. Reserve cannot replay after a crash,
unknown result or known failure; preserve the journal and reconcile using actual
reads. Do not erase it or create another journal as a retry loophole.
`status --state LEGACY_JOURNAL_JSON` remains read-only. No executor, generation,
original creation, process clearance or previous delivery is invented.

If the owner cannot fast-forward because its repair is uncommitted, load this
tool from a separate checkout of fetched main and point `repository` at the
preserved owner checkout. Installing metadata tooling does not require committing,
rebasing or editing the pending repair. The caller forwards the generated prompt;
it does not require the owner to invent a full controller journal.

### Durable adapter steps

For a restartable coordinator, use `prepare-step` instead of a bare `intent`.
Its JSON is the same intent object plus a stable `step` ID naming the exact
next turn. It saves the operation, immutable contract and operational checkpoint
atomically. Identical preparation returns status; a changed contract under the
same step is refused. `status` includes every saved step and its next action.

Use `dispatch-step` with `{"assignment":"A","step":"preflight-1"}`.
Only its first successful write returns `request.action:"invoke_once"` and the
exact saved request inside `request.request`. Invoke that request's actual
capability once, translating to the schema discovered on this runtime. The
reservation is durable before the request is returned. A crash after reservation,
including before the tool call, requires authoritative reconciliation; it never
permits dispatching the same step again. Once an authoritative read resolves
that exact operation, `complete_reconciled_step` directs `complete-step` to
settle using the retained proof even when the original result was lost. Applied
create/follow-up reconciliation still requires the exact child acknowledgment.
A crash before reservation leaves a prepared step available. Dispatch rechecks current guards and the saved contract.

Save the actual sanitized envelope using `save-step-result`, with input
`{"assignment":"A","step":"preflight-1","result":{...}}`, then use
`complete-step` with the assignment/step object. Saving and ingestion are separate
atomic commits, so restart can ingest saved evidence without executing a tool.
Identical result saves and completed-step replays are idempotent. Read ingestion
permits only its own result-save revision increment; other journal changes or
stale observation time require a fresh read. The original read envelope remains
saved unchanged. Unknown mutation effects and actual denials retain the existing
reconciliation and approval fences. A stale saved read stays preserved while
step status directs a fresh read.

This is a bounded adapter contract, not a live transport integration: one tool
invocation per reserved step, existing retry/continuation bounds, and the recorded
turn limit. The coordinator must consume the returned requests through supported
runtime tools. No remote actuation or automatic daemon is demonstrated by local
fixture tests. Keep exact landing ticket, claim, base, candidate, verification
receipt, publication result and ancestry evidence in the checkpoint's evidence
artifact; a `publication` phase alone never authorizes a retry or proves landing.

### Rolling reuse

`successor` takes `predecessor`, a single-assignment `manifest`, fresh `claims`,
and a full verified `binding` packet in the existing `bind-existing` input shape.
The manifest has `mode:"rolling-successor"`, `size:1`, `previous:null`, a fresh
reviewed outcome and the ordinary acceptance/dependency contract. The predecessor
must have managed landed acceptance, completed cleanup and no user rescue. The
binding must identify the same recorded pool/slot and predecessor, prove current
settlement and process/recovery/resource clearance, and preserve denial history
and protected paths. Admission and binding commit together.

One predecessor admits one successor. Exact replay is idempotent; a different
successor contract is refused. The other slots remain visible, including blocked
ones. Rolling reuse neither increases capacity nor supplies 2/5/10 staged rollout
credit. Ordinary batch admission still refuses unfinished batches. `status`
reports reusable settled slots and their successor links; the coordinator supplies
an actually reviewed next outcome rather than deriving work from `next_gate` text.

Result input has `schema:1`, `operation`, `execution_id`, `assignment`,
`effect: applied|not_applied|unknown`, `evidence`, and `result`. A successful
create's result must contain actual `thread`, `environment`, `generation`.
Bind the child prompt to the emitted operation/execution ID so its first and
final task reports can acknowledge that exact request. A successful enqueue is
not a child acknowledgment. An unreported generation requires an explicitly established incarnation from
the runtime/confirmed assignment, not a guessed hostname. A timeout, missing
reply, disconnect or client error means `unknown` unless authoritative evidence
proves otherwise. A pre-dispatch refusal can be `not_applied`.

```sh
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION ingest --input build/swarm/create-result.json
```

Dispatch results can be ingested at the current revision after other assignments
advance. Exact duplicate results are idempotent; conflicting duplicates fail.
Do **not** invoke create again to retry ingestion. If no reply survived, record
one `read` intent and execute the supported read on the existing identity.
A failed/unavailable read result may be `unknown` or `not_applied`; it clears
current availability but does not become an uncertain mutation. A later fresh
read may recover it without replaying any create/message/build. Read envelopes
additionally need fresh `observed_utc` and `revision` of the
ledger **after that read intent**. Result includes exact mapping,
`runtime: running|idle|completed|stopped|unknown`, `processes_stopped` (including
children, observed on that executor), `recovery` as described below, optional
`preflight`, optional `report`, optional `acknowledgments`, and optional `effects`.
Each acknowledgment is `{ "operation":"EXACT_OPERATION", "execution_id":
"EXACT_EXECUTION_ID", "status":"started|completed", "evidence":"ACTUAL_CHILD_MESSAGE_OR_RESULT" }`.
It must come from that child acknowledging its actual request; tool enqueue
receipts and old completed-turn statuses are insufficient. Until acknowledged,
`status` says `read_reconcile` and another continuation is refused.
`effects` maps pending operation IDs to `execution_id`, `effect`, `evidence`;
an applied create also supplies the mapping. Absence from an incomplete list
proves nothing. No matching authoritative request search means preserve unknown.
For a reconciled create, validate both the observed mapping and the create
proof against live assignments. Retain executor identity supplied by either
source; when both supply it, they must agree. An omitted executor in one source
does not erase or exempt the identity reported by the other.

A late unknown reply cannot undo an independently reconciled applied effect.
Only known `not_applied` permits a fresh create intent; `retry_budget:0..3`
bounds failed mutation retries by action. Successful report continuations have
a separate explicit `max_continuations:0..12` bound; they do not consume retries. Duplicate operation IDs never
redispatch. A follow-up uses explicit `parameters.phase:preflight|implementation`. A bounded
preflight repair turn can continue automatically before preflight passes; it has
no implementation authority. An implementation continuation needs the validated
preflight receipt below, a fresh exact `ownership` receipt, dependency handoffs,
an existing mapping, fresh read, acknowledged prior dispatches and confirmed
stopped child processes plus clear recovery operations/build slot. A running build is observed rather than duplicated.
An actual approval rejection uses `approval_denied:true`, `effect:not_applied`,
`result.action` and `result.error`; it produces `required_approval`. Report the
exact error to the parent and wait. The denial is latched separately from checkpoints, so editing a phase cannot
clear it. `approval` accepts `{ "assignment":"A", "receipt":{ "operation":
"DENIED_OPERATION", "execution_id":"EXACT_EXECUTION_ID", "action":"DENIED_ACTION",
"status":"approved", "route":"ACTUAL_APPROVAL_ROUTE", "evidence":"ACTUAL_APPROVAL_RESULT",
"observed_utc":"FRESH_UTC" } }` only after that actual route approves. This
records approval evidence; it never grants permission or invokes an override.
Record a fresh checkpoint/read before the authorized retry.

`status` distinguishes `read_reconcile`, `waiting_dependency`, `required_approval`,
`blocked`, `offer_create`, `read_runtime`, `observe_running`,
`validate_acceptance_or_continue`, `review_owned_cleanup`, `verified_complete`.
These are next actions for the adapter, not executed commands. Runtime
`completed` never sets `accepted`.

## Validated preflight and automatic two-session procedure

Preflight input is `{ "assignment":"A", "receipt":{...} }` for the `preflight`
command, or the same receipt as `result.preflight` in a correlated read.
The receipt requires `assignment`, actual `thread`, `environment`, `generation`,
`repository:"CathedralOS/Omega"`, immutable original assignment `base`, `source_revision` matching
the active reviewed revision (initially that base), `toolchain`, actual `execution_route`, `evidence`, integer
`doctor_exit:0`, `identity:{"name":"Jarod Castillo",
"email":"43973221+Smotto@users.noreply.github.com","scope":"repository-local"}`.
Adapter evidence must include effective author/committer checks, including
possible environment overrides. `resources` contains measured positive `cpus`,
`memory_bytes`, `disk_bytes`, integer `build_jobs` bounded by the CPU quota.
`smoke` contains actual integer `native_exit:0`, `interpreter_exit:0`, `command`,
normal package `preparation` and `evidence`. `backup` contains
`status:"confirmed-upload"`, actual `file_id`, `version`, `route`, `sha256` and
positive `bytes`. A local archive is refused. This receipt is immutable within a
reviewed source revision; `review-revision` records an audited integration transition
without changing the original assignment base. Other scope changes need a new
assignment/environment generation; replacing the executor needs a new explicit
assignment and fresh preflight. It is a recorded evidence gate, not a request
for human approval.

For the parent-supported create/read/send_message runtime:

1. Inspect actual schemas. Set `capabilities.tools.create` and `.read` to their
   exposed cloud tools, and `.follow_up` to the exposed `send_message` only if
   it supports continuing the existing task (as this runtime does). Declare no
   lifecycle tool when none is exposed. Initialize the controller on its actual
   supported executor and admit the reviewed two-assignment manifest.
2. Record a fresh preflight checkpoint and `create` intent separately for A and
   B, with `parameters.phase:"preflight"`, explicit model/effort and the existing
   environment selector. Invoke each create once with a self-contained bounded
   preflight/report prompt, exact assignment contract and request IDs. Ingest
   each actual result/mapping. A consumer may run bounded preflight while its
   declared dependency is waiting.
3. Read each existing thread. Extract actual child acknowledgments and the
   bounded preflight task result; correlate IDs, save result JSON and ingest at
   the current revision. Materialize confirmed artifacts through the supported
   route; the parent cannot open child-local files. Read that executor's actual
   process and recovery state. Record its exact fresh claim with `ownership`.
   Ingest the validated preflight receipt; never infer it from runtime completed.
4. For each assignment whose preflight, ownership and actual dependency gates
   pass, record a `follow_up` intent with phase `implementation` and invoke
   supported `send_message` once for that assignment. Independent A and B may
   continue on their separate executors. Each continuation carries
   the same authorized advance through tests, commit/push and cleanup, pausing
   only at genuine checkpoints/dependencies/required approval. Ingest enqueue
   result, then read until the child acknowledges this request. Do not enqueue
   another continuation on an old completed status. There is no sign-off wait.
5. Each independent assignment verifies and publishes its own candidate through
   the serialized landing queue. If a declared producer/consumer dependency
   exists, record its exact bilateral handoff and integration-owner protocol
   before continuing the consumer. For the optional trusted-site/digest example,
   B integrates A's preserved commits, revalidates and publishes the combined
   candidate through landing; A need not be individually publishable before
   that handoff. Record final acceptance for each distinct program against the
   landed bytes and exact receipts, rerunning/reusing evidence under the rules
   below.
6. Perform `cleanup-review`, execute authorized owned-resource cleanup, reobserve
   and `accept` each outcome. Continue automatically until both are accepted or
   accurately blocked. Admit five only after both satisfy the same end-to-end
   contract without user rescue. The controller's tests simulate these faults;
   this implementation does not launch the real trial.

## Dependency partition and ownership

A batch manifest has `id`, `size` (2, 5 or 10), `previous` (null for canary),
`publication:"serialized-landing"`, `dependency_review`, `assignments`, and
`dependencies`. Every assignment has `id`, full `base`, `owner`, exact claim `item`, optional
supported `board`, one real
`outcome`, exact `writes`, `reads`, `dependency_review`, `retry_budget`,
`acceptance`, `max_continuations:0..12`, and optional existing `claim_ticket`. Every acceptance requirement
has unique `id`, `kind`, exact `command`, complete `inputs` closure, integer
`exit`, `verdict`; native/interpreter also carry `expected_result` with authored
exit/output oracle. Dispatch must include native, interpreter, scoped tests and
negatives. Expected-red before evidence is required later in final acceptance.

Review the entire producer/consumer chain, generated digests/goldens, fixtures,
board item and shared implementation files before admission. Assign distinct
outcomes, not disjoint files. Path overlap alone neither creates a dependency
nor prevents admission. Review actual producer/consumer dependencies explicitly;
cycles reject. Each dependency has `id`, exact `producer`, `consumer`, `paths`
connecting producer writes to consumer reads, `kind`
(`producer-consumer|digest|golden|shared`) and `verification`. Each outcome keeps
one DRI. Independent outcomes may edit shared files in isolated worktrees and
reconcile incoming changes at landing. Same-outcome takeover requires handoff.
Existing claims-v1 tickets and path estimates remain valid.

After child claims, ingest exact ticket/path/owner evidence through `ownership`:
input `{"assignment":"A","claims":{...fresh snapshot...,"ticket":"ACTUAL_TICKET"}}`.
Handoff input has `dependency`, `producer`, `consumer`, `paths`, full `candidate`,
`producer_ticket`, `consumer_ticket`, `producer_receipt`, `consumer_receipt`,
`verification` matching dispatch, `verification_evidence`, `claim_evidence`,
`ownership:"reconciled"` (legacy `"single-writer"` also accepted), fresh `observed_utc`. Receipts are actual bilateral
messages or confirmed artifacts, not a coordinator's inferred permission. Verify
producer candidate, consumer integration and current claim ownership first.
Handoffs never modify/release claims. Same-outcome takeovers require explicit
handoff. Shared paths alone are not a handoff prerequisite; preserve and
reconcile peer changes. Dependency-gated consumers may create their bounded **preflight-only** turn
before a handoff, so each child can record exact claims and acknowledgment.
Implementation continuations stay waiting until the exact handoff.

## Final acceptance and safe cleanup

Completion auditing follows the actual task, required tests and commit/push.
`accept` records that completed result after verified publication/cleanup; it is
not a new publication approval stage. Reports and schema checks require no
routine parent sign-off. Existing in-task validation and landing guards remain
mandatory.

Before publication, preserve historical evidence and a candidate checkpoint.
Use the existing landing helper; no controller action extends its 180-second
lease. After rebase, always regenerate the exact-SHA landing receipt, even when
ordinary tests are reusable. Verify actual push-destination main by fetch and
ancestry, reconcile uncertain mutation effects, release owned claims/tickets,
and confirm worker/process quiescence. Parent materializes the child's evidence
and makes candidate/main Git objects available on its verification executor.
For separate fetch/push destinations, fetch the actual **push** destination into
`refs/remotes/origin/main` as the landing contract requires; do not use a stale
fetch-only mirror as publication evidence.

Acceptance input is `{"assignment":"A","repository":"LOCAL_VERIFICATION_REPO",
"evidence_root":"LOCAL_MATERIALIZED_EVIDENCE","manifest":{...},
"observation":{...}}`. The helper cannot remotely open a child's worktree.
Manifest schema 1 fields: `assignment`, `outcome`, final `candidate`, reserved
`base`, fetched `remote_main`, `user_rescue:false`, `environment` fingerprint
(host/toolchain/flags/external inputs), `before`, `historical`, `final_evidence`,
`final_receipt`, `receipt_artifact`. Reserved base must descend from assignment
base. `before` contains original assignment `revision`, `outcome`, `command`,
`verdict:"failed"`, integer `exit`, `kind:native|interpreter`,
`failure_kind:compile|run`, matching `environment`, and `artifact`. Its command
must match a dispatched program acceptance command; infrastructure refusal is
not a before-program failure. Every artifact has relative
`path` and `sha256`; files must materialize inside the evidence root.

`final_receipt` is the actual `omega-landing-receipt-v1` JSON from landing.verify
for final candidate/base with `exit_code:0` and complete affected packages.
`receipt_artifact` hashes its exact saved file. Each `final_evidence` row matches
one dispatch acceptance requirement by `id`, `kind`, `command`, `inputs`, `exit`,
`verdict`, with `revision`, `environment`, `artifact`, and positive `count` for
tests/negatives. Native/interpreter carry `program_result` exactly matching the
pre-dispatch authored `expected_result`. Command success alone is insufficient.

Historical rows are inventory and cannot replace a final row silently. Explicit
reuse is a final row's `reuse:{"historical_id":"ID","reason":"JUSTIFICATION"}`.
The source must match command/kind/environment and exact reviewed input closure;
Git blob/tree hashes must agree at historical and final commits. Include all
producer, consumer, fixture, digest, golden, tool and configuration inputs.
Changed inputs refuse reuse. History is retained separately; exact landing
receipt is always fresh for final SHA.

Both runtime read and final observation carry `recovery:{"assignment":"A",
"environment":"ACTUAL_ENVIRONMENT","generation":"ACTUAL_GENERATION",
"unknown_operations":[],"build_slot_clear":true,"evidence":"ACTUAL_EXECUTOR_RECOVERY_JOURNAL"}`.
Read the child journal and process tree; unresolved claim/push/build effects or a
held slot prevent continuation and acceptance. Matching assignment lanes in the
controller’s original recovery journal are checked too. Independent executors
keep independent slots. Final acceptance also checks the handed-off producer
candidate is an ancestor of the consumer’s final candidate.

Observation has fresh `observed_utc`, exact `revision`, `thread`, `environment`,
`generation`, terminal `runtime`, `landed_head`, `main`, `remote_evidence`,
`claims:[]`, `landing_tickets:[]`, `registry_evidence`, `processes_stopped:true`,
`workspace:{"head":"FINAL_SHA","clean":true,"primary":false,"owned":true,
"evidence":"ACTUAL_GIT_AND_RESOURCE_INVENTORY"}`. Checkpoint must be `landed`.
The adapter verifies inventory **before** removing anything:

```sh
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION cleanup-review --input build/swarm/acceptance.json
```

This automatically validates all final gates and records `review_owned_cleanup`, but leaves
acceptance unset. This name means an ownership/evidence inspection, not human sign-off. Cleanup
remains an authorized adapter operation under recovery.md:
reobserve owned clean non-primary worktree/branch and stopped processes; remove
only this lane's verified-landed resources. Preserve shared caches, unrelated
work, blocked WIP and uncertain operations. Record actual cleanup result and
locator as `cleanup:"completed"`, `cleanup_evidence`, retaining pre-removal
inventory. Supply a fresh observation/current revision, then:

```sh
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION accept --input build/swarm/acceptance.json
```

Only successful managed acceptance produces `verified_complete`. Explicitly reviewed
imported completion produces `verified_import` and cannot promote a staged batch. Neither cleanup review
nor accept deletes anything. Duplicate identical acceptance is idempotent;
wrong SHA, blocked checkpoint, unresolved effects, stale observation, missing
logs, incomplete cleanup or historical-only evidence fail.

## Bind a settled existing session

`bind-existing` attaches one genuinely existing settled session to an already
admitted new outcome. It performs no host action, launches nothing, creates no
synthetic create receipt and does not complete the previous outcome. It records
a separate applied local operation in the same recovery journal, retaining the
original create, host observation, prior settlement and denial history. An
identical operation/receipt replay is idempotent; conflicting replays fail.

```sh
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION bind-existing --input build/swarm/codex30-20261002/iteration1/bind-slot3.json
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json status
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION bind-existing --input build/swarm/codex30-20261002/iteration1/bind-slot28.json
```

Replace `CURRENT_REVISION` with the journal's returned current integer after
each write. Admit exactly two reviewed new outcomes first, using the existing
2/5/10 rules, fresh actual claims and serialized landing. Use distinct new
logical assignment IDs; do not overwrite an old outcome or its acceptance.
Binding is available only before the new row has any operations or mapping.
No unfinished controller batch may be hidden by adopting its sessions into a
new batch. A known prior controller assignment must already have verified landed
acceptance, no user rescue and completed cleanup. An externally coordinated
prior assignment may be explicitly released unfinished after preservation;
that disposition confers no acceptance or promotion credit.

The input object contains `assignment`, unique `operation`, `evidence_root`,
`receipt_artifact` and `receipt`. `receipt_artifact` is a hashed JSON file exactly
equal to `receipt`, whose fields are:

- `schema:1`, exact new `assignment`, and `source:{"pool":"RECORDED_POOL_ID",
  "slot":3}` (28 for the other slot).
- `creation`: actual original create `tool` matching inspected capabilities,
  actual original `receipt` locator, exact `thread`, optional actual
  `environment` and `generation` when originally exposed, and hashed `artifact`.
  Retrieve the original pool entry and tool result; do not issue a create intent
  or claim an adoption was a successful create.
- `observation`: actual supported read `tool` matching capabilities, actual
  `receipt` locator, fresh `observed_utc`, exact `thread`, `environment`,
  `generation`, actual `executor`, `identity_evidence`, `prior_assignment`,
  `runtime` (`idle`, `completed`, `stopped`), `processes_stopped:true`,
  `active_assignments:[]`, hashed `artifact`, and `recovery` with the prior
  `assignment`, matching environment/generation, `unknown_operations:[]`,
  `build_slot_clear:true` and actual `evidence`.
- `prior`: exact previous `assignment`, different `outcome`, `disposition`
  (`released` or `accepted`), actual settled `checkpoint` with no active command
  or session, `approval_required` (exact retained denial or null),
  `protected_paths`, hashed `artifact`, and `resources` containing fresh
  `observed_utc`, actual `evidence`, successful actual `claims_state`
  (`claimed` or `available`), successful actual `landing_state` from `landing.py
  status`, `claims:[]`, `landing_tickets:[]`, and
  nonempty `workspaces` inventory. Empty lists refer to this prior assignment's
  ownership, checked against actual registry receipts, not all global claims.
  Each workspace has `path`, `owned:true`, actual `evidence` and `state`
  (`clean`, `removed`, `preserved`). A preserved workspace needs explicit
  protected repository paths and a confirmed-upload `backup` with `file_id`,
  `version`, `route`, `sha256`, positive `bytes` and `status:confirmed-upload`.

Each creation/observation/prior artifact is a JSON packet with `normalized`
exactly equal to that receipt object minus its `artifact` field, and nonempty
`raw_result` holding recovered actual tool results, child reports, pool records
and registry/workspace evidence used to derive it. Hashes verify byte identity
and equality with the normalized receipt. The trusted coordinator adapter still
must inspect actual schemas, interpret the raw result honestly and correlate
host and child observations; this is not authentication of peers or automatic
proof of session identity. Missing executor/incarnation metadata or an
unavailable process, custody or registry observation is a named prerequisite.
Do not infer an environment from a thread ID, describe a running scout as idle,
or replace absent metadata with `unknown` or an invented identity. If original
create metadata exposed environment/generation, current values must match.

The transition rejects active/unconfirmed operations, active unrelated work,
stale or mismatched observations, duplicate live pool slots, threads, executor
identities, environments or outcomes. Binding does not clear a denial. Prior
protected/denied paths, including earlier bindings of that thread, remain fenced
against a renamed outcome. Existing records and paused backups stay intact.
No resource is deleted by this command.

After binding, record a fresh new-assignment `checkpoint` for the actual parent
binding event (`phase:preflight`, no active command/session, next gate establishing
the new child recovery lane). Then emit one bounded same-thread `follow_up` with `phase:preflight`
to establish the new logical child recovery lane. Only this preflight transition
can use the cleared prior recovery assignment; implementation and acceptance
require the new assignment identity. Obtain the actual child acknowledgment,
fresh read, new preflight, ownership and readiness before implementation intent.
Previously recovered reproduction evidence can be reviewed/reused when its
complete inputs and actual compiler provenance remain unchanged. Count selection
and reproduction time before binding and admission, without counting it twice.
Binding itself is session reuse, not provisioned capacity or a measured speedup.

For slots 3 and 28, the executable order is admit → genuine settled host reads
and recovered binding packets → bind each → fresh binding checkpoint → bounded preflight continuation →
read/ingest acknowledgment and new recovery lane → preflight → ownership →
readiness → fresh runtime read → implementation intent. Each continuation goes
through the existing durable intent/result protocol; final landing, cleanup review
and acceptance are unchanged. Wait for the requested wave audit before any live
implementation admission. A report blocked before compiler checking by cache
filesystem access or unavailable claims is not a reproduced compiler defect.

## Scouting and implementation readiness

`admit` reserves a bounded preflight contract; it does not admit implementation.
Before the first implementation `follow_up`, record a separate `readiness`
receipt after actual preflight, fresh exact ownership and any incoming bilateral
handoffs. Preflight continuations remain available for scouting blocked work.
Existing journals remain readable; an unfinished unscouted assignment cannot
start implementation. Final oracle/SHA/cleanup guards and staged 2/5/10 sizes remain; managed acceptance
also requires the saved readiness/dispatch audit trail described below. Readiness never completes a task or clears a denial latch.

```sh
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION readiness --input build/swarm/readiness.json
```

Input wraps `assignment`, `repository` (an accessible inspected Git checkout),
`evidence_root` (locally recovered artifact directory) and `receipt`. The receipt
has `schema:1`, unique attempt `id`, exact `assignment` and `environment`, full
`source_revision` for the reproduced observation, full `current_revision` equal
to the inspected checkout HEAD, `started_utc`, `observed_utc`, `cache` (`warm`,
`cold`, `unknown`), `complexity`, and explicit `assumptions`. Use genuine scouting
start time, including selection, reproduction and review before admission.
Each attempt covers its own work interval; do not repeatedly count the same
reproduction as fresh work. Duplicate identical IDs are idempotent. Preserve
unscheduled scouting evidence too; omit neither rejected candidates nor time
spent outside implementation when comparing the trial.

`scope` equals the assignment's immutable `base`, active `source_revision`, `outcome`, `writes`, `reads`,
`acceptance`, `dependency_review` and all declared incoming/outgoing
`dependencies`. `known_dependencies` lists those reviewed edge IDs; an extra
known prerequisite or missing reviewed edge records a dependency blocker.
Declare the edge and its actual producer/consumer scope rather than overriding
that result. `inputs` lists reviewed Git paths covering declared writes, reads
and acceptance inputs; parent trees cover descendants, including additions and
deletions. Include semantic prerequisites, authored oracle, compiler/toolchain
configuration and digests/goldens that affect this reproduction. A hashed
`review_artifact: {"path":"scope-review.json","sha256":"ACTUAL_SHA256"}`
contains the validity rationale, concrete producer/consumer review and remaining
assumptions. This is evidence-backed engineering judgment, **not proof that the
controller discovered every semantic dependency**. Inspect the recovered review
and command logs before recording the receipt; hashes establish byte identity,
not the truth of the adapter's observation.

`before` contains exact `revision`, `environment`, native/interpreter `kind`,
execution `command` matching acceptance, integer command `exit`, authored
`expected_result`, `failure_kind` (`compile` or `run`), and hashed `artifact`.
For a run include actual `program_result`. `classification` is
`valid-should-work`, `invalid-source`, `pending-approval`, or `compile-only`.
Only a compiler rejection of reviewed valid should-work source, or a failing
execution against the authored oracle, can become ready. Green, invalid,
unapproved and compile-only observations record `not-reproduced`. A native
compile that emitted bytes is not an execution observation. Missing
implementation is the repair hypothesis; do not classify it as an external
blocker. An actual external `blocker` has `kind` (`live-owner`,
`product-decision`, `executor-unavailable`, `denied-action`, `dependency`), exact
`reason` and actual `evidence`; it may omit `before` when no reproduction ran.

Receipts and ownership use the existing 120-second observation freshness bound.
Refreshing judgment need not rerun expensive reproduction when complete reviewed
Git input objects are unchanged. The controller checks source/base/current
objects, hashes the artifacts, and rechecks relevant checkout inputs/artifacts
at implementation intent. Unrelated commits and unrelated claim renewals are
neutral; changed relevant inputs, reviewed scope, same-outcome ownership or
prerequisites make readiness stale. Missing provenance fails closed. Stop and
revalidate newly discovered prerequisites. Once the implementation request is
confirmed applied for this exact scope, ordinary validation continuations retain
the existing recovery, fresh claims, dependency and denial gates; the repair's
own source edits do not require finding another red program.

`status` exposes `readiness` (`unscouted`, `ready`, `stale`, `blocked`,
`not-reproduced`), `scouting_attempts` and summed `scouting_seconds`; the journal
retains each attempt's reason, evidence, timestamps, cache and complexity. A
blocked/green result suggests `scout_or_revalidate`, never verified completion.
Metrics are reported worker work intervals, not a platform clock or statistical
speedup. Measure end-to-end time including scouting, implementation, validation,
landing and cleanup; disclose overlapping work, warm-cache and task-complexity
differences. Failed malformed receipt transactions do not write journal events;
retain their tool errors alongside trial evidence.

For the first live canary, the parent uses supported exposed create/read/follow-up
operations for exactly two independent currently red authored outcomes, each with
its own executor/preflight/claims and complete reviewed inputs. Record every
scouting attempt (including green or blocked selections), then emit implementation
intents only for both ready legs. Reconcile unknown operations on the same threads;
reuse each warm executor, one build per executor, and serialize official landing.
Both must independently satisfy unchanged exact-candidate acceptance and cleanup
with `user_rescue:false` before any 5-session promotion. This canary demonstrates
correctness and autonomy, not a statistical speedup. This increment launches or
stops no worker and performs no live canary.

## Reviewed integration revisions

An assignment's `base` remains the original before-program evidence anchor.
Its active `source_revision` initially equals that base. A changed producer input
cannot be treated as neutral reuse against the original source. After an actual
bilateral handoff, record a `review-revision` transition before reproducing on the
integrated source. This also covers reviewed relevant main changes; it does not
change ownership, oracle or dependency declarations.

```sh
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION review-revision --input build/swarm/revision-review.json
```

Input has `assignment`, accessible `repository`, recovered `evidence_root` and
`receipt`. The receipt has `schema:1`, unique `id`, exact `assignment`, correlated
`thread`/`environment`/`generation`, fresh `observed_utc`, concrete `reason`,
remaining `assumptions`, full `from_revision`/`to_revision`, current pre-transition
`scope` from `readiness_contract`, complete reviewed `inputs`, exact
`changed_paths`, and `dependencies:{"EDGE_ID":"HANDED_OFF_CANDIDATE_SHA"}` for
all incoming edges. A hashed `artifact` contains the actual diff, input closure,
semantic/prerequisite review and source/toolchain implications. Source paths and
Git objects are machine checked; semantic completeness remains reviewed judgment.

The target must be the inspected clean checkout HEAD and descend from the
current reviewed revision. Every declared incoming candidate must be handed off
and integrated into that target. Mismatched source, identity, scope, diff, dirty
inputs, unknown/unacknowledged operations, active processes, stale ownership or
an approval latch block the transition. The prior preflight is retained in
`preflight_history`, and original scouting/before records remain intact. The
transition clears current preflight and invalidates old readiness through the
new source contract. Obtain a new actual preflight (`base` remains original;
`source_revision` equals the reviewed target), then reproduce and record new
readiness there. No prior implementation turn can supply managed credit for a
new source contract. Final landing must include the reviewed source; the final
manifest still preserves the actual original-base before failure.

## Completion credit and meaningful failures

`accept` and ordinary `cleanup-review` require correlated applied create/binding
provenance plus an applied implementation continuation with completed child
acknowledgment and its saved `readiness_gate`. The gate must match the reviewed
source/scope, current preflight and handoffs, recorded ready scouting event and
a meaningful before failure. The receipt was fresh at intent; it need not still
be fresh after a long legitimate implementation. Readiness is copied into the
dispatch record before actuation, not retroactively asserted at completion.
An empty operation history or a final landed label cannot earn managed credit.

For an independently completed outcome outside that managed history, use the
explicit `import-completion` command with the same full final acceptance input
and `import_receipt`. The review has `schema:1`, exact `assignment`, exact
`candidate`, fresh `observed_utc`, actual archival `source`, concrete `reason`
and hashed `artifact`. Its packet uses the binding format (`normalized` equals
the review minus `artifact`, with actual recovered evidence in `raw_result`).
An import requires the same known mapping, preflight, meaningful original
before failure, oracle, final-SHA/build/test/negative receipts, actual remote
ancestry, reconciled ownership, stopped processes and verified cleanup. It
never substitutes for absent session binding provenance. `cleanup-review` may
carry that explicit `import_receipt` before imported resource cleanup; ordinary
`accept` cannot silently choose import mode from an extra input field.

```sh
python3 tools/swarm/cloud.py --state build/swarm/recovery/controller.json --revision CURRENT_REVISION import-completion --input build/swarm/imported-acceptance.json
```

Results carry `credit:managed` or `credit:imported`. Imported or unlabeled legacy
completion can settle old work without promoting a managed batch. Legacy
unlabeled records report `audit_completion_credit`; review them explicitly,
never assume missing history was managed. Ordinary stage promotion still needs
all outcomes verified with managed credit, no user rescue and complete cleanup.
A successful compile (exit 0) labeled failed is refused. A runtime wrong-output
failure may have command exit 0, but needs actual program exit/output fields
contradicting the authored oracle and a hashed observation artifact. Invalid
source, pending approval and compile-only classifications cannot be a meaningful
before failure. Correct oracle fields plus extra metadata are not a failure.

## One-session validation and adapter prerequisites

A one-session trial uses an explicit manifest `mode:"non-promoting-pilot"`,
`size:1`, `previous:null`, one real reviewed assignment, `dependencies:[]` and
the usual claim/ownership/acceptance/publication fields. Use ordinary `admit`;
size 1 without that mode is rejected. This pilot creates no permission to launch
a replacement worker. It must meet the same genuine existing-session binding,
preflight, readiness, managed implementation and final acceptance contracts.
Its batch can become `settled:true` but always has `complete:false`; it cannot
be the predecessor for 5 or 10. A later genuine size-2 staged canary still starts
with `previous:null`. Alternatively run only one leg of a genuinely reviewed
size-2 manifest and leave the other unstarted, without claiming batch completion.

The currently inspected root read tool returns recorded thread/turn/items, not
host incarnation or process guarantees. Root first recovers the genuine original
create/pool receipt and reads the exact existing thread/task through its exposed
schema. After the fixed controller is verified, an authorized read-only inspection on the
same settled thread can collect a child attestation before any controller binding
or managed dispatch. Use only the root's actual supported follow-up/read schemas:
correlate its actual root send receipt, returned turn ID and subsequent read
result; report actual executor/environment/incarnation evidence, source/compiler
provenance, measured CPU/memory/disk/jobs, actual commands/process handles and
exit/settlement inventory, child recovery journal and build slot, registry/landing
ownership, preserved source and confirmed remote backup IDs/version/hash/bytes.
Include genuine start/progress/observed timestamps and scouting/build/blocked
intervals; do not turn missing timing into a speedup claim.

That is a child observation via recorded tools, not a provider host guarantee.
Do not substitute hostname, kernel boot ID, turn ID or environment configuration
ID for an unavailable controller generation. An ended turn does not prove process
settlement. A local archive does not prove upload. Original create provenance is
not reconstructible from IDs. When these facts or supported artifact retrieval
are missing, report the exact prerequisite; no live binding, dispatch or claimed
pilot outcome follows. This tooling increment executes no live pilot.

## Staged trial gates

The parent selects and reproduces two **currently red real programs**, with
distinct real outcomes, one DRI per outcome and eligible executors. Implementation
and test paths may overlap; admission checks outcome ownership, not file fences.
Review the complete producer/consumer closure, including required digest and
golden updates, before admission. Independent assignments declare
`"dependencies":[]`; admission does not require a shared-file change or a
fabricated handoff. Already-green smoke fixtures are preflight evidence, not
these two advances.

When a real dependency requires coordination, declare its exact producer,
consumer, paths and verification. For example, if A changes a trusted site and
B is its designated digest integration owner, B reads A's site and declares
the actual digest edge A → B. B waits for A's exact verified candidate and
bilateral handoff before implementation continuation, independently revalidates
the citing entries and records the digest; both may run bounded preflight
turns first. A live peer's claim to the same digest outcome still requires a
handoff or contribution under its integration DRI; shared digest paths alone
do not block a distinct outcome. Select
independent eligible work when available rather than inventing a digest change
solely to test the controller. Deterministic dependency tests exercise handoff
and same-outcome rejection without inventing dependencies from shared paths.
This example applies only when separate producer and digest integration owners
are actually assigned. A coherent sample-driven compiler outcome keeps its needed
repairs, digest/golden updates and acceptance under one integration DRI; do not
split it into stage workers or manufacture a handoff to fit this example.

Bound manifests to exactly 2, then exactly 5, then exactly 10 assignments.
Admit 5 only with `previous` naming an accepted 2-session batch; admit 10 only
with `previous` naming an accepted 5-session batch. Every predecessor outcome
must satisfy the same landed/cleanup contract with `user_rescue:false`. A runtime
turn count, partial candidate or local receipt cannot satisfy promotion. Record
user intervention honestly; rescue prevents scale and requires a new canary.
Independent executors may build concurrently using their own journals; builds
sharing an executor serialize in its existing recovery slot. Publication always
serializes through landing. Retry budgets, bounded turn/report contracts and
actual capacity limits apply at every stage. This implementation runs **no live
batch**, provisions no sessions and injects faults only in tests.

```sh
python3 tools/tests/test_swarm_cloud.py -v
python3 tools/tests/test_swarm_recovery.py -v
python3 tools/tests/test_swarm.py -v
```
