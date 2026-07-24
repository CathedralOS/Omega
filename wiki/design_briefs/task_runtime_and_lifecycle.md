# Design Brief: Task Runtime And Lifecycle

Settled at the architectural level. The core claim and outcome spellings are
live; runtime-provider integration and the remaining library surface are still
under implementation. Chapter 18 is the user-facing authority. This brief
records the mechanical model so the implementation does not recreate an
`async machine`/`Future<T>` split, implicit detach, or a mandatory pool
abstraction. Direct-call suspension acknowledgement is required; only its exact
spelling and placement remain owner questions, and requiring it does not change
this task model.

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
runtime service's ordinary service reach and operational ceilings and are admitted like other
providers. A package may wrap that capability with pools or policy without
changing the normalized task contract.

A platform profile/provider plan may select a default task runtime at the
application root, and tests or components may inject a different provider for
the slots they own. Selection convenience does not make runtime authority
ambient: code that starts or controls tasks receives or owns the relevant
capability.

There is no `async machine` species, `Future<T>` transformation, bare `spawn`
block, implicit fire-and-forget, or privileged supervisor/task-group construct.
The pending suspension-keyword design concerns direct-call audibility, not task
creation or return-type transformation; the requirement for an acknowledgement
is settled.

## Custody, storage, and claim are separate

Starting a task establishes three relationships that must not be conflated:

1. The **runtime provider** takes operational custody of the activation and
   arranges execution, parking, waking, and cancellation according to its
   pinned contract.
2. A **storage owner** holds the activation's physical state. This may be a
   caller-provisioned Arena pool, an OS/runtime provider, a remote executor,
   or no persistent activation storage when execution completes inline.
3. The owner of **`Task<T>`** holds the linear lifecycle claim: the right and
   obligation to observe, request cancellation of, transfer, or terminally
   settle that activation.

`Task<T>` therefore does not universally contain or borrow the task stack. It
is normally a small provider/activation identity plus sealed lifecycle
authority. A provider may embed an already-completed outcome as an
optimization, but completion never silently settles the linear claim.

All physical storage still has an accountable owner: an Arena-issued
Allocation, a provider under its admitted resource contract, or the platform
under a boundary contract. Arena-backed Allocation is the in-model bounded case, not the
definition of all task storage.

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
- continuation/frame requirement, alignment, and pinning requirements;
- cancellation and suspension behavior needed by admission; and
- the target calling/entry plan.

The provider receives a generated descriptor plus the invocation's moved
arguments. The source expression is not an implicit `as`: qualification is
representation-identical, while activation planning creates a distinct
compiler artifact. The concise `start(M, args)` form may be transparent future
sugar, but v1 can use the already-honest `start<M>(args)` spelling.

The provider-independent normalized descriptor is live in `omega-task-plans`.
It records contract/entry/calling-plan identities, argument and terminal
layouts, continuation size/alignment, cancellation and distinct-activation
requirements, the local suspension-safety result, and separate migration
demand envelopes for safe-point versus asynchronous crossings. The validator
rejects unsafe possible suspension locally before any runtime is considered.
Compiler elaboration is now live for concrete `TaskRuntime::start<M>` and
`try_start<M>` specializations. It retains the concrete specialization symbol,
derives independent `may_suspend` and `may_block` bits from the target's checked
transitive suspension and blocking plans,
sizes the continuation from target layout plus canonical crossing live values,
joins safe-point carry demands, and emits `05_task_activations.json`. Missing
crossing evidence fails closed. Because every `Task<T>` claim exposes
cancellation-request authority, every activation plan requires a
cancellation-capable runtime. The checker also emits one conservative
all-instruction carry envelope per machine by joining every persistent slot,
parameter, local, call signature, aggregate/cast temporary, and reference
formation visible in checked trees. Checked `CarryFacts` also owns the
field-derived contained-machine topology in grouped arenas. Safe-point demands
join every descendant machine's crossing policy; asynchronous envelopes join
every descendant machine's all-instruction policy through the same cycle-safe
closure. A complete subtree envelope populates the asynchronous migration
demand; unresolved coverage remains explicitly absent and admission fails
closed. `05_carry_manifest.json` exposes both completeness, subtree size, and
the joined policy. It also publishes every canonical safe-point crossing with
its exact statement/call identity, target, and typed live-value/storage set.
Formal models and diagnostics consume that checked artifact; they do not
reinterpret source syntax to rediscover which values cross a park.

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
return, cancellation, and provider failure. Trap/abort behavior remains in the
machine/provider contract rather than being fabricated as a returned value.

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
task-runtime slot and opaque runtime representation; those owner questions are
tracked separately.

## Storage policies are provider/library choices

Before a pending `Task<T>` is returned, the provider has accepted custody and
established compatible storage. That storage may contain:

- moved arguments and captures;
- live locals and continuation/call state;
- a resume state/program location;
- scheduler, wake, and cancellation metadata; and
- a terminal outcome retained until settlement.

A stackful provider may use a stack plus task-control block. A compiler-lowered
provider may use a bounded continuation frame. A remote provider stores the
activation elsewhere. An inline provider may finish execution during start
and return an already-complete, still-unsettled `Task<T>`.

For local machines the compiler derives the activation requirement. The
provider validates that requirement against its storage plan. Frame size is a
necessary input, not the whole admission law: alignment, address stability,
the four-axis carry demand of values live at crossings, continuation
representation, and provider contract also participate.

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

The v1-safe path is moved owned arguments, shared immutable values, and
explicit synchronized capabilities. Starting a task that retains `&mut self`
while the parent continues using `self` is rejected by ordinary exclusivity.
Code should move independent work, split state into provably disjoint places,
or communicate through a mailbox/synchronized capability.

The suspension amendment must later state which loans may cross a park:
storage lifetime and pinning, aliasing, cancellation paths, and address
stability are independently required. Borrow/wait-cycle detection is a
valuable later theorem but not a prerequisite for the conservative task v1.

Carry policy itself is settled independently of that loan subset. Transparent
data derives a compiler-normalized four-axis policy; opaque data defaults
strict; type-wide `[carry(...)]` claims require proof/admission; and sealed
constructor domains may add per-mint permissions. At each crossing, canonical
place liveness determines which policies constrain the transition. This
replaces the provisional `[send]` property and any Rust-style `Send`/`Share`
marker model.

Suspension is enforced locally against possible suspension and cannot be
narrowed away by runtime selection. The provider-side normalized behavior
contract instead records preemption granularity, CPU migration/pinning,
host-thread migration/pinning, and continuation-storage movement. Admission
joins those three carry demands with behavior. Missing external evidence means
pessimistic behavior; a receipt may authorize reliance on a narrower opaque
claim but never changes the actual runtime. The contract rides the existing
provider-plan/admission spine for static and dynamically admitted runtimes.

The normalized runtime join is live in `omega-task-plans`. It selects the
correct migration-demand envelope from provider preemption granularity, rejects
missing all-instruction analysis for asynchronous providers, and checks frame
size/alignment, cancellation, inline completion, CPU/thread affinity, and
continuation address stability. Its pessimistic opaque-runtime contract admits
nothing accidentally. Each validated demand has a normalized identity over all
of those checked inputs; a successful admission derives its receipt identity
from the complete demand and complete runtime behavior contract instead of
accepting a caller-invented label. Provider-side qualification also uses the
shared trust spine rather than accepting behavior data as evidence: a freely
constructed `TaskRuntimeContract` becomes admissible only when an exact
`ProviderPlan` receipt binds the base plan identity and every behavior promise.
Any change to capacity, preemption, affinity, continuation movement,
cancellation, or inline completion changes that statement fingerprint and
requires re-admission. Receipt provenance is evidence, not runtime identity,
so switching from own-package development authority to a root grant does not
change the normalized runtime contract. The activation artifact uses compact
identities for tooling, while the sealed admission and lifecycle
carriers retain the complete validated demand and admitted runtime behavior.
Custody and settlement compare that exact evidence; compact fingerprint
collisions cannot substitute a different runtime contract or activation plan.
The artifact reports `pending_provider` until compiler provider selection
supplies that exact receipt. `TaskRuntime` provider-plan selection/wiring is
owner-blocked on the
provider-slot and checked behavior-publication decision recorded in
`OWNER_QUESTIONS.md`; the current provider spine selects boundary-trait slots,
while `TaskRuntime` is opaque boundary data and has no checked surface for the
capacity, preemption, migration, storage, cancellation, or inline-completion
statement. Its runtime representation is separately owner-blocked by the
general opaque `boundary data` representation question. Dispatch and
transactional start follow those decisions.

## Acceptance register

1. Direct `Worker::run(job)` is an ordinary call; only
   `runtime.start<Worker::run>(job)` creates a distinct activation.
2. A rejected `try_start` returns all moved arguments and supplied leases.
3. A returned `Task<T>` is linear even when execution completed inline.
4. `request_cancel()` preserves the claim; `finish()` terminally consumes it.
5. Moving a task into a record or supervisor transfers exactly one obligation.
6. Dropping, overwriting, or losing a live task on one branch is rejected.
7. A local pool/runtime cannot close while dependent task/storage claims live.
8. A provider whose storage or operational contract does not admit the derived
   activation plan is rejected before execution.
9. Arena-, OS-, remote-, and inline-backed providers share one task contract
   without sharing one physical storage representation.
10. No user program requires `spawn`, implicit detach, or a privileged
   task-group construct; any future suspension marker acknowledges a direct
   call and does not create a task.
11. A provider is rejected when its migration/thread/storage behavior cannot
    preserve every carry demand in the derived activation plan.
12. A local suspension or migration point rejects when any live value forbids
    it, even if a more capable runtime exists elsewhere.

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
   `05_task_activations.json` artifact, including checked effect reach,
   continuation layout, safe-point carry demands derived from canonical
   liveness, and a separately checked all-instruction envelope for
   asynchronous preemption. Incomplete type coverage remains absent and fails
   asynchronous admission closed. Every plan requires the cancellation
   support promised by the Task lifecycle.
4. The opaque core `TaskRuntime` boundary surface, normalized demand/admission
   identities, and receipt-qualified runtime behavior are live. The shared
   provider-plan receipt binds the complete behavior statement and provenance
   stays outside identity. Unresolved artifacts fail visibly as
   `pending_provider`. Provider selection/receipt wiring is owner-blocked on the
   task-runtime provider-slot/behavior-publication question; executable dispatch
   also awaits the general opaque-runtime representation decision. After those
   are settled, add transactional `start`/`try_start` ownership.
5. Connect the implemented normalized provider-provenance/child-lease ledger
   to selected runtime values and source `Task<T>` after the runtime-slot and
   opaque-carrier decisions land. The ledger already prevents premature
   close/reclaim and preserves a claim on failed settlement.
6. Implement continuation/frame lowering and a first provider; an inline
   provider is valid only where the pinned contract permits inline completion.
7. Add local carry checking and the conservative suspension-safe-loan subset,
   then expand it without weakening the storage/alias/cancellation theorem.
8. Build `ArenaTaskPool`, bounded mailbox, and supervisor reference packages;
   promote no additional language construct unless a package finds something
   semantically inexpressible.
