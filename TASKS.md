> `OWNER_QUESTIONS.md` contains unresolved owner decisions only. Settled
> language rulings live in the guide and design briefs; this file tracks open
> engineering work only. Completed work belongs in git history and canary
> headers.

# Tasks

Last pruned: 2026-07-18.

Omega's first real consumer is Cathedral (`../Cathedral`). General language
work takes priority, with Cathedral vertical slices used as acceptance tests.
Fetch `main` before taking a compiler task and avoid a lane whose newest commit
is already changing the same subsystem.

## Immediate queue

### Checked assembly, inbound entry plans, and the Cathedral timer

This is the critical path from the current serial-only Cathedral milestone to
the first timer tick. The design is recorded in
`wiki/design_briefs/os_memory_and_hardware_foundation.md`, chapter 19, and
chapter 23.

1. **ASM1 — contract surface.** Add `asm where` contracts, structured target
   operands/register constraints, explicit clobbers, and availability classes
   to the strict parsed block. Reject hidden exits and unmodeled memory access.
2. **ASM2 — expand the x86 catalog.** Add save/restore flags, `cli`, `sti`,
   `lidt`, fences, and the needed MSR/control operations. Direct assembly and
   abstract boundary services must contribute identical normalized
   reach/authority. Mark `iretq` and equivalent exits deriver-only.
3. **ASM3 — retire the transitional instruction binding.** Replace every
   `Binding::Instruction` customer with checked assembly, then delete the
   binding variant and its compatibility paths.
4. **ENT1 — trait-parent composition.** Implement ordinary parent resolution
   and validation needed by `Calling<C>` without confusing core policy parents
   with boundary service-reach composition.
5. **ENT2 — paired entry plans.** Normalize `CallPlan` for ABI placement and
   `StatePlan` for initial regime, interrupted register classes, save/restore,
   stack/preemption class, and permitted transitive machine-state use. Hash the
   evaluated plan into requirement identity while keeping emitted footprint
   evidence outside public contract identity.
6. **ENT3 — constrained entry codegen.** Derive entry stubs, specialize/codegen
   under the state ceiling, emit a checkable final footprint certificate, and
   validate after relaxation, veneers, thunks, and generated stubs.
7. **IDT1 — fragmented layouts and symbolic materialization.** Move
   `LayoutPlan` to name-keyed fragmented placements, validate exact source and
   destination tiling, add sealed symbolic `Entry(EntryStubId)` relocation
   sources, and generate the post-load split-address writer.
8. **IDT2 — installed-root ledger.** `lidt` installation consumes scoped IDT
   authority and records every installed entry as an external analysis root
   with effects, receipts, state plan, stack/IST class, nesting/WCSU, and
   component/version pins. The stack/IST policy is one fact consumed by both
   layout materialization and WCSU analysis.
9. **IDT3 — linear interrupt obligations.** Implement saved-mask guards and EOI
   obligations as provider-minted linear values with explicit consuming
   restore/complete operations. Do not use drop cleanup or interrupt-specific
   linearity rules.
10. **Cathedral timer acceptance.** Program PIT or LAPIC, install the IDT, post
    a bounded tick event, report ticks over the owned serial line, and `hlt`
    between ticks under QEMU. Negative rails: direct assembly cannot launder
    reach; user `iretq` rejects; incomplete fragment tiling rejects; forbidden
    final-artifact clobbers reject; omitted or double EOI rejects.

### Provider plans and retirement of `provides`

Provider plans are derived from `satisfies` closure. Checked adapters have
Omega bodies; irreducible leaves use
`satisfies Requirement via <Binding>;`. Target packages provide defaults and a
slot owner may override by type. The migration order remains load-bearing.

1. **PRV4a — text-call argument lowering.** Fix literal-to-`String` parameter
   threading through native host text calls. Field-backed text works; the
   literal form currently reports no encodable call sequence and blocks the
   Console row flip.
2. **PRV4b — Console adapters.** Add the standard `write_line`/`write` checked
   adapters over byte operations, compare them against the lossless built-in
   oracle, make both interpreter and native routes use them, then remove the
   built-in Console composite rows.
3. **PRV4c — target defaults and overrides.** Add target-package default
   provider types plus explicit type-per-slot build selection. Validate full
   coverage, signatures, transitive effect refinement, dependency closure,
   normalized identity, and ambiguity at the selected target only.
4. **PRV4d — remaining leaf mechanisms.** Wire authored/leaf Syscall rows into
   the same lowering and admission path. Preserve the deliberate interpreter
   refusal for arbitrary host imports rather than using evaluator `dlsym`.
5. **PRV4e — foreign format facts.** Move foreign offsets and bit constants
   from `Binding::Value` into programmable layout/format declarations and
   migrate filesystem leaves.
6. **PRV4f — compatibility deletion.** After the last consumers move, delete
   `call_shape`, `HostOperations`, `Value`, populate tables, `provides` syntax,
   and every compatibility consumer. Keep only the directed retirement
   diagnostic if useful.
7. **Supply-shape negatives.** Finish coverage for qualified
   `Binding::DllImport`, runtime-dependent `via` values, missing `satisfies`,
   repeated `effects`, signature mismatch, and admission/refinement failure.

### Compile-time machine parameters and generics

The source model is fixed: `<machine M>` requires an authored
`where machine M(args) -> Result` contract; selection such as
`map<Card::power>(items)` is compile-time symbol metadata, never a runtime
argument or inferred contract.

1. **MP4b — full template specialization.** Compose typed deep copy, one
   lexical-symbol remap, and fresh symbols to clone a complete template;
   group/rewrite calls per selected tuple; remove the single-tuple fence.
2. **MP5 — admitted templates.** Grant accepted templates once, preserve their
   contract identity, and validate each instance against argument contract IDs.
3. **MP6 — consuming slices.** Add `Seq` map/filter, nested proof schemas used
   by N5/N6, task-runtime machine selection, and build-surface canaries. No
   runtime callable values, dictionaries, or capture inference.
4. **Generic inference negative.** Resolve
   `canaries/pending/generics/runtime_generic_param_position_inference_exit`:
   a parameter that only happens to line up at existing call sites must not
   acquire an implicit machine contract.

## Correctness bugs and missing lowering

These are unblocked and should gain a focused pass/fail or differential canary
before the fix.

- **Restore the pass-canary baseline.** The broad compile gate is currently red
  in independent clusters: `CommutativeSemiring::mul_identity` satisfier
  ambiguity across the core Nat/Rat/rearrange corpus; default-domain length,
  equality, capacity, and standing-bound facts; anonymous
  exact-Rat arithmetic in runtime guards; and nonliteral exact float-to-int
  proofs. The fail gate additionally exposes
  unfinished arithmetic-measured `Nat` extraction/refutation; do not weaken
  those canaries to accept the earlier unsupported-tier rejection.
  Fix the implementation or migrate a stale canary only when the settled guide
  proves the canary spelling wrong; do not normalize a red pass corpus.
- **Constant-offset recast stale read.** A record view such as
  `&self.buf[K] as &Rec` can fold field reads against the zero-initialized
  static image after runtime writes. A small runtime-offset loop can likewise
  specialize to wrong fixed displacements. Fix frontend recast/const-fold
  provenance and add a genuinely runtime-but-small differential witness.
- **Pre-guard local initialization.** Resolve
  `texteq_local_guard_read_divergence` and its argument-forward twin. Dispatch
  guards need a per-branch pre-guard region for call-free local-data
  initializers without moving side effects ahead of arm selection. The
  trailing-state mutable-parameter divergence may share this phase boundary.
- **Threaded mutable receiver phases.** Resolve
  `trailing_state_mut_param_phase_divergence` and the same-type receiver
  aliasing fence for ambiguous multi-call states.
- **State-call binary local argument.** A binary-initialized local passed to a
  state call inside a proof-bearing guarded state still reports that a
  `CallArgument` binary expression needs runtime lowering. Make the state-value
  fold copy the local's slot.
- **Large-referee expression lowering.** Assignment RHS
  `self.x = ref.field` and `&self.field` call arguments must not resolve inline
  when the referee requires an address-bearing place.
- **Terminal value-machine call on boot paths.** Triage the remaining red
  `value_call_terminal` shape before Cathedral scheduler code relies on
  machine calls.
- **Trapping constant shift.** The state-values folder must not wrap a constant
  `Trapping` left-shift whose value overflows while runtime lowering traps.
- **Non-place record-pattern subjects.** Exhaustiveness validation currently
  lacks a declared type for non-place subjects. Preserve single evaluation and
  enforce the same missing/unknown-field law; add the copy-eligibility fence if
  non-copy extraction is not yet sound.
- **Unlowered payload fields.** When the next unsupported payload-field shape
  appears, turn `UnloweredCaseLiteralField` into a directed fail canary rather
  than silently widening lowering.

## Type, proof, and semantic-model work

### Dependent facts and frames

- **R1/R4 — cross-machine equalities.** Transport value equality such as
  `requires a.cols == b.rows` across machine boundaries and couple symbolic
  upper/lower witnesses used by recast and matrix shapes.
- **R3 — store-proof completion.** Replace permissive unbounded-store seeding
  with a sound post-entry fact plan without flipping valid corpus shapes.
- **R5 — frames.** Implement preserve-unless-written facts, the `stores`
  clause, state-arrival facts, and Houdini-style inference for facts crossing
  sibling calls.

### Domain facets, effects, termination, and trust

- **DOM1 — facet kinds.** Enforce predicate versus semantic facets through
  merges, joins, casts, and generic substitution with per-axis composition.
- **DOM2 — binding-site operators.** Resolve operator theory from declarations,
  mints, and `requires`; never from flow facts. Resolve tuples deterministically
  and reject collisions.
- **DOM3 — introduction authority.** Implement sealed-by-default domains,
  `introduction open`, and `MintAuthority<D>` with distinct missing-proof and
  missing-authority diagnostics.
- **DOM4 — normalized identity.** Finish the deterministic domain-expression
  normalizer and make it own type/monomorphization identity.
- **DOM5 — weakening.** Add `weakens_to` certificates and sealed-theory hashes
  that detect stale operator theories.
- **STR — semantic carrier cleanup.** Finish termination-plan integration,
  validation/resolution from normalized domain/machine/permission plans,
  lowering only from checked selections, and deletion of compatibility bools.
- **EFX — kinded effects completion.** Resolve boundary-trait and core members;
  compute transitive recursive fixed points; enforce public ceilings and pinned
  provider subsets; split artifact/diagnostic/trust-ledger output; migrate core,
  std, and canaries away from the lowercase global table as semantic canon.
- **TPR4/TPR6 — publication and progress profiles.** Serialize public
  termination omission/default rules in artifacts. Resolve sealed profile
  domains, grant-backed admission and receipts, and pinned progress premises.
  Profiles are never flow-inferred ranking evidence.
- **GR6 — remaining trust consumers.** Finish qualification authority,
  ProgressProfile minting/premises, and MachineContractPlan permission/provider
  admission through the existing grant/receipt carrier.

### Carry, multiplicity, task lifecycle, and allocation

- **CRY1–CRY6 — four-axis carry policy.** Propagate the normalized
  suspension/CPU/thread/address record through all trees and snapshots; parse
  `[carry(...)]` and remove `[send]`; derive transparent aggregates and generic
  bounds; add sealed per-mint facts; check canonical live sets locally; join
  activation demands with pessimistic admitted runtime behavior and emit
  diagnostic/artifact/model-export facts.
- **CML4 — finish multiplicity migration.** Remove downstream dependence on
  legacy move/drop arenas, cover remaining ownership forms and per-field debt,
  and lower semantic permission events into explicit backend transfers. Do not
  infer establishment from zero storage.
- **TR2b — transactional outcomes.** Preserve substituted linear debt through
  `Returned(T)` and `Rejected(Arguments)` rather than laundering it through an
  unconstrained generic payload.
- **TR3 — activation plans.** Elaborate `runtime.start<M>(args)` into contract
  and entry IDs, argument/result layouts, continuation/alignment/pinning, carry
  demands, cancellation, and effect metadata.
- **TR4 — runtime requirement and admission.** Add the `TaskRuntime` boundary
  requirement and ensure rejected start returns every moved argument and lease.
- **TR5 — custody and storage leases.** Track provider provenance and dependent
  child storage so close/reclaim rejects while claims remain live.
- **TR6 — continuations and first provider.** Lower continuations; admit inline
  completion only when the pinned contract permits it.
- **TR7 — suspension-safe loans.** Enforce the conservative moved/shared-
  immutable/synchronized subset and integrate carry checking.
- **TR8 — reference packages.** Build `ArenaTaskPool`, bounded mailbox, and
  supervisor packages, then migrate samples. Package ergonomics do not justify
  new core syntax without a semantic impossibility.
- **Allocator migration.** Replace ambient legacy `alloc` with explicit
  `Arena`/`Allocation` contracts and migrate Cathedral's obsolete bootstrap
  `Region`/`mint_region` carrier to `Extent`. Structural multiplicity, not a
  permanent semantic ban, governs debt-bearing `Allocation<T>`.
- **Vec and slices.** Implement owned dynamic `Vec<T>` storage plus
  `as_slice`/`as_mut_slice` over real allocation/extents.

### Mathematical and float libraries

- **N5 — Real boundary package.** Add the opaque carrier and accepted axiom
  package; claim-free boundary symbols need no grant, while accepted axioms use
  the ordinary trust carrier. Excluded middle remains an ungranted core
  boundary machine.
- **N6 — quotients.** Implement the settled
  `data Real = CauchySeq % converges_together` type expression, carrier-only
  `as` construction, respect/congruence obligations, and equivalence laws.
- **N7 — nested schemas.** Support proof data parameterized by machines and
  machine-parameter signatures that themselves take machine parameters.
- **N8 — construction corpus.** Build Cauchy Real, order, completeness, and
  well-definedness, retiring axioms through the normal boundary-upgrade path.
- **Divisibility theory.** Add `gcd_pos`, `gcd_dvd_left/right`, and
  `div_mul_cancel` when the Rat/quotient corpus demands them.
- **F6 — total float order.** Add named `TotalOrder` satisfiers for f32/f64
  using sign-magnitude integer comparison once satisfier dispatch serves.
- **F7 — float format providers.** Move IEEE format records into
  `omega::core` and express their lowering through checked provider plans and
  checked assembly, not the retiring `Binding::Instruction` variant.

## Layout, memory, and artifact foundation

- **L4/L5 — plan-laid views.** Derive projection over plan-laid byte views,
  complete non-scalar and mutable recast views, validate tiling beyond
  fact-free shapes, enforce validate/materialize mint exclusivity, and prove
  codec conformance through ordinary policy machines.
- **L6a — Extent.** Implement the opaque linear concrete-range carrier with
  sealed space/rights/provenance/era facts, move-split, common-origin merge,
  borrow-carrying subranges, and provider-backed map/unmap plus
  shootdown/quiescence.
- **L6b — AccessPlan and placed views.** Pair layout geometry with a separate
  normalized exact-access plan; derive sealed field access, snapshot reads,
  whole writes, and typed atomics while preserving borrow polarity and static
  reach. Never expose arbitrary-offset volatile access or per-access revocation
  probes.
- **L6c — symbolic materializer.** Add relocation-valued fragmented fields,
  phase/placement constraints, generated writers, and validation for
  loader-consumed versus post-handoff structures.
- **External loans.** Represent DMA/device borrowing with linear proxy tokens,
  completion/fence/cache obligations, and CPU-access exclusion through the
  ordinary permission context.
- **EXI1–EXI5 — admitted executable installation.** Add reusable admitted
  artifact identity, extent-backed linear `CodePlacement`, materialization,
  write freeze, final-byte/footprint validation, scoped installation,
  synchronous visibility, W^X reporting, and replacement/quiescence for live
  code. Use the minimal checked Omega container internally; PE/COFF is only a
  firmware envelope. No arbitrary byte-to-code path exists.
- **Wire runtime.** Implement runtime layout for wire values, additional
  encoding families, compatibility reports, and version negotiation.
- **Historical-format cleanup.** Delete the implemented `Versioned<T>` parser,
  IR, checker, and lowering paths; migrate every sample/canary to immutable era
  data, ordinary sum envelopes, provenance domains, codecs, and conversion
  machines. Remove the obsolete reconciliation record after references reach
  zero.

## Remaining language surfaces

- **Lifetimes.** Implement the decision-15 `'name` lifetime arc and borrow-
  carrying data needed by placed views and task storage.
- **Transition patterns.** Finish real pattern binding, multi-subject
  validation, domain-pattern proofs, and diagnostics. Preserve exhaustive
  record destructuring and single evaluation.
- **Const data parameters.** Add instantiation-time substitution, validation,
  layout diagnostics, and const-fact proof integration.
- **Trait defaults.** Implement conformance, reuse, override, and dispatch for
  trait machines whose body supplies the default. Do not restore a `default`
  keyword.
- **Dynamic traits.** Implement `dyn Trait` construction, descriptors carrying
  satisfier identity, vtable emission, dispatch lowering, and object-safety.
- **Equatable synthesis.** Provide a callable conformance surface rather than
  structural magic.
- **Build-time evaluation.** Add compile-time evaluation and trait generators
  for effect-free machines in value/refinement position.
- **Separate compilation and component artifacts.** Normalize imports, pinned
  contracts, provider selections, artifact identities, and replacement
  certificates without hashing private implementation witnesses into public
  identity.
- **Hot swap.** Implement liveness pins, quiescence proofs, and borrows as swap
  barriers through packages and admitted runtime operations; add no `replace`
  syntax.
- **Serialized capabilities.** Implement attenuation and revocation across
  boundaries.
- **Text domains and String retirement.** Establish `Utf8`/`NoNul` over
  `[u8]`, add the compile-time/runtime mint paths and loop-invariant proofs,
  migrate the corpus, then delete builtin `string`/`String` and backend
  special cases. Follow `wiki/architecture/string_retirement_execution.md`.
- **Atomics remainder.** Complete the memory-model and operation set beyond the
  existing first-stage read-modify-write operations.
- **Proof engine.** Continue induction and proof-data support required by
  layouts, quotients, and Real.

## Vertical acceptance slices

- **Termination firewall.** Pin one public `terminates` requirement inherited
  by acyclic and cyclic providers; swap descending and bounded-increasing
  witnesses without changing caller/import-slot identity; reject runtime
  non-tail lowering and ungranted progress profiles.
- **Kinded effects.** Demonstrate separate service reach and `Suspend`/`Block`
  members, recursive inference, public-ceiling failures, provider subset
  admission, and stable normalized IDs independent of prover strength.
- **Units.** Before broad generic work, implement two units in one dimension
  and pin: explicit conversion, scaled dimensionless results, distinct
  Energy/Torque kinds, generic preservation, no silent forgetting, and package
  coherence for operator tuples.
- **OS gauntlet.** Validate the foundation against UART/MMIO, page tables,
  DMA, shared-page IPC, IDT/timer entry, and SMP AP bringup. A customer that
  needs a new keyword or customer-shaped primitive returns to design review.

## Owner-blocked

- **CFI3–CFI5 — protected returns and final CFI.** The forward edge can proceed
  with sealed entry references and descriptor identity. Protected returns,
  continuation/exception/interrupt preservation, final indirect-site
  certificates, and foreign-provider isolation/receipts wait on
  `OWNER_QUESTIONS.md`. Executable installation prevents injection; it does not
  prove legal control transfer.

## Platform-gated verification

- **Linux hosts.** Run filesystem/time structural rows on real x86_64 and
  AArch64 Linux. `clock_gettime` additionally needs composite `timespec`
  lowering before it can be verified.
- **macOS/x86 and other unavailable hosts.** Keep target emission structurally
  pinned; do not claim runtime verification without the host.
- **Windows GUI callback entry.** Implement WndProc inbound entry stubs and a
  real title-bar close path using the general entry-plan work above.

## Deferred until a real customer

- Richer measured-recursion guards and multi-subject lexicographic cycles.
- Reduced-Rat divisibility theory beyond what N5/N6 demands.
- Async extent revocation beyond provider quiescence.
- Non-blocking executable-visibility tokens.
- Runtime-generated host code/JIT and arbitrary self-modifying code remain
  intentionally unsupported, not backlog items.
- Universe levels wait for a full-mathlib replay goal.
- A serious SSA/register-allocation/SIMD backend is post-1.0; correctness of
  current native output remains the active bar.
