# Chapter 18: Concurrency

Concurrency uses ordinary machines plus an admitted task-runtime capability:

```omega
let task: Task<WorkResult> =
    runtime.start<Worker::run>(move job);

do_other_work();

let outcome: TaskOutcome<WorkResult> = suspend block task.finish();
```

Calling `Worker::run(job)` runs in the current activation. Passing the static
machine symbol to `runtime.start<Worker::run>` asks the selected runtime to
establish another activation. There is no implicit capture, future, or detach.

Examples in this chapter illustrate the intended contracts, not complete native
task support. They assume the named library operations and types are in scope.
The start operation shown neither suspends nor blocks; the completion operation
shown permits both. Call markers always follow that operation's actual contract,
not the eventual behavior of the worker. The
[task-runtime specification](../spec/build/task_runtime.md) and its
[implementation note](../../omega-rust/omega/representations/task-plans/README.md)
separate required behavior from current planning support.

## Starting Is A Provider Operation

`TaskRuntime` is an ordinary boundary capability. Provider selection does not
grant ambient authority: the caller still needs the capability and must satisfy
the operation's admission and resource obligations.

`start<M>` requires those obligations to be proved. `try_start<M>` handles genuine
dynamic refusal through an ordinary sum:

```omega
transition runtime.try_start<Worker::run>(move job) {
    Started(task) -> keep(move task)
    Rejected(job, reason) -> recover(move job, reason)
}
```

Start is an ownership transaction. Rejection returns moved arguments and
caller-supplied reservations, or proves their transfer to another authorized
owner. Known static incompatibility rejects admission rather than becoming an
avoidable runtime failure. A pending task is returned only after matching
provider custody and storage exist.

An activation plan describes the exact entry, contract, layouts, carry demands,
and whole-call-graph stack requirement. A numeric capacity fact is not a
reservation: another starter must not be able to spend the same capacity.
Static planning is also not an invocation receipt. See
[transactional start](../spec/build/task_runtime.md#transactional-start).

## Task Is A Linear Lifecycle Claim

`Task<T>` is ordinary linear data: its owner must settle or transfer one
activation's lifecycle obligation. It can be stored for later work:

```omega
data WorkerState {
    case Idle;
    case Running(task: Task<WorkResult>);
}
```

Moving the task into `Running` transfers its obligation. Copying, overwriting,
dropping it at ordinary scope exit, or losing it on a branch cannot discharge
that obligation. `request_cancel()` retains the claim; requesting cancellation
does not prove execution stopped. `finish(self)` consumes it into a terminal
outcome such as `Returned(value)`, `Cancelled`, or `Failed(receipt)`.
Application-level recoverable failure belongs inside `T`, not a second meaning
of the outer lifecycle outcome.

Long-lived background work transfers ownership explicitly:

```omega
supervisor.adopt(move task);
```

This is not detach. The supervisor becomes responsible for settlement. Automatic
cleanup cannot join, park, fail, or silently settle a task.

## Suspension, Blocking, And Direct Calls

`suspends` and `blocks` are independent operational ceilings on ordinary machine
contracts. Their absence on a published contract supplies the corresponding
negative guarantee; private omissions infer from the body. Neither is a future
type, and neither implies positive progress or eventual completion.
A provider cannot satisfy a narrower requirement by hiding its wider behavior.

### Call-site acknowledgements

Direct calls acknowledge the statically permitted behavior:

```omega
operation();
suspend operation();
block operation();
suspend block operation();
```

These lines illustrate four different operational envelopes. The markers mean
*may*, not *did* or *must*: a permitted wait may complete immediately. Missing,
partial, redundant, or reversed acknowledgements reject. Abstract calls use the
published requirement's envelope; a proved `suspends false` or `blocks false`
refinement can narrow it.

Suspension parks the activation while retaining its fixed stack. The call must
be a complete statement, simple `let` right-hand side, transition subject, or
terminal expression. A blocking-only call may nest because it retains ordinary
call execution. A call permitting both uses the order `suspend block` and the
suspension position rule:

```omega
let guard: Guard = block mutex.lock();
let event: Event = suspend inbox.take();
let wide: Event = suspend block source.take();

// Rejected: partially evaluated expression state would cross suspension.
let total: u64 = prefix + suspend source.next();
```

The markers make waiting sites reviewable, especially while a guard or scarce
authority remains live. They do not change return types, propagate a different
contract, or create another activation. See
[call-site acknowledgements](../spec/language/effects.md#call-site-acknowledgements).

### Parked call lifecycle

A parked call is incomplete. It has established no result, executed no later
operations, and run no cleanup. Its values, loans, claims, and cleanup duties
remain in the activation's live frontier. Resumption continues that same call;
normal completion then makes its result available.

Parking is not a return, crash, or second local successor. A static crossing in
a loop may be visited repeatedly, but each parked dynamic occurrence has its
own linear resume custody. Cancellation creates no hidden edge that discards
this state. See [Terminal suspension](../spec/terminal-psi/calls_and_outcomes.md#suspension)
for the exact call/frontier and runtime-preservation joins.

Safety is not progress. Preserving parked custody does not prove the call will
resume. A root requiring bounded response needs finite-wait evidence for its
dependencies; a general root may instead report the absence of that guarantee.

### Preemption and safe points

Architectural preemption may pause and restore opaque register/stack state at
any instruction without changing the activation's semantic circumstances.
It requires no source safe point. A semantic safe point is an explicit operation
at which structured cancellation, permitted migration, or replacement can occur:

```omega
process_simd_chunk(buffer);   // no semantic suspension; preemption is possible
suspend scheduler.poll();    // explicit semantic safe point
```

Loops, state transitions, allocations, and ordinary calls are not implicit safe
points. Optimization cannot insert a suspending poll into a non-suspending
kernel. Blocking creates no safe point either; an unbounded wait can leave
cancellation response and replacement quiescence unbounded.

Whole-call-graph WCSU bounds simultaneously live stack space, not work or time.
Sequential work adds; sequential stack frames can reuse capacity after return.
Logical-work bounds create no native fuel counter or fuel-induced suspension.
Converting work to elapsed time requires a separate timing model. Hard-control
profiles reject unknown or absent finite response guarantees rather than force-
terminating a blocked holder. See [logical work](../spec/resources/logical_work.md).

## Carry Policy Is A Product

Values retained across execution transitions impose four independent demands:

| Axis | Possibilities |
| --- | --- |
| Suspension | Allowed or forbidden. |
| CPU | Same or any CPU. |
| Host thread | Same or any thread. |
| Address | Stable or movable. |

Transparent data combines its live fields' demands. Type-wide `[carry(...)]`
properties and per-value permissions use the same ordering. Admitted resource
claims begin strict; their contracts may establish `Carry::AcrossSuspend`,
`Carry::AnyCpu`, `Carry::AnyThread`, and `Carry::MovableAddress`.
`Carry::Portable` means all four permissions.

Moves, containment, loans, and conserved splits preserve provenance. Combining
origins takes the stricter demand on each axis. Forgetting a permission narrows
what is allowed; forgetting an authority qualification cannot erase its live
claim or carry obligation.

Exclusive transfer into another activation requires both ownership and runtime
compatibility. Sharing additionally needs a sanctioned shared-access contract;
copyability alone does not authorize concurrent mutation. CPU affinity and host-
thread affinity are distinct.

A live suspension-forbidden value rejects a possible suspension locally.
Provider selection cannot erase that ceiling. CPU/thread preservation instead
follows the activation's demands: portable values require neither guarantee.
A host that migrates outside semantic points must establish activation-wide
preservation when the activation may retain restricted values. Checked providers
derive it; opaque providers supply exact admitted evidence. A receipt does not
change the host's actual behavior.

Stack residents have stable addresses because their stack lease is nonmoving,
not because a runtime supplies an arbitrary promise. A live reference carrier
alone also does not justify crossing suspension: lifetime, pinning, aliasing,
cancellation outcomes, and address stability all need evidence. See
[carry](../spec/resources/carry.md) and [task loans](../spec/build/task_runtime.md#carry-and-loans).

## Task Storage: Accountable, Provider-Planned

Keep three owners separate:

| Owner | Responsibility |
| --- | --- |
| Runtime provider | Execute, park, wake, and honor cancellation contracts. |
| Storage owner | Account for backing and its lifetime. |
| `Task<T>` holder | Settle or transfer the lifecycle claim. |

Moving a task handle does not move its activation stack. Each local activation
receives fixed nonmoving backing matching its stack plan before execution and
retains it while parked. The parked control state is compiler/provider-owned;
source cannot project it from `Task<T>`, recast it as bytes, borrow another
activation's frames, or modify its saved return chain.

A hosted provider may allocate a stack; an Arena-backed provider can lease a
pre-provisioned slot; a remote provider accounts for execution-domain storage.
Inline completion can shorten storage lifetime but still returns an unsettled
linear task claim. These are provider strategies, not runtime-selectable
representations for already-lowered control state.

A bounded pool fixes maximum capacity while individual starts and settlements
account for availability. Tasks cannot outlive borrowed backing, and a pool or
runtime cannot close with dependent claims or leases. Fresh activation/lease
eras prevent reuse of stale settlement evidence. The
[task storage contract](../spec/build/task_runtime.md#three-owners) owns these
relationships and their exact runtime-instance accounting.

### Foreign execution

Opaque calls on the activation's stack contribute admitted same-stack demand.
A blocking-executor package may instead run native work on separately provisioned
worker stacks while the Omega caller parks. That package composes queues, moved
custody, and completion claims; it is not another language call kind.

Unreturned in-process work retains its worker, storage, and provider era.
Bounded recovery from a genuine hang requires suitable isolation. Pointers whose
use ends before return may use call-scoped borrows; retention after return needs
an ownership-conserving protocol. Callback entry has its own selected plan and
registration lifetime. See [foreign storage](../spec/build/foreign_storage.md)
and the [callback example](chapter_19_capabilities_effects_boundaries.md#foreign-callbacks-through-platform-adapters).

## Cancellation Is A Value At The Wait

Cancellation does not unwind arbitrary frames. It reaches source through an
ordinary outcome at a declared wait or safe point; source then takes its cleanup
path. This sketch assumes a suspending, nonblocking `take` contract:

```omega
data Take {
    case Cancelled;
    case Got(frame: Frame);
}

machine Worker::run(&mut self, ring: &mut Ring) {
    let taken: Take = suspend ring.take();
    transition taken {
        Take::Got(frame) -> work(frame)
        Take::Cancelled  -> cleanup()
    }
    ...
}
```

The provider must honor its cancellation-request operation, but that does not
promise every task eventually observes the request. A task that never suspends
may be finishable without being cancellable. Only terminal settlement consumes
the external task claim. Ordinary recoverable-outcome reasoning from
[Chapter 16](chapter_16_errors_traps_failure.md) still applies.

## There Is No Select

Multiple producers can post a case-bearing event into one mailbox. The consumer
waits once and uses an ordinary transition:

```omega
data Event {
    case None;
    case Packet(frame: Frame);
    case Tick;
    case Shutdown;
}

machine Server::run(&mut self) {
    let event: Event = suspend self.inbox.take();
    transition event {
        Event::Packet(frame) -> handle(frame)
        Event::Tick          -> heartbeat()
        Event::Shutdown      -> drain()
        _                    -> run()
    }
}
```

This is a library design, not a special select construct or a requirement that
every native event use one mechanism. Mailbox storage and send/receive endpoints
may have different owners. Closing a mailbox or losing a task must still leave
every undelivered linear payload with an accountable owner.

## Completion And Supervision

Using the opening example's possibly suspending and blocking finish operation:

```omega
transition suspend block task.finish() {
    Returned(result) -> use(result)
    Cancelled -> handle_cancellation()
    Failed(receipt) -> handle_provider_failure(receipt)
}
```

A supervisor owns task claims and policy: it can request cancellation, finish
children, classify outcomes, and restart work. It need not own frame storage or
be the runtime provider. A failure mailbox is optional; completion can read the
provider-held outcome directly. Pools, supervisors, and mailboxes remain ordinary
packages, with no ownerless fire-and-forget escape.

## Waitable Contracts: Retained Substrate Direction

A useful shared substrate is a small word/value wait plus wake-one/wake-many
boundary. Libraries can build completion waits, locks, barriers, and queues over
it where the target supports the required contract. This is an engineering
direction, not permission to describe unlike host mechanisms as equivalent.

Wait operations publish suspension/blocking and progress separately. Wake does
not inherit a waiting ceiling merely because it reaches the scheduler. A protocol
proof must know what can unblock a wait; otherwise the selected profile must
accept or reject that opaque dependency explicitly. See
[library and foreign providers](../spec/build/task_runtime.md#library-and-foreign-providers).

## Atomics

Ownership prevents ordinary shared mutation. Atomics provide sanctioned shared
access where a counter, lock, queue, or protocol actually needs it. They are
dedicated core types, not ordinary integers made implicitly atomic:

```omega
data TicketLine {
    next_ticket: AtomicU32;
}

machine TicketLine::take(&mut self) -> u32 {
    self.next_ticket.fetch_add(1, NoOrdering)
}
```

The result is the prior value observed by that atomic instruction, not a racing
earlier load. `NoOrdering` supplies the atomic access and per-location order,
not publication of other data.

Every operation names proof-static ordering. `Receive` can observe what preceded
a matching `Publish`; `ReceivePublish` combines those roles in a read-modify-write;
`GlobalOrder` participates in the shared global order. Loads cannot publish and
stores cannot receive. Compare-exchange has separate success/failure orderings;
failure is read-only and cannot exceed success's ordering.

Generic code selects one sealed requirement per operation, not a universal
atomic trait. Ordinary atomics and admitted placed accessors expose only their
supported operations. Width, alignment, representation, and resident-custody
conditions remain separate from arithmetic capability or shareability.

Compare-exchange has two independent choices: observing versus non-observing,
and decisive versus single-attempt. A single attempt may report `Uncommitted`
even when comparison matched. Non-observing exchange returns the proposal on
failure and displaced resident on success, conserving ownership. Constructing
an outcome value proves no atomic event occurred.

The [concurrency specification](../spec/language/concurrency.md) owns orderings,
observation guarantees, fences, and protocol requirements;
[placed access](../spec/resources/placed_access.md#atomic-compare-exchange-outcomes)
owns exact outcome types and operation-specific custody. Current support is
recorded in the [atomic implementation note](../../omega-rust/psi/foundation/language-core/atomic_operations.md).

A CPU fence synchronizes only through qualifying observations; it does not
publish arbitrary memory by itself. Same-context interruption ordering and
device/DMA visibility have different participants and evidence. In particular,
a successful device status does not prove release of a device loan. CPU access
returns only after exact release and ordering obligations are met. See
[device custody](../spec/resources/device_access.md).

## Concurrent Protocol Model

Atomic-event reasoning asks which observations and reorderings are legal.
Concurrent transition reasoning asks which activations own, wait for, release,
or unblock exact resources. Neither replaces ownership or initialization.
Service reach is an audit/authority set, not that concrete interaction graph.

Whole-composition extraction remains deferred until a concrete protocol or
safety-profile customer needs it. The intended sealed erased model retains
activation creation/bounds, resource identities, placement, priorities, wait/wake
edges, and selected provider premises. Ordinary proof machines consume it;
a guessed graph or bounded exploration supplies no proof authority.

Implementation properties such as linearizability belong to their conformances.
Composed deadlock freedom, starvation freedom, memory, and response guarantees
depend on topology and provider evidence and need revalidation when those change.
Dynamic creation remains legal; quantitative guarantees need a bounded
interference envelope or a proof quantified over the dynamic structure.

External arrival rates are assumptions with provenance unless an enforcing
limiter establishes the admitted-work bound. Its rejection or backpressure is
part of the service contract. A theorem explicitly covering eight participants
is different from merely exploring four participants without a proof. See
[protocol proofs](../spec/language/concurrency.md#protocol-proofs).

## Minimal Deadlock Shapes

Three useful questions for a protocol review are:

```text
Completion cycle:    A waits for B; B waits for A.
Lock inversion:     one path takes A then B; another takes B then A.
Missing producer:   receive has no reachable send, close, timeout, or event.
```

Barrier arrival counts and external waits need similar scrutiny. These are
obligations over the actual wait contracts, not special task-start syntax.

## Proof Modes

Build policy can ask for different guarantees: ownership safety, absence of
internal wait cycles, audited host blocking, or progress under exact admitted
premises. A memory-safe program does not automatically have bounded response.
Reports must make the selected guarantee and its assumptions explicit.

Multicore blocking analysis needs a scheduler and resource-sharing protocol.
A single-core priority-ceiling theorem does not extend automatically: partition
resources, prohibit cross-core sharing, or select a proved multiprocessor protocol
with its own blocking analysis. These are deployment choices, not source keywords.

## Connection To Boundaries

Host and OS waits use the same boundary-contract system as other imported
operations. A wait either has the unblocking/progress evidence the selected
profile requires or remains an explicit opaque dependency. The report names
what was admitted; the checker does not manufacture a completion guarantee from
the absence of visible code. See [Chapter 19](chapter_19_capabilities_effects_boundaries.md).
