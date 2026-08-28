# Design Brief: Concurrency And Atomics

Current as of 2026-08-19. This brief records the surviving concurrency model
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

Generic atomic code uses one sealed `omega::core` requirement per primitive
operation rather than one universal atomic trait. This is not a departure from
the ordinary RAM API: direct core atomics deliberately retain the familiar
load/store/swap/compare-exchange/fetch shape. The split is required because a
placed accessor's exact operation subset is computed from placement admission,
not authored as one wrapper type per subset. Core atomics and placed accessors
conform to the same requirements, so a helper asks only for the operations it
uses.

The family contains distinct load, store, swap, observing and non-observing
decisive compare-exchange, observing and non-observing single-attempt
compare-exchange, and individual fetch requirements. Decisive versus
single-attempt and observing versus non-observing failure are independent axes.
Every receiver is shared; the operation contract, rather than `&mut`, authorizes
the atomic event. Ordering is proof-static operation data. A requirement has
compiler-owned atomic semantics: core atomics and admitted placements derive
exact conformance, an exact-forwarding wrapper may preserve it mechanically,
and any other realization needs checked proof or admitted provider evidence.
A lookalike trait grants nothing. Missing conformance makes an operation
unavailable; arithmetic capability never manufactures hardware support.

The public requirement identities and result shapes are:

| Requirement | Operation result |
|---|---|
| `AtomicLoad<T>` | observed `T` |
| `AtomicStore<T>` | no value |
| `AtomicSwap<T>` | prior `T` |
| `AtomicCompareExchange<T>` | `Exchanged | Mismatched(observed: T)` |
| `AtomicCompareExchangeOnce<T>` | `Exchanged | Mismatched(observed: T) | Uncommitted(observed: T)` |
| `AtomicTryExchange<T, Key>` | `Exchanged(displaced: T) | Mismatched(proposed: T)` |
| `AtomicTryExchangeOnce<T, Key>` | `Exchanged(displaced: T) | Mismatched(proposed: T) | Uncommitted(proposed: T)` |
| `AtomicFetchAdd<T>` and each other fetch requirement | prior `T` |

Load/store/swap/fetch take one legal ordering. All compare-exchange
requirements take separate success and failure orderings. New hardware
operations such as fetch-min or fetch-max extend the family additively through
new requirements under the same rules.

Every operation first requires a statically fixed representation that fits one
target/provider-supported atomic transfer width at the required alignment.
Additional eligibility is per operation:

- load duplicates and therefore requires a duplicable resident;
- store discards the displaced resident and therefore requires it to be freely
  discardable;
- swap conserves one value into and one value out of the cell and may transfer
  an affine or linear resident when the placement owns that resident through
  Stable initialization;
- both observing compare-exchange requirements expose the resident on failure
  and therefore require a copyable resident;
- both non-observing requirements return the proposed value on every failure
  and may transfer affine or linear custody when the placement owns its
  resident. Their copyable `Key` and selected raw-transition law prove the exact
  comparison encoding without constructing a second owned `T`; success returns
  the displaced resident unless the same selected law proves it discardable;
  and
- each fetch requirement proves its exact operation law over raw
  representations.

Load/store/swap require the corresponding total decode, total encode, and
round-trip representation laws. Observing compare-exchange compares the stored
representation with `encode(expected)`, not user equality. Non-observing
exchange compares against the selected `Key` encoding and does not expose the
resident on mismatch or an uncommitted attempt. A fetch proof ranges over every
raw representation the provider says may be read, authorizes the exact raw
transition and operand encoding, and proves that decoding the result equals the
logical operation. External/device placements never derive fetch or exchange
from generic reads and writes; only an explicitly supplied provider operation
can conform. Identity encoding over a primitive total carrier is the
conservative first implementation.

Atomic ownership and cross-activation movement remain separate. A
provider-opened view never owns device content and therefore cannot derive an
affine or linear swap. A local
cell may hold an activation-bound affine value, but the cell becomes
cross-activation shareable only when its resident type is transferable.
Diagnostics report the resident type and crossing rather than claiming the
local atomic conformance disappeared.

Atomic operations name one ordering from a closed, operation-checked
vocabulary:

| Ordering | Contract |
|---|---|
| `NoOrdering` | request no cross-operation ordering beyond the single atomic access and its per-location modification order |
| `Receive` | when the access observes a matching publication, subsequent operations may rely on what preceded that publication |
| `Publish` | preceding operations are ordered before the publication |
| `ReceivePublish` | receive through the read and publish through the write of one read-modify-write operation |
| `GlobalOrder` | participate in one global order shared by every global-order atomic operation |

The conventional literature calls these relaxed, acquire, release,
acquire-release, and sequentially consistent ordering. Those terms are useful
for target and proof references; they are not the source vocabulary. Loads
admit `NoOrdering | Receive | GlobalOrder`, stores admit
`NoOrdering | Publish | GlobalOrder`, and read-modify-write operations admit
the complete vocabulary. Compare-exchange failure performs only a read, so its
ordering may not publish and may not be stronger than the success ordering.

Each legality relation is a nominal proposition over the proof-static ordering
argument, such as `valid_store_order(order)`. A concrete operation discharges
it by closed case analysis; generic code may carry it outward in `requires` or
`ensures`. This is the same proposition-family surface used by quotient
relations, without turning an ordering into a runtime policy choice.

Decisive compare-exchange returns either `Exchanged` or
`Mismatched(observed)`. Its target realization may retry unsuccessful
load-linked/store-conditional attempts and carries the resulting
target-relative work attribution. Single-attempt compare-exchange returns
`Exchanged`, `Mismatched(observed)`, or `Uncommitted(observed)`; the last case
means the comparison matched but the attempt did not commit, without asserting
that another participant caused the failure. Both failure arms use the
read-compatible failure ordering, while `Exchanged` uses the success
read-modify-write ordering.

The implemented first slice carries its ordering through normalized
load/store/fetch-add/fetch-sub/fetch-xor/fetch-or/fetch-and/swap/
compare-exchange operations and exact x86-64/AArch64 instruction selection.
The source parser, access-plan records, diagnostics, canaries, and sample
corpus use the settled vocabulary above; conventional literature spellings
reject instead of becoming aliases.
The shared compiler ordering carrier also keeps observing decisive
compare-exchange distinct from observing single-attempt compare-exchange, with
the exact success/failure orderings and permission axis preserved through
access-plan authorization. Source admission for the single-attempt form is not
implemented yet: the checked/source trees do not have its three-arm closed
result carrier, so mapping it to the decisive prior-value carrier would lose
`Uncommitted`. The case shapes are settled, but the public nominal result-type
identities and case-qualification paths remain an owner language-design
question. Until that identity is settled and the carrier is implemented, both
the checked interpreter and the legacy native state-graph boundary reject any
forged single-attempt operation before execution or lowering.
It does not yet constitute the formal memory model: the language relations,
their global-order axioms, and proofs that each target mapping refines them
remain required.
Fetch and swap return the instruction-observed prior. Compare-exchange returns
`Exchanged` or the observation carried by its failure arm; success does not
repeat the expected value. A separate ordinary read is forbidden because it
races the RMW. Swap is a first-class carrier rather than synthetic
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

`Receive` has the stronger release-consistency baseline: every target must
realize the ordering needed by the portable source contract. An AArch64 target
therefore uses its strong acquire instruction by default. A selected protocol
proof may authorize the weaker processor-consistent acquire instruction when
every additional execution it permits preserves that protocol's published
facts. This optimization is proof-scoped: shared unspecialized code remains on
the baseline unless its whole composition is proved.

## Portable fences

`Atomic::fence` is a compiler-known core operation over a dedicated
`FenceOrdering` whose cases are `Receive | Publish | ReceivePublish`. It creates
a normalized language event even when target refinement proves that no machine
instruction is needed. A fence alone publishes no memory. Synchronization
requires a qualifying atomic observation, such as a no-ordering load that
reads from the publication store between the corresponding fences.

```omega
payload.write(42);
Atomic::fence(Publish);
ready.store(true, NoOrdering);

if ready.load(NoOrdering) {
    Atomic::fence(Receive);
    use(payload);
}
```

The checked model retains full relation names: `sequenced_before`,
`reads_from`, `modification_order`, `synchronizes_with`, `happens_before`, and
`global_sequential_order`. Abbreviated academic relation names are neither
source vocabulary nor report labels. Target lowering supplies a validated
realization of the same event; scheduler migration may affect that realization
but never creates synchronization that source code can depend upon.

Checked ISA fences, device/MMIO/DMA visibility barriers, cache maintenance,
and asynchronous same-context ordering remain distinct facilities with their
actual participants and contracts. Ordinary portable atomics range over
coherent atomic memory rather than a target-selected semantic scope.

Cross-device ordering is expressed by sealed semantic provider operations, not
by strengthening `reaches` or adding a universal fence. A provider may expose a
complete operation such as DMA submission, or lower-level publication,
acquisition, cache-maintenance, MMIO-notification, and completion operations
from which a checked driver derives it. Each operation emits normalized
requirements naming its exact range, mapping, observer/device instance, and
ordering scope. Every requirement must be discharged by derived or
policy-permitted admitted evidence; an open requirement rejects.

Publication evidence is tied to the published range and current write state.
Any later write whose frame intersects that range invalidates the evidence, so
a stale publication cannot authorize a doorbell. The erased evidence proves
source composition but creates no machine dependency: the publication
operation itself contributes a scoped ordering event to terminal Psi, and
lowering must preserve it. On a coherent target that event may require no
instruction; on a non-coherent target it may require cache maintenance and a
barrier.

Device acquisition is not a freely mintable persistent fact. It consumes
completion evidence tied to the same request, device instance, mapping, and
range. When completion also returns custody, the resulting CPU view may be
Stable. If the device may continue writing, acquisition only orders subsequent
observations and the placement remains External.

The current non-authorizing foundation represents the five sealed operation
families as distinct provider-coverage demands. Each demand retains an opaque
exact-subrange context over the complete active mapping structure, an opaque
context over the complete admitted schema/device correspondence, and a nominal
ordering-scope identity. Exact structural closure rejects missing, extra,
duplicate, and structurally drifted provider assertions while preserving every
input for retry. This carrier retains only closed structural coverage; it does
not prove provider admission or create an ordering event,
publication/acquisition evidence, completion, custody, or lowering authority.
Source emission, provider-selection admission binding, and scope/event
realization remain required.

Still required:

- the remaining fetch-and-modify surface;
- contention tests once concurrent activation is runnable;
- formal atomic-access and fence axioms, including the complete global-order
  semantics, plus mechanically checked x86-64/AArch64 target refinement;
- normalized portable-fence operations and target lowerings;
- proof-scoped AArch64 weaker-acquire selection after target measurements
  justify specialization machinery;
- cross-activation ownership/borrow/access enforcement independent of `[copy]`;
- source-emitted device/DMA requirements, admitted provider-selection binding,
  scoped ordering events, completion-bound acquisition, and publication
  invalidation through ordinary write frames;
- and the deferred compiler-issued composition model when a concrete protocol
  or deployment profile requires whole-system proof.

## Proof model

The checker extracts two related models from checked machine graphs. The
atomic-event model explores legal observations and reorderings under the
relations above. The transition model tracks protocol state and resources:

```text
processes = concurrently activated machine graphs
resources = task completions, locks, queues, barriers, waits, external events
edges     = waits-for, owns, releases, unblocks
```

Types already establish structural invariants such as initialization,
exclusive ownership, and claim conservation. Protocol packages may add
semantic properties such as publication validity, linearizability, FIFO order,
or absence of lost wakeups. A stale observation is an error only when it
violates such a property.

The first useful transition obligations are:

- no task-completion cycle;
- no lock-order cycle;
- every internal receive has a reachable producer, close, cancellation, or
  timeout path;
- every barrier can reach its required arrival count;
- every external wait is modeled, accepted at a boundary, or rejected by the
  selected build policy.

Positive progress is conditional on selected provider evidence. The checker
must not claim fairness for an OS primitive whose contract does not provide it.

There is no ambient environment-premise language and no stronger, graph-shaped
`reaches`. Ownership, access, receiver polarity, handle multiplicity, claims,
and `invokes` define the topologies a package admits. When a concrete customer
requires whole-composition properties, the compiler will assemble those facts
with activation creation, concrete resource identity, wait/wake edges,
priorities, core placement, and selected provider evidence into a sealed erased
composition model. Ordinary proof machines check that model at composition or
deployment time. Implementation properties travel with the selected
conformance; deadlock, starvation, memory, and response properties belong to
the composed artifact and are revalidated after topology or provider changes.

Dynamic spawning remains legal. Quantitative guarantees require fixed
topology, conserved creation permits, enforced admission bounds, or a proof
quantified over the dynamic structure. Bounded exploration is only a testing
technique and produces no language contract or artifact property. A theorem
whose statement itself includes a participant bound remains an ordinary proved
property.

## Device and interrupt direction

Device memory is a distinct capability-gated volatile/MMIO surface. Mapping a
region requires explicit authority; operations reach the relevant hardware
boundary service. Authority possession and service reach are separate axes.

Interrupt handlers enter through a restricted boundary/calling plan. They must
fit ceilings that exclude forbidden suspension/blocking behavior and satisfy
their target's reentrancy/resource rules.

`Atomic::interruption_fence` orders compiler-visible coherent-memory operations
between ordinary execution and an asynchronously entered handler on the same
execution context. It establishes neither cross-core synchronization nor
device visibility. The operation is admitted only when installed external-root
evidence identifies the handler, vector/signal route, execution context, and
interruptible code relationship. Source spelling cannot assert that
relationship; until installation supplies the evidence, the operation rejects.

Architectural preemption may pause and restore opaque machine state at any
instruction; it does not require a language safe point and does not authorize
cancellation, migration, or replacement there. Semantic safe points occur only
at explicit may-suspend operations or authored scheduling constructs. A target
that may otherwise migrate an activation at arbitrary instructions must pin the
activation whenever its possible live values demand CPU/thread preservation;
there is no generic `SafePoints | Asynchronous` runtime mode.

The compiler never inserts a semantic safe point as an ordinary optimization.
A hot non-suspending kernel may be architecturally preempted while an outer
machine places explicit polls between bounded chunks. Restricted terminal-Psi
fixed-work checking may close the segment between those points. Otherwise the
report is `Unknown` or retains the exact blocking/foreign edge with no finite
guarantee. Blocking creates no safe point.

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
14. `Atomic::fence(Publish)` plus a no-ordering store synchronizes with a
    no-ordering load plus `Atomic::fence(Receive)` only when that load
    `reads_from` the publication store.
15. A target may emit no instruction for a portable fence only while retaining
    target-refinement evidence for the normalized event.
16. `Atomic::interruption_fence` rejects without installed-root evidence for
    the same-context asynchronous-entry relationship.
17. A bounded search publishes no protocol property; a theorem deliberately
    quantified over a bounded participant set retains that bound in its
    statement.

## Implementation and deferred design work

- Suspension amendment: fixed-stack park/resume lowering and
  suspension-safe loans.
- `TaskRuntime` selection, WCSU-derived activation `StackPlan`, transactional
  start outcome, task/provider provenance, and child-lease accounting.
- Canonical-IR fuel metering and restricted fixed-work safe-point segments.
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
- Deferred sealed composition-model extraction and proof-machine consumption,
  only after a concrete protocol or deployment profile requires them.
