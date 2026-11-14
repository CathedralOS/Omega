# Atomic vocabulary and downstream admission

Contract: [concurrency and atomic observation](../../../../wiki/spec/language/concurrency.md).
[atomic vocabulary](src/atomic.rs) owns the shared normalized atomic ordering vocabulary.
The [access-plan owner](../access-plans/README.md) separately validates placed
permissions, resident custody, and specialized requests.

Keep decisive/single-attempt and observing/non-observing axes independent.
Recognizing an ordering or publishing a core outcome type does not admit the
source operation, its result custody, a Terminal event, or target realization.
An unsupported single-attempt route must reject rather than erase `Uncommitted`
into a decisive scalar carrier. Generic outcomes retain payload multiplicity;
neither result shape nor zero initialization proves an event happened.

Every atomic family takes shared access to its storage. The access-plan vocabulary
carries that fact as `AccessOperation::receiver_polarity`, and validation's
`value_custody::atomic_operations` reads it so a store, fetch, swap, or
compare-exchange on an atomic cell is admitted through `&self` while ordinary
assignment keeps its exclusive demand.

Atomic source, interpreter, and native consumers require the same exact
operation and instruction-observed prior. A separately loaded prior is not a
valid implementation of swap/fetch/exchange. Target retries keep their work
attribution even when the source has one primitive event.

## Serial producer route

The parser's arithmetic-shaped carrier is decoded once, by
`validation::atomic_load_carrier`, `atomic_assignment_carrier`, or
`atomic_once_carrier`, into the
sealed operation, the accessed place, and the authored operands. Checking
plans one `CheckedAtomicAccessPlan` per operation: a writing carrier's result
placeholder reserves the dense binding the event binds to the observed prior,
and each operand keeps its own `AtomicOperand` scalar row. Lowering emits
the plan as one Terminal `AtomicAccess` at a complete rooted storage path.
An empty projection names a whole dedicated cell; an inline record cell keeps
its actual final field identity (terminal-psi `control_flow.rs`).
The codec's independent operation verifier
(`terminal-psi/codec/src/foundation_validation.rs`)
and interpreter consume that event; the verifier refuses illegal orderings,
runtime-selected locations, bounded writes without their checked resident
establishment obligation, mistyped operands or results, and events on ordinary
scalar storage.
`StructuralTypeShape::AtomicCell` retains a whole cell's resident contract;
`StructuralFieldType::Atomic` and `AtomicBoundedInteger` retain the same contract
for inline fields. The common endpoint query supplies resident type and bounds
without inventing a containing field. Shared loans authorize atomic events,
not ordinary stores or non-atomic snapshots. An exact typed atomic loan keeps
that contract and never becomes a plain scalar reference. Interpreter backing
selection and native geometry consume the same complete endpoint.
Single-attempt compare-exchange binds one structural local directly. Its retained
receiver and two operands are an evaluation roster, not an ordinary call. The
source checker requires the sealed canonical outcome with the exact resident
type; Terminal admission rejoins its three-case schema, operand types and legal
orderings. Both single-attempt families share this event. The retained result
contract distinguishes observing failure payloads from non-observing proposal
payloads, and the latter's successful displaced value. The complete result has
its declared multiplicity: the observing scalar outcome is copyable, while the
non-observing outcome remains affine even for a copyable scalar resident.

The concrete non-observing scalar route uses the dedicated cell's builtin
identity representation for its exact resident and comparison key. The
receiving outcome type never supplies that correspondence. Custom keys,
user-selected encoding laws, affine residents and placed-only permissions need
their own admission evidence; this scalar route must not infer them.

Serial execution commits a matching attempt and does not invent interference.
Native selection keeps the instruction's three-way tag: x86 CMPXCHG is decisive
in that attempt, while an AArch64 exclusive store may leave a matching attempt
Uncommitted. Failure returns the proposal for the non-observing family, and
commit returns the instruction-observed displaced value. Equality of the key
and prior does not substitute for the native commitment tag.

Omega's terminal-to-abstract lowering (`machine/operation/atomic_events.rs`)
keeps each Terminal event as one `AbstractOperation::AtomicEvent` at the same
`AbstractAtomicLocation` (root place and complete cell path). It adds the
`reads_from` and `modification_after` edges that the optimization unit's
happens-before replay rechecks. A backward latest-write analysis follows the
actual whole-storage edge bindings, so a state parameter keeps the same modification history under its
own place identity. The independent unit checker reconstructs that relation
from its current edges and rejects substituted backing, stale initial-residency
claims and unknown storage. Different known paths retain a canonical list of
all latest residencies, including initial residency and a prior iteration of the
same event. Independent replay requires the exact complete list; it grants no
observed-value equality or cross-activation synchronization. An intervening
non-atomic write or call lending the atomic backing, including a shared call, expires knowledge of its writer.
The next event may retain an `AfterEffect` boundary naming that exact latest
operation on every path; independent replay reconstructs the boundary, without
claiming which internal write supplied the resident or recovering an old value.
Shared access does not prove an unchanged atomic resident. Source and Terminal proof composition independently
expire observations that shared calls may mutate; copied SSA values survive.
The event retains its block-local root and the ordinary shared receiver path,
including stored-reference crossings. Arrival and local declarations supply
access and layout through target lowering and independent native selection.
Ambiguous edge bindings still refuse rather than guessing a storage identity.

## Portable fence route

The sealed `core/atomic.omg` declaration supplies `Atomic::fence` and its
copy `FenceOrdering` cases. Checking selects one proved case from the argument's
captured value at operand completion; runtime-unknown or overwritten case
facts reject. Calls producing the ordering still execute once. Exact sealed
symbol and case handles authorize the primitive; an authored machine with the
same spelling keeps its ordinary body semantics.

Terminal `AtomicFence` retains the ordering without a location, result or
observation edge. Omega keeps that event through abstract, target and legalized
operations. Selection uses a register-free external-effect instruction, emitting
`DMB SY` on AArch64 and `MFENCE` on x86-64 for all three orderings. These full
barriers conservatively implement the requested ordering; the effect catalog
and independent byte decoder distinguish them from a no-op or storage access.
Terminal interpretation advances the event without inventing concurrent
participants or synchronization. A fence alone proves no publication.

Instruction mappings and serial differential tests are not proofs of concurrent
observations. Complete memory-model/fence axioms, target-refinement proofs,
contention coverage with real concurrent activation, and proof-scoped weaker
acquire selection remain distinct obligations on [TASKS.md](../../../../TASKS.md).
