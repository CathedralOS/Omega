# Design Brief: Concurrency And Atomics

Current as of 2026-07-18. This brief records the surviving concurrency model
after decisions 20–22. Chapter 18 is the user-facing authority. The suspension
amendment remains open; obsolete mandatory-`await`, spawn-only suspension,
single-level carry-set, blanket loan-death, and Join-on-drop designs are not
part of this document.

## Settled language model

- Concurrency runs ordinary machines. There is no `async machine` species and
  no `Future<T>` transformation.
- `spawn` creates concurrent work using ordinary move/copy/borrow rules.
  Borrowing spawns are lexically scoped.
- `Join<T>` is linear. `join`, `cancel`, or authorized `detach` consumes the
  obligation; scope exit cannot do so implicitly.
- Automatic cleanup may relinquish affine resources but may not suspend or
  fail.
- Suspension and worker blocking are distinct operational possibilities:
  `Suspend` and `Block` in decision 22's normalized effect row. Absence is the
  corresponding negative guarantee.
- A provider's row must refine the pinned scheduler/wait requirement. A
  blocking provider cannot satisfy a suspend-only slot.
- Cancellation is an explicit outcome observed at a wait/safe point; Omega
  does not unwind or interrupt arbitrary states.
- Multiplexing is a library/data problem: producers post case-bearing events to
  a bounded queue. The language does not need a `select` construct.
- Task and continuation storage are compiler-planned and bounded by explicit
  activation capacity. Bounded does not mean one retained frame.

## Suspension amendment still required

Suspension propagates through ordinary calls with no call-site marker. The
compiler retains the bounded chain of planned frames and the machine's row
carries `Suspend` transitively.

The amendment must still settle:

- the concrete continuation/frame representation and capacity proof;
- which shared, mutable, pinned, or owned loans may cross suspension;
- cancellation and failure timing across a suspended call chain;
- the precise wait-provider contract and positive progress hypotheses.

No implementation may restore mandatory `await`, forbid all suspending calls,
or kill every loan at suspension merely because those rules simplify lowering.

## Wait substrate

The retained direction uses one small futex-shaped scheduler boundary: wait on
a word/value condition and wake one or more waiters. Higher-level joins,
mutexes, barriers, channels, sockets, and event queues are libraries over that
contract where the target permits it.

Reach and temporal behavior remain separate. `wake_one` reaches the scheduler
service without parking; a wait operation may carry `Suspend`, `Block`, or both
according to its pinned contract. Fairness, deadlines, and eventual wakeup are
positive provider hypotheses, not implied by either row member.

The “one substrate” direction is an engineering constraint, not permission to
hide truly different host mechanisms behind a false contract. Opaque waits are
accepted boundary behavior and must remain visible in trust/progress reports.

## Sharing and the memory model

Ownership is the default race-prevention mechanism. Concurrent graphs do not
share mutable ordinary data unless a type's contract provides a sanctioned
shared-access operation.

- `Send`: a value may move to another concurrent graph.
- `Share`: shared references may cross concurrent graphs under the type's
  contract.
- Ordinary data defaults to neither merely because it is copyable; these are
  distinct properties.
- Atomics are dedicated core types, never an implicit mode on ordinary
  integers.

Omega adopts the C11/Rust ordering vocabulary and SC-DRF model:
`Relaxed`, `Acquire`, `Release`, `AcqRel`, and `SeqCst`. Atomic operations name
their ordering. Stage-1 atomic operations and the memory model are landed; the
remainder is engineering.

Still required:

- the full load/store/swap/fetch/compare-exchange surface;
- separate success/failure ordering validation;
- standalone fences and target lowering proofs;
- `Send`/`Share` enforcement independent of `[copy]`;
- volatile/MMIO types and ordering contracts;
- the proof model for relaxed visibility.

## Proof model

The checker can extract a finite model from checked machine graphs:

```text
processes = spawned machine graphs
resources = joins, locks, queues, barriers, waits, external events
edges     = waits-for, owns, releases, unblocks
```

The first useful obligations are structural:

- no join cycle;
- no lock-order cycle;
- every internal receive has a reachable producer, close, cancellation, or
  timeout path;
- every barrier can reach its required arrival count;
- every external wait is modeled, accepted at a boundary, or rejected by the
  selected build policy.

Positive progress is conditional on declared environment/provider hypotheses.
The checker must not claim fairness for an OS primitive whose contract does not
provide it.

## Device and interrupt direction

Device memory is a distinct capability-gated volatile/MMIO surface. Mapping a
region requires explicit authority; operations reach the relevant hardware
boundary service. Authority possession and service reach are separate axes.

Interrupt handlers enter through a restricted boundary/calling plan. They must
fit a ceiling that excludes forbidden `Suspend`/`Block` behavior and satisfy
their target's reentrancy/resource rules. The exact entry and MMIO spellings
remain open.

Safe-point preemption is the default direction. Fully asynchronous preemption
requires a separate saved-register/stack-context design and is deferred unless
hard-real-time requirements force it.

## Acceptance cases

1. A wake-only scheduler call does not acquire `Suspend`.
2. A blocking provider cannot satisfy a suspend-only slot.
3. A live `Join<T>` at scope exit is rejected.
4. Automatic cleanup never joins, parks, or reports failure.
5. Two borrowing spawns must consume their joins before the borrowed scope
   ends.
6. Ordinary shared mutable data is rejected unless its type supplies a valid
   concurrent-access contract.
7. Atomic ordering is explicit and invalid success/failure pairs reject.
8. A deadlock proof depending on fairness requires a provider contract that
   actually promises fairness.

## Open design work

- Suspension amendment: continuation representation and suspension-safe loans.
- Scheduler contracts using decision 23's sealed progress profiles; general
  trace propositions and profile entailment remain deferred.
- Scheduler operation details and provider admission tests.
- Full `Send`/`Share` checker.
- Atomic remainder and formally checked target lowerings.
- Lock-free reclamation/resource algebra frontier.
- MMIO/volatile and interrupt-entry source surfaces.
- TLA-style extraction and proof modes.
