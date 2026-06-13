# Chapter 17: Concurrency

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
  the spawned graph is self-contained.
- Dropping a `Join<T>` JOINS: an unconsumed handle blocks at its scope's end
  until the child completes. Strict result use (frozen decision 9) already
  prevents silently ignoring the handle.

## Scoped Spawns (no keyword)

There is no `scope` construct: the lexical block IS the scope. A spawn may
borrow parent locals; the borrows are ordinary loans, loans must end before
the block ends, and drop-of-`Join` joins -- so the borrow checker forces every
borrowing spawn to be joined inside the block, with no new syntax:

```omega
machine Main::main(&mut self) {
    let mut totals: [u32; 2] = [0, 0];

    {
        let first: Join<()> = spawn { Worker::run(&self.ring, &mut totals[0]) };
        let second: Join<()> = spawn { Worker::run(&self.ring, &mut totals[1]) };
    }   // handles drop here -> implicit joins; the loans end; tasks are dead

    let sum: u32 = totals[0] + totals[1];   // legal: provably no live task
}
```

Disjoint `&mut` windows (distinct array elements above) follow the ordinary
borrow rules. A spawn that escapes any enclosing borrow scope stays
move/copy-only.

## Suspension: The `await` Marker (amends frozen decision 16)

Blocking is still calling -- a wait is an ordinary call to a boundary wait
primitive, not a separate `async` type, so there is NO function coloring and no
`Future`. But the call is MARKED with `await`, so every suspension point is
visible in source rather than hiding behind a plain call. Decision 16's
original no-keyword stance is amended here for exactly that visibility:

```omega
machine Server::handle(&mut self) {
    let frame: Frame = await self.ring.take();   // PARK here, visibly
    self.process(frame);                          // straight-line code resumes
}
```

The model:

- Waiting originates ONLY at boundary wait primitives -- a `Scheduler`
  boundary trait (`wait_until_nonzero(flag: &AtomicU32) effects suspend;`
  plus `wake_one`). Per-target bindings ride the existing host-provider
  machinery: hosted targets bind futex/WaitOnAddress syscalls; Cathedral
  userland binds the scheduler capability; the Cathedral kernel implements
  it over hlt/interrupt wakeups. Waiting lives where it physically exists,
  the same reflex that puts era tags only at boundaries (decision 14).
- `await` marks the call; `suspend` is the effect it carries. One concept,
  two spellings -- `await` at the call site, `effects suspend` on the machine
  signature -- and the compiler REQUIRES `await` on any call that carries
  `suspend`, so a park can never hide in a plain call. This is call-site
  marking, not signature coloring: the marker never infects a caller's type.
- A parked task is just data: machine frames are planned storage, not a
  native stack, so the continuation-capture problem that forces `Future` as a
  type in stackless languages does not exist here. `await` is a visibility
  marker, not a continuation type.
- SUSPEND-IN-CALL IS FORBIDDEN. A machine carrying `suspend` can be SPAWNED but
  not CALLED: ordinary calls run to completion and cannot park their caller, so
  `suspend` does not propagate up through call sites. Suspension is therefore
  never nested through a call chain -- a parked task's carry-set is always a
  SINGLE machine's locals at its own `await`, never a chain of suspended
  frames. This is the enforceable form of "calls run to completion," and it is
  what keeps carry-set storage single-level (see Task Storage). A helper that
  must wait is restructured as its own spawned machine + channel, not a call.
- Borrows may not live across an `await` (the world moves while parked).
  Effect ceilings forbid `suspend` where parking is illegal -- a trait
  requirement without `suspend` IS the interrupt-handler safety rule; build
  artifacts surface every `await`.
- The ATOMIC-STATE guarantee, now exact: a task is parked ONLY at its own
  `await` points; a call never parks the caller. This is NOT mutual exclusion
  -- other tasks run simultaneously on other cores; cross-task safety comes
  from ownership, `[send]`, and atomics. The language is scheduler-agnostic (a
  host may preempt); the guarantees come from ownership, never from
  non-preemption.

## Task Storage: No Stack Sizes

General recursion does not exist (self-calls are tail self-loops) and frames
are planner-computed, so the compiler knows each spawned machine's EXACT
worst-case storage. Nobody declares a stack size; overflow is impossible by
construction. Task pools are per-machine-type `M x N`: M computed, N
declared per spawn site (Embassy/RTIC precedent); spawning past N is a proof
obligation or boundary failure. Region-backed dynamic N arrives with the
allocator arc.

Because suspend-in-call is forbidden, the per-task carry-set is SINGLE-LEVEL:
the live locals of one machine at its own `await`, sized to the MAX over that
machine's await points, never the sum -- a task is parked at exactly one point
at a time, so reserving every await point's locals at once would be waste. And
N is not a free constant: the rigorous form DERIVES it from the finite resource
the task parks on (a single-consumer mailbox -> 1; a permit/budget pool -> its
capacity; a channel -> its endpoint count), so spawning is capability-gated and
`M x N` is a proven bound, not a guess -- a wrong N fails a model-checked
invariant at design time, not as an OOM in production. The run-to-completion
actor pattern (one machine with a receive loop, handlers that `transition` back
to it) collapses the carry-set to the actor's own `self`: nothing is held
across the `await` but state that already had to exist. A continuation across
several `await`s is threaded as data in a `self` field (a sum tagging the step),
not as a paused call stack. Such a field is sized to its biggest case like any
sum; shrinking it with out-of-line handles is the author's call, optionally
pinned by a `[max_size = N]` property checked against the layout report
(chapter 19).

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
    let taken: Take = await ring.take();
    transition taken {
        Take::Got(frame) -> work(frame)
        Take::Cancelled  -> finish()    // ordinary transition; nothing interrupted
    }
    ...
}
```

A task that never suspends is joinable but not cancellable -- its effect
surface says which kind it is. Cancellation rides the same propagation
channel as recoverable errors ([chapter 15](chapter_15_errors_traps_failure.md));
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
    let event: Event = await self.inbox.take();   // ONE wait, ONE word

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

If the worker traps, the first model should probably trap the whole thread
group. Programs that want recoverable failure should return an explicit result
shape from the spawned machine.

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

## Waitable Contracts: One Primitive

Deadlock checking only works when waitable operations are visible -- and
visibility is cheap here because there is exactly ONE wait mechanism: the
futex-shaped boundary primitive (wait on a word's value, wake N waiters).
Everything that blocks is library code over it:

- `Join<T>::join` waits on the child's completion word.
- `Mutex<T>::lock` waits on the lock word (happy path never waits).
- `Barrier<N>::wait` waits on the arrival-count word.
- `Pipe::read` / `Socket::recv` / event queues wait on their buffer words;
  the OS/ISR side POSTS to the word and wakes.

The anti-sprawl rule is deliberate (no epoll/eventfd/io_uring zoo): no
second wait mechanism, ever. Every blocking operation therefore carries the
`suspend` effect by inference, and "what can unblock it" is always "who
writes this word" -- a question the deadlock model below can actually
answer.

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
  ([Memory Layout And ABI](chapter_19_memory_layout_abi.md)).

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
- Progress-proven: external waits require fairness, timeout, cancellation, or
  explicit environment assumptions.

Servers, kernels, drivers, CLIs, and embedded firmware do not all want the same
definition of "may block." The proof mode should be explicit in build artifacts.

## Connection To Boundaries

Host and OS waits are part of the same contract system as other imported
entries.

If a platform entry can block, its contract must say what unblocks it or mark
the wait as boundary/opaque. A proved-concurrency build may reject opaque waits.

This keeps the language honest: the checker can prove the parts it can see, and
the build report names the boundary providers for the parts it cannot.
