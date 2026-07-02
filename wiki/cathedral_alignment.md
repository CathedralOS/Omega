# Cathedral Alignment: Language Gaps By Urgency

Cathedral (`../Cathedral`) is an operating system that will be written in Omega.
Its design (see `Cathedral/wiki/design/part_0_foundations/01_omega_substrate.md`)
bets the entire OS architecture on specific Omega features. This page
cross-references those bets against the language's actual implementation status
and sorts the gaps by *when ignoring them starts costing rework*.

Statuses: IMPLEMENTED (runtime canaries), STRUCTURAL (parses/typechecks, no
semantics), DESIGN-ONLY (chapter exists, nothing implemented), ABSENT (not in
any doc).

The one-line summary: **Cathedral's load-bearing bets are precisely the
chapters that are currently syntax-only (wire data, versioned data, boundary
registry, concurrency), plus a handful of systems-language fundamentals no doc
covers at all (atomics, volatile/MMIO, enum payloads, separate compilation).**

## Tier 1 — address while the language is still cheap to change

These are decisions whose *absence is silently being decided* by ongoing
implementation work. Each one gets more expensive to retrofit every month.

> Scaffolding status: the clear-direction items below are now CAPTURED in the
> language guide with footnotes marking the muddy parts — ZII (chapter 19),
> enums (chapter 1), atomics (chapter 17), volatile/MMIO (chapter 19),
> freestanding targets + hardware facts (chapter 18), whole-program
> assumptions (wiki/architecture/whole_program_assumptions.md) — and recorded
> as Current Answers in the appendix. `wire data` (chapter 20) and versioned
> data (chapter 21) were already design-complete in the guide; their gap is
> implementation only.

> ZII update (frozen decision 8): the language guarantee is LAYER 1 only —
> zero is always a valid value (compiler-side, free). "Zero means empty" is
> the opt-in `[zero_init]` type property; Cathedral requires it on OS-surface
> types as policy. The item below predates that split; its "collision" worry
> is resolved by facts-describe-established-values + the property opt-in.

1. **ZII as a language guarantee — and its collision with bounded invariants.**
   Cathedral adopts "the all-zero bit pattern is a valid, coherent value for
   every construct" as a system-wide convention. Omega never promises this.
   Worse, it is currently *contradicted* by the language's own features: a
   field with invariant `1..=100`, a non-zero field default, or an enum whose
   first variant is not the empty case all make zeroed memory an invalid
   inhabitant. This is a semantic rule that must be designed now (e.g. "every
   `data` admits zero; invariants describe *established* values, with a
   distinguished zero/empty state outside the domain" — or Cathedral's bet is
   revised). Every `data` declaration written before the rule exists is
   potential debt. Cheap first step: a validation pass (or lint) that flags
   zero-excluding declarations, plus a guide section stating the commitment.

2. **`wire data` semantics** (STRUCTURAL today) — **construct retired as
   design 2026-07-02** (`design_briefs/programmable_layouts.md`): one
   declaration form (`data`) with *optional* identity numbers + `retired N;`;
   serialization moves to layout policies (`OmegaLayout` grammar family,
   durability as a build-time plan grade consumed by `Store<T>`-class APIs +
   a publish-time predecessor diff). The *work* is unchanged in substance —
   schema identity validation, tag diagnostics, codec generation — but the
   surface it lands on is plain `data` + the plan/deriver machinery, so ch20
   needs a rewrite before implementation starts. Still the natural next
   differential-oracle-friendly feature: derived codecs are pure functions
   with byte-exact expected outputs.

3. **Versioned `data` + migration machines** (STRUCTURAL today). "The spine of
   Cathedral's no-reboot upgrade story." Historical member blocks are parsed
   then *skipped by symbol resolution*. Needed: historical-shape symbols,
   migration matching (`Counter::v1(old)`), version-scoped machine binding,
   and — the part that interacts with everything else — layout/serialization
   rules for old shapes. The semantics design should be settled early even if
   lowering is staged, because it constrains layout and `wire data`.

4. **Separate compilation and a component artifact model** (ABSENT). Omega is
   a whole-program compiler emitting one image; the runtime model is a single
   global frame region with absolute offsets and one fused dispatch loop.
   Cathedral requires components that are compiled, signed, shipped, loaded,
   and **hot-swapped independently** (hermetic static linking per component,
   machines as swap points, content-addressed code dedup). Nobody needs to
   build the loader today — but every codegen decision that assumes
   whole-program-with-absolute-addresses deepens the eventual rework. Worth an
   explicit architecture note: which backend layers are allowed to assume
   whole-program, and which must stay relocatable/per-component.

5. **Concurrency: pick the model's hard answers** — CORE MODEL DECIDED
   (frozen decision 16, 2026-06-12; implementation not started): typed
   state clusters CAN suspend; no await/no keyword — waiting originates at
   a futex-shaped `Scheduler` boundary trait (Cathedral kernel implements
   it over hlt/interrupts; userland binds the scheduler capability);
   `suspend` is an inferred effect; cancellation is a value at the wait;
   scoped spawns borrow; task pools are exact compiler-computed frames; no
   select (one-mailbox sums — exactly the IPC-ring shape). Cathedral's
   scheduler chapter can now align against chapter 17 + the design brief.
   Remaining sign-offs: C2-C5 (task unit, Join scopes, atomics-only
   sharing, C11 intrinsics) in TASKS.md's register.

6. **Atomics and a memory model** — direction scouted + chapter 17 now
   carries the Rust-like atomics section (distinct core types, five C11
   orderings); the scout recommendation (compiler intrinsics, C11 model
   wholesale, atomics-only sharing with Mutex as library) awaits sign-off
   as C4/C5 in TASKS.md's register. The wait primitive itself is decided
   (decision 16): `wait_until_nonzero(&AtomicU32)` / wake — atomics are
   the words everything parks on, so C4/C5 gate IPC, the scheduler, and
   `spawn` implementation.

7. **Freestanding target + hardware access vocabulary** (ABSENT/narrow).
   Boot needs: a target with *no* host bindings and a custom entry (the
   current target model assumes stdout/stdin/process capabilities), linker/
   section/physical-address control for the image writers, **volatile/MMIO
   access semantics** (absent from chapter 19), and inline asm grown beyond
   `asm { jmp state(...) }` (CR3/MSR/port-IO instruction contracts — the
   appendix already lists the open questions; they need answers). The direct
   image emission bet is aligned with this; the gap is the freestanding
   flavor of it. **Worked samples now exist**
   (`design_briefs/freestanding_boot_and_hardware_facts.md`, 2026-07-02): six
   samples walk a UEFI x86-64 handoff (entry provider → memory-map exit dance →
   MMIO/volatile → CR3/MSR → interrupt mask → interrupt entry). They surface
   that boot needs **no new `unsafe` regime** — every act is a declared axiom at
   an enumerable boundary — and (after deflating the first draft's "world
   facts" ask) **no new fact kind either**: every machine-state predicate
   resolves to an existing mechanism — evidence tokens (`IrqGuard`,
   `FinalMemoryMap`), value invariants (page tables built from the kernel
   prototype), owned per-CPU state, or a one-time audited axiom list at the
   entry boundary. Foreign structs ride the layout-policy machinery
   (`programmable_layouts.md`). Remaining asks: no-host target + entry
   spelling, lowering-contract vocabulary (volatile `exactly_once`,
   `clobbers tlb`), interrupt-entry convention, and the **bounded C-ABI export
   table** for the firmware seam (one UEFI-subset contract; vendor firmware on
   commodity hardware, an Omega UEFI payload on reference platforms — see the
   brief's firmware-seam section).

8. **Case members (sum/mixed data shapes)** — SUM SHAPES IMPLEMENTED
   (2026-06-10): `case` members with named payloads parse, validate,
   interpret, and LOWER NATIVELY (tag-prefix construction writes, payload
   member reads, tag dispatch with payload binding in transition arms; all
   oracle-verified). Case-subset domains and MIXED shapes (common fields + a
   case part) remain pending, as does payload-aware structural equality
   (interim: `==` against a payload-bearing case is a compile error). See
   chapter 1 + TASKS.md frozen decisions 7/8.

## IPC + scheduler alignment (2026-06-13)

Cathedral's `part_3/00_ipc_and_service_invocation` and
`part_2/01_scheduler_and_resources` lean directly on the concurrency model;
reconciliation against the amended chapter 17 + the atomics work:

VALIDATED (the ch17 await-amendment + atomics serve these docs):
- The scheduler doc restates ch17's SINGLE-LEVEL carry-set + N-derived-from-
  the-parked-on-resource almost verbatim — the amendment was made to support
  it. The no-select ONE-MAILBOX model is cited as the blessed `many_to_one`
  actor shape; awaiting-is-calling + no-coloring is what lets Cathedral have
  "no userspace async runtime, just stdlib channels". All consistent.

CRITICAL-PATH SHARPENING:
- The IPC `many_to_one` mailbox REQUIRES atomic claim-a-slot (`fetch_add`
  index bump + `compare_exchange` claim + a per-slot publish release-store).
  So the stage-1 atomic RMW ops being non-atomic DESUGARS (fetch_add,
  compare_exchange — see TASKS / review §1b) makes the MPSC mailbox UNSOUND,
  not just theoretically racy. The "make RMW ops real LOCK instructions"
  task gates IPC; it is not optional polish. (load/store are fine — plain
  aligned mov is atomic on x86; the producer's release-store on the index is
  covered.)

NEW GAPS these docs surface (design TBDs, no clear implementation action yet):
- **Wake-reason sum.** Park must return `Signaled | PeerDied | Revoked |
  Timeout`, not just ready-or-cancelled. ch17's cancellation-as-value is a
  SUBSET; it must generalize to a wake-reason sum, with `PeerDied`/`Revoked`
  sourced from the OS grant arena (the one liveness fact only the kernel
  has). Fold into ch17's cancellation section when the scheduler arc starts.
- **`SharedRegion<Untrusted>` — a THIRD memory category** ch18 does not name:
  adversarially-mutable (neither proved nor boundary-accepted), reads return
  raw/unproven values, snapshot-then-validate to close the TOCTOU hole
  (a shared-mutable re-read after a check is unsound). ch18 currently knows
  only proved and boundary-accepted memory.
- **`protocol <Name> version vN { call ...; stream ...; }`** — a typed RPC
  surface over `wire data` (the IPC doc's typed-layer example). No language
  construct yet; rides the wire-stage-2 + capabilities-as-values work.

## Tier 2 — note now, design later (TBD register)

Flagged by Cathedral as "what Omega must grow," or absent from both wikis.
None block current compiler development; all should stay visible.

- **TBD: serialized capability representation** preserving attenuation +
  revocability across IPC/reboot/network (Cathedral's #1 flagged gap).
  Depends on `wire data` + the capability runtime story.
- **TBD: quiescence proofs** under interrupts, timers, async work, hardware
  (the hot-swap precondition). Depends on the concurrency model.
- **TBD: borrows as swap back-pressure** — borrow checker refusing to let a
  borrow outlive a machine swap point; cross-IPC borrows.
- **TBD: multi-version concurrency mode** — old + new machine versions
  running simultaneously with versioned dispatch (when quiescence is
  impractical).
- **TBD: operation-capabilities for secrets** (`Capability<SignWithKey(K)>`)
  and **purpose-tagged authority** (`Capability<Read<X>, Purpose<Y>>`) —
  likely generics + domains, may need no new core feature; prove it.
- **TBD: interrupt-handler entry convention** — how hardware enters the state
  graph (a machine entry with a target-specific calling convention?).
- **TBD: allocator story** — `Vec` has no runtime; `alloc` is an effect name
  only. A kernel wants explicit allocator/arena capabilities, not an ambient
  heap. Decide before implementing `Vec` lowering, not after.
- **TBD: repr control for hardware structures** — packed, explicit
  offsets/alignment, untagged unions (page-table entries, descriptor tables,
  device registers). Chapter 19 has `repr native` only.
- **TBD: function pointers / first-class machine references** — driver
  dispatch tables; partially covered by `dyn Trait` (single-impl works,
  multi-impl backend pending).
- **TBD: const evaluation** — const params are structural; compile-time
  function evaluation is unspecified. Kernels lean on this hard.
- **TBD: authority-flow completeness** — facts through returns/derives across
  nested calls (the package capability manifest is only as good as this
  inference).
- **TBD: effects vocabulary operationalization** — `device_io`, `memory_map`,
  `dma`, `interrupt_mask`(?) exist as names at best; each needs a definition
  of what it gates.
- **TBD: deterministic scheduler / virtual-provider injection hooks** for
  simulation (Cathedral's testing story; the recursive-provider pattern).
- **TBD: tail calls into machines** (already in the language appendix).
- **TBD: partition-tolerant lease semantics, remote attestation facts,
  CRDT/merge obligations** — distributed tier, farthest out.
- **TBD: trusting-trust resistance + TCB minimization for the self-hosted
  compiler.** Under SAS the compiler IS the isolation boundary, so the
  toolchain's trust story is a first-class deliverable: a hand-audited
  bootstrap seed (no Thompson trusting-trust binary in the lineage), a small
  proof-checking kernel that re-checks artifacts, and a verified Omega→machine
  translation (CompCert/CakeML-style). Trust bottoms out at {seed, checker,
  specs, hardware}; Gödel caps it at small-but-nonzero, and a proof only shows
  code-meets-spec (validation of the spec stays human). **Sequencing: SKIP in
  the current Rust implementation** — you bootstrap *from* an existing
  toolchain, and the differential oracle is the interim mitigation — **and make
  it canon once Omega is self-hosted in Omega.** Novel as a default-canon
  stance; earned here because the compiler is Cathedral's security kernel.

- **TBD: information-flow / secrecy labels.** Cathedral wants a *propagating*
  secrecy label on values (a `Secret<T>` taint — NOT a content-domain and NOT a
  `[property]` bracket; secrecy is about provenance/policy and must FLOW through
  operations: `hash(secret)` is secret). It would *derive* a component's
  side-channel isolation level and its constant-time obligation automatically
  instead of hand-passing them. This is real information-flow typing (label
  creep, declassification, covert channels) — a genuine new feature, farthest-out
  tier.
- **TBD: constant-time verification.** Given the secrecy labels above, the
  checker CAN prove the constant-time *discipline* (no secret-dependent branch or
  memory index) and codegen CAN restrict secret-touching code to
  data-independent-timing instructions. What it CANNOT prove is constant
  wall-clock — that is a hardware fact (ARM DIT / Intel DOITM) and lands in the
  {hardware} TCB. So constant-time = provable discipline on one named hardware
  assumption; depends on the label feature above.

## What is already aligned (no action)

- Machines/states/transitions as inspectable graphs — implemented, and the
  state graph artifact (`07_graph`) is exactly what Cathedral wants to schedule.
- Ownership/borrowing/moves — implemented; the single-writer-by-construction
  IPC story builds on what exists.
- Domains, contracts, proof obligations — implemented at the depth Cathedral
  currently needs.
- Direct native image emission with no external linker — Cathedral's boot
  chain wants exactly this.
- The differential interpreter oracle: under a single-address-space OS,
  **compiler correctness is the isolation boundary** (kernel_architecture.md
  lists miscompilation as an open trust question). The oracle is the standing
  mitigation; growing it alongside the backend is strategic, not hygiene.

## Maintenance

When a Tier 1 item gets a real design decision, record it in TASKS.md's
"Resolved Design Decisions" and update this page. When Cathedral's substrate
doc changes its "What Omega Still Needs to Grow" list, re-sync the TBD
register.
