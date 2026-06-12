# Design Brief: Concurrency Model + Atomics

Scouted 2026-06-12. Status: AWAITING SIGN-OFF (decisions C1-C5 in TASKS.md).

## Current State

Sketch-only (chapter 17): `spawn`/`Join<T>` syntax parses; execution model
undefined; every target declares `threads = disabled`; zero canaries.
Ownership rules shared mutation out by construction. The single fused
dispatch loop assumes one sequential execution path. Atomics are absent
entirely (no types, no orderings, no memory model).

## Recommendations (per hard question)

1. **Suspension**: explicit `yields` modifier on states; the dispatcher
   parks before entering and resumes on unpark. Visible in the state graph,
   no async coloring, maps cleanly to proof obligations. (Rejected: full
   async/await coloring; implicit suspension via CPS lowering.)
2. **Unit of concurrency**: spawned machine = one task (Go/Chapel-like);
   per-task frame discipline chosen now so separate compilation can reuse
   it. (Rejected: per-machine-instance, per-state-cluster.)
3. **Cancellation**: structured concurrency — Join SCOPES borrow into
   spawned blocks; scope drop cancels children; deadlines attach to the
   scope. Cancellation tokens can layer later.
4. **Sharing**: atomics-only at the language level; `Mutex<T>` is a CORE
   LIBRARY type built over atomic spin-locks, not a primitive. Matches
   "ownership is the mutual exclusion" + the `[send]` property (decision 8).
5. **Atomics surface**: compiler intrinsics (`AtomicU32/U64/Bool/Usize`;
   load/store/swap/fetch_add/sub/and/or/compare_exchange), full five-order
   set (Relaxed/Acquire/Release/AcqRel/SeqCst). (Rejected: boundary
   operators with contracts — ordering isn't audited; library indirection.)
6. **Memory model**: adopt C11 wholesale (SC-DRF). The proof model tracks
   waits, not orderings.

## Blast Radius

Frontend (spawn expr, Join<T>, Ordering enum, `yields`), type checker
([send] verification, ordering args), state graph (spawn sites), proof model
(deadlock detection: join cycles, lock order), control-flow lowering
(per-task frames, yield points), ISA (x86_64 LOCK prefixes; aarch64
LDAR/STLR/LL-SC), runtime ABI (spawn_task, park/unpark, Join::join).
~6 layers.

## Staging

1. **MVP**: parse/typecheck spawn + Join; lower `spawn` to a BLOCKING call
   (sequential execution, correct semantics, no scheduler). Canary: spawn
   compiles, join result typed.
2. **Atomics + true parallelism**: atomic types + intrinsic lowering both
   ISAs, scheduler + task frames, deadlock proof checks. Gate: shared-ring
   IPC works (atomics on indices, plain ops on slots).
3. **Structured concurrency**: Join scopes w/ cancel-on-drop, Mutex library
   type, deadlines, `yields` parking. Gate: Cathedral scheduler semantics.

## Cross-references

chapter_17, chapter_18, cathedral_alignment items 5-6,
Cathedral part_2 scheduler + part_3 IPC docs, whole_program_assumptions.md.
