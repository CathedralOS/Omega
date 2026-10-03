# Cloud coordination adapter

`cloud.py` adds cloud assignment records to the existing schema-2 recovery
journal. It uses the same `Store`, process locks, atomic writes and revision
checks as `recovery.py`. It has no network client, daemon, provider endpoint,
credentials, permission bypass, worker launcher or resource deleter. The coordinator executes the emitted tool requests through the runtime it actually
has. An authorized advance runs through required tests, commit/push, verified
ancestry and cleanup. Bounded report turns continue automatically under that
authority; no routine parent sign-off or separate reviewer is required.
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
live item conflicts and path overlaps are rejected.

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
`environment` and `lifecycle_action`; credentials and permission bypass fields
are rejected. The emitted `request` carries the immutable contract,
dependencies, reporting bound, tool name and execution ID. **Only after intent
is durable**, execute the actual tool once. Construct its prompt from the
contract plus governing repository links and existing owned walls. Translate
parameters to supported tool fields; never forward this envelope as an invented
API schema. Save the sanitized result file before attempting ledger ingestion.

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
`repository:"CathedralOS/Omega"`, assignment `base`, `source_revision` matching
that base, `toolchain`, actual `execution_route`, `evidence`, integer
`doctor_exit:0`, `identity:{"name":"Jarod Castillo",
"email":"43973221+Smotto@users.noreply.github.com","scope":"repository-local"}`.
Adapter evidence must include effective author/committer checks, including
possible environment overrides. `resources` contains measured positive `cpus`,
`memory_bytes`, `disk_bytes`, integer `build_jobs` bounded by the CPU quota.
`smoke` contains actual integer `native_exit:0`, `interpreter_exit:0`, `command`,
normal package `preparation` and `evidence`. `backup` contains
`status:"confirmed-upload"`, actual `file_id`, `version`, `route`, `sha256` and
positive `bytes`. A local archive is refused. This receipt is immutable for an
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
board item and shared implementation files before admission. Manifest overlap
checks are not a source-code dependency discovery engine. Undeclared write/read
edges and cycles fail. Each dependency has `id`, exact `producer`, `consumer`,
`paths` connecting producer writes to consumer reads, `kind`
(`producer-consumer|digest|golden|shared`) and `verification`. Shared files have
**one writer**: designate an integration owner, or partition into new serial
legs after a recorded handoff. Two assignments writing one file are refused,
even with a handoff or request to “finish.” The global claims-v1 directory fence
remains in force; this helper neither uses nor grants `--allow-overlap`.

After child claims, ingest exact ticket/path/owner evidence through `ownership`:
input `{"assignment":"A","claims":{...fresh snapshot...,"ticket":"ACTUAL_TICKET"}}`.
Handoff input has `dependency`, `producer`, `consumer`, `paths`, full `candidate`,
`producer_ticket`, `consumer_ticket`, `producer_receipt`, `consumer_receipt`,
`verification` matching dispatch, `verification_evidence`, `claim_evidence`,
`ownership:"single-writer"`, fresh `observed_utc`. Receipts are actual bilateral
messages or confirmed artifacts, not a coordinator's inferred permission. Verify
producer candidate, consumer integration and current claim ownership first.
Handoffs never modify/release claims. Known external peer conflicts require that
peer's coordination and a new reviewed partition; a generic finish instruction
is insufficient. Dependency-gated consumers may create their bounded **preflight-only** turn
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

Only successful acceptance produces `verified_complete`. Neither cleanup review
nor accept deletes anything. Duplicate identical acceptance is idempotent;
wrong SHA, blocked checkpoint, unresolved effects, stale observation, missing
logs, incomplete cleanup or historical-only evidence fail.

## Scouting and implementation readiness

`admit` reserves a bounded preflight contract; it does not admit implementation.
Before the first implementation `follow_up`, record a separate `readiness`
receipt after actual preflight, fresh exact ownership and any incoming bilateral
handoffs. Preflight continuations remain available for scouting blocked work.
Existing journals remain readable; an unfinished unscouted assignment cannot
start implementation. Final exact-candidate acceptance and 2/5/10 promotion
are unchanged. Readiness never completes a task or clears a denial latch.

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

`scope` equals the assignment's `base`, `outcome`, `writes`, `reads`,
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
neutral; changed relevant inputs, reviewed scope, overlapping ownership or
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

## Staged trial gates

The parent selects and reproduces two **currently red real programs**, with
distinct real outcomes and disjoint, eligible implementation/test ownership.
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
turns first. A live peer's digest ownership still blocks that scope. Select
independent eligible work when available rather than inventing a digest change
solely to test the controller. Deterministic dependency tests exercise handoff
and overlap rejection without requiring a shared dependency in every live batch.

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
