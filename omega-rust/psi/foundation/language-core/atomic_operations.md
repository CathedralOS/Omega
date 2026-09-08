# Atomic vocabulary and downstream admission

Contract: [concurrency and atomic observation](../../../../wiki/spec/language/concurrency.md).
[atomic.rs](src/atomic.rs) owns the shared normalized atomic ordering vocabulary.
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

Atomic source, interpreter, and native consumers require the same exact
operation and instruction-observed prior. A separately loaded prior is not a
valid implementation of swap/fetch/exchange. Target retries keep their work
attribution even when the source has one primitive event.

Instruction mappings and serial differential tests are not proofs of concurrent
observations. Complete memory-model/fence axioms, target-refinement proofs,
contention coverage with real concurrent activation, and proof-scoped weaker
acquire selection remain distinct obligations on [TASKS.md](../../../../TASKS.md).
