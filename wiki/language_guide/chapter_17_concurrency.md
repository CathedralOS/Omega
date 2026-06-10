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

- Captured values must be copied or moved.
- Borrowed parent-stack data cannot cross into a spawned block.
- Moved values are unavailable to the parent after the spawn.
- Shared mutation must go through data types whose contracts permit concurrent
  access.
- A spawn used as a statement is fire-and-forget when the proof checker proves
  the spawned graph is self-contained.

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

## Waitable Contracts

Deadlock checking only works when waitable operations are visible.

Every blocking operation must declare what can unblock it. That includes
standard runtime types and host/OS boundaries.

Examples:

- `Join<T>::join` waits until the spawned machine completes.
- `Mutex<T>::lock` waits until the mutex is unlocked.
- `Barrier<N>::wait` waits until `N` participants arrive.
- `Pipe::read` waits until a matching write, close, timeout, or external event.
- `Socket::recv` waits on external network input unless a timeout or cancel path
  is part of the contract.

The declaration can live in a standard type, platform entry, syscall surface, or
boundary host package. The important rule is that blocking must not be invisible.

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
