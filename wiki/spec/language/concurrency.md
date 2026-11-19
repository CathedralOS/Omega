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
by ordinary atomics and admitted accessors:

| Requirement | Result |
| --- | --- |
| `AtomicLoad<T>` | Observed value. |
| `AtomicStore<T>` | No value. |
| `AtomicSwap<T>` | Displaced value. |
| `AtomicCompareExchange<T>` / `AtomicCompareExchangeOnce<T>` | Observing decisive / single-attempt outcome. |
| `AtomicTryExchange<T, Key>` / `AtomicTryExchangeOnce<T, Key>` | Non-observing decisive / single-attempt outcome. |
| `AtomicFetchAdd<T>` and individual fetch requirements | Instruction-observed prior value. |

[Access policies](../resources/access.md#atomic-compare-exchange-outcomes)
owns the exact outcome types, canonical cases, resident-custody rules, and
operation-specific conformance. The explicit parameter providing access to the
atomic storage is shared. Fixed representation,
admitted width/alignment, total decode/encode, and round-trip laws remain
operation requirements; arithmetic capability cannot manufacture hardware
support. Exact-forwarding wrappers may derive conformance mechanically; other
realizations need checked proof or admitted evidence.

Representation [recasts preserve access authority](../layouts/recasts.md#access-authority-and-concurrent-observation).
The shared parameter of an atomic store still carries mutation permission;
casting an ordinary shared reference cannot manufacture it. Conversely, an
atomic shared view cannot become an ordinary read view without the backing
exclusion required for non-atomic observation. Genuine exclusive access may
support representation-compatible recasts while preserving all validity and
custody obligations. Atomic-to-atomic views retain their operation and
observation requirements. This rule does not itself establish a mutex's
payload-custody protocol.

[Domain access policy](domains.md#domain-owned-read-and-write-authority) also
applies to atomic events: observation requires the applicable read grant,
mutation the write grant, and a read-modify-write event both. Shared atomic
access is not an exemption from the domain's writer list; authorized machines may lend the
exact access to an atomic helper without making that helper a new establishment
route. Policy and custody survive representation casts. None of these grants
alone proves that a mutex holds exclusive access to its separate payload.

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

The rules here preserve the required source guarantees; instruction selection
does not by itself constitute the formal memory model.

## Protocol proofs

Atomic-event analysis concerns legal observations/reorderings. Transition analysis
concerns concurrently activated machine graphs and exact resources: completion,
locks, queues, barriers, waits, and external events, related by waits-for, owns,
releases, and unblocks edges. Neither model replaces ordinary ownership,
initialization, or conservation.

Protocol packages may additionally require publication validity, linearizability
or FIFO order. A legal stale observation is not an error by itself; it must
satisfy the protocol's applicable properties, including lost-wakeup protection.

### Default admission obligation

Synchronization checking is mandatory. A closed executable composition or
installation must establish absence of internal synchronization deadlock under
its checked protocol contracts and explicitly admitted provider/environment
premises. An unresolved obligation refuses admission. Diagnostics distinguish
an established protocol violation from an obligation the checker cannot establish.
A library may publish symbolic resource relationships and obligations for its
consumer to discharge without choosing the consumer's runtime or topology.

The guarantee includes partial deadlock: a nonempty group of waiting participants
whose dependencies close over unavailable protocol resources/actions or
unsatisfiable protocol conditions, accounting for admitted enabling actions.
Unrelated runnable work does not make that group safe. A runnable activation's
missing termination promise alone is not evidence of synchronization deadlock.
Spins and retries that await another participant's progress count as waits even
when no `block` or `suspend` occurs.
Alternative and jointly required enabling actions remain distinct; a cycle in
a flattened may-depend graph alone is not a demonstrated reachable deadlock.

This obligation does not establish starvation or livelock freedom, termination
of every activation, an external peer's eventual response or a response deadline.
Scheduler starvation is distinct from a synchronization state with no permitted
way forward. External waiting must have an exact modeled/admitted boundary:
an idle server may wait for input, but crossing a package boundary cannot turn
an internal worker or channel dependency into an external assumption.

### Retained protocol contracts

Checked operations derive and retain synchronization behavior in the ordinary
semantic representations and published contracts. Forwarding calls and wrappers
substitute and preserve it. The retained relationships account for:

| Relationship | Required information |
| --- | --- |
| Resource subjects | Exact or symbolic identities, possible aliases, modes, lifetimes and instance relationships. |
| Protocol transitions | Acquisition, release, enqueue, park, spin, wake, completion and closure, with their ordering and atomicity. |
| Retention | Guards, capacity permits, worker slots, execution backing and context exclusions held through a wait. |
| Enabling behavior | The actions, participants and state transitions that can end a wait, including alternatives and joint requirements. |
| Outcomes | Success, failure, cancellation, timeout, owner death and execution-domain termination, with custody consequences. |
| Premises | Exact selected provider, scheduling, placement and external-event assumptions and their provenance. |

This is semantic contract information, not a new source keyword or a graph-shaped
`reaches` row. `blocks`, `suspends`, carry and synchronous `invokes` relationships
keep their meanings; none substitutes for the protocol relationships. Existing
ownership, loan and installed-context identities supply their subjects rather
than a second name-based resource analysis.

A checked synchronization implementation establishes its relationship to the
protocol through checked operations and proof rules. Custom atomic protocols
have the same obligation. A type or machine's spelling establishes no acquire,
release or wake semantics. An opaque provider publishes a protocol contract
whose implementation relationship is checked or admitted at the existing
foreign/provider boundary. Missing behavior is unknown, never an empty summary;
an arbitrary caller assertion or a model-file digest establishes no contract.
The same rule applies to runtime callables whose bodies are unavailable.
Absent `blocks` or `suspends` declarations do not supply a missing synchronization
contract or establish that a callable cannot wait through an atomic protocol.

Externally relevant changes to waiting, acquisition, retention, ordering or
outcomes change the published synchronization contract and undergo ordinary
compatibility checks. A may-wait summary supplies no eventual-release promise.
Positive progress uses authored/inherited protocol and
[termination contracts](termination.md#published-guarantees-and-private-witnesses),
or exact local evidence where those rules permit it. Termination's omission
rule is unchanged; private witnesses remain private.

Conditional progress premises cannot discharge themselves across calls or
package boundaries. "A completes provided B completes" and "B completes provided
A completes" supply no independent completion, release or wakeup guarantee.
Mutually dependent premises require a checked joint argument accounting for
initial state, enabling actions and the progress relied upon. A cyclic protocol
topology may satisfy that argument, for example through an initial message and
checked request/reply ordering; a blanket ban on cyclic topology is insufficient.
Ordinary synchronous-call and termination rules still apply.

### Locks and protected computation

Infer strict acquisition constraints for potentially waiting nested acquisitions
over actual resource relationships. Holding A while waiting to acquire B requires
A before B; the opposite constraint conflicts unless checked protocol evidence
excludes the dangerous combination. Parameters may alias one nonrecursive lock;
instances of one type may instead be independent locks. Neither variable names
nor types establish the distinction. Read/write modes, upgrades, recursive acquisition
and queued-writer behavior remain part of the protocol.

While an ordinary exclusion guard is held, unrelated waits reject unless a
checked protocol establishes their safety. Ordered nested acquisition and
operations that release the relevant guard before waiting may establish it.
Callbacks obey the same rule through their published behavior. Protected
computation establishes local progress to release or to checked protocol/outcome
transitions that account for continued retention. An unproved computation cannot
retain an ordinary guard indefinitely. These local obligations do not require
a surrounding server to terminate, and a may-wait summary alone never supplies
the eventual-release premise used by another operation.

A non-waiting attempt or bounded sequence that can finish without another
participant's progress is not a blocking acquisition. A retry loop depending on
that progress must establish its wait relationship. Dynamic acquisition
can use a stable order established by construction or checked contracts; an
address, name or allocation sequence supplies no order without such evidence.
Checked ordered-acquisition operations and other runtime-enforced protocols may
establish the same discipline without enumerating every resource instance.

### Execution contexts and reentry

A context must not wait for progress by a context that it prevents from running.
Check the actual interrupt source, CPU placement, priority, nesting and
preemption relationships retained at
[external-root admission](../build/external_roots.md#admission-and-removal).
Interrupt exclusion used to justify acquisition begins before acquisition,
lasts through release, and restores the prior state. One CPU's mask does not
exclude another CPU or a non-maskable source. Proven non-waiting atomic access,
partitioned ownership and deferred work may establish safety without masking.

Synchronous reentry includes a callback acquiring its suspended caller's held
resource, even when the callback is a different machine. An acyclic provider
call graph alone is insufficient. Scheduling and execution-backing protocols
must likewise permit a required releaser or wake action to execute; service
reach and a declared priority ceiling alone establish no such fact.

### Completion and finite capacity

Task completion, queue slots, buffer permits, worker capacity and execution
leases participate in the same analysis. Reject self-wait, circular completion,
and a group of parents occupying all workers while waiting for children that
need those workers. A parent-child shape alone establishes no progress.
Suspension discharges only the resources its actual protocol releases.
Checked helping, released execution resources or independently reserved capacity
may discharge the dependency; a current availability observation without a
conserved reservation does not.

### Communication and wakeup

A sender's existence is insufficient. Account for protocol phase, initial
messages, full buffers, endpoint closure, barrier membership/arrival counts,
and the actions needed to enable a receive. Two peers that both receive before
sending differ from a checked request/reply protocol. Standard protocols include
one-shot completion, structured request/reply, bounded producer/consumer and
phased barriers with explicit membership.

An observational wait predicate terminates, observes identified protected state,
and permits no mutation, unrelated effects, waiting, suspension or abnormal
outcome. Predicate purity alone establishes no lost-wakeup protection. Testing
the condition, registering a waiter, releasing protection and notification must
form a checked protocol. In particular, a condition wait's atomic release and
later reacquisition differ from an ordinary wait retaining the same guard.
Notification to recheck establishes neither the predicate nor completion.
More expressive predicates may establish the same obligations through checked
evidence. The first automatic floor below promises no general communication
proof synthesis and imposes no one-Boolean-field rule.

Cancellation or timeout is an enabling alternative only when the selected
protocol provides an actionable transition and the resources to execute it.
A cancellation request alone, or a timer serviced only by an exhausted executor,
does not break a dependency. Finite atomic operations remain distinct from
unbounded retry protocols. Atomic ordering alone establishes no eventual
observation of a pending publication or eventual wakeup; protocol evidence
covers the permitted memory observations and any progress premises it uses.

### Initialization and terminal outcomes

A task, handler or callback becomes externally callable only after its required
state and resources are established. Initialization waits, publication, drain,
teardown and replacement undergo the same dependency checks. Waiting for a
handler to drain while holding a lock that handler needs is a synchronization
failure. Global sequential initialization and a fixed task population are not
universal requirements.

Automatic cleanup retains its existing obligation to return normally without
blocking, suspension or abnormal outcomes. Explicit waiting shutdown is ordinary
checked protocol code. A crash or exit ending an entire owning execution domain
may abandon internal state under the existing
[survivor rules](process_exit.md#abandonment-and-survivors); it establishes no
release. A surviving participant or external resource requires the applicable
isolation, owner-death, recovery or reset contract. Owner death never silently
unlocks potentially invalid payload state. A permitted crash/exit route or the
ability to kill a domain is not proof that deadlock cannot occur before it ends.

### Composition, evidence and checking cost

Resolve symbolic subjects and obligations against the selected executable or
installed closure, including actual providers, capacities and context relations.
Unused eligible providers add no dependency. Replacement and topology changes
recheck the affected contracts and premises. Dynamic tasks/resources are legal
when their construction and transfer preserve the discipline; a population bound
alone proves no deadlock property. Quantified construction proofs and conserved
reservations may cover dynamic families.

Implementation properties travel with conformances. Protocol refinement must
preserve the progress used by consumers, accounting for internal retries and
stuttering; preserving successful traces while introducing additional stuck
behavior is insufficient. Reuse follows existing
[producer trust and admission](../terminal-psi/product.md#publication-and-separate-consumption).
Finite exploration supplies evidence only for its enforced bound, complete
exploration and sound implementation relationship; small instances and unfinished
searches are tests, not proofs of a larger composition. Runtime instrumentation
can expose mismatches but an unobserved failure establishes no theorem.

### First required automatic floor

The first floor fixes a sufficient discipline for exclusive locking. Every
conforming checker derives and composes the necessary summaries for bodies and
wrappers in this fragment without a handwritten protocol model or search hint:

| Part | First-floor condition |
| --- | --- |
| Locks | Nonrecursive exclusive locks with established resource identities and equal/distinct alias relationships. |
| Control | Finite sequences and conditionals/joins, using complete synchronization contracts for calls and already-checked iteration summaries. No unrolling or discovery of new loop summaries is required. |
| Protected computation | Local progress and outcome evidence is already established under the ordinary termination/protocol rules. The synchronization analysis need not invent a termination proof. |
| Context | The relevant entry/resource relationships and any interrupt or preemption exclusions are established. |

Substitute actual resource subjects through calls, identify established aliases,
and remove paths already proved unreachable. For remaining alternatives, join
the possibly held resources and acquisition constraints by union. Each potentially
waiting acquisition of B while A may be held contributes A before B; a release
removes the exact held subject on that outcome. The floor discharges the locking
obligations when these strict constraints are acyclic, protected progress and
outcome dispositions hold, and context exclusions cover the actual relationships.
An acquisition-order cycle fails this sufficient discipline unless additional
evidence resolves it; it is not by itself a demonstrated reachable deadlock.

The domain is finite normalized summaries of those forms, of any finite size;
there is no additional lock-count, path-count or synchronization-search cutoff.
The bound is on the permitted inference forms, not an implementation-selected
number of locks or a promise to explore interleavings. Checking has polynomial
work in the size of those summaries and their resource/control relationships.
As with [required automatic discharge](../proofs/contracts.md#required-automatic-discharge),
this floor has no optional search allowance: valid in-fragment compositions
pass under every proof-work policy, including the default and
[`--floor-only`](../build/command_line.md#checker-conformance).
Failure to accept one is a checker defect, not exhausted optional proof search.
Work is still charged as
[compiler work](../resources/logical_work.md#proof-discovery-and-evidence-checking).

Read/write upgrades, recursive locks, custom retry protocols, unresolved alias
relationships and richer communication protocols are outside this first floor.
They may satisfy the same obligations through additional checked evidence or
legitimate boundary admission. Standard completion/capacity and communication
automation can extend the same contracts; missing implementation support refuses
the affected composition before it can receive a deadlock-free verdict.

Retain compact per-body transitions and symbolic dependencies through the
pipeline. The automatic fragment must not require exhaustive interleaving search,
all-pairs resource tables or enumeration of every instance in a dynamic family.
Derive summaries once and compose them from published contracts. Additional
proof search may extend acceptance, but exhausted search supplies no permission
and must report the unresolved obligation.

### Stronger progress and response guarantees

External arrival rates are never inferred. They require admitted provenance or
an enforcing rate limiter that derives an admitted-work bound; its rejection or
backpressure remains part of the service contract.

Starvation freedom and bounded response additionally require the applicable
scheduler service, critical-section progress/contention, arrival and capacity
premises. Priority inheritance or ceilings are provider protocols, not a
universal scheduler or a response theorem by themselves.

Quantitative guarantees need fixed topology, conserved creation permits,
enforced admission bounds or a theorem quantified over the dynamic structure.
Changes to topology or providers also revalidate the affected memory, progress
and response guarantees, not only deadlock freedom.

Multicore response and blocking guarantees select both a scheduler and a
resource-sharing protocol. A single-core priority-ceiling theorem does not
establish bounds for cross-core sharing. A deployment must partition resources
per core, forbid that sharing, or select a proved multiprocessor protocol with
its own blocking analysis.
