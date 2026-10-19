# Atomic vocabulary and downstream admission

Contract: [concurrency and atomic observation](../../../../wiki/spec/language/concurrency.md).
[atomic.rs](src/atomic/mod.rs) owns the shared normalized atomic ordering vocabulary.
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
`validation::atomic_load_carrier` / `atomic_assignment_carrier`, into the
sealed operation, the accessed place, and the authored operands. Checking
plans one `CheckedAtomicAccessPlan` per operation (typed-trees-to-checked-trees
`execution/terminal_unit/atomic_operations.rs`): a writing carrier's result
placeholder reserves the dense binding the event binds to the observed prior,
and each operand keeps its own `AtomicOperand` scalar row. Lowering emits
the plan as one Terminal `AtomicAccess` on a record's scalar field, the
location a scalar field store names (terminal-psi `control_flow/atomic.rs`).
The codec, independent verifier (`validation/atomic_access.rs`) and
interpreter consume that event; the verifier refuses illegal orderings,
runtime-selected locations, bounded fields, mistyped operands or results, and
events on ordinary scalar storage.
`StructuralFieldType::Atomic` retains dedicated-cell identity separately from
its scalar carrier. Shared loans authorize these events, not ordinary stores
or non-atomic snapshots. Atomic fields do not expose a plain scalar reference.
Single-attempt compare-exchange binds one structural local directly. Its retained
receiver and two operands are an evaluation roster, not an ordinary call. The
source checker requires the sealed canonical outcome with the exact resident
type; Terminal admission rejoins its three-case schema, operand types and legal
orderings. Serial execution and bounded CMPXCHG/LSE CAS retain the third case
without inventing spurious failure.

Omega's terminal-to-abstract lowering (`machine/operation/atomic_events.rs`)
keeps each Terminal event as one `AbstractOperation::AtomicEvent` at the same
`AbstractAtomicLocation` (root place, carrier path, field). It adds the
`reads_from` and `modification_after` edges that the optimization unit's
happens-before replay rechecks. A backward latest-write analysis follows the actual whole-storage edge
bindings, so a state parameter keeps the same modification history under its
own place identity. The independent unit checker reconstructs that relation
from its current edges and rejects substituted backing, stale initial-residency
claims and ambiguous writes. An intervening non-atomic write or call lending
the atomic backing, including a shared call, expires knowledge of its writer.
The next event may retain an `AfterEffect` boundary naming that exact latest
operation on every path; independent replay reconstructs the boundary, without
claiming which internal write supplied the resident or recovering an old value.
Shared access does not prove an
unchanged atomic resident. Source and Terminal proof composition independently
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
