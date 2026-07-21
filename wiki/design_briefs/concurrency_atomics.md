# Design Brief: Concurrency And Atomics

Current as of 2026-07-20. This brief records the surviving concurrency model
after decisions 20–22 and the task-runtime settlement. Chapter 18 is the
user-facing authority; the detailed lifecycle record is
[task_runtime_and_lifecycle.md](task_runtime_and_lifecycle.md). The suspension
amendment remains open. Mandatory `await`, spawn-only suspension, bare `spawn`,
single-level carry sets, blanket loan death, erased `Join<T>`, implicit detach,
and Join-on-drop designs are not canon.

## Settled language model

- Concurrency runs ordinary machines. There is no `async machine` species and
  no `Future<T>` transformation.
- An admitted `TaskRuntime` provider starts a named machine supplied as a
  compile-time machine-symbol parameter. The compiler derives the activation
  plan; no runtime function value or capture inference is implied.
- `Task<T>` is a linear lifecycle claim. `finish` terminally consumes it;
  `request_cancel` retains it; moving it into another owner transfers the
  obligation. Scope exit and implicit detach cannot discharge it.
- Start is transactional. Dynamic rejection returns every moved argument and
  caller-supplied storage lease.
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
- Runtime custody, physical storage ownership, and lifecycle-claim ownership
  are separate. The compiler plans local activation requirements; admitted
  providers may use Arena-backed, OS, remote, or inline storage strategies.
- Pools, supervisors, mailboxes, and task groups are library data/policy, not
  language constructs. `ArenaTaskPool` is the bounded reference package, not
  the universal task model.

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
a word/value condition and wake one or more waiters. Higher-level task
completion, mutexes, barriers, channels, sockets, and event queues are
libraries over that contract where the target permits it.

Reach and temporal behavior remain separate. `wake_one` reaches the scheduler
service without parking; a wait operation may carry `Suspend`, `Block`, or both
according to its pinned contract. Fairness, deadlines, and eventual wakeup are
positive provider hypotheses, not implied by either row member.

The “one substrate” direction is an engineering constraint, not permission to
hide truly different host mechanisms behind a false contract. Opaque waits are
accepted boundary behavior and must remain visible in trust/progress reports.

## Carry, sharing, and the memory model

Ownership is the default race-prevention mechanism. Concurrent graphs do not
share mutable ordinary data unless a type's contract provides a sanctioned
shared-access operation.

- Type-wide carry guarantees use the compiler-built-in four-axis
  `[carry(...)]` property; transparent data derives and opaque data defaults
  strict. Sealed domains may establish additional per-mint permissions.
- Moving an exclusively owned value to another concurrent graph is checked
  from ownership plus the destination runtime's carry behavior. It needs no
  separate `Send` marker.
- A shared reference may cross only when its referent's borrow/access contract
  sanctions concurrent sharing (for example atomics or a mediated protocol)
  and its carry demands are compatible. It needs no separate `Share` marker.
- Copyability proves neither transfer placement nor shared mutation safety.
- Atomics are dedicated core types, never an implicit mode on ordinary
  integers.

Omega adopts the C11/Rust ordering vocabulary and SC-DRF model:
`Relaxed`, `Acquire`, `Release`, `AcqRel`, and `SeqCst`. Atomic operations name
their ordering. Source validation now treats those names as a closed vocabulary:
loads admit `Relaxed | Acquire | SeqCst`, stores admit
`Relaxed | Release | SeqCst`, and compare-exchange failure ordering may neither
release nor be stronger than its success ordering. Stage-1 atomic operations
and the memory model are landed.
Load/store/fetch_add/fetch_sub/swap/compare_exchange preserve their ordering
through normalized operations and exact x86_64/aarch64 target lowering.
Fetch/swap/CAS write the instruction-observed
prior into the language result; a separate ordinary read is forbidden because
it races the RMW. Swap is a first-class carrier rather than synthetic
arithmetic and lowers to implicitly locked `XCHG` on x86_64 or the selected LSE
`SWP` form on aarch64. `fetch_sub` performs the subtraction at the exact atomic
width: x86_64 negates the operand before one locked `XADD`; aarch64 does the
same before its ordering-selected `LDADD` form. The serial interpreter models
the same observation explicitly and is pinned by a focused differential test
rather than treating the carrier as an ordinary transparent expression.

Still required:

- the remaining fetch-and-modify surface;
- contention tests once concurrent activation is runnable;
- standalone fences and target lowering proofs;
- cross-activation ownership/borrow/access enforcement independent of `[copy]`;
- volatile/MMIO types and ordering contracts;
- the proof model for relaxed visibility.

## Proof model

The checker can extract a finite model from checked machine graphs:

```text
processes = concurrently activated machine graphs
resources = task completions, locks, queues, barriers, waits, external events
edges     = waits-for, owns, releases, unblocks
```

The first useful obligations are structural:

- no task-completion cycle;
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
3. A live `Task<T>` at scope exit is rejected.
4. Automatic cleanup never settles a task, parks, or reports failure.
5. A rejected task start returns all moved arguments and storage leases.
6. Ordinary shared mutable data is rejected unless its type supplies a valid
   concurrent-access contract.
7. Atomic ordering is explicit and invalid success/failure pairs reject.
8. A deadlock proof depending on fairness requires a provider contract that
   actually promises fairness.
9. Arena-, OS-, remote-, and inline-backed runtimes refine the same task
   requirement without sharing one storage representation.
10. A pool/runtime cannot close while dependent task claims or leases remain.
11. A suspension or migration rejects when any canonically live value's carry
    policy forbids it, regardless of whether that value is copyable.

## Implementation and deferred design work

- Suspension amendment: continuation representation and suspension-safe loans.
- `TaskRuntime` requirement, activation-plan artifact, transactional start
  outcome, task/provider provenance, and child-lease accounting.
- Core `Task<T>` lifecycle outcome and terminal-consumer implementation.
- `ArenaTaskPool`, bounded mailbox, and supervisor reference packages.
- Scheduler contracts using decision 23's sealed progress profiles; general
  trace propositions and profile entailment remain deferred.
- Scheduler operation details and provider admission tests.
- Four-axis carry policy, structural derivation, per-mint sealed facts, local
  live-set checking, and runtime admission.
- Atomic remainder and formally checked target lowerings.
- Lock-free reclamation/resource algebra frontier.
- MMIO/volatile and interrupt-entry source surfaces.
- TLA-style extraction and proof modes.
