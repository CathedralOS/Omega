# Atomic vocabulary and downstream admission

Contract: [concurrency and atomic observation](../../../../wiki/spec/language/concurrency.md).
[atomic.rs](src/atomic/mod.rs) owns the shared normalized atomic ordering vocabulary.
[Core surface tests](../../../omega/compiler/compiler/tests/atomic_core_surface.rs)
pin exact public outcome identities and payload shapes. The
[access-plan owner](../access-plans/README.md) separately validates placed
permissions, resident custody, and specialized requests.

Keep decisive/single-attempt and observing/non-observing axes independent.
Recognizing an ordering or publishing a core outcome type does not admit the
source operation, its result custody, a Terminal event, or target realization.
An unsupported single-attempt route must reject rather than erase `Uncommitted`
into a decisive scalar carrier. Generic outcomes retain payload multiplicity;
neither result shape nor zero initialization proves an event happened.

Every atomic family takes a shared receiver. The access-plan vocabulary
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
fields, mistyped operands or results, and modifying events through a shared
borrow. That last refusal is deliberate: Terminal types do not yet mark atomic
cells, so the source's shared-receiver form (`shared_receiver_atomic_store`)
still stops at checked planning. Fences and single-attempt compare-exchange
have no producer yet.

Instruction mappings and serial differential tests are not proofs of concurrent
observations. Complete memory-model/fence axioms, target-refinement proofs,
contention coverage with real concurrent activation, and proof-scoped weaker
acquire selection remain distinct obligations on [TASKS.md](../../../../TASKS.md).
