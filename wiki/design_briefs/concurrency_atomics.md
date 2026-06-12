# Design Brief: Concurrency Model + Atomics

Scouted 2026-06-12. Status: C1 DECIDED (revised in discussion — see below);
C2-C5 awaiting sign-off (TASKS.md register).

## C1 As Decided (supersedes the `yields` recommendation below)

NO suspension keyword, NO await. The model:

- Waiting originates ONLY at boundary wait primitives — a `Scheduler`
  boundary trait (`wait_until_nonzero(&AtomicU32) effects suspend`,
  `wake_one`). Per-target bindings via the existing host-provider
  machinery: hosted = futex/WaitOnAddress syscalls; Cathedral userland =
  scheduler capability; Cathedral kernel = implements it over
  hlt/interrupt wakeups. Waiting lives where it physically exists — the
  same reflex as decision 14's "the era bit lives at boundaries".
- `suspend` is an INFERRED transitive effect (decision-12 machinery).
  Machines may declare it on signatures (`effects suspend`) as the
  reader-facing marker, checked against inference like any effect.
- AWAITING = CALLING. A call into a suspending machine may park the task
  inside the callee; the caller's code is straight-line. No Future
  reification is needed because machine frames are planned storage, not
  native stack — a parked task is just its state + frame region (the
  continuation-capture problem that forces await in stackless languages
  is pre-solved by the execution model).
- Enforcement over vigilance: (1) borrows may not live across a call site
  carrying `suspend` (the world moves while parked); (2) effect ceilings
  forbid `suspend` where parking is illegal — a trait with no `suspend`
  in its requirement IS the ISR-safety rule; (3) atomicity is DERIVED:
  a state that calls no suspending machine runs uninterrupted.
- Visibility: declared effects on public signatures (house style), and
  the state-graph/boundary artifacts surface every suspension point
  ("show me everywhere this program can park" is a browsable query).
- `select` DISSOLVES (decided same discussion): there is no select
  construct. Multiplexing is data-level — producers post into ONE mailbox
  carrying a case-bearing sum; the consumer does one wait and one ordinary
  transition over the sum (Erlang's one-mailbox model). The deferred work
  is a core MPSC event-queue library over the wait primitive, not syntax.
- Scoped spawns need no keyword: the lexical block is the scope (loans
  force the join; dropping a `Join<T>` joins; unconsumed handles join at
  block end). Free spawns stay move/copy-only.
- Task storage: per-machine-type pools of EXACT compiler-computed
  worst-case frame size (no recursion + planned frames = no stack sizes,
  no overflow). Declared N per pool; Region-backed dynamic N later.
- Atomic-state guarantee is derived and documented: "your task cannot park
  mid-body unless the body calls a suspending machine" — NOT mutual
  exclusion; the language stays scheduler-agnostic.
- Cancellation is a value at the wait (pending ch15 alignment): cancelled
  scope -> child's wait returns the zero `Cancelled` case; machine takes
  its own cleanup path; drops run normally; nothing is interrupted
  mid-state. Rides the ch15 recoverable-condition channel.
- Waitable surface: futex-shaped and SINGULAR (wait on word / wake N);
  everything else is library; ISRs/IO post to words. No second wait
  mechanism, ever.
- Termination note from the same discussion: const-eval/comptime needs no
  new termination rule either — general recursion does not exist in the
  language (self-calls are tail self-loops; loops carry
  decreases/measures), so existing discipline covers it (register M3).

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
