# Chapter 18: Concurrency

Concurrency starts with one primitive idea:

```omega
spawn {
    Worker::run(move job);
}
```

`spawn` runs a block concurrently. It does not inject channels, callbacks, or
hidden runtime parameters. Values used by the spawned block follow the same
copy/move rules as ordinary calls, with stricter lifetime requirements.

Working rules:

- Captured values must be copied or moved -- UNLESS the spawn is scoped (below).
- Moved values are unavailable to the parent after the spawn.
- Shared mutation must go through data types whose contracts permit concurrent
  access.
- A spawn used as a statement is fire-and-forget when the proof checker proves
  the spawned graph is self-contained; statement-form spawn is the explicit,
  authorized detach operation.
- `Join<T>` is linear. `join`, `cancel`, or an authorized `detach` consumes it.
  A live handle at scope exit is a compile error. Automatic cleanup never
  blocks, and strict result use alone would not catch a bound handle that
  reaches scope end.

## Scoped Spawns (no keyword)

There is no `scope` construct: the lexical block IS the scope. A spawn may
borrow parent locals; the borrows are ordinary loans and every borrowing spawn
must be explicitly joined or cancelled before those loans can end:

```omega
machine Main::main(&mut self) {
    let mut totals: [u32; 2] = [0, 0];

    {
        let first: Join<()> = spawn { Worker::run(&self.ring, &mut totals[0]) };
        let second: Join<()> = spawn { Worker::run(&self.ring, &mut totals[1]) };
        first.join();
        second.join();
    }   // handles were consumed; the loans end; tasks are dead

    let sum: u32 = totals[0] + totals[1];   // legal: provably no live task
}
```

Disjoint `&mut` windows (distinct array elements above) follow the ordinary
borrow rules. A spawn that escapes any enclosing borrow scope stays
move/copy-only.

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
- `Join<T>` is linear and must be consumed explicitly; and
- a loan may cross suspension only when the eventual suspension model can
  prove its storage, pinning, aliasing, and cancellation safety. Blanket
  acceptance and blanket rejection are both premature.

Suspension composes through ordinary calls, with the effect propagated/inferred
and bounded continuation storage planned by the compiler. Public rows are
explicit ceilings; internal rows infer. Exact
continuation lowering and suspension-safe-loan rules remain the queued
suspension amendment. See
[effects_authority_and_observation.md](../design_briefs/effects_authority_and_observation.md).

## Task Storage: Bounded, Compiler-Planned

Measured, tail-only runtime recursion leaves an acyclic lowered call graph, so
the compiler can compute a finite worst-case activation bound. If ordinary
calls may suspend, a parked continuation can contain a bounded chain of
planned frames; bounded does not mean single-frame or free. Task capacity is
frame requirement times maximum simultaneous activations, with the activation
bound declared or proved.

Task pools are capacity-bearing resources. Their budget is the planned frame
requirement times the maximum simultaneous activations, with the activation
bound declared or justified from a finite resource such as permits, endpoints,
or a region budget. Spawning past the admitted capacity is a proof obligation
or explicit boundary failure. Actor-shaped machines often collapse retained
storage to their owned `self`; that remains a useful pattern rather than a
restriction imposed on all suspending helpers. Region-backed dynamic capacity
arrives with the allocator arc.

## Cancellation Is A Value At The Wait

There is no unwinding, so a task is never interrupted mid-state. Cancelling
a scope makes each child's current or next WAIT return the zero case
instead of a ready value; the machine transitions to its own cleanup path
and drops run as frames retire normally:

```omega
data Take {
    case Cancelled;            // zero case: the parked wait was cancelled
    case Got(frame: Frame);
}

machine Worker::run(&mut self, ring: &mut Ring) {
    let taken: Take = ring.take();  // may suspend under the eventual effect contract
    transition taken {
        Take::Got(frame) -> work(frame)
        Take::Cancelled  -> finish()    // ordinary transition; nothing interrupted
    }
    ...
}
```

A task that never suspends is joinable but not necessarily cancellable -- its
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

## Joined Work

If the result of `spawn` is kept, it is a join handle.

```omega
data Job {
    id: u32;
}

data WorkResult {
    job_id: u32;
    ok: bool;
}

data Worker {
}

machine Worker::run(job: Job) -> WorkResult {
    WorkResult {
        job_id: job.id,
        ok: true
    }
}

machine Scheduler::run(job: Job) -> WorkResult {
    let handle: Join<WorkResult> = spawn {
        Worker::run(move job)
    };

    handle.join()
}
```

`Worker::run` returns `WorkResult`. The `spawn` expression returns
`Join<WorkResult>` because the machine is running concurrently.

Programs that want recoverable failure return an explicit result sum from the
spawned machine. Trap propagation and thread-group termination belong to the
spawn/scheduler contract and must be settled there rather than inferred from
`Join<T>`.

## Fire And Forget

A spawn does not need to produce a value.

```omega
data Logger {
}

machine Logger::write(line: String) {
    platform_log(line);
}

machine App::run(message: String) {
    spawn {
        Logger::write(move message);
    }
}
```

If the spawn result is not bound, the proof checker treats it as intentionally
unjoined and proves the spawned graph does not depend on the parent stack.

## Waitable Contracts: Retained Substrate Direction

Deadlock checking requires visible wait contracts. The retained v1 direction
uses one futex-shaped scheduler boundary (wait on a word/value condition and
wake N waiters), with higher-level operations implemented as libraries where
the target permits it:

- `Join<T>::join` waits on the child's completion word.
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
IPC, so they sit below `spawn` in the implementation order even though they
appear later in this chapter.[^atomics-open]

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
processes = spawned machine graphs
resources = joins, locks, queues, barriers, pipes, fd waits, external events
actions = machine transitions and waitable operations
edges = waits-for, owns, releases, unblocks
```

Then it can check properties such as:

- A `join` does not wait on a spawned graph that waits back on the joiner.
- Lock acquisition order has no cycle.
- A blocking receive has a reachable sender, close, timeout, or external-event
  assumption.
- A barrier can reach its required arrival count.
- A host wait is either modeled, boundary, or rejected in the selected proof
  mode.

This is not arbitrary-threaded-code magic. The language makes enough structure
visible that the compiler can build a finite proof model.

## Minimal Deadlock Shapes

Join cycle:

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
into `spawn`.

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
