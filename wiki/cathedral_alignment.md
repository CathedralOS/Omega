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

2. **`wire data` semantics** (STRUCTURAL today). Cathedral's IPC, networking,
   on-disk storage format, package compatibility checking, and (eventually)
   serialized capabilities are all `wire data`. Schema validation, duplicate/
   reserved tag diagnostics, version compatibility rules, and encoder/decoder
   generation need to become real. This is also the natural next differential-
   oracle-friendly feature: encoders/decoders are pure functions with byte-
   exact expected outputs.

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

5. **Concurrency: pick the model's hard answers** (DESIGN-ONLY; every target
   declares `threads = disabled`; zero canaries). Chapter 17's `spawn`/`Join`
   sketch is fine, but Cathedral needs answers to the questions the appendix
   already lists: *can typed state clusters suspend across ticks* (that IS the
   scheduler-suspension question), cancellation/deadline propagation
   (structured concurrency), and how "no locks because ownership" interacts
   with a real scheduler. The OS design cannot finalize its scheduler chapter
   until the language answers these.

6. **Atomics and a memory model** (ABSENT — the word "atomic" appears in no
   language chapter). Cathedral's one IPC primitive is a shared-memory ring
   driven by "plain reads and writes plus atomics," and any SMP kernel needs
   ordered atomics. Decide the shape now: language primitives, boundary
   operators with contracts, or a core library over compiler intrinsics — and
   which orderings exist. This gates IPC, the scheduler, and `spawn`.

7. **Freestanding target + hardware access vocabulary** (ABSENT/narrow).
   Boot needs: a target with *no* host bindings and a custom entry (the
   current target model assumes stdout/stdin/process capabilities), linker/
   section/physical-address control for the image writers, **volatile/MMIO
   access semantics** (absent from chapter 19), and inline asm grown beyond
   `asm { jmp state(...) }` (CR3/MSR/port-IO instruction contracts — the
   appendix already lists the open questions; they need answers). The direct
   image emission bet is aligned with this; the gap is the freestanding
   flavor of it.

8. **Case members (sum/mixed data shapes)** — DESIGN DECIDED, implementation
   pending. There is no separate `enum` type: alternatives are a member class
   of `data` (`case` members, named payload fields, MIXED shapes = common
   fields + a case part in one declaration), so case-bearing types get
   domains, versions, and `wire data` for free, and case-subset domains
   replace shadow enums. See chapter 1 + TASKS.md frozen decision 7. The
   typed trees already model this (`DataMember::Field | Variant`,
   `DataShapeKind::Mixed`); the work is parser (`case` syntax, payloads),
   pattern binding, and layout/lowering. Cathedral's typed `Failure` causes
   and every driver/protocol record want this; the longer samples avoid it,
   the more code is written around it.

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
