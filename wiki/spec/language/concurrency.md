# Concurrency and atomic observation

Concurrency activates [ordinary machines](machines.md) through an admitted
[task runtime](../build/task_runtime.md). It does not create an async machine
species or transform results into futures. Ownership prevents ordinary shared
mutation; a shared reference crosses activations only under a sanctioned
concurrent-access contract and compatible [carry](../resources/carry.md).
Copyability, exclusive transfer, and shared mutation are separate permissions.

Pools, supervisors, bounded mailboxes, and multiplexed case-bearing events are
ordinary library data and policy. There is no language `select` or privileged
task-group construct. Physical storage, provider custody, and task-claim
ownership remain independently accountable.

## Atomic operations

Atomics are dedicated core types, not implicitly atomic ordinary integers.
Generic code uses one sealed core requirement per primitive operation, shared
by ordinary atomics and admitted placed accessors:

| Requirement | Result |
| --- | --- |
| `AtomicLoad<T>` | Observed value. |
| `AtomicStore<T>` | No value. |
| `AtomicSwap<T>` | Displaced value. |
| `AtomicCompareExchange<T>` / `AtomicCompareExchangeOnce<T>` | Observing decisive / single-attempt outcome. |
| `AtomicTryExchange<T, Key>` / `AtomicTryExchangeOnce<T, Key>` | Non-observing decisive / single-attempt outcome. |
| `AtomicFetchAdd<T>` and individual fetch requirements | Instruction-observed prior value. |

[Placed access](../resources/placed_access.md#atomic-compare-exchange-outcomes)
owns the exact outcome types, canonical cases, resident-custody rules, and
operation-specific conformance. All receivers are shared. Fixed representation,
admitted width/alignment, total decode/encode, and round-trip laws remain
operation requirements; arithmetic capability cannot manufacture hardware
support. Exact-forwarding wrappers may derive conformance mechanically; other
realizations need checked proof or admitted evidence.

Observing compare-exchange compares stored representation with `encode(expected)`,
not user equality. Non-observing exchange uses the selected copyable key's
encoding law. Fetch proofs cover every provider-readable raw representation,
authorize its exact transition and operand encoding, and connect decoded output
to the logical operation. External reads/writes do not synthesize fetch/exchange.

Fetch and swap return the prior observed by the atomic instruction or successful
retry attempt, never by a separate racing load. Compare-exchange success does
not duplicate the expected value. Outcome construction/zero initialization proves
no atomic event; only the checked operation contract does. The zero outcome tag
is mismatch, not success. If its payload is not zero-valid, the outcome remains
unusable until established. Pre-execution storage can use an ordinary
`Empty | Ready(outcome)` wrapper rather than altering the operation's cases.

## Orderings

Ordering is explicit proof-static operation data, not a runtime policy choice.
Load/store/swap/fetch have one ordering; compare-exchange has separate success
and failure orderings.

| Ordering | Required meaning |
| --- | --- |
| `NoOrdering` | No cross-operation ordering beyond the atomic access and per-location modification order. |
| `Receive` | When a matching publication is observed, subsequent operations may rely on what preceded it. |
| `Publish` | Preceding operations are ordered before publication. |
| `ReceivePublish` | Receive through the read and publish through the write of one RMW. |
| `GlobalOrder` | Participate in the global order shared by all global-order atomic operations. |

Loads allow `NoOrdering | Receive | GlobalOrder`; stores allow
`NoOrdering | Publish | GlobalOrder`; RMW success allows all five. Failure is
read-only: it cannot publish or exceed the success ordering. These legality
conditions may be proved by concrete case analysis or carried through generic
`requires`/`ensures`; naming a condition supplies no proof. Conventional
relaxed/acquire/release/acquire-release/sequentially-consistent terms are useful
references, not additional source spellings.

Decisive compare-exchange returns exchanged or mismatch. Its realization may
retry unsuccessful load-linked/store-conditional attempts, retaining the
target-relative work attribution. Single-attempt operations additionally return
`Uncommitted`: comparison matched but the attempt did not commit, without
asserting another participant caused it. Both failure cases use failure ordering;
success uses the RMW ordering. A nominal single source operation does not hide
unbounded target retries from resource analysis.

`Receive` has the strong release-consistency baseline. A target may select a
weaker processor-consistent acquire only when a protocol proof covers every
additional execution and preserves its published facts. Unspecialized shared
code keeps the baseline unless its complete composition is proved.

## Fences and participants

`Atomic::fence` accepts a dedicated `FenceOrdering` with
`Receive | Publish | ReceivePublish`. It emits a normalized language event even
when target refinement permits no machine instruction. A fence alone publishes
no memory; synchronization requires a qualifying atomic observation. In
particular, publication-fence followed by no-ordering store and a no-ordering
load followed by receive-fence synchronize only when that load reads from the
publication store.

Retained relation names are `sequenced_before`, `reads_from`,
`modification_order`, `synchronizes_with`, `happens_before`, and
`global_sequential_order`. Scheduler migration may affect realization but cannot
create source-visible synchronization. Portable atomics range over coherent
atomic memory, not a target-selected semantic scope.

Checked ISA barriers, device/cache/DMA visibility, and asynchronous same-context
ordering have distinct participants and contracts. [Device protocols](../resources/device_access.md)
own scoped publication/acquisition and custody release. `Atomic::interruption_fence`
orders ordinary code and an asynchronous handler on the same execution context,
not cross-core or device observation; exact installed-root evidence is required
under [external-root admission](../build/external_roots.md#interruption-ordering).

The complete formal atomic/fence axioms, global-order semantics, and mechanically
checked target-refinement proofs remain unfinished. The rules here preserve the
required source guarantees without claiming that instruction selection already
constitutes that formal memory model. Implementation coverage belongs beside
the [atomic vocabulary note](../../../omega-rust/psi/foundation/language-core/atomic_operations.md).

## Protocol proofs

Atomic-event analysis concerns legal observations/reorderings. Transition analysis
concerns concurrently activated machine graphs and exact resources: completion,
locks, queues, barriers, waits, and external events, related by waits-for, owns,
releases, and unblocks edges. Neither model replaces ordinary ownership,
initialization, or conservation.

Protocol packages may require publication validity, linearizability, FIFO order,
or no lost wakeups. A stale observation is erroneous only when it violates such
a property. Initial composition obligations include absence of completion/lock
cycles, reachable producer/close/cancellation/timeout for receives, achievable
barrier arrival counts, and explicitly modeled or admitted external waits.
Fairness and other positive progress depend on exact selected provider evidence,
not service reach or absence of blocking.

There is no ambient environment-premise syntax or graph-shaped reach row.
Ownership/access, receiver polarity, handle multiplicity, claims, and synchronous
`invokes` edges constrain admitted topology. When a concrete customer requires
whole-composition proof, a sealed erased model must additionally retain activation
creation, resource identities, wait/wake edges, priorities, placement, and
provider evidence. Ordinary proof machines consume it at composition/deployment.
This extraction remains deferred, not implicit authority supplied by a bounded
search or a proposed graph format.

Implementation properties travel with their conformances; composed deadlock,
starvation, memory, and response guarantees require revalidation after topology
or provider changes. Dynamic creation is legal, but quantitative guarantees
need fixed topology, conserved creation permits, enforced admission bounds,
or a theorem quantified over the dynamic structure. Bounded exploration is a
test, not a theorem. A theorem explicitly quantified over a bounded participant
set remains valid within that stated bound.
