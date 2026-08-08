# Tasks

Last pruned: 2026-08-07.

This file is the current execution queue, not a changelog. Commits, canaries,
architecture pages, and design briefs retain completed implementation history.
A task belongs here only when it names:

- the remaining work;
- the owning design and code area;
- a real blocker, if one exists; and
- a concrete acceptance condition.

Before taking a task, fetch `main`, inspect the newest commits in that lane, and
avoid overlapping another active change. Commit and push coherent milestones.
Engineering difficulty is not a design blocker. Unresolved owner decisions live
in `OWNER_QUESTIONS.md`; deliberately deferred research stays at the end of this
file.

## Ownership firewall

Psi operates on Omega files and owns parsing plus all target-neutral semantics
through terminal Psi. Omega consumes terminal Psi and owns provider
installation, optimization, ABI/storage realization, native emission, and
general execution machinery. Target backends own unavoidable ISA, ABI,
object-format, and relocation encoding. Cathedral owns OS data structures,
policies, protocols, and lifecycle.

If Cathedral cannot express a subsystem, identify the missing general Omega
primitive or mark the slice blocked. Do not implement page tables, descriptor
tables, schedulers, process tables, timer queues, or drivers as compiler-owned
Rust models.

Compiler validation and code generation may consume general plans. They must
not acquire customer-shaped semantic types, lifecycle states, writers,
scanners, or receipts.

## Execution order

The numbered groups express dependency order, not an exclusive assignment.
Independent compiler lanes may proceed in parallel when their files and
semantic owners do not overlap.

### P1 — Authority, content, and admitted roots

Owners:

- `wiki/design_briefs/authority_values_and_boundary_evidence.md`
- `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`
- `wiki/language_guide/chapter_8_domains.md`

Remaining:

- **ENTRY-CONTENT-ROOTS.** Finish target-declared typed slots around the live
  ordinary `builder.roots.bind(target::ProgramEntry, Exact::machine)` binding
  and exact backend entry selection. Hosted free/receiver-bound source-shape
  checks and receiver ZII validation are live. A target profile owns each slot identity, schema, direction,
  lifecycle, cardinality, and exact-requirement, complete-conformance, or
  entry-machine binding shape; `build.omg` names the exact entry machine and
  performs no discovery. Let a target entry schema expose only the parameters
  its program author must handle. A hosted schema normally exposes none; a
  freestanding schema may expose admitted image and initial-storage roots.
  Generate the physical bridge from the target's arrival requirement and
  selected calling policy, derive and compose the bridge's complete contract, and call the bound
  entry through its declared source shape. A free entry gets no implicit state.
  An entry with one `&mut self` receiver gets exactly one ZII-valid receiver,
  provisioned beneath an admitted entry storage root and lent only for that
  activation. Record its target-selected image or runtime-storage placement,
  derive image sections as subextents, and allocate later frames/task stacks
  from existing roots. Migrate the corpus and remove the compatibility fallback
  that still recognizes `main`/`Main::run` only when no root binding exists. Do
  not recognize a unique export by convention, and do not introduce ambient
  `static` storage.
- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Connect a real
  content-bearing source program to the existing terminal-Psi rows. Add sealed
  content-introduction and custody-exit frontier rows; derive residual geometry
  for partial bodyless boundaries and admit only provider custody acceptance.
  Infer only identity-preserving reshuffles; partition-changing primitives must
  author a theorem and wrappers may compose it.
- **ROOT-INTRODUCTION-AND-BACKING.** Give every content-capable root one internal
  algebra account and classify each fresh establishment occurrence from its
  authority source: compiler-provisioned sealed declared capacity is
  program-local; selected admitted issuance is provider-backed. A checked
  runtime establishment may expose or transform an existing account but never
  originate one. Keep nominal data and algebra denominators free of origin
  policy. Record exact route, capacity, lineage,
  qualification, backing identity, and provenance per root. An operation that
  realizes content against an external substrate must identify an exact
  qualified root and carry correspondence to the same selected provider;
  matching denominator arithmetic alone grants no authority. Report modeled
  identity coverage and reject cross-root recomposition.
- **BOUNDARY-ISSUANCE — depends on the conservation work above.** Derive
  per-invocation geometry from ordinary parameters, entry places, and returned
  values. Retain external ownership, fresh issuance, custody delegation,
  aliasing class, and partitioned succession separately. Provider assertions
  may attest custody; they may not supply computable interval arithmetic.
- Finish routed task-claim establishment, stack-resource authority,
  cancellation conformance, and transactional custody under TR3-TR8. Deferred
  acknowledgements lease the installed interrupt root and controller
  configuration; reconfiguration drains them rather than revoking them.

Acceptance: reconstructed carriers mint no authority; every introduced content
claim traces to compiler-provisioned sealed local capacity or admitted provider
issuance; external effects have an exact root-to-provider backing chain;
partition and residual arithmetic are compiler-derived; overlapping children,
gaps without a custody exit, algebra drift, receipt replay, and cross-root
recomposition reject.

### P2 — Source-visible materialization and placed access

Owners:

- `wiki/design_briefs/programmable_layouts.md`
- `wiki/design_briefs/os_memory_and_hardware_foundation.md`
- `wiki/language_guide/chapter_20_memory_layout_abi.md`

#### L4/L5 — plan-laid views

- Finish source-visible validation/materialization over owned storage.
- Complete non-scalar tiling and mutable-view establishment beyond the live
  record/fixed-array/interior-slice representation checks.
- Keep raw bytes from establishing typed facts without the selected validated
  plan and exact field identities.

#### L6b — `AccessPlan` and `Placed<P, T>`

- Derive borrowed and owned `Placed<P, T>` establishment and retirement from
  `Extent in Granted`; source spelling uses ordinary `&`/`&mut` subrange
  projections, not `ExtentLoan`.
- Implement `Stable` adopt/initialize/validate and `External` adopt. Borrowed
  cleanup ends inside the loan; owned destruction returns
  `Extent in Granted & Vacant` before general allocator/free integration.
- Derive per-field readable, destructive-read, writable, and atomic accessors.
  Preserve logical field extents separately from whole-transfer effect
  footprints. Destructive reads and RMW reserve the complete affected transfer
  unit.
- Enforce per-operation representation and transfer laws: total decode for
  externally readable fields, total/value-proved encode for writes, exact
  provider-supported width/alignment, and operation-specific atomic laws.
  External multi-transfer reads, synthesized RMW, and External initialization
  remain rejected.
- Keep admission polarity (Omega-view alias exclusion) separate from access
  permission. Access-plan rights authorize External reads/writes; `&mut` must
  not falsely claim exclusivity against the device.
- Connect x86-64/AArch64 emission for admitted whole-container External and
  atomic transfers. Publish one sealed core requirement per atomic operation;
  missing conformance means the operation is unavailable.
- Retain schema/device correspondence, optional runtime revision evidence, and
  provider-instance identity separately from storage compatibility.

#### L6c — symbolic materialization

- Carry symbolic sources, placement constraints, immutable post-handoff bytes,
  exact footprint, and invocation plan through final artifacts.
- Connect final placed fragments to source-level provider invocation after
  materialization establishment. Provider preparation must not generate host
  code.
- Bind validation to exact final bytes and placement; fingerprints remain
  report/cache identity, never authority.

Acceptance: UART/MMIO, shared-page IPC, and ordinary RAM use one extent/layout
foundation with different profiles. Misalignment, insufficient rights,
unplanned offsets, narrow External writes, destructive reads through a
repeatable accessor, overlapping transfer footprints, forged profile evidence,
and unsupported atomic operations reject before code generation.

### P3 — Terminal Psi, proof-carrying artifacts, and fuel

Owners:

- `wiki/architecture/pipeline/terminal_psi.md`
- `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`
- `wiki/architecture/bootstrap_lattice/proof_kernel.md`

Remaining:

- **PSIIR.** Grow terminal production in obligation-complete vertical slices:
  operation semantics, generated obligations, sound proof rules, interpreter
  behavior, Omega lowering, canonical encoding, and fuel identity land
  together. Add general blocks, calls, aggregate values, structural places,
  cleanup/transfer actions, and boundary operations without restoring an
  Omega-to-Psi bridge.
- Replace all remaining terminal-path `ExpressionHandle` and source-tree
  dependencies with lowered values and predicates. Absorb useful StateGraph /
  ControlFlow topology, then retire the legacy backend lane as consumers move.
- **CRASH-CONTRACT.** Parse `crashes Cause Scope` route clauses and explicit
  `crash Cause;` terminals. Normalize fingerprinted per-cause/per-scope buckets,
  derive path-conditioned crash sites and damage minima, refine routes at calls,
  and enforce sparse per-cause context maxima. Terminal Psi v22 now carries an
  explicit no-successor crash terminator with closed `Trap`/`Abort` cause,
  nominal damage-scope demand, and a canonical machine-local frontier lower
  bound; validation, canonical encoding, fuel, and direct interpretation are
  live, and Omega native lowering rejects it explicitly pending target crash
  plans. Finish source production and route/context normalization; remove the
  parser's transitional lowering of contextual `trap` to an ordinary terminal
  edge.
  Omega installation must bind nominal scopes to a selected fault plan and
  prove the realized scope lies between each surviving route demand and its
  context maximum.
- Re-root the reference interpreter and abstract-operation construction fully
  on decoded, verified terminal Psi. Preserve the shared interpreter/native
  oracle over the same IR.
- **PCC verifier closure.** The artifact determines its complete obligation
  set; proof bundles only discharge it. Connect `psi-terminal-verifier` to the
  low-rung proof-kernel calculus and choose one auditable closure recorded in
  the architecture: a low reference artifact verifier, a checked
  obligation-reconstruction derivation, or an explicitly trusted Psi verifier.
  A Psi-hosted proof-kernel port is not by itself this closure.
- **IRFUEL.** Extend the live acyclic entry/segment certificates to loops and
  build-time migration. Add attributed response outcomes only after terminal
  Psi has wait/foreign edges from which the verifier can derive them. Migrate
  Cathedral hard roots and later add native metering that preserves accounting
  provenance. Keep target WCET and wall-clock conversion separate.
- **PROOF-RELEVANCE-MIGRATION:** implement binding-level `[erased]` relevance,
  checked noninterference, erased-stripped layout, and obligation preservation.
  Explicit relevance takes precedence over the transitional “recursive means
  proof-only” classifier; non-layoutable `Type` values remain legal only in
  erased positions. Do not infer carrier relation roles from relevance.
- **EFFECTFUL-TYPED-COMPUTATION:** specify the value/computation judgments
  connecting effectful machines to the future typed proof calculus. Treat both
  migrations as staged semantic work, not prerequisites for extending the
  existing terminal vocabulary.

Acceptance: a canonical terminal artifact can be verified after source and
producer state are discarded; the verifier independently reconstructs every
obligation and rejects missing/extra/mismatched evidence; interpretation and
native execution consume that same verified artifact; proof replacement does
not change semantic identity. Crash sites are never represented as ordinary
terminal transitions or absent cleanup, concrete safe invocations can disprove
all crash routes, and installation rejects fault plans that kill either too
little damaged state or too much context-owned state.

### P4 — Calling plans, final footprints, and callbacks

Owners:

- `wiki/design_briefs/calling_plans.md`
- `wiki/design_briefs/os_memory_and_hardware_foundation.md`
- `wiki/language_guide/chapter_23_inline_assembly.md`

#### ENT2c — normalized ABI lowering

- Remove remaining production paths that reconstruct ABI placement from target
  catalogs instead of consuming the selected `CallPlan + StatePlan`.
- Finish retained foreign-storage custody and provider-owned view invalidation.
  Borrow-derived custody cannot survive return; durable retention consumes an
  owned claim and ends through an explicit protocol receipt.
- Add a focused write-only view model rather than disguising write-only foreign
  access as readable memory.
- Keep named no-plan encoders only as differential oracles. Production layout,
  emission, and relocation must require the retained authoritative plan and
  fail closed when it is absent or incompatible.

#### ENT3 — final state-footprint validation

Remaining:

- finish complete entry/body-region enumeration, including format-owned thunks,
  veneers, and generated stubs;
- derive the complete final register/machine-state union and require exact
  equality with earlier `StatePlan` evidence; and
- set certificate completeness only after every executable byte belongs to one
  validated compiler or admitted-format class. Do not add an
  interrupt-specific or second whole-image decoder.

#### ENT4 — registered callbacks

- Let one named static boundary machine satisfy a foreign callback requirement;
  retain its exact calling/state plans and emit the thunk only from selected
  binding lowering.
- Model durable registration as a linear package value with explicit unregister
  and any required code/component lease.
- Implement the narrow Windows `user32` canary (`RegisterClassEx`,
  `CreateWindowEx`/`WM_NCCREATE`, `GetMessage`, `DispatchMessage`,
  `DefWindowProc`, `DestroyWindow`, `UnregisterClass`) without exposing a raw
  code address.
- Derive `Atomic::interruption_fence` same-context evidence from the installed
  external-root route; reject it elsewhere.

Acceptance: changing a normalized plan changes lowering or rejects; forbidden
state introduced anywhere in final executable text rejects; a registered
callback cannot outlive its registration/code lease or smuggle application
state through a raw address.

### P5 — Cathedral bring-up over general Omega primitives

#### BUMP-ALLOCATOR-CANARY — package allocator

- After P1 conservation is source-usable, implement a package-level bump
  allocator over one qualified `Extent`. Two allocations must coexist; release
  cleans and returns the exact subextent without restoring bump-tail capacity;
  reset succeeds only after full recomposition; finish returns the original
  backing.
- Implement owned `Vec<T>` and then `Vec<u8>::Utf8` only after choosing the
  allocator contract needed for cleanup, authority return, and capacity reuse.

#### Address translation

- Build Cathedral's page-table hierarchy, validation states, installation, and
  teardown in Omega source using `source/drivers/facts/x86_page_table_entry.omg`.
- Use pre-reserved storage for the fixed bootstrap table; dynamic hierarchy
  allocation waits for the package allocator. Do not restore a compiler-owned
  page-table model.

#### Exception roots and first timer

- Materialize fatal/diagnostic entries for every architectural exception before
  enabling interrupts.
- Provision dedicated per-CPU double-fault/NMI/machine-check stacks and one
  non-nesting maskable-IRQ stack class; preserve the selected `StatePlan`.
- Bring up PIT+PIC first and LAPIC as the production provider. The hard root only
  acknowledges, records time, publishes a coalesced wake, and returns; fan-out
  runs in an ordinary task.

Acceptance: QEMU installs Cathedral-owned memory/interrupt structures, reports
timer ticks over owned serial output, and halts between ticks. No
customer-shaped compiler concept is introduced.

## Parallel compiler and language lanes

### Frames, reach, and trust

- **R5:** extend exact inferred may-write summaries and relational candidates
  beyond the live acyclic/cycle-safe statement/value-call coverage. Preserve
  facts outside complete frames and treat opaque or unresolved frames as
  conservative fences. Do not restore an authored `stores` clause.
- **STR/EFX:** finish independent normalization/publication of service reach,
  suspension, blocking, termination, mutation, and trust; retire remaining
  legacy umbrella names after consumers migrate.
- **TPR4/TPR6:** connect progress-profile grants and receipts without putting
  private ranking witnesses into public identity.
- **GR6:** finish qualification/trust consumers and their artifact rows.

Acceptance: contract axes normalize independently, wrappers cannot launder
reach or trust, and private proof improvements do not change public identity.

### Multiplicity, tasks, and execution

- **CML4:** construct `EdgeCleanupPlan` after outgoing-value materialization and
  transfer-map commitment. Add reverse-declaration cleanup, contextual cleanup
  contract checking, structural partial-value cleanup, nominal-drop
  partial-move rejection, repeated-cycle resource composition, and
  conservation/backend-ledger reporting.
- **TR3-TR8:** finish whole-call-graph WCSU derivation, bind exact `StackPlan`
  evidence, reserve fixed nonmoving `StackLease`s, validate preservation and
  cancellation conformances, transfer arguments transactionally, lower
  park/resume, and implement the suspension-safe-loan subset.
- **BLOCKEXEC:** implement an ordinary package-level blocking executor with
  bounded queues, moved custody, linear completion claims, suspension, and
  provider selection. A hung in-process worker cannot be killed safely;
  bounded recovery requires process isolation.

Acceptance: linear debt cannot disappear through cleanup or aggregation;
CPU/thread-restricted activations require selected preservation evidence; task
and allocation handles expose no compiler-owned stack/control storage.

### Propositions, quotients, and mathematics

Owner: `wiki/design_briefs/law_bearing_relations_and_quotients.md`.

Remaining N6/N8 work:

- Implement the witness-bearing proposition brace form: exactly one
  carrierless evidence-interface entry, no executable body, and a normalized
  fingerprint distinct from transparent `=` expansion.
- **SELECTED-WITNESS-EVIDENCE:** bind a selected named
  conformance block to one carrierless proof term that introduction and
  elimination can reopen. Consume its complete normalized requirement map;
  do not infer evidence from attached state names.
- Add the subjectless conformance-block form used by carrierless evidence
  interfaces. It has a package-scoped name and the same closed normalized row
  map as a carrier-owned conformance; no arbitrary parameter is inferred as
  its subject.
- Add proof-only selected-conformance projection and by-value carrierless `dyn`
  from the complete conformance-block map.
- Add `Respects` over compiler-derived parallel callable argument telescopes.
  Positions are semantic and source names are debug aliases. Derive the
  representative-dependent domain by semantic dependency, the pointwise input
  relation from the selected quotient relations, and the result relation from
  the requested lifted codomain.
- Add proposition-valued heterogeneous constructor lifts selected for exact
  `(quotient relation, container family)` pairs. Transparent dependent records
  lift in dependency order; coarser earlier-field relations generate checked
  proposition-transport obligations owned by the quotient. Do not add global
  carrier roles or an ambient/default relator.
- Extend R6's typed carrier-family binder so reusable relator traits quantify
  over a constructor and expose proposition-valued members. This is the
  higher-kinded/index-telescope prerequisite already owned by the dependent
  ladder, not a quotient-local parallel abstraction.
- Gate runtime decider derivation when a lifted relation depends on erased
  `Type` content: require checked determination by the runtime projection or
  report the undetermined component.
- Migrate `%` from executable-Boolean relations and suffix law discovery to
  proposition evidence plus explicit selected conformances after the work
  above.
- Expand the checked `Nat`, `Int`, `Rat`, sequence/Cauchy, and approximation
  corpus needed for `Real`; keep `Real` proof-only and core-level.

Acceptance: an admitted axiom cannot license quotient formation; selected
Reflexive/Symmetric/Transitive evidence and every `Respects` proof are explicit;
different witnesses establish one stable proposition identity and eliminate
through its declared interface.

### Float providers

Owner: `wiki/design_briefs/float_semantics.md`.

Remaining F7 work:

- provide feature-qualified x86-64 FMA or a checked binary32/binary64 software
  implementation, then select the generic x86 FMA slots;
- retain equally target-specific semantic-edge evidence for every other
  admitted hardware realization; and
- complete the wider proof/`Real` connection under N6/N8.

Checked-result float/integer conversion remains blocked on the separate
checked-result arithmetic decision listed below.

### Lifetimes, dynamic traits, and build-time evaluation

- Finish general outlives constraints, persistent owners, aggregate borrow
  propagation, parameter-backed storage, broader runtime-indexed expressions,
  state-parameter loan-root rebasing, and exact R5 preservation.
- Continue local borrowed `dyn` lowering from the live nominal-conformance
  selection and descriptor representation. Parse and check complete
  conformance implementation blocks, normalize inherited trait-qualified rows,
  instantiate defaults per conformance, and emit adapters/tables only from that
  retained map. Bare exact-requirement satisfiers never license `dyn`.
- Complete hermetic semantic evaluation: invocation-specific crash-route
  refinement, target-semantic capsule, separate semantic result and usage
  identities, deterministic progress, and constant/runtime equivalence.
- Add `Hermetic | Receipted | Volatile` observation ceilings and publish realized
  replay/rebuild provenance separately from source semantics.
- Complete the ordinary `Build` API and package executor: bind dependency aliases
  to exact sources, compile each dependency build against package-scoped
  providers, reject ambient/general filesystem escape, and recheck generated
  Omega under the consuming artifact's runtime ceilings.
- Harden the resolver as a separate authority boundary with revision/content
  verification, archive path containment, expansion limits, scoped destination
  writes, and receipts. Generate the unified dependency/build/trust lock,
  fingerprint imported boundary claims as one package claim set, and invalidate
  root acceptance on any member diff. Release-capable standard providers must be
  hermetic or receipted; volatile observations remain explicit development
  policy and fail source-rebuildable release.

### Components and executable trust

- **FFIVAL:** run the narrow Windows `user32` boundary-coherence slice after
  ENT4, using existing activation, custody, registration, stack, and reach
  machinery.
- **TCBMANIFEST:** finish build-profile selection and derive executable TCB
  metadata from selected-provider closure. Keep known entries separate from
  proved completeness; retain provider/executable/plan identity, execution
  scope, origin, implementation evidence, and independent containment axes.
- Extend separate-compilation artifacts with target/runtime stack needs,
  mapping cohorts, two-sided import/export validation, boundary multiplicity,
  custody receipts, and enumerable state roots. Runtime drain/coexistence,
  migration scheduling, and resource provisioning remain consumer/runtime
  work.
- **REPLACE-OPAQUE:** extend replaceable-component tests beyond the live mapping quarantine,
  manifest union, service handover, callback gateway/unregister, and era-ledger
  slice. Proven quiescence is the only route back to reusable mapping capacity.
- Implement serialized capability attenuation/revocation only after the
  component carrier and custody rules are complete.

### Atomic ordering and device protocols

- **ATOMIC-EVENT-MODEL — design blocked.** Define the formal portable event
  model and x86-64/AArch64 refinement before enabling general protocol
  verification or global-order fences. Placed atomic accessors, checked ISA
  barriers, and installed-root same-context evidence do not wait for it.
- Add sealed provider requirements for DMA publication/acquisition, cache
  maintenance, MMIO notification, and posted-write completion. Every emitted
  requirement must be discharged or reject.
- Bind publication evidence to exact range/write state so intersecting writes
  invalidate it. Acquisition consumes request- and instance-bound completion
  evidence. Terminal Psi retains the actual ordering event; erased proof values
  and generic call effects are not lowering barriers.

### Wire runtime and executable installation

- Extend repeated encode/decode to `Vec<T>` after allocator obligations land.
  Packed scalar decode into `&[T]` remains unsupported because variable-width
  encodings cannot form a zero-copy scalar view.
- Connect retained semantic artifacts to loader/provider execution; implement
  trusted/PCC and final-footprint validators; complete target W^X/coherence
  reporting and uninstall/replacement joins.
- Keep arbitrary runtime bytes-to-code, JIT, and raw executable addresses
  unsupported.

Acceptance: only an admitted reusable artifact plus consumed placement authority
can produce installed code; validation binds exact final bytes and placement.

## Blocked index

These entries are pointers, not duplicate specifications.

- **Q1:** content-namespace origin-policy spelling; blocks local/provider-backed
  root origination and provenance.
- **ATOMIC-EVENT-MODEL:** blocked on the portable atomic axioms and target
  refinement choices in `wiki/language_guide/appendix_open_questions.md`.
- **CHECKED-RESULT-ARITHMETIC:** blocked on whether failure-returning checked
  arithmetic earns a distinct public carrier beyond exact-by-default
  obligations and existing policy families.

## Platform-gated verification

- Run the Linux host/time/filesystem and `IntegerAt` metadata paths natively on
  AArch64. x86-64 WSL and cross-target structural coverage already exist; do
  not claim runtime verification without the host.
- Build and run the Windows GUI callback canary through ENT4; do not pass a raw
  code address or add a Win32-only escape.
- Keep unavailable hosts structurally tested and report the missing runtime
  leg explicitly.

## Vertical acceptance slices

- **Allocator:** qualified root -> two live subextents -> cleanup/retirement ->
  exact recomposition -> original root returned.
- **PCC:** canonical Psi -> independently reconstructed obligations -> checked
  proof bundle -> interpretation/native agreement after producer state is gone.
- **OS:** UART/MMIO -> Cathedral page tables -> DMA -> hostile/trusted shared
  pages -> exception/timer entry -> SMP AP bring-up, with no customer-shaped
  compiler primitive.
- **Control state:** checked assembly cannot hide stack/control mutation;
  provider exits match their plans; external loans remain inside their extent;
  parked continuations remain non-addressable.

## Deferred until a real customer

- concurrent whole-system composition proofs for deadlock, starvation, memory,
  and response bounds;
- richer measured-recursion guards and multi-subject lexicographic cycles;
- reduced-rational divisibility theory beyond current quotient work;
- asynchronous extent revocation beyond provider quiescence;
- non-blocking executable-visibility tokens;
- runtime-generated host code, JIT, and arbitrary self-modifying code;
- independent final-byte CFI certificates and optional
  CET/PAC/shadow-stack hardening;
- universe levels before a full math-library replay goal;
- reusable fragmented allocation until a growable-container/backend customer
  states its retirement, authority-return, and immediate-reuse demands; and
- an optimizing SSA/register-allocation/SIMD backend beyond current correctness
  requirements.
