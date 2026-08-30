# Design Brief: Task Runtime And Lifecycle

Settled at the architectural level. The core claim and outcome spellings are
live; runtime-provider integration and the remaining library surface are still
under implementation. Chapter 18 is the user-facing authority. This brief
records the mechanical model so the implementation does not recreate an
`async machine`/`Future<T>` split, implicit detach, or a mandatory pool
abstraction. Direct calls use the settled `suspend` and `block`
acknowledgements. Those markers do not change this task model.

## Direction

Concurrent activation is an admitted service over ordinary machines:

```omega
runtime.start<Worker::run>(move job)
runtime.try_start<Worker::run>(move job)
```

`Worker::run` remains an ordinary named machine. Calling it directly executes
it in the caller. Supplying it as a compile-time machine argument to a task
runtime asks an admitted provider to execute a distinct activation.

`TaskRuntime` is the working name for an `omega::core` boundary requirement or
capability surface, not a new declaration kind. Its operations contribute the
runtime service's ordinary service reach and operational ceilings and are
admitted like other providers. A package may wrap that capability with pools or
policy without changing the normalized task contract.

A platform profile/provider plan may select a default task runtime at the
application root, and tests or components may inject a different provider for
the slots they own. Selection convenience does not make runtime authority
ambient: code that starts or controls tasks receives or owns the relevant
capability.

There is no `async machine` species, `Future<T>` transformation, bare `spawn`
block, implicit fire-and-forget, or privileged supervisor/task-group construct.
`suspend` and `block` concern direct-call audibility, not task creation or
return-type transformation.

## Custody, storage, and claim are separate

Starting a task establishes three relationships that must not be conflated:

1. The **runtime provider** takes operational custody of the activation and
   arranges execution, parking, waking, and cancellation according to its
   pinned contract.
2. A **storage owner** holds the activation's physical state. This may be a
   caller-provisioned Arena pool, an OS/runtime provider, or a remote executor.
   The owner supplies one fixed, nonmoving stack for the settled local
   representation; inline completion may shorten its lifetime but does not
   erase the provision.
3. The owner of **`Task<T>`** holds the linear lifecycle claim: the right and
   obligation to observe, request cancellation of, transfer, or terminally
   settle that activation.

`Task<T>` therefore does not universally contain or borrow the task stack. It
is normally a small provider/activation identity plus sealed lifecycle
authority. A provider may embed an already-completed outcome as an
optimization, but completion never silently settles the linear claim.

All physical storage still has an accountable owner: an Arena-issued
Allocation, a provider under its admitted resource contract, or the platform
under a boundary contract. Arena-backed Allocation is the in-model bounded
case, not the definition of all task storage.

## Machine target elaboration

Omega already has compile-time machine-symbol parameters. Task start uses that
surface rather than inventing runtime function values or capture inference:

```omega
runtime.try_start<Worker::run>(move job)
```

The `<machine M>` substitution is monomorphized. The compiler derives a static
activation description containing at least:

- normalized machine-contract and entry identities;
- argument and terminal-outcome layouts;
- a `StackPlan<M>` derived from whole-call-graph WCSU, target alignment, and
  entry/calling-plan overhead;
- canonical suspension crossings and the carry obligations induced by values
  live at each crossing;
- any activation-wide CPU/thread preservation obligation that the selected
  scheduler must establish;
- cancellation and suspension behavior needed by the selected start operation;
  and
- the target calling/entry plan.

The provider receives a generated descriptor plus the invocation's moved
arguments. The source expression is not an implicit `as`: qualification is
representation-identical, while activation planning creates a distinct
compiler artifact. The concise `start(M, args)` form may be transparent future
sugar, but the current surface uses the already-honest `start<M>(args)` spelling.

The source truth is deliberately smaller than the current
`omega-task-plans` prototype. One new activation receives one fixed, nonmoving
stack sized from WCSU and retains that stack while parked. Direct suspension
does not negotiate or allocate a second continuation buffer. The activation
descriptor therefore asks for a `StackLease` satisfying `StackPlan<M>`, not for
a runtime-authored continuation-capacity record.

Compiler elaboration remains live for concrete `TaskRuntime::start<M>` and
`try_start<M>` specializations and emits `05_task_activations.json`. The
artifact now carries a fixed-stack `StackPlan`, canonical suspension-crossing
identities, and demand-driven CPU/thread preservation obligations; the
superseded continuation-demand and `SafePoints | Asynchronous` runtime-join
fields are gone. It is an Omega-owned post-check sidecar, not a field of
`CheckedTrees`: target layout, calling-plan, stack, and selected-runtime state
must not enter Psi checked semantics. Its current byte count is still the local
machine/park-frontier layout bridge. The provider-independent stack planner can
now validate exact local-frame summaries and seal the maximum aligned live
chain across an acyclic same-stack call graph. Sequential sibling calls share
capacity, opaque same-stack leaves require an explicit admitted contribution,
and missing edges, residual cycles, unreachable summaries, invalid alignment,
and arithmetic overflow reject. The opaque-leaf carrier is sealed behind an
admission gate: it exact-matches the selected provider plan's domain-separated
commitment and authored requirement identity, retains the historical compact
plan value only as a report coordinate, and retains a nonzero independent
receipt plus validated bytes/alignment. Its compact report identity and a
separate domain-separated contribution commitment include the strong plan
commitment and all of that evidence; later WCSU projection retains the strong
contribution commitment.
Call-graph consumers therefore cannot construct a trusted contribution from a
byte count directly. Compiler collection of those summaries, binding the
composition evidence into the activation `StackPlan`, and stack reservation
remain the fixed-stack lowering rung below.
External-root ordinary and entry-epoch stack compositions follow the same
rule: each result retains the exact nesting relation, per-root inputs, and
admitted realization evidence. Their compact FNV values are report/cache
coordinates only and never replace structural replay at admission.
`05_carry_manifest.json` remains useful
because it names each suspension crossing and its typed live-value/storage
frontier; tools consume that checked artifact rather than reinterpret source.

`TaskRuntime` is now an ordinary boundary trait. Every concrete activation
fact also retains the exact selected provider-plan identity and exact authored
`start` or `try_start` requirement identity. Missing selection, duplicate slot
selection, requirement drift, and provider narrowing of the published static
machine contract reject. This is static provider/operation binding, not a
fabricated runtime-instance or per-invocation receipt.

The activation's historical specialization FNV is retained only as
`specialization_report_fingerprint`. Provider planning also derives a
domain-separated SHA-256 `TaskSpecializationCommitment` from the exact checked
TaskRuntime specialization: normalized requirement identity, selected
operation, package-qualified declaration/type ownership, the exact target and
entry signature including parameter modes, and the target machine-contract
commitment. Runtime receipt validation carries that commitment—not the compact
report coordinate—into the invocation binding, so a compact-equal
specialization substitution cannot activate or alias the original runtime
child.

The provider-independent selection gate is also live in `omega-task-plans`.
It consumes one exact checked-conformance or admission-receipt identity for
each demanded CPU/host-thread preservation axis, rejects missing or mismatched
evidence, and folds the validated executor selection into the task lifecycle
claim and dependency record. The dynamic seam is now structural too: one
provider invocation receipt must match the Omega-owned activation fact's exact
selected runtime, provider plan, `start`/`try_start` requirement and operation,
and activation-plan identity before the lifecycle ledger accepts it. The
receipt binds the runtime instance and preservation evidence, and invocation
and receipt identities are single-use within that instance. This is
deliberately not a generalized runtime behavior/supply record or source fact;
routed establishment of the source `Task<T>` value remains later TR3–TR8 work.

Architectural preemption does not itself create a semantic crossing. A runtime
may stop and restore opaque register/stack state without changing the
activation's semantic circumstances. Cancellation delivery, migration,
replacement progress, and other structured changes occur only at declared
semantic suspension points or under an explicitly held pin/restriction
contract. If a target may otherwise migrate an executing activation at an
arbitrary instruction, it must establish an activation-wide CPU/thread
preservation claim or reject an activation whose possible live values demand
one; the language does not publish a generic preemption-granularity mode.

## Start is transactional

There are two ordinary operations:

- `start<M>` requires its capacity/admission obligations to be discharged and
  returns `Task<T>` directly.
- `try_start<M>` handles genuinely dynamic admission and returns an ordinary
  sum.

Conceptually:

```omega
transition runtime.try_start<Worker::run>(move job) {
    Started(task) -> keep(move task)
    Rejected(job, reason) -> recover(move job, reason)
}
```

Failure is ownership-transactional. A rejected start returns every moved
argument and caller-supplied reservation, or proves that another named owner
accepted them. No argument, linear obligation, or storage lease disappears
into a failed provider call. Static provider/plan incompatibility should fail
at validation or admission rather than becoming an avoidable runtime case;
runtime outcomes cover capacity and environmental failure that genuinely
remain dynamic.

## Task lifecycle

`Task<T>` is `[linear]`. Establishing one creates one lifecycle obligation.
The obligation may be:

- terminally settled by `finish()` or another declared terminal operation;
- transferred into another owner, including an ordinary supervisor value; or
- preserved in a path-sensitive data case for later work.

It may not be copied, overwritten, dropped at ordinary scope exit, or abandoned
on one control-flow arm. `request_cancel()` requests a state transition and
retains the claim; it does not prove that execution stopped or release the
claim. A combined terminal stop operation may request cancellation and wait,
but its contract must expose that it may suspend or fail.

`finish()` consumes the claim and returns an ordinary outcome sum. The
task-produced `T` remains responsible for application-level recoverable
failure; the outer task outcome distinguishes lifecycle events such as normal
return, cancellation, and provider failure. `Trap`/`Abort` routes remain in the
machine/provider `crashes` contract rather than being fabricated as a returned
value.

Conditional ownership uses an ordinary sum:

```omega
data WorkerState {
    case Idle;
    case Running(task: Task<WorkResult>);
}
```

Storing `Task<T>` in another data value transfers the obligation into that
value. Transferring it to `supervisor.adopt(move task)` changes the owner; it
does not detach the activation into ownerless space.

## Provider and storage lifetime

A task claim records provider provenance. A claim backed by borrowed or
partitioned storage cannot outlive that storage, and a local runtime/pool
cannot close while dependent claims remain. The source type should remain
`Task<T>` where provenance inference is unambiguous; this origin is permission
state, not nominal result-type identity.

The implementation shape is deliberately left to the permission/resource
algebra. Plausible representations include an owned child lease, a provenance
edge that pins a provider resource, or a storage reservation transferred into
the activation and returned at settlement. The invariant is fixed even though
the carrier is not: close/reclaim must prove that every child claim and lease
has been reconciled.

The normalized provider-side accounting carrier is now live in
`omega-task-plans`. One ledger belongs to one admitted runtime instance.
Accepting an activation records the exact activation admission plus either a
persistent `{storage owner, lease era}` edge or an admitted inline-completion
fact before it yields a non-clonable lifecycle claim. Cancellation validates
the claim without removing that record. Terminal settlement consumes the exact
claim and releases its recorded storage relationship; a failed cross-instance
settlement returns the claim. Runtime close and storage reclamation fail while
a matching record remains live. Activation and storage-lease identities are
single-use within the instance, so recycling storage requires a fresh lease
era rather than replaying an old provenance edge.

This is normalized provider accounting, not a second source-visible task or
lease type. Connecting it to `Task<T>` awaits the ordinary selected
task-runtime realization plus routed establishment and admitted
boundary-machine evidence for the runtime's ordinary authority value; those
implementation dependencies are tracked separately.

## Storage policies are provider/library choices

Before a pending `Task<T>` is returned, the provider has accepted custody and
established compatible storage. That storage may contain:

- moved arguments and captures;
- the activation's fixed stack and live call state;
- a resume state/program location;
- scheduler, wake, and cancellation metadata; and
- a terminal outcome retained until settlement.

For the settled local representation, WCSU derives `StackPlan<M>` and the
provider transfers a matching fixed, nonmoving `StackLease` into the
activation. A park retains that same lease. A remote provider provisions an
equivalent stack in its own execution domain. An inline provider may run the
distinct activation immediately on its provisioned stack and return an
already-complete, still-unsettled `Task<T>`.

A future stackless representation remains possible, but it is a different
pre-lowering activation plan. A machine supported under both representations
is lowered twice; a replaceable runtime cannot reinterpret one lowered
activation between them. No source/runtime contract advertises a generic
continuation representation.

The provider validates stack layout and availability plus any demanded
CPU/thread preservation. Address stability is not an admitted runtime promise:
it follows from the nonmoving stack lease. `start` requires a reservation or
proof of availability; `try_start` may return the moved arguments and
reservation when dynamic capacity is unavailable.

### Arena-backed reference model

`ArenaTaskPool` is the standard bounded-storage reference package and the
likely Cathedral default, not a language primitive:

```text
Arena -> ArenaTaskPool -> bounded activation leases
```

The Arena supplies bounded Allocations and capacity authority. The pool
adds slot/arena layout, free-capacity accounting, and runtime integration.
Provisioning fixes a maximum; start and settlement still perform dynamic
accounting. A static proof or owned reservation may make start infallible;
shared dynamic capacity uses `try_start`/`try_reserve`.

An explicit reservation remains a real linear resource under interference. A
fact such as `available >= 1` is sufficient only when the caller also owns the
permission that prevents another starter from spending that capacity.

## Supervisors and mailboxes are libraries

A supervisor is ordinary application data and policy over task claims. It may
own child handles, failure/event endpoints, restart policy, and optionally a
pool. It neither becomes the task runtime nor necessarily owns child frames.
Basic task completion does not require a supervisor or mailbox: `Task<T>` can
reach its provider-held outcome directly.

A bounded mailbox is likewise a queue record over owned or provider-held
storage. Its backing storage, sending endpoints, and receiving endpoint may
have different owners. When messages may contain linear values, task death and
mailbox closure must preserve a surviving owner for every undelivered payload.

Arena-backed pools/mailboxes and fixed-layout supervisors form a useful
bounded reference profile. Hosted OS allocation, remote execution, and inline
execution remain valid providers when their admitted contracts say so.

## Borrowing and sharing

The conservative path is moved owned arguments, shared immutable values, and
explicit synchronized capabilities. Starting a task that retains `&mut self`
while the parent continues using `self` is rejected by ordinary exclusivity.
Code should move independent work, split state into provably disjoint places,
or communicate through a mailbox/synchronized capability.

The current rejection-first subset admits no loan merely because its carrier is
live across a park. Widening it requires evidence for storage lifetime and
pinning, aliasing, ordinary cancellation-result paths, and address stability on
the exact `SuspensionCrossingId`. Borrow/wait-cycle detection is a valuable
later theorem but not a prerequisite for the conservative task model.

Carry policy itself is independent of that loan subset. Ordinary data derives a
compiler-normalized four-axis policy from its fields and explicit type-wide
contract. Accepted resource claims originate strict and result contracts may
add the positive per-claim permissions described in chapter 7. Checked claims
derive from inherited provenance. At each crossing, canonical place liveness
determines which policies constrain the transition.

Suspension is enforced locally against possible suspension and cannot be
narrowed away by runtime selection. Runtime admission is demand-driven rather
than a universal behavior-supply lattice:

```text
portable activation
    -> no migration obligation

activation that may retain SameCpu
    -> selected runtime must establish CPU preservation

activation that may retain SameThread
    -> selected runtime must establish host-thread preservation
```

Checked runtimes derive those conformances from their scheduler construction.
Opaque hosted providers may require an ordinary admission receipt. Missing
evidence fails closed, and a receipt authorizes reliance on a provider claim
without changing its behavior. The selected provider identity and exact
preservation evidence travel with the activation and `Task<T>` lifecycle
claim; no freely constructed opaque runtime value can borrow another provider's
admission.

Cancellation is likewise an operation/conformance, not a boolean field in a
runtime behavior record. Since the core `Task<T>` surface exposes
`request_cancel`, its selected runtime must implement the corresponding
contract. Inline completion is a property of the selected `start` operation,
not activation storage. Stack capacity is an owned/reserved resource. With
those facts in their proper homes, `TaskRuntimeContract`,
`RuntimeBehaviorContract`, and the generalized
`ActivationDemand <= RuntimeSupply` join have no surviving semantic role.

The `omega-task-plans` Rust crate no longer implements that retired join:
continuation capacity, preemption granularity, continuation movement, and
inline-completion runtime fields have been removed. Its lifecycle ledger is
now explicitly downstream of an already selected runtime and exact activation
plan rather than pretending to perform provider admission. Provider selection,
routed establishment, transactional start, stack leases and WCSU-backed
provisioning, and source-level ledger connection remain ordinary implementation
work under the settled model in
[`authority_values_and_boundary_evidence.md`](authority_values_and_boundary_evidence.md).

## Architectural preemption and semantic safe points

Architectural preemption and semantic suspension are separate:

```text
architectural preemption
    pause at any instruction
    preserve opaque machine state
    resume without changing semantic circumstances

semantic safe point
    declared may-suspend operation
    canonical live frontier is known
    structured cancellation/migration/quiescence may occur
```

Hardware or an OS may preempt an Omega activation for fairness without source
cooperation. That event is not a cancellation point, does not permit claim
reconstruction, and cannot manufacture CPU/thread migration permission.
Semantic safe points occur at explicit `suspend` calls such as waits, joins, or
an authored `scheduler.poll()`. State transitions, loop backedges, ordinary
calls, allocation, and optimizer-chosen locations are not implicit safe points.

This keeps hot kernels honest. A non-suspending SIMD chunk may be
architecturally preempted at any instruction; an outer machine places an
explicit poll between chunks when it wants bounded semantic response.
The compiler must never insert a may-suspend poll as an ordinary optimization.

`block` creates no semantic safe point. Unless the selected blocking operation
publishes a finite wait ceiling, cancellation finalization, quiescence, and the
next structured response remain unbounded through that call. Reports preserve
the responsible call/path and the bounded computation around it rather than
collapsing the result to an unattributed infinity.

The restricted terminal-Psi fixed-work checker may close a segment ending at
the next safe point. Otherwise it reports `Unknown` or retains the exact
blocking/foreign edge with no finite guarantee. WCSU proves space, not work or
wall-clock latency. A target may convert checked work to time only through a
separately derived or admitted timing model whose trust provenance remains
visible.

## Foreign calls and stacks

A checked Omega call contributes derived WCSU. An opaque foreign call does not.
The foreign binding must therefore select an execution placement:

```text
direct foreign call
    -> runs on the current activation stack
    -> admitted foreign stack contribution enters this StackPlan

provider-stack/component call
    -> caller accounts for its checked local stub
    -> foreign provider owns a separately provisioned stack
```

The normalized stack composer implements that boundary directly: checked
same-stack calls are graph edges, admitted same-stack leaves are explicit
byte/alignment contributions, and provider-stack or new-activation transfers
have no child edge in the current stack domain. It composes the maximum live
chain rather than summing sequential callees and retains the exact validated
frame and admission identities behind the sealed result.

A callback requirement carries its own `Calling<C>` entry plan. A named static
Omega machine satisfying that requirement enters through the generated thunk.
The plan chooses provider-stack continuation, provider-stack preflight against
the exact Omega WCSU plus target reserve, or a target-supported owned stack.
Preflight proves the predicted segment fits; a hard-limited owned stack also
detects underestimation at its own boundary. Opaque foreign frames remain in
the provider stack domain.

For an installed external root, that execution-stack choice is refined by a
context-indexed epoch realization. Each admissible arrival context has a finite
enter/body/exit sequence whose epochs state the active domain, per-domain
occupancy, and nesting allowance. The body epoch alone receives the checked
Omega WCSU. Relative `Interrupted` follows the active parent epoch through
nested entry, while target rules, emitted-stub derivation, or admitted opaque
evidence establish the non-body portions.

A platform adapter removes ordinary application code from the native recursive
dispatch graph. It classifies which of its own operations may synchronously
re-enter, defines restricted synchronous handlers, checks their ordinary Omega
reach locally, and queues other events until native dispatch returns. Direct
raw callbacks remain trust-relative; a chain-scoped depth limit is useful only
when the protocol supplies a valid unavailable result. A future modular
higher-order callback-summary analysis is recorded as unbuilt and is not a
prerequisite for the adapter construction.

Trust composes globally by the weakest input while retaining every supporting
provenance edge. A derived Omega WCSU used on a provider stack therefore retains
the provider's admitted stack and behavior premises. A checked Omega provider
may derive its own facts from its body.

A hosted blocking executor is an ordinary package, not a task-runtime mode or
language call kind. It may pool guarded native stacks and keep native blocking
off no-block scheduler workers, but it does not prove completion or
cancellation. Pool exhaustion, retained custody, cancellation finalization,
and shutdown remain ordinary resource obligations. Storage used only before
return may be borrowed; storage used after return moves into an ordinary linear
protocol claim through ownership conservation.

The compiler task canary now carries an admitted suspension-only permission
through a qualified selected-machine entry, local transfer, canonical
safe-point liveness, and the current `05_task_activations.json` fixed-stack and
crossing artifact. Static-machine specialization normalizes the qualified entry
back to its underlying runtime carrier while retaining the qualification as
proof evidence. The retired preemption-mode, all-instruction, and
continuation-address compatibility fields are no longer emitted. This fixture
is intentionally affine: conservation and provider custody for linear task
arguments remain TR3–TR8 work.

## Acceptance register

1. Direct `Worker::run(job)` is an ordinary call; only
   `runtime.start<Worker::run>(job)` creates a distinct activation.
2. A rejected `try_start` returns all moved arguments and supplied leases.
3. A returned `Task<T>` is linear even when execution completed inline.
4. `request_cancel()` preserves the claim; `finish()` terminally consumes it.
5. Moving a task into a record or supervisor transfers exactly one obligation.
6. Dropping, overwriting, or losing a live task on one branch is rejected.
7. A local pool/runtime cannot close while dependent task/storage claims live.
8. A provider that cannot supply a `StackLease` satisfying the WCSU-derived
   `StackPlan` is rejected before execution.
9. Arena-, OS-, remote-, and inline-backed providers share one task contract
   without sharing one physical storage representation.
10. No user program requires `spawn`, implicit detach, or a privileged
    task-group construct; `suspend` and `block` acknowledge a direct call and do
    not create a task.
11. A provider is rejected when it cannot establish the CPU/thread
    preservation obligations induced by the activation's possible live values.
12. A local suspension or migration point rejects when any live value forbids
    it, even if a more capable runtime exists elsewhere.
13. Architectural preemption neither requires nor creates a semantic safe
    point; migration and structured cancellation occur only under the relevant
    checked suspension/pinning contracts.
14. A blocking call without a finite wait ceiling reports unbounded semantic
    response with the responsible call path.
15. A foreign same-stack call contributes admitted stack demand; a package
    blocking executor owns a separate stack and cannot launder unbounded
    completion into a bounded suspension claim.
16. Parking leaves the exact call incomplete, establishes no result, runs no
    cleanup, and creates no second local successor. Resumption continues that
    same invocation before ordinary source control proceeds.
17. `request_cancel()` cannot dispose a parked continuation. It changes the
    ordinary safe-point outcome; source cleanup runs as frames retire normally,
    and only `finish(self)` consumes the external task claim.
18. A wait without accepted finite-response evidence reports
    `NoFiniteGuarantee(Edge(edge), UnboundedWait)` at the responsible call.
    Bounded-response and termination profiles reject it; safety still retains
    every parked value and claim without duplication or discard.

## Engineering sequence

1. Retire the stage-1 synchronous `spawn` desugar and erased `Join<T>` parser
   fiction with directed migration diagnostics.
2. Core `[linear] Task<T>`, `TaskOutcome<T>`, and
   `StartOutcome<T, Arguments>` are live. Symbol-keyed generic substitution
   preserves conditional payload debt through `Returned(LinearT)` and
   `Rejected(LinearArguments)`, with pass and scope-loss canaries pinning both
   sides. Receiver types now preserve shared `&self` versus consuming `self`;
   lifecycle canaries prove `request_cancel` retains the claim and `finish`
   consumes it into `TaskOutcome<T>`. Four negative canaries also pin that the
   compiler/provider-owned parked continuation is absent from the task claim:
   ordinary code cannot project, recast, address, or mutate it. Typed local and
   parameter member validation covers applied generic carriers and rejects a
   missing member before backend lowering.
3. Concrete compile-time machine-symbol specializations now retain their
   executable instance identity. `TaskRuntime::start<M>` and `try_start<M>`
   elaborate into validated activation plans and the normalized
   `05_task_activations.json` artifact. Migrate its current continuation-layout,
   preemption-mode, and all-instruction-supply fields to WCSU-derived
   `StackPlan`, canonical suspension crossings, and demanded CPU/thread
   preservation. Keep incomplete derivation fail-closed.
4. Retire `TaskRuntimeContract`, `AdmittedTaskRuntimeContract`,
   `PreemptionGranularity`, and the generalized activation/runtime join from
   `omega-task-plans`. The ordinary selected `TaskRuntime` provider plan and
   operation-specific requirement are connected to each concrete activation.
   The normalized provider-instance/invocation receipt now binds that static
   fact, its exact activation plan, and the executor's preservation evidence
   before lifecycle accounting. Next connect stack resource/reservation,
   cancellation conformance, and routed source establishment; then add
   transactional `start`/`try_start` ownership.
5. Connect the implemented normalized provider-provenance/child-lease ledger
   to selected runtime values and source `Task<T>` after provider selection and
   routed establishment lands. The ledger already prevents premature
   close/reclaim and preserves a claim on failed settlement.
6. Implement fixed nonmoving stack lowering, WCSU-backed `StackPlan`, stack
   reservation, and a first provider. A future stackless plan is a separate
   lowering, not a runtime contract mode.
7. Retain one Terminal suspension-call plan keyed by the exact call operation
   and existing `SuspensionCrossingId`; preserve its checked live frontier and
   carry demands without adding a local suspension terminator. Join that row in
   activation realization to inherited CPU/thread preservation demands,
   `ActivationCarryObligations`, the WCSU `StackPlan`, stack lease, and selected
   runtime evidence. Then expand the current conservative suspension-safe-loan
   subset without weakening the storage/alias/cancellation theorem.
8. Build `ArenaTaskPool`, bounded mailbox, and supervisor reference packages;
   promote no additional language construct unless a package finds something
   semantically inexpressible.
9. Implement compiler-service Terminal-Psi metering plus restricted fixed-work entry and
   safe-point segment checking; keep logical work, response wait, and target
   timing conversion distinct.
10. Implement registered callback lowering and the Windows acceptance slice
    under the calling-plan/boundary lane. Keep any blocking executor as an
    ordinary package rather than adding either facility to `TaskRuntime`.
