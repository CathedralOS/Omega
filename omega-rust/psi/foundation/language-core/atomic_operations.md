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
and each operand keeps its own `AtomicOperand` scalar row. Lowering rejoins
the plan to the carrier (`emission/atomic_sources.rs`) and emits one Terminal
`AtomicAccess` on a record's scalar field, the location a scalar field store
names (terminal-psi `control_flow/atomic.rs`). The codec, independent
verifier (`validation/atomic_access.rs`) and interpreter consume that event;
the verifier refuses illegal orderings, runtime-selected locations, bounded
fields, mistyped operands or results, and events on ordinary scalar storage.
`StructuralFieldType::Atomic` retains dedicated-cell identity separately from
its scalar carrier. Shared loans authorize these events, not ordinary stores
or non-atomic snapshots. Atomic fields do not expose a plain scalar reference.
Fences and single-attempt compare-exchange
have no producer yet.

Omega's terminal-to-abstract lowering (`machine/operation/atomic_events.rs`)
keeps each Terminal event as one `AbstractOperation::AtomicEvent` at the same
`AbstractAtomicLocation` (root place, carrier path, field). It adds the
`reads_from` and `modification_after` edges that the optimization unit's
happens-before replay rechecks. A backward latest-write analysis follows the actual whole-storage edge
bindings, so a state parameter keeps the same modification history under its
own place identity. The independent unit checker reconstructs that relation
from its current edges and rejects substituted backing, stale initial-residency
claims, ambiguous writes and intervening non-atomic writes or calls lending
the atomic backing, including shared calls. Shared access does not prove an
unchanged atomic resident. Source and Terminal proof composition independently
expire observations that shared calls may mutate; copied SSA values survive.
The event retains its block-local root: arrival declarations supply access and
layout through target lowering and independent native selection. Projected or
ambiguous edge bindings still refuse rather than guessing a storage identity.

Instruction mappings and serial differential tests are not proofs of concurrent
observations. Complete memory-model/fence axioms, target-refinement proofs,
contention coverage with real concurrent activation, and proof-scoped weaker
acquire selection remain distinct obligations on [TASKS.md](../../../../TASKS.md).
