# Chapter 18: Concurrency

Concurrency uses ordinary machines plus an admitted task-runtime capability:

```omega
let task: Task<WorkResult> =
    runtime.start<Worker::run>(move job);

do_other_work();

let outcome: TaskOutcome<WorkResult> = suspend task.finish();
```

`Worker::run` is not a special async function. Calling `Worker::run(job)` runs
it in the current activation; supplying the machine symbol to
`runtime.start<Worker::run>(job)` asks a runtime provider to establish a
distinct concurrent activation.

The model has no bare `spawn` block, `async machine`, `Future<T>`, implicit
detach, or privileged task group. Starting, cancellation, completion, storage
provisioning, and supervision are ordinary contracted operations over explicit
capabilities and linear data. A direct call whose contract permits suspension
uses `suspend`; one that permits worker blocking uses `block`; one that permits
both uses `suspend block`. These acknowledgements create neither a future nor a
distinct activation.

`TaskRuntime` is a working core boundary requirement/capability, not a new
language construct. Starting and controlling a task reach that service through
the ordinary reach/provider model from chapter 19.

The full custody/storage/claim model and implementation sequence are recorded
in [Task Runtime And Lifecycle](../design_briefs/task_runtime_and_lifecycle.md).

## Starting Is A Provider Operation

Task start has a prove-or-handle pair. `start<M>` requires its admission and
capacity obligations to be discharged. `try_start<M>` exposes a genuinely
dynamic refusal as an ordinary sum:

```omega
transition runtime.try_start<Worker::run>(move job) {
    Started(task) -> keep(move task)
    Rejected(job, reason) -> recover(move job, reason)
}
```

The reference `start`/`try_start` contract shown here neither parks nor blocks
the current activation, so these calls are unmarked. Another admitted operation
with a wider contract would use the corresponding marker; the behavior of
`Worker::run` after activation does not affect the start call.

A rejected start returns every moved argument and caller-supplied reservation.
Start is an ownership transaction: no value or linear obligation disappears
into a provider that failed to establish the activation.

The `<Worker::run>` spelling uses the compile-time machine-parameter mechanism
from chapter 13. It is not a runtime function pointer or inferred capture. The
compiler monomorphizes the target and emits a static activation plan containing
the normalized contract, entry identity, argument/result layouts, and a
`StackPlan` derived from whole-call-graph WCSU. The provider receives that plan
plus the invocation's moved arguments and must supply a matching fixed,
nonmoving `StackLease`.

## Task Is A Linear Lifecycle Claim

`Task<T>` is ordinary `[linear]` data. Its owner has the right and obligation
to settle or transfer one provider-held activation. The task may be stored in
a record or sum for later work:

```omega
data WorkerState {
    case Idle;
    case Running(task: Task<WorkResult>);
}
```

Moving the task into `Running` transfers the obligation into that data value.
A later machine may consume it with `finish()` or transfer it again. A live
task may not be copied, overwritten, dropped at ordinary scope exit, or lost
on one branch.

Implementation staging: the core `Task<T>` claim carrier, generic terminal and
start outcome sums, and ordinary boundary-trait `TaskRuntime::start` /
`try_start` requirements have landed. Each concrete activation binds the exact
selected runtime provider plan and operation requirement; this does not yet
manufacture a dynamic runtime-instance or invocation receipt. Qualifier-aware
generic payload propagation preserves a
substituted linear result or rejected argument bundle instead of silently
losing its obligation through an unconstrained generic field. Concrete static
targets produce `05_task_activations.json`; it now records a fixed-stack
`StackPlan`, canonical suspension-crossing identities, and demand-driven
CPU/thread preservation instead of the retired continuation-size,
preemption-mode, and all-instruction runtime-supply fields. Its current stack
byte count is the local machine/park-frontier layout bridge; whole-call-graph
WCSU composition and stack reservation remain lowering work. Every activation
plan requires the cancellation operation promised by the `Task` lifecycle. The
core
lifecycle calls are ownership-checked: receiver types keep shared `&self`
distinct from consuming `self`, `request_cancel` preserves the claim, and
`finish` consumes it into the conditional terminal outcome. Provider
provenance/admission/dispatch and executable fixed-stack lowering remain.

`request_cancel()` retains the claim: requesting cancellation does not prove
that execution stopped. `finish()` may suspend, consumes the claim, and returns
an ordinary terminal-outcome sum such as returned, cancelled, or provider
failure. Application-level recoverable failure remains part of `T`.

Long-lived background work transfers its claim to an owner:

```omega
supervisor.adopt(move task);
```

That is ordinary ownership transfer, not detach. The supervisor becomes
responsible for eventual settlement.

## Suspension, Blocking, And Direct Calls

Suspension is an operational property of an ordinary machine. Independent
`suspends` and `blocks` clauses publish the two ceilings; absence
of each is the corresponding negative guarantee. A suspended activation retains
its fixed nonmoving stack. Checked source already derives one canonical
`SuspensionCrossingId`, exact live frontier, and four-axis carry policy for each
possibly suspending call. A first bounded Terminal rung now retains an injected
checked crossing for a receiver-free direct scalar call whose complete frontier
contains only primitive scalar parameters, straight-line locals, and call
arguments with no live claims. General Terminal/runtime retention and expansion
of the conservative suspension-safe-loan subset remain.

Positive progress remains separate. Pinned operation contracts may carry
owner-classified `ProgressProfile` domains established through exact admitted
boundary grants. A termination guarantee records authored public schemas and
exact call-instantiated premises; merely mentioning the qualified capability,
or declaring `suspends` or `blocks`, creates none. Local receipts and
composition-bound provider receipts may discharge exact instances. General
trace entailment remains deferred.

The constraints are:

- suspension is an operational part of the ordinary machine contract, not a
  `Future` return type or a separate `async machine` species;
- a caller/context imposes service, operational, and resource ceilings, so a
  provider that may suspend or block cannot satisfy a slot that omits the
  corresponding clause;
- suspension composes through ordinary calls through the normalized contract;
  `suspend` and `block` change source acknowledgement only, not propagation or
  lowering;
- automatic cleanup may execute but may never suspend, block, or fail;
- `Task<T>` is linear and must be settled or transferred explicitly; and
- a loan may cross suspension only when the eventual suspension model can
  prove its storage, pinning, aliasing, and cancellation safety. Blanket
  acceptance and blanket rejection are both premature.

Suspension composes through ordinary calls, with the suspension plan propagated/
inferred and WCSU providing the activation's fixed stack bound. Public
operational clauses are explicit ceilings; private omissions infer. Format 74 /
vocabulary 77 preserve the bounded scalar Terminal site/plan pair without
adding a CFG edge. Structural, persistent, receiver-bearing, threaded-local,
claim-bearing, Unit, boundary, and dynamic frontiers, park/resume lowering, and
evidence-backed widening of the suspension-safe-loan subset remain engineering
work under the settled model.
See
[effects_authority_and_observation.md](../design_briefs/effects_authority_and_observation.md).

### Call-site acknowledgements

Both operational possibilities are explicit at direct calls:

```omega
operation();
suspend operation();
block operation();
suspend block operation();
```

The prefixes mean **may**, not **did** or **must**. A suspending or blocking
operation may complete immediately on a particular invocation. The marker says
the statically known contract permits the behavior and makes the pause point
reviewable while borrows, claims, guards, and other live state remain held.
An unmarked call is statically guaranteed to do neither.

The distinction between the markers remains structural. Suspension parks the
activation and materializes a continuation, so the call must be a complete
statement, simple `let` right-hand side, transition subject, or terminal
expression. Blocking retains the ordinary stack, so a blocking-only call may
nest. A call that permits both uses the fixed order `suspend block` and follows
the suspension position rule.

```omega
let guard: Guard = block mutex.lock();
let event: Event = suspend inbox.take();
let wide: Event = suspend block source.take();

// Rejected: partially evaluated expression state would cross suspension.
let total: u64 = prefix + suspend source.next();
```

Missing, partial, redundant, and reversed acknowledgements reject against the
call's statically known operational envelope. A call through an abstract
requirement carries its published possibilities. Generic code that does not
need the wide surface may require a transparent `suspends false` or
`blocks false` refinement and thereby remove the corresponding marker.

The prefixes are not `await`: they create no `Future`, do not change a return
type, and do not start another activation. `runtime.start<M>` creates a distinct
activation because that is the operation's meaning. The call to `start` itself
is marked only if its own contract may suspend or block the current activation;
the eventual behavior of `M` does not mark the start call.

### Parked call lifecycle

A potentially suspending call has one ordinary completion continuation. If it
parks, the call remains incomplete: its result is not established, later
operations have not executed, and the activation's fixed stack, live values,
claims, and cleanup obligations remain owned by the parked activation. Parking
runs no cleanup and is not a local CFG terminal, crash, return, or source-visible
continuation value.

Terminal Psi therefore retains suspension as an interprocedural operational row
on the exact call rather than as a second local successor. The row binds the
call operation to its existing `SuspensionCrossingId`, checked live-place and
claim frontier, and per-value carry policy. The separate activation realization
inherits the crossing's `preserve_cpu` and `preserve_host_thread` demands and
joins them to `ActivationCarryObligations`, the WCSU-derived stack plan, and
selected provider/runtime evidence. Neither layer may rederive liveness or
affinity from source shape. The ordinary call result becomes available only
after the same incomplete invocation resumes and completes.

A call site inside a cycle keeps one static `SuspensionCrossingId`, while each
dynamic visit that parks creates a distinct linear parked occurrence. Static
crossing identity is reusable schema; runtime resume custody is neither copied
nor shared between iterations.

Cancellation creates no hidden edge out of parked state. `request_cancel()`
retains the `Task<T>` claim and requests that a checked safe-point operation
eventually produce its cancellation-shaped ordinary result. Source then takes
its declared transition and retires frames normally. `finish(self)` alone
terminally consumes the external task claim and reports `Returned`, `Cancelled`,
or `Failed`. Crash routes remain separate no-successor outcomes.

A wait with no accepted finite response contract is not silently assumed to
resume: response analysis reports a structured `NoFiniteGuarantee` with the
responsible edge. A cyclic component without a quantitative bound names its
derived component identity and reports `Unranked`, `UnboundedRank`, or the exact
responsible edge as its cause. A general root may publish that absence of a
finite guarantee; a root that requires bounded response or termination rejects
it. This progress judgment is separate from the safety fact that parked custody
remains neither duplicated nor discarded.

In I/O-heavy code, `block` may be common. Its purpose remains local: reviewers
can see the exact waiting sites and, especially, whether a block occurs while a
guard or scarce authority is live. In such a module the unmarked immediate call
is itself useful signal.

### Preemption and safe points

Architectural preemption may pause an activation at any instruction, preserve
its opaque register/stack state, and resume it without changing its semantic
circumstances. It requires no source safe point. A semantic safe point is
narrower: an explicit may-suspend operation at which canonical liveness is
known and structured cancellation, permitted migration, or replacement
quiescence may occur.

```omega
process_simd_chunk(buffer);   // suspends false; hardware may still preempt
suspend scheduler.poll();    // authored semantic safe point
```

Loops, state transitions, allocation, ordinary calls, and optimizer-selected
locations are not implicit safe points. The compiler may not insert a
suspending poll as an ordinary optimization. A blocking call creates no safe
point; without a finite wait contract, semantic response through that call is
unbounded and tooling reports the responsible path.

WCSU bounds simultaneously live stack space, not work. The restricted
terminal-Psi checker calls the greatest total charged along one admitted path
the **maximum logical work**, measured in fuel units. Sequential work adds;
exclusive branches take their maximum. This is deliberately different from
WCSU, where stack reclaimed after one sequential call can be reused by the
next. The checker may analyze a complete entry or a segment to the next
semantic safe point. Its report is `Bounded(K, evidence)`, `Unknown(reason)`, or
`NoFiniteGuarantee(subject, cause)`. `subject` is the responsible edge or the
verifier-derived cyclic component; a component cause is `Unranked`,
`UnboundedRank`, or a directed edge whose contract prevents a finite ceiling.
A wall-clock observation is not a theorem; converting a ceiling to time
requires a separate target timing model and retains that model's trust
provenance. See
[`canonical_ir_fuel_and_resource_provisioning.md`](../design_briefs/canonical_ir_fuel_and_resource_provisioning.md).

Maximum-logical-work evidence supports compiler-service budgets, static
reports, and target timing analysis. Native execution has no logical-fuel
meter, sponsor-driven slicing, exhaustion transfer, or fuel-induced
suspension. Installing such evidence therefore changes neither the program's
control flow nor its suspension interface.

A hard-control profile that requires bounded response rejects both
`Unknown(reason)` and `NoFiniteGuarantee(subject, cause)`. It does not make an
unbounded lock or wait safe by force-terminating its holder, and it need not
build a dynamic wait-for graph merely to admit primitives that the profile
forbids.

## Carry Policy Is A Product

Values that remain live across scheduler/storage transitions contribute four
independent demands: suspension allowed or forbidden, same/any CPU,
same/any host thread, and stable/movable address. Type-wide guarantees use the
compiler-built-in `[carry(...)]` property from chapter 7. Transparent data
derives structurally. An admitted resource claim begins maximally strict and
its result contract may establish the positive per-value facts
`Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, and
`Carry::MovableAddress`. `Carry::Portable` transparently expands to their
conjunction.

Checked-internal claims derive their policy from inherited provenance and
storage. Claim transfers, aggregate containment, and conserved splits preserve
permissions per axis; combined origins select the most restrictive demand.
Forgetting a permission narrows what the value may do, while forgetting an
authority qualification leaves its undischarged provenance and carry demand
intact.

Moving exclusively owned data into another activation is legal when ordinary
ownership and the target runtime's carry contract both permit it. Sharing also
requires the relevant borrow polarity and atomic or protocol contract. CPU
affinity remains independent of host-thread affinity.

The enforcement sites are not symmetric. Suspension is a static reach
question: at a call or park, canonical place liveness plus possible suspension
decides legality, and provider selection cannot erase that ceiling.
CPU/thread preservation is demand-driven. A portable activation asks nothing.
An activation that may retain `SameCpu` or `SameThread` values requires the
selected runtime to establish the corresponding preservation claim, commonly
by pinning its worker or activation while that demand is live. A target that
may otherwise migrate execution at arbitrary instructions must establish
activation-wide preservation or reject the activation; there is no generic
`SafePoints | Asynchronous` runtime mode.

Checked providers derive preservation from their scheduler construction.
Opaque providers require an ordinary admission receipt. Missing evidence fails
closed. The receipt authorizes reliance on the claim without changing provider
behavior, and its provenance does not enter normalized runtime identity.
Address stability of stack-resident values follows from the fixed nonmoving
`StackLease`, not a separate runtime promise.

Cancellation support is a selected operation/conformance, stack capacity is a
resource reservation, and inline completion belongs to the concrete `start`
operation. Omega does not combine these independent facts into a universal
runtime-supply record.

The deferred composition-proof model will consume the same policies,
provenance anchors, operation contracts, concrete interaction edges, and
provider evidence; carry is an input to that model, not a miniature trace
language. Transitive `reaches` remains an audit/authority set and is not used as
the concrete interaction graph.

## Task Storage: Accountable, Provider-Planned

Task execution has three deliberately separate owners:

- the runtime provider has operational custody of the activation;
- an Arena-issued Allocation, provider, remote runtime, or platform owns its
  physical storage;
  and
- the holder of `Task<T>` owns the linear lifecycle claim.

`Task<T>` therefore does not universally contain or borrow a task stack. It is
normally a small provider/activation identity plus lifecycle authority. Moving
the handle does not move the activation's stack.

A parked stack and resume state remain compiler/provider-owned control storage. Ordinary
Omega code cannot project it from `Task<T>`, recast it as bytes or an address,
borrow another activation's frames, or mutate its saved return chain. Parking
does not turn compiler-owned live control state into ordinary addressable data.
The provider may retain and resume that storage but may not move the settled
stackful representation. This preserves the same return-integrity argument
across suspension, cancellation, and component replacement that applies while
the activation is running.

This opacity is enforced, not merely reserved by the model. Negative
canaries reject projecting, recasting, taking the address of, or mutating a
`continuation` through `Task<T>`. The same typed-place validation applies to
ordinary data locals and parameters, including applied generic carriers, so a
missing member cannot silently become a zero/default backend value.

Measured, tail-only runtime recursion leaves a bounded lowered call graph. If
ordinary calls may suspend, the parked stack can retain a bounded chain of
compiler-planned frames; bounded does not mean single-frame or free. WCSU plus
target calling plans derive stack bytes and alignment. The runtime transfers a
matching fixed, nonmoving `StackLease` into the activation before returning a
pending task.

Storage strategy is not fixed by the language:

- a hosted provider may allocate an OS stack internally;
- an Arena-backed provider may lease a pre-provisioned stack slot;
- a remote provider may keep no local activation frame; and
- an inline provider may return an already-complete, still-unsettled task when
  the pinned contract permits inline completion.

A future stackless representation is a distinct pre-lowering plan. Supporting
both representations means lowering the machine twice; a runtime does not
choose a representation for already-lowered control state.

`ArenaTaskPool` is the standard bounded-storage reference package and likely
Cathedral default, not a task primitive. It imposes slot/arena layout and
dynamic availability accounting on an Arena. Provisioning fixes a maximum;
each start still consumes capacity and settlement releases it. Under shared
interference, an available-capacity proposition does not replace ownership of
a real reservation; dynamic sites use fallible start/reserve operations.

Provider provenance remains attached to the claim. A task backed by local
storage cannot escape that storage's lifetime, and a pool/runtime cannot close
while dependent claims or leases remain. The exact child-lease representation
is part of the permission/resource-algebra implementation.

### Foreign execution

A direct opaque foreign call runs on the current activation stack and therefore
contributes an admitted same-stack demand to that activation's `StackPlan`.
An ordinary blocking-executor package may suspend an Omega caller and run
native code on a bounded pool of guarded OS-worker stacks. This is assembled
from activations, queues, moved custody, linear completion claims, suspension,
and provider selection; it is not a language call kind. The safe point is
reached when the caller parks, while native completion and later admission may
remain unbounded. A detached in-process call pins its worker, storage, and
provider era until return; bounded recovery from a hang requires isolation.
Retained native pointers use call-scoped borrows when use ends before return,
or ordinary ownership-conserving protocol claims when it does not. Registered
callback entry is settled: a named static machine satisfies the callback
requirement, the binding emits its plan-driven thunk, and a durable protocol
returns a linear registration value. Platform adapters normalize native
re-entry into locally checked handler surfaces.

## Cancellation Is A Value At The Wait

There is no unwinding, so cancellation never semantically interrupts a task
mid-state. Architectural preemption may still preserve and restore opaque state
there. A provider whose contract supports cancellation delivers the request at
a stated safe point, commonly by making the current or next wait return the zero
case instead of a ready value. The machine transitions to its own cleanup path
and drops run as frames retire normally:

```omega
data Take {
    case Cancelled;            // zero case: the parked wait was cancelled
    case Got(frame: Frame);
}

machine Worker::run(&mut self, ring: &mut Ring) {
    let taken: Take = suspend ring.take();
    transition taken {
        Take::Got(frame) -> work(frame)
        Take::Cancelled  -> cleanup()   // ordinary transition; nothing interrupted
    }
    ...
}
```

A task that never suspends is finishable but not necessarily cancellable -- its
complete machine contract says which cancellation behavior it supports.
Cancellation rides the same propagation
channel as recoverable errors ([chapter 16](chapter_16_errors_traps_failure.md));
the exact spelling follows that chapter's model.

## There Is No Select

Waiting on multiple sources is a DATA design, not a control construct:
producers post into ONE mailbox carrying a case-bearing sum, and the
consumer does one wait and one ordinary transition (Erlang's one-mailbox
model; also exactly Cathedral's IPC-ring shape):

```omega
data Event {
    case None;                  // zero case
    case Packet(frame: Frame);
    case Tick;
    case Shutdown;
}

machine Server::run(&mut self) {
    let event: Event = suspend self.inbox.take();

    transition event {                       // a completely ordinary transition
        Event::Packet(frame) -> handle(frame)
        Event::Tick          -> heartbeat()
        Event::Shutdown      -> drain()
        _                    -> run()
    }
}
```

The NIC interrupt posts `Packet`, the timer posts `Tick` -- interrupts and IO
completions POST TO WORDS. The core library owes a multi-producer
single-consumer event queue over the wait primitive; the language owes
nothing.

## Completion And Supervision

Finishing a task is an ordinary possibly-suspending machine call. It uses the
ordinary acknowledgement and does not turn the result into a future:

```omega
machine Scheduler::run(runtime: &TaskRuntime, job: Job) -> WorkResult {
    let task: Task<WorkResult> =
        runtime.start<Worker::run>(move job);

    do_other_work();

    transition suspend task.finish() {
        Returned(result) -> result
        Cancelled -> cancelled_result()
        Failed(receipt) -> provider_failure(receipt)
    }
}
```

The core library spells the outer lifecycle sum `TaskOutcome<T>` with
`Returned`, `Cancelled`, and `Failed` cases. Task completion, cancellation, and
provider failure belong to that outer outcome; recoverable failure produced by
`Worker::run` belongs inside `WorkResult` (or its application sum).

There is no ownerless fire-and-forget operation. A caller that does not retain
the result transfers the task to ordinary owner data, commonly a supervisor:

```omega
machine App::start_logging(
    &mut self,
    runtime: &TaskRuntime,
    line: LogLine
) {
    let task: Task<LogResult> =
        runtime.start<Logger::write>(move line);

    self.logs.adopt(move task);
}
```

A supervisor is policy over owned task claims: it may retain handles, request
cancellation, finish children, classify outcomes, and restart work. It is not
the task runtime and need not own child frame storage. A failure mailbox is an
optional event-loop/library choice rather than a condition of task execution.

Arena-backed pools, bounded mailboxes, and supervisors are reference packages
over ordinary ownership, linearity, and boundary providers. The language adds
no pool, mailbox, nursery, scope, or manager construct.

## Waitable Contracts: Retained Substrate Direction

Deadlock checking requires visible wait contracts. The retained direction
uses one futex-shaped scheduler boundary (wait on a word/value condition and
wake N waiters), with higher-level operations implemented as libraries where
the target permits it:

- `Task<T>::finish` waits on the activation's completion state.
- `Mutex<T>::lock` waits on the lock word (happy path never waits).
- `Barrier<N>::wait` waits on the arrival-count word.
- `Pipe::read` / `Socket::recv` / event queues wait on their buffer words;
  the OS/ISR side POSTS to the word and wakes.

The abstraction must remain honest. A target mechanism that cannot refine the
pinned wait contract is an accepted/opaque boundary rather than a fake futex.
Wait operations declare `suspends`, `blocks`, or both as their contracts
require; wake-only operations declare neither merely because they reach the
scheduler.
“What can unblock this wait?” remains part of the temporal contract used by the
deadlock model below.

## Atomics

Ownership is the primary mutual-exclusion story: two concurrent graphs cannot
hold `&mut` to the same data, so most code never sees a data race. Atomics are
the sanctioned carve-out -- the "data types whose contracts permit concurrent
access" -- for the places where genuinely shared mutable state is the point:
schedulers, shared rings, counters, and lock-free structures.

Atomics are dedicated core types with an explicit ordering on every operation.

```omega
data TicketLine {
    next_ticket: AtomicU32;
}

machine TicketLine::take(&mut self) -> u32 {
    self.next_ticket.fetch_add(1, NoOrdering)
}
```

Working rules:

- Atomic types are distinct core types (`AtomicU32`, `AtomicU64`,
  `AtomicBool`); ordinary integers never silently become atomic.
- Every operation names its ordering: `NoOrdering`, `Receive`, `Publish`,
  `ReceivePublish`, or `GlobalOrder`. These correspond to the conventional
  relaxed, acquire, release, acquire-release, and sequentially consistent
  literature terms while naming the relationship the source operation asks
  for.
- Operation legality is checked before lowering: loads allow
  `NoOrdering | Receive | GlobalOrder`; stores allow
  `NoOrdering | Publish | GlobalOrder`;
  read-modify-write success orderings allow the full vocabulary. A
  `compare_exchange` failure ordering performs only a load, so it cannot be
  `Publish`/`ReceivePublish` or stronger than the success ordering.
  These legality judgments are normalized proposition applications (for
  example, `valid_store_order(order)`), so a generic atomic helper may carry
  the same fact through `requires` or `ensures`. See
  [Chapter 10](chapter_10_compile_time_proofs.md#proposition-declarations).
- The operation set is load, store, swap, `compare_exchange` (with separate
  success/failure orderings), and the fetch-and-modify family.
- The implemented load/store/fetch_add/fetch_sub/fetch_xor/fetch_or/fetch_and/swap/
  compare_exchange slice carries ordering as normalized operation data through
  both backends. The parser accepts exactly the vocabulary above and rejects
  the conventional literature spellings as source names. Instruction selection
  is not by itself a formal memory-model or target-refinement proof.
  Fetch and swap return the prior observed by that instruction, not by a
  preceding load. Compare-exchange reports success without repeating the
  expected value and carries the instruction observation on failure. Swap uses
  a first-class carrier and
  therefore does not manufacture an arithmetic-domain proof obligation.
  `fetch_sub` performs exact-width two's-complement subtraction through one
  locked `XADD`/ordered `LDADD`. `fetch_xor` lowers to an ordering-selected
  `LDEOR` on AArch64 and to a locked `CMPXCHG` retry loop on x86_64; its result
  is the prior value observed by the successful attempt.
  `fetch_or` uses the same successful-attempt rule, lowering to ordered `LDSET`
  on AArch64 and the shared locked retry loop on x86_64.
  `fetch_and` lowers to complement-plus-ordered-`LDCLR` on AArch64 and the
  shared locked retry loop on x86_64.
- Atomics are exempt from the exclusive-`&mut` aliasing rule by their
  contracts: shared access is the type's documented purpose, not a borrow
  checker escape used elsewhere.
- A zeroed atomic is the value zero, consistent with zero initialization
  ([Memory Layout And ABI](chapter_20_memory_layout_abi.md)).

Generic helpers use one sealed `omega::core` requirement per atomic operation,
not one universal atomic trait. Ordinary core atomics and placed accessors
conform to the same requirements. A helper may therefore require load and
compare-exchange without claiming that its argument also supports fetch-add,
and a placed accessor exposes exactly the subset admitted by its selected
provider. Requirement receivers are shared, ordering remains explicit
proof-static operation data, and a lookalike user trait grants no atomic
semantics.

Every atomic operation requires a fixed representation that fits one
target/provider-supported atomic width and alignment. Further eligibility is
operation-specific: load requires duplication, store requires the displaced
resident to be discardable, and swap conserves the incoming and outgoing
values and may therefore transfer an affine or linear resident owned by a
Stable initialized placement. Cross-activation sharing is checked separately
and requires the resident type to be transferable.

Compare-exchange has two independent axes and four ordinary flat core outcome
types:

```omega
pub data AtomicCompareExchangeOutcome<T> {
    case Mismatched(observed: T);
    case Exchanged;
}

pub data AtomicCompareExchangeOnceOutcome<T> {
    case Mismatched(observed: T);
    case Exchanged;
    case Uncommitted(observed: T);
}

pub data AtomicTryExchangeOutcome<T> {
    case Mismatched(proposed: T);
    case Exchanged(displaced: T);
}

pub data AtomicTryExchangeOnceOutcome<T> {
    case Mismatched(proposed: T);
    case Exchanged(displaced: T);
    case Uncommitted(proposed: T);
}
```

The declaration order above is canonical and therefore fixes tags zero, one,
and, where present, two. Case paths are the ordinary flat nominal paths, such
as `AtomicCompareExchangeOnceOutcome::Uncommitted`; no enclosing atomic-result
namespace is implied. `Outcome` describes these expected closed terminal
states, but does not reserve `Result` or impose one universal result taxonomy:
an ordinary library remains free to declare a generic `Result<T, E>` when that
shape is useful.

`AtomicCompareExchange<T>` is decisive and observing.
`AtomicCompareExchangeOnce<T>` is its observing single-attempt sibling. Both
require a copyable resident because failure exposes its current value.
`AtomicTryExchange<T, Key>` and `AtomicTryExchangeOnce<T, Key>` are the
non-observing decisive and single-attempt siblings. Every try-exchange branch
returns exactly one owned `T`: the uncommitted proposal on failure and the
displaced resident on success. They may therefore transfer affine or linear
custody when the placement owns the resident. `Key` is a copyable comparison
input governed by one exact selected encoding law. The key is not returned;
neither the key nor the law parameterizes the runtime outcome type.
The selected law cannot erase Type-side custody: success always returns the
displaced resident, and ordinary multiplicity alone decides whether the caller
may discard it. Each generic outcome is an ordinary affine container whose
active payload carries any substituted affine or linear debt; no new
multiplicity rule is introduced.

For both axes, mismatch and uncommitted outcomes use the read-compatible
failure ordering and success uses the success ordering. Comparison is over the
stored representation selected by the operation law, not user-defined
equality. `Once` always denotes a weak single attempt; it never means
non-observing.

The zero tag is deliberately `Mismatched`, never success. A freshly constructed
outcome establishes only its case and payload; no outcome value by itself proves
that an atomic event occurred. That authority comes from the operation contract
and checked call. If zero does not establish the payload `T`, the outcome is not
usable until constructed. Storage that must exist before execution uses a
separate `Empty | Ready(outcome)` wrapper rather than adding a non-outcome case
to these exact operation results.

The current compiler preserves the observing single-attempt operation as a
distinct checked ordering and permission identity, but does not yet admit its
source call or lower it. Implementation must add the four carriers and retain
their exact nominal identity, case order, payload schema, and conditional
linear debt through checked source, Terminal Psi, interpretation, native
lowering, package review, and replay. Until then, the single-attempt operation
continues to reject rather than erase `Uncommitted` by using the decisive
carrier. The existing decisive source operation migrates from its legacy
observed-prior scalar to `AtomicCompareExchangeOutcome<T>` with a directed
diagnostic; it does not retain a scalar overload.

`Receive` uses the strong portable baseline. A target may select a weaker
acquire instruction only when a protocol proof establishes that every
additional execution preserves the protocol's published facts. Shared
unspecialized code remains on the baseline.

`Atomic::fence` accepts the dedicated cases
`Receive | Publish | ReceivePublish`. A fence is a normalized atomic-memory
event even when its target realization emits no instruction. It synchronizes
only through a qualifying atomic observation; it does not publish arbitrary
ordinary memory by itself. Checked ISA barriers and device/DMA visibility are
separate target/provider contracts.

`Atomic::interruption_fence` provides compiler ordering between ordinary
execution and an asynchronously entered handler on that same execution
context. Installed-root evidence must establish that relationship; source code
cannot assert it. The operation provides neither cross-core synchronization
nor device visibility.

DMA publication, device acquisition, cache maintenance, MMIO notification,
and posted-write completion are separate device-protocol roles. They are not
stronger spellings of `Atomic::fence`: a CPU fence can correctly order CPU
participants while establishing nothing for a device. The intended checked
driver model exposes those roles as explicit custody transitions rather than
hiding them behind an untyped fence.

The first source-admitted rung is deliberately narrower. An opaque or hosted
provider may expose one complete DMA service boundary selected and admitted by
`build.omg`; its internal publication, cache, notification, and completion
steps remain provider-private. This compatibility boundary does not authorize
checked source to compose the five roles or manufacture their evidence. A
source-visible primitive family waits for a concrete checked driver whose
protocol fixes the exact role-specific arguments and results.

Build selection admits the provider and its scope-capability schema; it does
not mint one ordering-scope occurrence. A runtime device, queue, or session may
have many occurrences under one selected provider. The installed provider
issues each sealed occurrence, and source may carry or pass that capability but
cannot construct, inspect, or compare its identity. Every emitted operation
binds the exact mapped range, mapping, stable device instance, runtime scope
occurrence, and role. The role discriminant is an input to canonical identity,
not merely a selector used while decoding an otherwise identical payload.

Starting an admitted transfer creates a linear pending loan bound to one exact
external-loan occurrence. Pre-commit rejection returns every consumed
candidate unchanged. Acquisition restores Stable CPU custody only when exact
completion evidence proves that the external borrower released that loan.
Device status and custody release are independent: a failed device operation
may have released the range, while a success-looking report that does not prove
release cannot restore CPU access. A non-releasing, stale, mismatched, or
incomplete completion therefore returns the pending loan and completion
candidate for resolution rather than fabricating Stable custody. Missing
provider coverage rejects compilation or installation and has no runtime
`Rejected` arm; ordinary runtime device failures are explicit protocol
outcomes, not crash edges.

Publication evidence is invalidated by any later write whose frame intersects
the published range. Passing erased evidence to a doorbell does not itself
create machine ordering; the publication operation contributes the scoped
ordering event that terminal Psi and target lowering must preserve. Acquisition
of device-written data consumes matching completion evidence. It establishes a
Stable CPU view only when the protocol also returns custody; otherwise the
storage remains External.

The current compiler foundation retains these five provider-operation roles as
distinct, exact structural rows and can close a supplied test row set against
one-to-one provider assertions. No checked source operation emits them. That
staging carrier neither proves provider
admission nor authorizes anything: source emission, ordering-scope validation,
Terminal events, publication/acquisition evidence, completion, custody
transitions, and target lowering are not implemented by it.

Atomics underpin the waitable types above (`Mutex`, `Barrier`) and shared-ring
IPC, so they sit below a concurrent task-runtime provider in the implementation
order even though they appear later in this chapter.

## Concurrent Protocol Model

The proof checker extracts atomic events from concurrent machine graphs. Atomic
reports spell the relations
`sequenced_before`, `reads_from`, `modification_order`, `synchronizes_with`,
`happens_before`, and `global_sequential_order`; abbreviated academic names are
not source or report vocabulary.

Ownership, receiver polarity, handle multiplicity, protocol state, claims, and
`invokes` already constrain which concurrent compositions are legal. A package
proves its implementation for every topology that its public API admits. Omega
does not add ambient environment assumptions to package contracts, and
`reaches` remains the transitive set of boundary services used for authority
and auditing rather than a concrete interaction graph.

When a concrete customer requires whole-composition protocol properties, the
compiler will assemble the facts it already tracks into one canonical sealed
proof-static model at final composition or deployment:

```text
activations = activation classes, creation bounds, core placement, priorities
resources   = concrete tasks, locks, queues, barriers, waits, external events
actions     = transitions, atomic events, spawn/join, acquire/release, wait/wake
edges       = invokes, waits-for, owns, releases, unblocks
premises    = selected scheduler, timing, fairness, and provider evidence
```

Only the compiler constructs this erased model. Ordinary proof machines may
consume it through `omega::core`; automatic profile checks cover known
disciplines such as ordered acquisition, structured joins, session endpoints,
and conserved permits. The checker verifies supplied proofs and does not search
for one. This model is deliberately deferred until a protocol or safety profile
needs it.

Implementation properties such as linearizability attach to the selected
implementation or conformance. Deadlock freedom, starvation freedom, bounded
memory, and response bounds attach to the composed deployment artifact with
their exact premises and trust provenance. A hot swap, provider change, or
topology change revalidates those properties.

Useful composition obligations include:

- A `finish` does not wait on an activation that waits back on its claimant.
- Lock acquisition order has no cycle.
- A blocking receive has a reachable sender, close, timeout, or external-event
  assumption.
- A barrier can reach its required arrival count.
- A host wait is either modeled, boundary, or rejected in the selected proof
  mode.

Structural and parametric guarantees do not require a closed activation set:
ownership remains race-free under dynamic spawning, ordered acquisition cannot
form a lock cycle, and a session protocol may govern every dynamically created
session. Quantitative whole-system guarantees instead require a closed
interference envelope: fixed topology, creation bounded by conserved permits,
enforced admission rates, or an authored proof quantified over the dynamic
structure. An external arrival rate is never inferred; it is admitted with
provenance or converted into a derived admitted-work bound by an enforcing rate
limiter whose rejection/backpressure is part of the service contract.

Bounded exploration is testing, not a contract or artifact guarantee. A theorem
whose statement deliberately contains a bound remains an ordinary theorem; for
example, proving a protocol for at most eight participants is distinct from
searching four participants without a proof.

## Minimal Deadlock Shapes

Task-completion cycle:

```text
A waits for B
B waits for A
```

Lock-order inversion:

```text
thread 1: lock A, then lock B
thread 2: lock B, then lock A
```

Missing producer:

```text
worker waits on queue.recv
no reachable send, close, timeout, or external event can unblock it
```

These are proof obligations over waitable contracts, not special cases baked
into task-start syntax.

## Proof Modes

Different builds may ask for different concurrency guarantees.

- Memory-safe concurrency: ownership, move/copy, and borrow rules hold.
- Internal-deadlock-free: no cycle among known internal waitable resources.
- Blocking-audited: every waitable host boundary is modeled, boundary, or
  reported.
- Progress-admitted: external waits name granted progress profiles, timeout,
  cancellation, or explicit provider evidence. General machine-side progress
  proofs wait for the deferred composition model.

Multicore response and blocking guarantees additionally select a scheduler and
resource-sharing protocol. A single-core priority-ceiling theorem does not lift
to cross-core shared resources. A deployment profile must partition resources
per core, forbid cross-core sharing, or select a proved multiprocessor protocol
with its own blocking analysis. These are provider/profile choices, not source
keywords.

Servers, kernels, drivers, CLIs, and embedded firmware do not all want the same
definition of "may block." The proof mode should be explicit in build artifacts.

## Connection To Boundaries

Host and OS waits are part of the same contract system as other imported
entries.

If a platform entry can block, its contract must say what unblocks it or mark
the wait as boundary/opaque. A proved-concurrency build may reject opaque waits.

This keeps the language honest: the checker can prove the parts it can see, and
the build report names the boundary providers for the parts it cannot.
