# Task activation and lifecycle

A selected `TaskRuntime` boundary provider executes an ordinary named machine
as a distinct activation. `runtime.start<Worker::run>(job)` and
`runtime.try_start<Worker::run>(job)` use a compile-time machine-symbol argument,
not capture inference or a runtime function value. A direct `Worker::run(job)`
remains an ordinary call. There is no bare spawn, implicit detach, future
transformation, or privileged supervisor. [Call acknowledgements](../language/effects.md#call-site-acknowledgements)
describe only what the start/control operation may do to its caller.

Root profiles may select default providers and owners may override their slots,
but starting/controlling a task still requires a held runtime capability.
Selection does not grant ambient authority.

[Anonymous callable environments](../language/anonymous_machines.md#activation-and-foreign-callbacks)
may be ordinary arguments to a specialized named entry adapter. The adapter
retains the exact start requirement, activation description, and transactional
ownership behavior below. It is library adaptation, not implicit capture/spawn
inside start or a runtime function-pointer argument. Direct anonymous calls
remain in the caller's activation.

## Three owners

| Relationship | Responsibility |
| --- | --- |
| Runtime custody | Execution, park/wake, cancellation under the pinned provider contract. |
| Physical storage | Accountable backing and lifetime for activation state. |
| `Task<T>` ownership | Linear right and obligation to observe, cancel-request, transfer, or settle that activation. |

`Task<T>` need not contain or borrow the stack; its provider/activation identity
and lifecycle authority are distinct. Arena pools, hosted providers, remote
executors, and inline execution can satisfy the same task contract without
sharing one storage representation. Inline completion may shorten storage
lifetime or embed an outcome, but does not settle the returned linear claim.

## Activation description and stack

The compiler monomorphizes the machine argument and derives an activation
description retaining exact machine contract/entry, argument/outcome layouts,
target calling plan, whole-call-graph WCSU `StackPlan<M>`, canonical suspension
crossings with live carry demands, activation-wide CPU/thread preservation,
and cancellation/operational behavior required by the selected start operation.
Planning is a generated artifact, not an implicit representation-identical cast.

Each local activation receives one fixed nonmoving stack and retains it while
parked. Direct suspension allocates no separate continuation buffer. Final frame
and spill demand, alignment, entry overhead, and live call chains feed WCSU;
a numeric fit alone supplies no `StackLease`. Before execution the provider
must establish backing satisfying the exact plan. Ordinary calls acquire no
new spill-exhaustion route. Remote execution provisions the equivalent in its
execution domain; inline execution still has an accountable stack provision.
Alternate lowered representations are not runtime-selectable modes of this plan.

Target/provider selection and physical plans remain in the
[build-owned companion](../terminal-psi/product.md#build-owned-companion), not
Psi checked semantics. `05_task_activations.json` reports activation/stack plans,
canonical crossing identities, and preservation obligations.
`05_carry_manifest.json` reports the checked typed live-value/storage frontier;
tools consume it rather than re-reading source. Report emission does not establish
provider custody, a complete WCSU producer theorem, or executable task support.

Static binding retains the exact selected runtime plan and authored start or
try-start requirement. Reject missing/duplicate selection, requirement drift,
or narrowing of the published static-machine contract. The domain-separated
specialization commitment binds operation, exact checked requirement,
package-qualified ownership and target/entry signature with parameter modes,
and machine-contract commitment. Compact specialization coordinates are reports.

The runtime invocation receipt must join that static description, exact provider
plan/operation, activation plan, runtime instance, and required preservation
evidence. Invocation and receipt identities are single-use within that instance.
Static plans are not invocation receipts or routed source establishment.

## Transactional start

`start<M>` returns `Task<T>` when capacity/admission obligations are discharged.
`try_start<M>` handles genuine dynamic capacity/environmental uncertainty with
`StartOutcome<T, Arguments>`: `Started(task)` or
`Rejected(arguments, reason)`. Known static plan/provider incompatibility rejects
validation or admission rather than becoming avoidable runtime failure.

Rejection returns every moved argument and caller-supplied reservation/lease,
or proves transfer to another named authorized owner. No linear value disappears
in a failed provider call. A pending task is returned only after provider custody
and compatible storage exist. Reservation remains real authority under
interference: an `available >= 1` fact suffices only with permission preventing
another starter from spending that capacity.

## Linear task claim

`Task<T>` is linear. `request_cancel(&self)` retains the claim; it requests a
transition, not proof that execution stopped. `finish(self)` terminally consumes
it and returns ordinary `TaskOutcome<T>` cases `Cancelled`, `Returned(value)`,
or `Failed(receipt)`. Application-level recoverable errors remain inside `T`;
the outer sum describes lifecycle outcomes. Trap/abort remain declared crash
routes, not fabricated returned cases. Generic result containers retain each
active payload's substituted linear debt.

The task can move into a record, selected case, or ordinary supervisor, thereby
transferring its obligation. Copying, overwriting, branch loss, scope drop, or
implicit fire-and-forget cannot discharge it. Automatic cleanup cannot join,
park, fail, or settle the task. A combined terminal stop operation exposes its
own cancellation/wait/failure contract.

Provider provenance is permission state, not an extra nominal result parameter.
A task backed by borrowed/partitioned storage cannot outlive it. Closing a local
runtime/pool or reclaiming storage requires reconciliation of every child claim
and lease. An owned child lease, pinning provenance edge, or transferred
reservation may implement this invariant; none is a second source task type.

One ledger belongs to one admitted runtime instance. Each accepted child records
exact activation admission and either storage owner/lease era or admitted inline
completion. Cancel requests retain that record. Settlement removes only the
matching claim and storage relationship; cross-instance failure returns the
claim. Reuse requires a fresh activation/lease era, not replay of old evidence.

## Suspension and cancellation

[Terminal suspension](../terminal-psi/calls_and_outcomes.md#suspension) owns the
incomplete call and its exact live frontier. Parking establishes no result or
cleanup edge; resumption continues that same invocation. The parked continuation
is compiler/provider-owned, not an addressable field of `Task<T>`.

Cancellation is cooperative and arrives through ordinary outcomes at declared
wait/safe points. Requesting it cannot dispose a parked continuation or unwind
arbitrary frames. Source follows the resulting normal control flow and cleans
up as frames retire; only the terminal task operation settles the external
claim. A never-suspending task may be finishable without being cancellable.

Architectural preemption may preserve opaque state at any instruction without
becoming semantic suspension or granting migration/replacement/cancellation.
Semantic safe points require explicit may-suspend calls or authored scheduling
operations such as a poll. State transitions, backedges, ordinary calls,
allocation, and optimizer locations are not implicit safe points. Optimizations
cannot insert may-suspend polls into a non-suspending kernel.

Blocking creates no semantic safe point. Without a finite wait ceiling, it
leaves cancellation finalization/quiescence/structured response unbounded;
reports retain the responsible call/path and bounded surrounding computation.
Restricted [logical-work analysis](../resources/logical_work.md) may certify a
segment ending at the next safe point, but WCSU proves space, not response or
wall time. Bounded-response/termination profiles need accepted finite-wait
evidence for any otherwise unbounded wait.

## Carry and loans

Starting with a retained exclusive reference excludes conflicting parent access.
Move independent work, split provably disjoint places, or use synchronized
capabilities. A live reference carrier alone does not authorize a loan across
park: its exact crossing needs storage lifetime/pinning, aliasing, cancellation-
outcome, and address-stability evidence. Implementation may conservatively reject
unsupported loans, but blanket loan death is not the language rule.

[Carry](../resources/carry.md) remains demand-driven. Local possible-suspension
checks cannot be weakened by runtime selection. A runtime establishes each
required CPU/thread preservation axis by checked construction or exact admitted
receipt; portable activations demand neither. A host able to migrate outside
semantic points must establish activation-wide preservation when possible live
values need it. Address stability follows from the nonmoving lease, not a freely
asserted runtime property. Provider identity and evidence travel with the task.

Cancellation conformance, inline completion, and stack reservation are separate
operation/resource facts, not fields of a universal runtime behavior lattice.
The runtime must implement the cancellation request contract exposed by `Task`;
that does not promise every task eventually observes the request.

## Library and foreign providers

An Arena-backed task pool is a bounded reference package, not a language
primitive. Provisioning fixes maximum capacity; starts/settlements still account
dynamically. Supervisors own claims, event endpoints, and policy without
necessarily owning frames. Completion can reach provider-held outcomes directly;
it needs no mailbox. Mailbox storage and send/receive endpoints may have separate
owners; closure/task death must preserve a surviving owner for every undelivered
linear payload.

The shared wait-substrate direction is a small word/value wait plus wake-one/
wake-many boundary. Libraries may build completion, locks, barriers, queues,
and I/O waits where the target supports that contract. It is an engineering
direction, not permission to merge distinct host mechanisms under false
guarantees. Wake need not park; wait's suspension/blocking and
[positive progress](../language/termination.md#opaque-progress-profiles) remain
independent explicit premises.

Foreign execution either contributes admitted same-stack demand or uses a
separately provisioned provider stack while the caller accounts for its stub.
[Stack demand](../resources/storage.md) and [entry stacks](../resources/entry_stacks.md)
own composition. Callback plans/registration own inbound execution and its
restricted synchronous graph; an ordinary hosted blocking executor is a package,
not a task-runtime mode. Pooling guarded native stacks can spare no-block workers
but cannot prove completion, cancellation, or shutdown. Post-return storage
moves into a linear protocol claim; pre-return storage may be borrowed.

The [task-plans implementation note](../../../omega-rust/omega/representations/task-plans/README.md)
records current static binding/accounting support and missing execution joins.
