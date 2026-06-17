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
stubbed to alias `copy`, no `Share`, no fences/interrupt control, no
`Scheduler` trait).

## The frame (architectural decision)

Omega owns the **language model**, not a **scheduler**:

- The language owns: the spawned-machine **stackless suspend/resume lowering**
  (machines already ARE stackless state machines — own this natively, never via
  an LLVM-coroutine backend, the mistake that cost Zig years), the **data-race
  type discipline** (`Send`/`Share`), the **memory model**, the **atomics**,
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
- **D5 — `Send`/`Share` data-race discipline: DECIDED, Rust-grade, strict.**
  Lands alongside #27. Naming: keep **`Send`** ("value may move to another
  task"); **rename Rust's `Sync` → `Share`** ("a reference to me may cross
  tasks"; `T: Shared ⟺ &T: Send`) — more honest than `Sync` (which never meant
  "synchronized"). Strict-but-simple first, then **gradually loosen** — each
  level a strict superset, so loosening never breaks compiled code (start
  strict→loose, never loose→strict):
    - **L0 (now):** move-only into a task (= today's reject-`&`/`self` rule,
      formalized as `Send`). No sharing.
    - **L1:** `Share` via synchronized wrappers — `&Mutex<T>`/`&Atomic<T>` cross.
    - **L2:** immutable sharing — `&T` shareable when `T` is deeply immutable.
    - **L3 (the prize, maybe far):** region/capability-proven **disjoint mutable
      sharing** — share mutable data lock-free when the type system *proves*
      non-overlapping access (Pony `iso`, Verona regions, RustBelt). **The
      invariant "no data races" is constant; what grows is the set of programs
      the prover can admit.**
  Bare metal additionally needs a SEPARATE mechanism — an **interrupt-re-entrancy
  rule** (effect ceilings: a trait with no `suspend` IS the ISR-safety rule).
  `Send`/`Share` does NOT cover ISR-vs-main on one core. This needs the
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
3. **`Send`/`Share` checker** (replace the `send`=`copy` stub) + the
   interrupt-re-entrancy effect rule (needs adding `suspend` to the effect set).
4. **Deadlock model** (cyclic-wait / lock-order / missing-producer) with the
   corrected honesty above.
5. **Structured spawn** (D1) + cancellation + the timeout primitive.
6. **`Scheduler` interface** (OQ4) + the 1:1 hosted backend.
7. **Cathedral bare-metal substrate** — the context-switch keystone (OQ2),
   interrupt enable/disable + save/restore, fences, MMIO/device ordering, the
   restricted static-task subset (D4). Design before promising the kernel story.
8. **DEFER:** the cooperative modeled scheduler (D6); enforced real-time (D2).

---

# 2026-06-15 (cont.) — Working-session resolutions

A follow-up design conversation resolved most of the open questions above and
added several mechanism-level decisions. Where this conflicts with anything
earlier, **this wins.**

## Decisions made / refined

- **D5 naming — RESOLVED: `Send` / `Share`.** Keep `Send` ("value may move to
  another task"); the share-capability is **`Share`** ("a reference to me may
  cross tasks"; `T: Share ⟺ &T: Send`). Drop `Sync` (it never meant
  "synchronized" — a plain immutable struct qualifies with zero synchronization).
  A symmetric verb pair reads as a capability, not a state.

- **D7 — Preemption model: RESOLVED = SAFE-POINT preemption (primary).** The
  compiler inserts a cheap yield-check at known points (loop back-edges /
  `decreases`-measure decrements / state transitions). A runaway task is
  preempted *promptly but only at a compiler-known point where the live set is
  known* — so a suspended task is ALWAYS "data at a known point," exactly like a
  cooperative `await` park. Cooperative `await` and preemption thus share ONE
  representation; the only difference is whether the yield was voluntary (await)
  or flag-triggered (a safe-point poll observing the preempt flag). This
  **preserves the bounded-state / provable-memory / no-overflow guarantees** that
  full async preemption (arbitrary-instruction capture) would break. asm cost: a
  predicted-not-taken `cmp [flag],0; jne slow` (~1-2 cyc amortized); STRIDE the
  poll (every Nth back-edge) or use a page-fault-based poll to amortize tight
  loops. Omega controls codegen, so it can GUARANTEE every loop carries a
  safe-point (the thing that makes safe-point unreliable in other systems).
  **Full async preemption is deferred** as a possible *backstop for hard-real-time
  tasks only* (and only then do you pay the stackful register/stack-capture
  price, for those tasks). This **largely dissolves OQ2** — see below.
  - ⚑ **REVISIT IN DEPTH (flagged 2026-06-15):** safe-point preemption's one gap
    is *guaranteed-bounded worst-case preemption latency*, which **hard real-time**
    requires — that is the scenario that may force the full-async
    (interrupt-anywhere) solution and its stackful arbitrary-state capture. When
    Cathedral's real-time story is taken up (D2), re-open this trade-off
    explicitly: cooperative/safe-point (bounded state, soft latency) vs full-async
    (hard latency bound, stackful capture, breaks the bounded-state guarantee for
    those tasks). Do NOT treat safe-point as the final word for the real-time
    path.

- **Stack discipline (frames the above): RESOLVED principle.** No recursion →
  every call chain has a statically-known max depth → exact worst-case stack
  usage (WCSU) per task, allocated up front → **stack overflow is impossible by
  construction.** Preemptible tasks (if any) get a pre-sized dedicated stack, so
  even a preemptive context switch is "save ~16 registers + swap SP between
  coexisting bounded stacks" — bounded and overflow-proof, the same flavor as the
  `M × N` task bound. (With safe-point preemption as primary, the stackful path is
  rarely if ever needed.)

- **Suspension granularity — RESOLVED (closes OQ5): `await` call-sites, not
  arbitrary transitions.** A state may suspend by `await`-ing *inside its body*
  (typically opening a waiting state with `await self.inbox()` then transitioning
  over the received case-bearing sum). The transition itself does not implicitly
  park. So the networked-state-machine vision is intact: the high-level protocol
  machine stays a CLEAN finite state machine, and async is *delegated* to the
  lower machine/primitive being awaited (socket, mailbox, timer). The only change
  from "every transition can implicitly suspend" is that every park is a VISIBLE
  `await` — which keeps the parked carry-set single-level (the `M × N` bound) and
  is the "show me everywhere this can park" property. No function-coloring: the
  FSM structure carries no `async` noise.

- **Device / volatile memory — RESOLVED in mechanism (closes OQ7 shape; exact
  surface still TBD).** A distinct capability-gated type (`Mmio<T>` /
  `Volatile<T>`): constructible ONLY via a `map_device(phys_range, cap)` boundary
  holding `Capability<MapDevice(range)>`; carries the `device_io` effect; explicit
  ordered volatile read/write, no operator sugar, no coercion from `&T`.
  **The abuse-prevention is the VIRTUAL MEMORY SYSTEM, not the type:** an MMIO
  access is just a `mov` whose address the MMU routes to a device physical frame.
  The grant = the kernel installing a page-table entry pointing at that frame;
  revocation = tearing the PTE down (straggler faults). You cannot fabricate the
  routing — an unmapped/normal address faults or hits RAM, never the device. The
  type is the *discipline* (ordered access + effect visibility + no accidental
  construction); the MMU is the *enforcement*.
  **Dual lowering of the same source:** (1) MAPPED/direct for proved/trusted
  drivers — direct fenced load/store, NO per-access trap, enforced continuously
  and free by MMU+IOMMU (the VFIO/userspace-driver model); (2) MEDIATED for
  sandboxed/untrusted/TEST drivers — the same `reg.write(...)` lowers to a shared
  ring or a trap-and-emulate, so a monitor can emulate the device and the driver
  doesn't know (hypervisor MMIO emulation; the sandbox can hand a RAM page +
  service it over IPC).
  **Trap-storm avoidance (the perf concern):** naive trap-per-access is fatal, so
  the hot path uses a SHARED-MEMORY RING + doorbell (virtio: plain `mov`s, one
  signal per batch) or POLLING (zero traps); trap ONLY for genuinely *synchronous*
  registers (a read/write whose side effect the emulator must compute before the
  instruction retires).
  **OS primitives the device surface needs:** `map_device` (cap-gated), ordered
  read/write + fences, IRQ registration, DMA-buffer alloc + IOMMU map, cache
  flush/invalidate.

- **IRQ model — RESOLVED (consistent with the scheduler doc's "interrupt =
  message").** Vectored: the hardware (x86 IDT / ARM vector table) routes IRQ N to
  a handler by number, saving minimal state. Top-half ISR does the MINIMUM —
  ack/EOI, maybe read one status word, **post + wake** the registered driver task
  (literally a `mov` into a shared word + set-ready, or nothing extra if the
  driver POLLS) — then `iret`. Bottom-half (the driver task) does the real work
  when scheduled. `await self.irq()` parks the driver until the top-half wakes it.
  Interrupt enable/disable (`cli`/`sti`-class) is the critical-section primitive
  and lives in the static-task kernel subset (the ISR-safety / effect-ceiling
  rule). Waking a parked task is cheap (set-ready + maybe reschedule); the
  context switch to run it is the bounded register-save.

- **Reclamation — RESOLVED strategy (closes OQ6 shape).** Owned/borrowed data:
  affine ownership + **drop (RAII)**, same as Rust — GC rejected (a runtime that
  pauses tasks; wrong for bare-metal/real-time), refcounting is a library type,
  regions are L3-additive, pure linear is too heavy. Lock-free shared structures:
  **quarantine to ONE blessed channel/mailbox primitive** (built on real atomics,
  #27) with an SMR scheme (hazard pointers / epoch / RCU) for that one primitive;
  app code uses ownership-TRANSFER (move the message in — no shared readers, no
  reclamation question), never hand-rolled CAS. Its scope is tiny precisely
  because the model is message-passing.

- **"Pointers with no `unsafe`" — RESOLVED framing.** Omega already achieves this
  for the SEQUENTIAL case: using a pointer emits the obligation "target is
  live/owned/borrowed within this pointer's lifetime," discharged by the borrow
  analysis — that IS proof-obligations-from-usage, no `unsafe` keyword. It is
  *sufficient for ~all code*. It is INSUFFICIENT only for lock-free reclamation,
  where (a) the lifetime stops being lexical (safe-to-free = "last concurrent
  reader in another task is done"), (b) aliasing is required not forbidden, and
  (c) the hard obligation moves to the FREE, not the deref. The tractable form of
  that extra contract is a TYPED discipline (regions / hazard-as-type / a linear
  token off the CAS), proven sound ONCE (the RustBelt pattern), then checked
  cheaply at use — i.e. L3. NOTE: no production language does safe-surface
  lock-free reclamation today (Rust uses `unsafe` inside crossbeam/std, proven
  externally); L3 aiming for locally-checkable contracts would be a genuine
  contribution, not catch-up — which is why quarantine-now is honest, not lazy.

- **Oracle contract — RESOLVED (closes OQ3): confluence + seeding.** Default on
  CONFLUENCE (message-passing, no shared mutable state → deterministic final
  state regardless of interleaving → interp and native agree by construction);
  add a SEEDED/deterministic scheduler for the non-confluent cases so both sides
  hit the same interleaving; bounded schedule enumeration is at the implementer's
  discretion as coverage grows. (Methodology decision, not a language feature.)

## Open-question status after this session

- **OQ1 (L3 disjoint *mutable* sharing):** still DEFERRED; mechanism = typed
  regions/capabilities; it is also the home of the "pointers-no-unsafe for
  lock-free" goal above.
- **OQ2 (context-switch keystone):** LARGELY DISSOLVED by D7 (safe-point
  preemption keeps suspended state as data-at-a-known-point; no arbitrary
  stackful capture). Residual: only if hard-real-time forces full async
  preemption do you need the register/stack context-switch design (deferred).
- **OQ3 (oracle contract):** RESOLVED above.
- **OQ4 (`Scheduler` interface completeness):** still OPEN, parked for later.
  Settled so far: minimal surface (`park`/`wake_one`/`wake_all`/`yield` + a timed
  wait) returning a WAKE REASON (`Signaled`/`PeerDied`/`Revoked`/`Timeout`, per
  the Cathedral scheduler doc). Open: `wake_one` vs `wake_all` default; the
  FAIRNESS promise (proof-relevant — a borrowed OS futex is not FIFO/fair, so
  liveness hypotheses built on it may not hold); timed-wait placement.
- **OQ5 (suspension granularity):** RESOLVED above (await call-sites).
- **OQ6 (GC-free reclamation):** RESOLVED in strategy above.
- **OQ7 (device-memory type):** mechanism RESOLVED above; exact source surface
  (type spelling, ordering args) still TBD.
