# Chapter 18: Concurrency

Concurrency uses ordinary machines plus an admitted task-runtime capability:

```omega
let task: Task<WorkResult> =
    runtime.start<Worker::run>(move job);

do_other_work();

let outcome: TaskOutcome<WorkResult> = task.finish();
```

`Worker::run` is not a special async function. Calling `Worker::run(job)` runs
it in the current activation; supplying the machine symbol to
`runtime.start<Worker::run>(job)` asks a runtime provider to establish a
distinct concurrent activation.

The model has no bare `spawn` block, `async machine`, `Future<T>`, mandatory
`await` marker, implicit detach, or privileged task group. Starting,
cancellation, completion, storage provisioning, and supervision are ordinary
contracted operations over explicit capabilities and linear data.

`TaskRuntime` is a working core boundary requirement/capability, not a new
language construct. Starting and controlling a task reach that service through
the ordinary effects/provider model from chapter 19.

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

A rejected start returns every moved argument and caller-supplied reservation.
Start is an ownership transaction: no value or linear obligation disappears
into a provider that failed to establish the activation.

The `<Worker::run>` spelling uses the compile-time machine-parameter mechanism
from chapter 13. It is not a runtime function pointer or inferred capture. The
compiler monomorphizes the target and emits a static activation plan containing
the normalized contract, entry identity, argument/result layouts, and
continuation/storage requirements. The provider receives that plan plus the
invocation's moved arguments.

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

## Suspension (elaboration pending)

Suspension is an operational property of an ordinary machine. Decision 22
supplies distinct `Suspend` and `Block` row members; absence of each is the
corresponding negative guarantee. The continuation, loan, and lowering rules
remain to be frozen.

Decision 23 keeps positive progress separate. Pinned operations/providers may
carry sealed opaque progress profiles authorized through boundary grants. A
termination guarantee records the actual required profiles; the presence of
`Suspend` or `Block` says only that such an event is possible and cannot name
what will wake it. General trace entailment remains deferred.

The constraints that *are* settled are:

- suspension is an operational part of the ordinary machine contract, not a
  `Future` return type or a separate `async machine` species;
- a caller/context imposes an effect and resource ceiling, so a provider whose
  row contains `Suspend` or `Block` cannot satisfy a slot that omits it;
- suspension composes through ordinary calls without a call-site marker;
  visibility comes from inferred effects, public contract ceilings,
  diagnostics, and artifacts;
- automatic cleanup may execute but may never suspend or fail;
- `Task<T>` is linear and must be settled or transferred explicitly; and
- a loan may cross suspension only when the eventual suspension model can
  prove its storage, pinning, aliasing, and cancellation safety. Blanket
  acceptance and blanket rejection are both premature.

Suspension composes through ordinary calls, with the effect propagated/inferred
and bounded continuation storage planned by the compiler. Public rows are
explicit ceilings; internal rows infer. Exact
continuation lowering and suspension-safe-loan rules remain the queued
suspension amendment. See
[effects_authority_and_observation.md](../design_briefs/effects_authority_and_observation.md).

## Task Storage: Accountable, Provider-Planned

Task execution has three deliberately separate owners:

- the runtime provider has operational custody of the activation;
- a Region, provider, remote runtime, or platform owns its physical storage;
  and
- the holder of `Task<T>` owns the linear lifecycle claim.

`Task<T>` therefore does not universally contain or borrow a task stack. It is
normally a small provider/activation identity plus lifecycle authority. Moving
the handle does not move a parked continuation.

Measured, tail-only runtime recursion leaves a bounded lowered call graph. If
ordinary calls may suspend, a parked continuation can retain a bounded chain
of compiler-planned frames; bounded does not mean single-frame or free. The
activation plan records frame/continuation size, alignment, address-stability,
and related requirements. The runtime provider must admit that plan against
its storage contract before returning a pending task.

Storage strategy is not fixed by the language:

- a hosted provider may allocate an OS thread stack internally;
- a Region-backed provider may lease a pre-provisioned frame slot;
- a remote provider may keep no local activation frame; and
- an inline provider may return an already-complete, still-unsettled task when
  the pinned contract permits inline completion.

`RegionTaskPool` is the standard bounded-storage reference package and likely
Cathedral default, not a task primitive. It imposes slot/arena layout and
dynamic availability accounting on a Region. Provisioning fixes a maximum;
each start still consumes capacity and settlement releases it. Under shared
interference, an available-capacity proposition does not replace ownership of
a real reservation; dynamic sites use fallible start/reserve operations.

Provider provenance remains attached to the claim. A task backed by local
storage cannot escape that storage's lifetime, and a pool/runtime cannot close
while dependent claims or leases remain. The exact child-lease representation
is part of the permission/resource-algebra implementation.

## Cancellation Is A Value At The Wait

There is no unwinding, so a task is never interrupted mid-state. A provider
whose contract supports cancellation delivers the request at a stated safe
point, commonly by making the current or next wait return the zero case instead
of a ready value. The machine transitions to its own cleanup path and drops run
as frames retire normally:

```omega
data Take {
    case Cancelled;            // zero case: the parked wait was cancelled
    case Got(frame: Frame);
}

machine Worker::run(&mut self, ring: &mut Ring) {
    let taken: Take = ring.take();  // may suspend under the eventual effect contract
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
    let event: Event = self.inbox.take();   // one wait source; may suspend

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

Finishing a task is an ordinary possibly-suspending machine call; it needs no
`await` marker:

```omega
machine Scheduler::run(runtime: &TaskRuntime, job: Job) -> WorkResult {
    let task: Task<WorkResult> =
        runtime.start<Worker::run>(move job);

    do_other_work();

    transition task.finish() {
        Returned(result) -> result
        Cancelled -> cancelled_result()
        Failed(receipt) -> provider_failure(receipt)
    }
}
```

The exact core outcome names remain library spelling, but the separation is
semantic: task completion, cancellation, and provider failure belong to the
outer lifecycle outcome; recoverable failure produced by `Worker::run` belongs
inside `WorkResult` (or its application sum).

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

Region-backed pools, bounded mailboxes, and supervisors are reference packages
over ordinary ownership, linearity, and boundary providers. The language adds
no pool, mailbox, nursery, scope, or manager construct.

## Waitable Contracts: Retained Substrate Direction

Deadlock checking requires visible wait contracts. The retained v1 direction
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
Wait operations carry `Suspend`, `Block`, or both as their contracts require;
wake-only operations carry neither merely because they reach the scheduler.
“What can unblock this wait?” remains part of the temporal contract used by the
deadlock model below.

## Atomics

Ownership is the primary mutual-exclusion story: two concurrent graphs cannot
hold `&mut` to the same data, so most code never sees a data race. Atomics are
the sanctioned carve-out -- the "data types whose contracts permit concurrent
access" -- for the places where genuinely shared mutable state is the point:
schedulers, shared rings, counters, and lock-free structures.

The direction is Rust-like atomics: dedicated core types with explicit
orderings on every operation.

```omega
data TicketLine {
    next_ticket: AtomicU32;
}

machine TicketLine::take(&mut self) -> u32 {
    self.next_ticket.fetch_add(1, Ordering::Relaxed)
}
```

Working rules:

- Atomic types are distinct core types (`AtomicU32`, `AtomicU64`, `AtomicBool`,
  `AtomicUsize`); ordinary integers never silently become atomic.
- Every operation names its ordering: `Relaxed`, `Acquire`, `Release`,
  `AcqRel`, `SeqCst` -- the C11/Rust vocabulary, because hardware, existing
  literature, and audit expectations all speak it.
- The operation set is load, store, swap, `compare_exchange` (with separate
  success/failure orderings), and the fetch-and-modify family.
- Atomics are exempt from the exclusive-`&mut` aliasing rule by their
  contracts: shared access is the type's documented purpose, not a borrow
  checker escape used elsewhere.
- A zeroed atomic is the value zero, consistent with zero initialization
  ([Memory Layout And ABI](chapter_20_memory_layout_abi.md)).

Atomics underpin the waitable types above (`Mutex`, `Barrier`) and shared-ring
IPC, so they sit below a concurrent task-runtime provider in the implementation
order even though they appear later in this chapter.[^atomics-open]

[^atomics-open]: Open details: whether atomics lower as compiler intrinsics or
boundary operators with instruction contracts (intrinsics are the working
assumption -- they need exact codegen, not auditability, and have no authority
semantics); standalone fences; whether `SeqCst` is restricted or discouraged
in proofs; and how the TLA-style model treats relaxed-ordering visibility
(first cut: the deadlock model ignores ordering and only tracks waits).

## TLA-Style Model

The proof checker can extract a small transition model from concurrent machine
graphs.

```text
processes = concurrently activated machine graphs
resources = task completions, locks, queues, barriers, pipes, fd waits, external events
actions = machine transitions and waitable operations
edges = waits-for, owns, releases, unblocks
```

Then it can check properties such as:

- A `finish` does not wait on an activation that waits back on its claimant.
- Lock acquisition order has no cycle.
- A blocking receive has a reachable sender, close, timeout, or external-event
  assumption.
- A barrier can reach its required arrival count.
- A host wait is either modeled, boundary, or rejected in the selected proof
  mode.

This is not arbitrary-threaded-code magic. The language makes enough structure
visible that the compiler can build a finite proof model.

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
  cancellation, or explicit environment assumptions. General machine-side
  progress proofs wait for trace logic.

Servers, kernels, drivers, CLIs, and embedded firmware do not all want the same
definition of "may block." The proof mode should be explicit in build artifacts.

## Connection To Boundaries

Host and OS waits are part of the same contract system as other imported
entries.

If a platform entry can block, its contract must say what unblocks it or mark
the wait as boundary/opaque. A proved-concurrency build may reject opaque waits.

This keeps the language honest: the checker can prove the parts it can see, and
the build report names the boundary providers for the parts it cannot.
