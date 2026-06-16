# Design Brief: Concurrency Model + Atomics

Scouted 2026-06-12. Status: C1 DECIDED (revised in discussion — see below);
C2-C5 awaiting sign-off (TASKS.md register).

> **2026-06-15 Scheduler-Strategy Review appended at the bottom of this file**
> (after a multi-agent research pass + adversarial review). It ratifies the
> overall frame (primitives-only core; consumer-provides-scheduler), records
> decisions D1–D6, splits the proof promises into safety vs liveness, lists
> corrections to the prose above, and catalogs the open questions + build order.
> Where the review and the older text conflict, **the review wins.**

## C1 As Decided (AMENDED 2026-06-13 in chapter 17 — chapter is authority)

AMENDMENT: an `await` call-site marker WAS reintroduced, and a new hard rule
SUSPEND-IN-CALL IS FORBIDDEN was added. A wait is still an ordinary call (no
`async`/`Future`, no signature coloring), but it is marked `await` at the
call site and the compiler requires `await` on any `suspend`-carrying call
(visibility, not coloring). Because a `suspend` machine can be SPAWNED but
not CALLED, suspension never nests through a call chain and the per-task
carry-set is SINGLE-LEVEL (M = max over a machine's own await points; N
derived from the finite resource parked on, making `M x N` a model-checked
bound rather than a guess). Everything else below stands. The original
no-keyword text is kept for the record:

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

---

# 2026-06-15 — Scheduler-Strategy Review (decisions + open questions)

Source: a multi-agent research pass (systems languages, runtime languages,
OS-dev requirements, formal verification of concurrency, memory models) +
ground-truth of the Omega codebase + a four-lens adversarial review. This is
**design ratification + a build order, NOT a description of working code** —
the primitives below are ~0% built today (no `suspend` effect in the closed
effect set, atomics are name-only with NON-atomic RMW desugars, `send` is
stubbed to alias `copy`, no `Shared`, no fences/interrupt control, no
`Scheduler` trait).

## The frame (architectural decision)

Omega owns the **language model**, not a **scheduler**:

- The language owns: the spawned-machine **stackless suspend/resume lowering**
  (machines already ARE stackless state machines — own this natively, never via
  an LLVM-coroutine backend, the mistake that cost Zig years), the **data-race
  type discipline** (`Send`/`Shared`), the **memory model**, the **atomics**,
  and the **`Scheduler` interface**.
- The language owns **no concrete scheduler**. The `Scheduler` interface is the
  injection seam (like the allocator). "Primitives-only core" means the core
  does not *depend on* a baked-in scheduler — it does not mean Omega never
  *ships* one (see D6).

| Target | Scheduler strategy | Why |
|---|---|---|
| Hosted (Win/Linux/macOS, x86+ARM) | Borrow the OS scheduler (1:1 threads); optional cooperative executor later | Lowest-risk, no runtime; what every scheduler-free systems language does |
| Bare-metal (Cathedral) | Omega provides primitives only; Cathedral provides the scheduler | The kernel *is* the scheduler; a built-in one is the thing the OS author must rip out |
| Future (Wasm/SPIR-V) | Falls out of the same interface | Stackless lowering + injected scheduler is target-agnostic |

A single strategy does NOT serve both: hosted Omega is a *client* of a
scheduler; Cathedral *is* a scheduler.

## Decisions

- **D1 — Structured concurrency: DECIDED structured.** A spawned task is bound
  to a lexical scope that cannot exit until its children finish or are
  cancelled (the existing scoped-spawn rules in C1 already lean this way).
  Free/detached spawns require an explicit owner. Buys compositional
  cancellation/timeout, no leaked tasks, and a **bounded waits-for graph** for
  the deadlock proof. (Fire-and-forget via explicit detached scope — manageable.)
- **D2 — Enforced liveness / real-time: DEFERRED (theoretical future feature).**
  Liveness ships as *conditional* theorems ("under fair scheduling, progresses").
  Enforced bounded-time/real-time only if a consumer demands it, and then via a
  **Ravenscar-class scheduler inside Cathedral** — never an Omega-owned runtime.
- **D3 — Non-multi-copy-atomic targets (POWER, ARMv7): OFF the target list.**
  x86-TSO and ARMv8 are both multi-copy-atomic (no IRIW), so the verified memory
  model needs no POWER/RC11 repairs. It is the CPU memory-consistency model, not
  instructions, that we decline to support. Reversible later at significant cost.
- **D4 — Cathedral restricted static-task sub-language: DECIDED yes (required).**
  Static, build-time tasks; raw atomics + MMIO; explicit context switch; **no
  `spawn`/`await`.** The kernel scheduler is written in this subset (breaks the
  bootstrap regress — the scheduler can't be written in terms of an `await`
  model that lowers *through* the scheduler). Precedent: Oxide's Hubris (static
  task set, no runtime creation). The full `spawn`/`Join` surface sits *above*
  the kernel. **Syntax is open; the hard open design is the context-switch /
  save-restore representation (OQ2).**
- **D5 — `Send`/`Shared` data-race discipline: DECIDED, Rust-grade, strict.**
  Lands alongside #27. Naming: keep **`Send`** ("value may move to another
  task"); **rename Rust's `Sync` → `Shared`** ("a reference to me may cross
  tasks"; `T: Shared ⟺ &T: Send`) — more honest than `Sync` (which never meant
  "synchronized"). Strict-but-simple first, then **gradually loosen** — each
  level a strict superset, so loosening never breaks compiled code (start
  strict→loose, never loose→strict):
    - **L0 (now):** move-only into a task (= today's reject-`&`/`self` rule,
      formalized as `Send`). No sharing.
    - **L1:** `Shared` via synchronized wrappers — `&Mutex<T>`/`&Atomic<T>` cross.
    - **L2:** immutable sharing — `&T` shareable when `T` is deeply immutable.
    - **L3 (the prize, maybe far):** region/capability-proven **disjoint mutable
      sharing** — share mutable data lock-free when the type system *proves*
      non-overlapping access (Pony `iso`, Verona regions, RustBelt). **The
      invariant "no data races" is constant; what grows is the set of programs
      the prover can admit.**
  Bare metal additionally needs a SEPARATE mechanism — an **interrupt-re-entrancy
  rule** (effect ceilings: a trait with no `suspend` IS the ISR-safety rule).
  `Send`/`Shared` does NOT cover ISR-vs-main on one core. This needs the
  `suspend` effect, which does not exist yet.
- **D6 — First-party modeled scheduler as a proof target: DECIDED yes,
  eventually, COOPERATIVE — DEFERRED additive.** A cooperative scheduler that
  switches only at `await` makes the interleaving set small + syntactically
  visible → model-checking is tractable *and* precise. It flips deadlock-freedom
  from conservative "safe under ALL schedulers" (rejects good programs) to
  precise "safe under THIS scheduler" (admits them), and liveness from *trusted*
  to *discharged*. The agnostic primitives are the portable floor; the modeled
  scheduler is the opt-in strong-proof layer ("the dial"). Plausibly an
  *extension* of the existing dispatch loop (transitions are the yield points),
  not a rewrite.

## Proof model: safety vs liveness (the scheduler question, settled)

**Does Omega need to build/own a scheduler for its proof promises? No — not for
any SAFETY promise.** The split:

- **SAFETY** (data-race-freedom; deadlock-freedom = the *system* never wedges
  into a no-move state; protocol fidelity; partial correctness) is provable
  **unconditionally** and holds against **every scheduler, including
  adversarial** — because "safe under all interleavings" IS adversarial-robust by
  definition. Requires real atomics + a proven memory-model lowering; requires
  **no scheduler**.
- **LIVENESS** (termination, progress, starvation-freedom, eventual delivery) is
  provable **only conditional on a fairness assumption**, and is **logically
  impossible** to prove against a fully adversarial scheduler (proving progress
  against "the scheduler that never runs B" is unsatisfiable — not a tooling
  gap). The theorem *states* its fairness hypothesis; it is discharged by
  trusting the OS (hosted) or by an owned, verified scheduler (D6 / D2).
- **Deadlock-free** (whole system never stuck) is SAFETY; **starvation-free**
  (every thread gets its turn) is LIVENESS. A deadlock-free system can still
  starve a thread. We promise the former unconditionally; the latter only under
  fairness.

So: prove "nothing bad happens" against *all* schedulers; prove "something good
happens" against *fair* schedulers, and state "fair" out loud. Adversarial
*progress* is not a target because it is a contradiction; adversarial *safety*
is the strong promise and is free.

## Corrections to the prose above / earlier framing (from adversarial review)

- **"Adopt C11 wholesale (SC-DRF)" is REFINED.** DRF-SC applies to **ordinary
  memory only**. **Device/MMIO memory is a SECOND model** (ordered, no
  elision/reordering/coalescing) and is Cathedral-gated — DRF-SC is *unsound*
  there. Cache coherence also cannot be assumed pre-boot. Needs a type-level
  ordinary-vs-volatile/device distinction (OQ7).
- **`Relaxed` is OUT of the verified core.** No sound axiomatic relaxed model
  both forbids out-of-thin-air reads and preserves optimizations (~25-yr-open).
  Verify at **SC + Acquire/Release + NonAtomic** (covers locks, message-passing,
  refcounts). `Relaxed` may exist only as an unverified escape hatch. (The one
  chapter-17 atomic example uses `Relaxed` — migrate it.)
- **Memory model = one IR model → two FORMALLY-VERIFIED ISA lowerings**
  (x86-TSO, ARMv8; Lasagne-style). **RMW atomicity (#27) is the immediate
  blocker AND on the critical path for the deadlock proof's *binary* soundness**
  — a deadlock-free MODEL is unsound while `compare_exchange` lowers to a
  non-atomic load-then-store (which it does today).
- **Deadlock-freedom is NOT monolithically scheduler-free.** Only the *join
  cycle* is pure scheduler-free safety. *Lock-order inversion* is a potential
  deadlock provable either conservatively (rejects correct programs) or via a
  scheduler-model-quantified check. *Missing-producer* is a partial-liveness
  approximation wearing a safety costume, not pure safety.
- **"Cathedral provides the scheduler" RELOCATES, not discharges, the
  obligation** — every Cathedral concurrency theorem is conditional on
  Cathedral's own (unverified) scheduler being fair/atomic/wake-correct.

## Open questions

- **OQ1 (the prize):** can the proof system prove **L3 — disjoint mutable
  sharing** (region/capability), enabling lock-free mutable sharing? Highest
  value, hardest. Precedents: Pony `iso`, Verona regions, RustBelt.
- **OQ2 (the keystone):** Cathedral's **context-switch / save-restore
  representation**, reconciled with the stackless await-only model — which
  explicitly *cannot* represent a PREEMPTED task's full mid-instruction state
  ("a transition does not push a frame"; suspend-in-call forbidden). Design this
  before promising the kernel story.
- **OQ3:** concurrency **oracle contract** — does the differential
  interpreter-vs-native oracle pin a seeded/deterministic schedule (Loom-style)
  and use the deadlock model as bounded schedule enumeration? Needed before the
  first real concurrent artifact ships. Stays meaningful while the model is
  confluent (message-passing, no shared mutable state).
- **OQ4:** `Scheduler` interface completeness — `wake_one` vs `wake_all`,
  fairness class (weak/strong), timed waits; validate against all three backends
  (1:1 OS, cooperative, Cathedral).
- **OQ5:** are **all** transitions suspension points (uniform, no coloring) or a
  marked subset? Shapes the proof surface.
- **OQ6:** **safe memory reclamation without GC** for lock-free structures
  (hazard pointers / epoch / RCU) — the Cathedral mailbox needs it.
- **OQ7:** exact surface for the **device/volatile memory** type distinction.

## Build order (sequencing)

1. **#27** — real-atomic RMW + the SC+AcqRel+NonAtomic memory-model spec + two
   formally-verified ISA lowerings (add the ordinary-vs-device distinction to the
   spec now), validated by a Loom/CDSChecker-style checker. Unblocks everything;
   on the critical path for the deadlock proof's binary soundness.
2. **Concurrency oracle contract** (OQ3) so #1 can be tested; atomic mailbox as
   the worked example (+ resolve reclamation, OQ6).
3. **`Send`/`Shared` checker** (replace the `send`=`copy` stub) + the
   interrupt-re-entrancy effect rule (needs adding `suspend` to the effect set).
4. **Deadlock model** (cyclic-wait / lock-order / missing-producer) with the
   corrected honesty above.
5. **Structured spawn** (D1) + cancellation + the timeout primitive.
6. **`Scheduler` interface** (OQ4) + the 1:1 hosted backend.
7. **Cathedral bare-metal substrate** — the context-switch keystone (OQ2),
   interrupt enable/disable + save/restore, fences, MMIO/device ordering, the
   restricted static-task subset (D4). Design before promising the kernel story.
8. **DEFER:** the cooperative modeled scheduler (D6); enforced real-time (D2).
