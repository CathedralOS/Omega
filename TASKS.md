> `OWNER_QUESTIONS.md` contains unresolved owner decisions only. Settled
> language rulings live in the guide and design briefs; this file tracks open
> engineering work only. Completed work belongs in git history and canary
> headers.

# Tasks

Last pruned: 2026-07-20.

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

The checked x86 catalog includes structured control-register, MSR, flags,
fence, and interrupt-mask operations. `iretq`, `sysret`/`sysretq`, and `eret`
are deriver-only. Do not expose source-level `lidt` before IDT2: the current
freestanding-root authority bridge cannot record installed inbound roots, so a
raw catalog entry would create an effect/WCSU audit hole.

The first ENT2 slice is implemented in `omega-calling-conventions`: normalized
register/value-placement vocabularies, deterministic `CallPlan + StatePlan`
identity, MS-x64/SysV-x64/AAPCS64/Linux-syscall evaluators, plan validation,
and a separately fingerprinted footprint-evidence carrier. Existing host
bindings can be evaluated through this model as an independent oracle while
their hardcoded encoders remain in service. The inbound process-entry
argument prologue now consumes the normalized native `CallPlan`'s exact
register and width on x86-64 and AArch64; incoming stack arguments and scalar
float register locations are covered as well. Integer process-entry results
now carry the plan-selected result register through both native encoders. Flat
one-to-four-member AAPCS64 homogeneous floating-point aggregate entry
parameters are classified from their normalized record layout and spread
across the selected vector registers. Nested and general aggregate ABI
classification remains; unsupported mixed/general entry signatures retain
the compatibility path without panicking the compiler. Generic Linux syscall
leaves now evaluate the normalized syscall policy at emission and pass its
exact parameter registers, number register, and supervisor-call immediate into
both ISA encoders; the legacy binding fields no longer choose those facts on
that path. The normalization seam also enforces policy, supervisor-call control,
stack/shadow facts, and the encoder scratch/clobber ceiling. Runtime-storage
x86-64 syscall arguments now stage through volatile `r11`/`rax` instead of
silently destroying callee-saved `r15`; AArch64 large-offset marshalling reuses
the plan-selected `x8` number register, which the plan now declares clobbered.
Composite runtime-text byte and line syscalls now consume the same normalized
placements:
the AArch64 encoders honor the plan-selected registers and supervisor-call
immediate, while the fixed x86-64 sequences reject plans they cannot realize
instead of silently choosing an ABI. AArch64 C/import calls and their results
now evaluate AAPCS64 from selected operand shapes and pass the plan's exact X/V
registers and stack placements to the ISA encoder, including scalar stack
arguments and flat HFA arguments/results. The general Microsoft x64 import path
now derives its policy from the concrete target, evaluates argument/result shapes,
consumes the plan's register and shadow-relative stack placements, and rejects
non-Microsoft x86 policies
instead of silently applying Win64. Microsoft x64 vtable and firmware
service-table calls now use the same plan-driven register/stack marshaller,
with dispatch-only table pointers excluded from the wire signature and
plan-selected results checked before storage. The ordinary host-operation
Windows imports now consume evaluated Microsoft x64 placements. `GetStdHandle`,
`ExitProcess`, and `Sleep` use the plan-driven general marshaller with
byte-identical relocation layouts.
`GetAsyncKeyState` now consumes planned RCX/RAX placements while preserving
its required 16-bit zero-extension result transform.
Windows time out-parameter calls now evaluate their actual foreign signatures:
one planned RCX pointer plus an ignored planned RAX `BOOL` for QPC/QPF, or void
for `GetSystemTimePreciseAsFileTime`; the temporary stack slot remains an
encoder materialization detail. The x86 constant-result routing remains
operation-specific so QPF cannot be mistaken for AArch64's frequency constant.
Composite `ReadFile`/`WriteFile` calls now model their actual five-parameter
signature and ignored `BOOL` result; RCX/RDX/R8/R9, the shadow-relative fifth
argument, and the scratch-slot reservation all come from that evaluated plan.
The dedicated runtime line and byte Windows sequences reuse that exact layout
plus an actual one-DWORD/RAX `GetStdHandle` plan, preserving their fixed widths
and relocation sites. AArch64 fragmented calls and source-selected policies
remain below. Ordinary/firmware entry lowering now validates a combined
boundary plan with no interrupted state, no save/restore obligation, a
provider-selected stack, non-preemptive entry semantics, and a transitive state
ceiling derived exactly from the ABI volatile-register classes.

1. **ENT2b — source policy evaluation and identity (OWNER-BLOCKED: see
   `OWNER_QUESTIONS.md` section 2).** Evaluate the policy type
   selected by `Calling<C>` against each requirement signature, validate the
   resulting `BoundaryEntryPlan`, and put its normalized fingerprint—not merely
   `C`'s symbol—into published requirement identity.
2. **ENT2c — lowering migration and concrete entry state.** Express the
   existing MS-x64, SysV-x64, AAPCS64, Linux-syscall, and firmware lowering
   choices through the normalized plan; continue beyond the completed
   register- and stack-resident process-entry argument paths and integer entry
   results and generic and runtime-text Linux syscall paths to C/firmware
   outbound calls/results and compatibility-binding differential checks; the
   register-resident AArch64 C/import slice is complete, including exact
   plan-selected argument/result registers and fail-closed unsupported
   placements. The general Microsoft x64 import path likewise consumes exact
   planned register/stack/result placements and target-derived policy, as do
   Microsoft vtable and firmware service-table calls. Ordinary composite
   x86-64 host operations are now plan-checked through their actual foreign
   signatures, as are the dedicated runtime line/byte Windows sequences.
   Compatibility syscall rows are differentially checked against normalized
   number-register and supervisor-call facts on both Linux architectures; the
   generic encoders additionally reject incompatible policy/control/stack/
   shadow/clobber contracts and keep all marshalling scratch inside the
   normalized ordinary-clobber ceiling.
   Scalar AAPCS64 outbound stack placements now reserve aligned outgoing space,
   materialize integer/pointer or float values through caller-saved scratch
   registers, store at plan-selected offsets, restore SP after the call, and
   feed the same overhead into layout and both relocation walkers. Flat
   two-to-four-member HFA arguments now remain one by-value operand through
   selection and consume every plan-selected vector-register fragment; grouped
   placements also drive layout and relocation accounting. When the vector bank
   is exhausted, the same operand copies each member into its contiguous planned
   stack area. Authored flat HFA results now preserve one aggregate result place
   and spill every plan-selected vector-register fragment through one relocated
   base. The AArch64 import normalization seam now also rejects plans whose
   policy, call/return control, 16-byte stack alignment, zero-shadow-space
   contract, or ordinary-clobber ceiling cannot cover the encoder's fixed
   caller-saved scratch set; placement is no longer the only enforced plan
   facet. Continue making the plan authoritative across compatibility paths.
   The concrete x86 interrupt
   `StatePlan`, stack/IST, nesting, and acknowledgement policy used by Cathedral
   is OWNER-BLOCKED on `OWNER_QUESTIONS.md` section 3.
3. **ENT3 — constrained entry codegen.** Derive entry stubs, specialize/codegen
   under the state ceiling, emit a checkable final footprint certificate, and
   validate after relaxation, veneers, thunks, and generated stubs.
4. **IDT1 — symbolic materialization (normalized foundation complete).**
   `LayoutPlan` now uses compiler-issued field keys normalized back to names;
   repeated `Bits` entries validate exact logical-source tiling plus
   destination bounds/overlap, while ordinary plan-laid values require one
   `At` per field. Normalized sealed `Data(DataSymbolId) | Entry(EntryStubId)`
   sources now derive resolved writes, native whole-pointer relocations, and
   post-handoff writer records while rejecting loader-consumed unresolved
   fragments. Object/image relocation sites are section-qualified, generic
   `Absolute64` relocations patch initialized data on both native families,
   and PE rebasing records data sites correctly. Native symbolic actions now
   lower with an explicit materialization origin rather than fake instruction
   metadata. Normalized placement constraints now join layout alignment with
   permitted address range, build/load/post-handoff phase, machine-regime
   identity, and artifact-installation scope, and validate concrete sites. The
   decoded constraint record is now bound into artifact admission and must match
   the exact record carried by the claimed placement at materialization, so a
   provider cannot substitute weaker constraints behind the admitted placement
   plan identity. Wire entry identities from selected artifacts and lower the
   now-derived atomic post-handoff writer programs to generated machine code.
5. **IDT2 — installed-root ledger.** Add `lidt` only as an installation path
   that consumes scoped IDT
   authority and records every installed entry as an external analysis root
   with effects, receipts, state plan, stack/IST class, nesting/WCSU, and
   component/version pins. The stack/IST policy is one fact consumed by both
   layout materialization and WCSU analysis.
6. **IDT3 — linear interrupt obligations.** Implement saved-mask guards and EOI
   obligations as provider-minted linear values with explicit consuming
   restore/complete operations. Do not use drop cleanup or interrupt-specific
   linearity rules.
7. **Cathedral timer acceptance.** Program PIT or LAPIC, install the IDT, post
    a bounded tick event, report ticks over the owned serial line, and `hlt`
    between ticks under QEMU. Negative rails: direct assembly cannot launder
    reach; user `iretq` rejects; incomplete fragment tiling rejects; forbidden
    final-artifact clobbers reject; omitted or double EOI rejects.

### Provider plans and retirement of `provides`

Provider plans are derived from `satisfies` closure. Checked adapters have
Omega bodies; irreducible leaves use
`satisfies Requirement via <Binding>;`. Target packages provide defaults and a
slot owner may override by type. The migration order remains load-bearing.

1. **PRV4b — Console adapters.** The honest owned-`String` to
   borrowed-byte-view runtime path (`as_view`/`bytes`) now runs in both engines,
   and standard `Console::write` and `Console::write_line` are checked Omega
   code: self-forwarding adapters walk that view with measured `Slice::Length`
   state transitions and reach only `write_byte`. Field-backed, literal-backed,
   and empty-line cases run differentially; the checked-tree canary pins both
   calls to their adapters, and the lossless built-in plan oracle remains green.
   More than 1,300 exact duplicate Console declarations now import that package.
   The remaining local declarations are intentionally different carrier,
   effect, or proof fixtures; migrate those with their owning surfaces, then
   remove the composite compatibility rows under PRV4f.
2. **PRV4c — target defaults and overrides.** Candidate plans are now keyed by
   provider type, unrelated conformance closures never combine, and only the
   selected covering candidate reaches adapter or leaf lowering. Explicit
   type-per-slot `build.omg` selection now validates the provider against the
   loaded dependency closure and is confined to the build root's slot-owner
   authority. Add target-package default provider declarations and extend the
   same contract to test/component slot owners. Coverage, signature conformance,
   transitive effect refinement, normalized identity, and selected-target-only
   ambiguity are already enforced.
3. **PRV4e — foreign format facts.** Move foreign offsets and bit constants
   from `Binding::Value` into programmable layout/format declarations and
   migrate filesystem leaves.
4. **PRV4f — compatibility deletion.** After the last consumers move, delete
   `call_shape`, `HostOperations`, `Value`, populate tables, `provides` syntax,
   and every compatibility consumer. Keep only the directed retirement
   diagnostic if useful.
### Compile-time machine parameters and generics

The source model is fixed: `<machine M>` requires an authored
`where machine M(args) -> Result` contract; selection such as
`map<Card::power>(items)` is compile-time symbol metadata, never a runtime
argument or inferred contract. MP4b now groups complete call-site tuples,
deep-copies each additional template body with fresh lexical symbols, rewrites
calls to their concrete states, and runs distinct type and static-machine
specializations in both engines. MP5 now captures a binder-positional universal
template-contract identity before substitution, spends one trust receipt for an
accepted template, binds every instance to the checked contract identities of
its selected static machines, and exports that relation in the machine-contract
manifest. Contract changes invalidate instances; implementation-body-only edits
remain contract-invisible.

1. **MP6 — remaining consuming slices.** `Seq`'s consuming `map`/`filter` are
   now core machines: recursive static-machine selections specialize to direct
   calls, with no runtime callable, dictionary, or capture inference. Still add
   the nested proof schemas used by N5/N6, task-runtime machine selection, and
   the remaining build-surface canaries.

## Correctness bugs and missing lowering

These are unblocked and should gain a focused pass/fail or differential canary
before the fix.

## Type, proof, and semantic-model work

### Dependent facts and frames

- **R5 — frames.** Direct and acyclic transitive internal calls, plus resolved
  boundary calls, now preserve linear arithmetic facts, recast and
  boundary-range witnesses, dependent entry and forwarding facts, and exact
  default-domain valuations outside their conservatively instantiated
  receiver/exclusive-argument may-write paths. Value-position calls use the
  same summaries recursively; when body analysis is unavailable (unknown,
  transitioning, static-machine, or cyclic callees), their ownership-bounded
  fallback invalidates the whole receiver plus explicit mutable arguments but
  preserves unpassed caller locals. Finish the `stores` clause, explicit
  state-arrival contracts, and broader Houdini-style inference for facts
  crossing sibling calls.

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

- **CRY1–CRY6 — four-axis carry policy.** The normalized
  suspension/CPU/thread/address record now survives syntax, resolved, typed,
  syntax snapshots, and a checked `CarryFacts` plan that separates authored
  minimums from effective derived policies. `[carry(...)]` requires all
  four axes, `[send]` has a directed retirement diagnostic, transparent
  aggregates derive per-axis intersections, and data/machine generic bounds
  compare the complete policy (including specialization admission). Concrete
  generic instantiations derive through symbol-keyed argument substitution,
  including nested wrappers. Opaque `boundary data` carriers now parse without
  a public shape or layout, cannot be constructed by ordinary code, default to
  the strict effective carry policy, and reject permissive property claims
  until admission can provide receipts. Statement-bound canonical liveness now
  rejects parameters and locals whose effective policy forbids suspension when
  they remain live across a direct or transitive `Suspend` call. Field-segment
  liveness also tracks attached-data fields and compatibility machine-owned
  cells through reachable state transitions without collapsing them into
  whole-`self`; effect, borrow, flow, and contract analyses join calls by the
  shared `(state, statement, ordinal)` identity. Intra-statement checking keeps
  that preorder identity while applying left-to-right evaluation: call
  arguments count as live during the call, and later operands cross an earlier
  nested suspending call. Call-carried generic parameters read the target
  declaration's normalized carry bounds rather than a same-spelled caller
  parameter. The legacy `Machine::contains` carrier has no source parser and
  must be deliberately retired or reintroduced before subtree carry semantics
  have a real customer. Continue with admitted and sealed per-mint facts,
  activation-demand joins against pessimistic admitted runtime behavior, and
  diagnostic and model-export consumers. Checked builds now emit
  `05_carry_manifest.json`, keeping authored minimums separate from effective
  derived policies with all four axes structured.
- **CML4 — finish multiplicity migration.** Remove downstream dependence on
  legacy move/drop arenas, cover remaining ownership forms and per-field debt,
  and lower semantic permission events into explicit backend transfers. Do not
  infer establishment from zero storage.
- **TR2b — transactional outcomes.** Preserve substituted linear debt through
  `Returned(T)` and `Rejected(Arguments)` rather than laundering it through an
  unconstrained generic payload.
- **TR3 — activation plans.** The normalized `omega-task-plans` candidate and
  validator are live for contract/entry/calling-plan IDs, argument/outcome
  layouts, continuation size/alignment, cancellation, distinct-versus-inline
  execution, local suspension safety, and separate safe-point/asynchronous
  migration-demand envelopes. Connect `runtime.start<M>(args)` elaboration,
  canonical liveness/carry derivation, and effect metadata.
- **TR4 — runtime requirement and admission.** The normalized demand/behavior
  join is live: provider storage/capacity, cancellation, inline behavior,
  preemption granularity, CPU/thread migration, and continuation movement fail
  closed against the activation plan; unknown runtimes are pessimistic. Add the
  `TaskRuntime` boundary requirement/provider-plan integration and ensure a
  rejected transactional start returns every moved argument and lease.
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

- **N6 — quotients.** Implement the settled
  `data Real = CauchySeq % converges_together` type expression, carrier-only
  `as` construction, respect/congruence obligations, and equivalence laws.
- **N7 — nested schemas.** Support proof data parameterized by machines and
  machine-parameter signatures that themselves take machine parameters.
- **N8 — construction corpus.** Build Cauchy Real, order, completeness, and
  well-definedness, retiring axioms through the normal boundary-upgrade path.
- **F6 — total float order.** Add named `TotalOrder` satisfiers for f32/f64
  using sign-magnitude integer comparison once satisfier dispatch serves.
- **F7 — float format providers.** `FloatFormat::BINARY32` and
  `FloatFormat::BINARY64` now live in `omega::core` as ordinary semantic data.
  Replace the hardcoded IEEE lowering bootstrap with checked target
  conformances, derived provider plans, and checked assembly; there is no
  instruction-binding compatibility path.

## Layout, memory, and artifact foundation

- **L4/L5 — plan-laid views.** Derive projection over plan-laid byte views,
  complete non-scalar and mutable recast views, validate tiling beyond
  fact-free shapes, enforce validate/materialize mint exclusivity, and prove
  codec conformance through ordinary policy machines.
- **L6a — Extent.** The normalized conservation foundation is live in
  `omega-extents`: admitted one-shot root grants mint nonempty ranges;
  move-split preserves exact geometry; only compatible siblings from one
  split lineage merge; attenuation cannot add open-set rights; failed
  consuming operations return their authority; and one borrow-carrying loan
  derives shared/exclusive polarity from its parent. Fixed-destination mapping
  now consumes virtual authority while independently owning, shared-borrowing,
  or exclusive-borrowing its source; unmap returns reusable ranges only after
  an exact provider receipt releases stale translations and establishes its
  open completion facts. Connect these models to the opaque Omega `[linear]`
  carrier, sealed fact establishment, provider execution/effects, and source
  APIs.
- **L6b — AccessPlan and placed views.** The separate normalized validator is
  live: name-keyed entries pin exact transfer width, stable/external/atomic
  observation, ordinary and atomic permissions, exported versus
  provider-private access, and static service reach. Validation checks fixed
  layout geometry, rejects multi-container one-access laundering and public
  external RMW, and enforces borrow polarity at operation authorization. Add
  the Omega-authored policy surface, source-level borrow-carrying access
  values, and exact external/atomic lowering. Provider-admitted placed-view
  grants now check an actual Extent loan's space, provenance, open-set rights,
  size, and permitted static reaches; field authorization derives polarity
  from that loan and mints the only token primitive lowering may accept. Never
  expose arbitrary-offset access or per-access revocation probes.
- **L6c — symbolic materializer.** The normalized source/action plan and
  loader-versus-post-handoff validation are live. Range/alignment/phase/regime/
  installation-scope constraints are normalized, concrete-site validated, and
  bound through decoded artifact construction, admission evidence, placement,
  and materialization without permitting constraint substitution. Add source
  identity derivation/integration and lower the provider-resolved post-handoff
  writer programs to generated machine code. Writer programs already validate their
  concrete site, resolve each sealed target once, stage all writes, and publish
  atomically. Native whole-pointer actions already lower into section-qualified
  object relocations with materialization provenance.
- **External loans.** The normalized `omega-extents` model is live: a token
  borrow-carries the real Extent loan; device-read requires shared polarity;
  device-write requires exclusive polarity; admitted grants pin borrower,
  space, provenance, open-set rights, and an open set of completion facts; an
  exact provider receipt must establish borrower release plus every required
  fence/cache/provider fact. Connect it to Omega linearity/permission contexts
  and provider execution, then build the DMA slice. Bidirectional sharing
  remains an explicit atomic/coherence protocol, not ordinary lending.
- **EXI1–EXI5 — admitted executable installation.** The normalized
  `omega-executable-installation` ladder is live: immutable artifacts gain a
  reusable sealed admission only from exact evidence; one-shot extent-backed
  placement authority advances through frozen and exact-final-byte validated
  states; installation consumes artifact/placement/scope/audience-specific
  authority plus synchronous visibility evidence; W^X enforcement is reported;
  and every failed linear transition returns its inputs. The normalized container
  validator is live over checked-layout decode output: bounds and range
  arithmetic are checked, semantic sections are exact and non-overlapping,
  unknown required sections reject, and unknown optional sections remain
  informational with zero admission authority. Connect it to actual
  schema/layout byte decoding and the closed relocation validator; implement
  admission/PCC and final-footprint validators, materializer/installer
  providers, Omega linear integration, and provider-backed
  quiescence/replacement execution. Code-placement claims already validate the
  actual Extent base/length against normalized range, alignment, phase, regime,
  and installation-scope constraints before materialization. The normalized
  retirement path already distinguishes visibility from quiescence, requires
  X removal and write-authority restoration, and returns the exact placement
  for reuse only after an exact scoped receipt. PE/COFF remains only a firmware
  envelope; no arbitrary byte-to-code path exists.
- **Wire runtime.** Implement runtime layout for wire values, additional
  encoding families, compatibility reports, and version negotiation.

## Remaining language surfaces

- **Lifetimes.** Implement the decision-15 `'name` lifetime arc and borrow-
  carrying data needed by placed views and task storage.
- **Transition patterns.** Record and case-payload binding, renaming/waiving,
  exhaustive field spelling, and `field: value` equality patterns are live
  with single subject evaluation. Finish multi-subject validation,
  domain-pattern proofs, and diagnostics.
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
