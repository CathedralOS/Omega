# Design Brief: Concurrency And Atomics

Current as of 2026-07-27. This brief records the surviving concurrency model
after decisions 20–22 and the task-runtime settlement. Chapter 18 is the
user-facing authority; the detailed lifecycle record is
[task_runtime_and_lifecycle.md](task_runtime_and_lifecycle.md). Continuation
representation and suspension-safe loans remain open. Direct-call
acknowledgements are settled and do not restore `async machine`, `Future<T>`,
or spawn-only suspension. Bare `spawn`, single-level carry sets, blanket loan
death, erased `Join<T>`, implicit detach, and Join-on-drop designs are not
canon.

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
- Suspension and worker blocking are distinct operational possibilities,
  published through independent `suspends;` and `blocks;` clauses. Absence is
  the corresponding negative guarantee.
- Calls acknowledge those possibilities independently with `suspend` and
  `block`. A call that may do both is spelled `suspend block operation()`.
  These are checked may-acknowledgements, not commands and not execution
  operators.
- A provider's service and operational ceilings must refine the pinned
  scheduler/wait requirement. A blocking provider cannot satisfy a slot that
  permits suspension but omits blocking.
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

## Direct-call acknowledgement

An ordinary call whose statically known operational envelope may park the
current activation is prefixed with `suspend`. A call whose envelope may occupy
the current worker while waiting is prefixed with `block`. Both markers are
required when both possibilities are present:

```omega
operation();
suspend operation();
block operation();
suspend block operation();
```

The shared reason is that execution may pause at the marked call while the
activation's borrows, claims, guards, and other live state remain held. The
markers describe what the selected contract permits; an individual invocation
may complete immediately. They do not force waiting, alter propagation, create
a task or future, change a return type, select a provider, or enter contract
identity.

The checker uses the statically known call envelope. Local checked calls may
use their checked summary; imports, requirements, generic calls, and boundary
operations use their pinned envelope; dynamic calls use the per-requirement
envelope statically retained by the value. A transparent refinement that proves
`suspends false` or `blocks false` removes that marker requirement. Missing,
partial, and redundant markers reject, so an unmarked call guarantees both axes
false at that site.

Suspension creates a continuation boundary, so a `suspend` call must be the
whole operation in a statement, simple `let` right-hand side, transition
subject, or terminal expression. It cannot be nested in an argument, operator,
aggregate, or condition whose partially evaluated state would become hidden
continuation state. Blocking preserves the ordinary stack and may nest, though
binding a result first is often clearer. The canonical order is
`suspend block`, never `block suspend`.

Compiler-synthesized adapters record the same acknowledgement in checked
artifacts; authored generator and adapter bodies write the markers normally.
Automatic cleanup and hermetic semantic evaluation admit neither operational
possibility, so no marker can make such a call legal there.

## Suspension amendment still required

Suspension propagates through ordinary calls. The compiler retains the bounded
chain of planned frames and the machine's suspension plan propagates
transitively. `suspend` makes the direct call searchable and reviewable but does
not change propagation or lowering.

The amendment must still settle:

- fixed-stack park/resume lowering against WCSU-derived `StackPlan`;
- which shared, mutable, pinned, or owned loans may cross suspension;
- cancellation and failure timing across a suspended call chain;
- the precise wait-provider contract and positive progress hypotheses.

No implementation may restore an `async machine`/`Future<T>` split, forbid all
suspending calls, or kill every loan at suspension merely because those rules
simplify lowering. Call-site acknowledgement is a static check over possible
suspension and blocking, not a future transformation.

## Wait substrate

The retained direction uses one small futex-shaped scheduler boundary: wait on
a word/value condition and wake one or more waiters. Higher-level task
completion, mutexes, barriers, channels, sockets, and event queues are
libraries over that contract where the target permits it.

Reach and temporal behavior remain separate. `wake_one` reaches the scheduler
service without parking; a wait operation may declare `suspends`, `blocks`, or
both according to its pinned contract. Fairness, deadlines, and eventual wakeup are
positive provider hypotheses, not implied by either row member.

The “one substrate” direction is an engineering constraint, not permission to
hide truly different host mechanisms behind a false contract. Opaque waits are
accepted boundary behavior and must remain visible in trust/progress reports.

## Carry, sharing, and the memory model

Ownership is the default race-prevention mechanism. Concurrent graphs do not
share mutable ordinary data unless a type's contract provides a sanctioned
shared-access operation.

- Type-wide carry guarantees use the compiler-built-in four-axis
  `[carry(...)]` property; transparent data derives structurally. Accepted
  resource claims begin strict and result contracts may establish positive
  per-claim permissions.
- Moving an exclusively owned value to another concurrent graph is checked
  from ownership plus the destination runtime's carry behavior.
- A shared reference may cross only when its referent's borrow/access contract
  sanctions concurrent sharing (for example atomics or a mediated protocol)
  and its carry demands are compatible.
- Copyability governs duplication; ownership governs transfer, and access
  contracts govern shared mutation.
- Atomics are dedicated core types with explicit operations and orderings.

Omega adopts the C11/Rust ordering vocabulary and SC-DRF model:
`Relaxed`, `Acquire`, `Release`, `AcqRel`, and `SeqCst`. Atomic operations name
their ordering. Source validation now treats those names as a closed vocabulary:
loads admit `Relaxed | Acquire | SeqCst`, stores admit
`Relaxed | Release | SeqCst`, and compare-exchange failure ordering may neither
release nor be stronger than its success ordering. Stage-1 atomic operations
and the memory model are landed.
Load/store/fetch_add/fetch_sub/fetch_xor/fetch_or/fetch_and/swap/
compare_exchange preserve their ordering through normalized operations and
exact x86_64/aarch64 target lowering.
Fetch/swap/CAS write the instruction-observed
prior into the language result; a separate ordinary read is forbidden because
it races the RMW. Swap is a first-class carrier rather than synthetic
arithmetic and lowers to implicitly locked `XCHG` on x86_64 or the selected LSE
`SWP` form on aarch64. `fetch_sub` performs the subtraction at the exact atomic
width: x86_64 negates the operand before one locked `XADD`; aarch64 does the
same before its ordering-selected `LDADD` form. The serial interpreter models
the same observation explicitly and is pinned by a focused differential test
rather than treating the carrier as an ordinary transparent expression.
`fetch_xor` uses the ordering-selected `LDEOR` form on aarch64. Because x86_64
has no single instruction that both XORs memory and returns its prior value, it
uses a locked `CMPXCHG` retry loop; only the successful instruction's observed
prior is returned. The retry loop is part of target lowering, not source-level
control flow and not a license for a racing ordinary read.
`fetch_or` follows the same returned-prior contract: aarch64 uses its
ordering-selected `LDSET` form, while x86_64 reuses the locked `CMPXCHG` retry
lowering and returns the successful attempt's observation.
`fetch_and` complements its mask before ordered `LDCLR` on aarch64 and uses
the shared locked retry lowering on x86_64, preserving the same successful
instruction-observation contract.

Still required:

- the remaining fetch-and-modify surface;
- contention tests once concurrent activation is runnable;
- standalone portable atomic fences and target lowering proofs
  (**OWNER-BLOCKED:** `OWNER_QUESTIONS.md` #13). Checked ISA fences already
  retain their target-specific instruction contracts, but they must not be
  treated as the portable atomic-memory-model operation until its source,
  ordering, and scope contract is settled;
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
fit ceilings that exclude forbidden suspension/blocking behavior and satisfy
their target's reentrancy/resource rules. The exact entry and MMIO spellings
remain open.

Architectural preemption may pause and restore opaque machine state at any
instruction; it does not require a language safe point and does not authorize
cancellation, migration, or replacement there. Semantic safe points occur only
at explicit may-suspend operations or authored scheduling constructs. A target
that may otherwise migrate an activation at arbitrary instructions must pin the
activation whenever its possible live values demand CPU/thread preservation;
there is no generic `SafePoints | Asynchronous` runtime mode.

The compiler never inserts a semantic safe point as an ordinary optimization.
A hot non-suspending kernel may be architecturally preempted while an outer
machine places explicit polls between bounded chunks. Maximum abstract work
between such points depends on the normalized bounded-work plan in
`OWNER_QUESTIONS.md` #17. Blocking creates no safe point; absent a finite wait
ceiling, semantic response is unbounded through the named blocking edge.

## Acceptance cases

1. A wake-only scheduler call does not acquire a suspension ceiling.
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
12. Missing or redundant `suspend`/`block` acknowledgements reject against the
    statically known call envelope.
13. A suspending call nested inside another expression rejects; a blocking-only
    call may nest because it creates no continuation boundary.

## Implementation and deferred design work

- Suspension amendment: fixed-stack park/resume lowering and
  suspension-safe loans.
- `TaskRuntime` selection, WCSU-derived activation `StackPlan`, transactional
  start outcome, task/provider provenance, and child-lease accounting.
- Normalized bounded-work plan after owner question #17.
- Core `Task<T>` lifecycle outcome and terminal-consumer implementation.
- `ArenaTaskPool`, bounded mailbox, and supervisor reference packages.
- Scheduler contracts using decision 23's sealed progress profiles; general
  trace propositions and profile entailment remain deferred.
- Scheduler operation details and provider admission tests.
- Four-axis carry policy, structural derivation, per-claim permission facts, local
  live-set checking, and runtime admission.
- Atomic remainder and formally checked target lowerings.
- Lock-free reclamation/resource algebra frontier.
- MMIO/volatile and interrupt-entry source surfaces.
- TLA-style extraction and proof modes.
