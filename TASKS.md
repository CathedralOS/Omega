# Tasks

Last pruned: 2026-08-05.

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

- **ENTRY-CONTENT-ROOTS — blocked on `OWNER_QUESTIONS.md` Q4.** Connect the
  live `ProgramStorageEntry::enter` requirement to Build selection and generated
  target entry stubs. Bind emitted image and initial-storage geometry to its
  exact qualified parameter positions. Derive statics as subextents and allocate
  later frames/task stacks from the returned storage pool. Do not recognize
  `main`, `Main::run`, or a unique export by convention.
- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Connect a real
  content-bearing source program to the existing terminal-Psi rows. Add sealed
  content-introduction and custody-exit frontier rows; derive residual geometry
  for partial bodyless boundaries and admit only provider custody acceptance.
  Infer only identity-preserving reshuffles; partition-changing primitives must
  author a theorem and wrappers may compose it.
- Add the closed namespace-origin policy and internal algebra account for every
  content-capable root. `ProgramLocal` capacity is owner-authorized declaration
  supply; `ProviderBacked` capacity requires admitted issuance. Report modeled
  identity coverage and do not let quantity-only conservation imply unit
  identity.
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
claim traces to a declared local root or admitted provider root; partition and
residual arithmetic are compiler-derived; overlapping children, gaps without a
custody exit, algebra drift, receipt replay, and cross-root recomposition reject.

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
- **PROOF-RELEVANCE-MIGRATION:** define explicit relevance and its precedence
  over the transitional “recursive means proof-only” classifier.
- **EFFECTFUL-TYPED-COMPUTATION:** specify the value/computation judgments
  connecting effectful machines to the future typed proof calculus. Treat both
  migrations as staged semantic work, not prerequisites for extending the
  existing terminal vocabulary.

Acceptance: a canonical terminal artifact can be verified after source and
producer state are discarded; the verifier independently reconstructs every
obligation and rejects missing/extra/mismatched evidence; interpretation and
native execution consume that same verified artifact; proof replacement does
not change semantic identity.

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

- **WITNESS-EVIDENCE-CLAUSE — blocked on Q5.** Ratify the dedicated declaration
  position for a nominal proposition's fingerprinted evidence interface.
- **SELECTED-WITNESS-EVIDENCE — blocked on Q1 and Q5.** Bind a selected named
  conformance to one carrierless proof term that introduction and elimination
  can reopen. Do not infer it from attached state names.
- Add proof-only selected-conformance projection and by-value carrierless `dyn`
  after Q1 supplies the complete requirement-to-satisfier map.
- Add `Respects` over normalized callable argument records after Q3 settles its
  source/identity surface.
- Migrate `%` from executable-Boolean relations and suffix law discovery to
  proposition evidence plus explicit selected conformances after Q1/Q3/Q5.
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
  selection and descriptor representation. **Dynamic adapter/table emission is
  blocked on Q1**; do not guess a conformance's satisfiers from names.
- Complete hermetic semantic evaluation: invocation-specific admission,
  target-semantic capsule, separate semantic result and usage identities,
  deterministic progress, and constant/runtime equivalence. Abnormal non-return
  reporting is blocked on Q2.
- Add `Hermetic | Receipted | Volatile` observation ceilings and publish realized
  replay/rebuild provenance separately from source semantics.

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

- **Q1:** dynamic conformance satisfier maps, selected witness evidence,
  proof-only conformance projection, and `%` migration.
- **Q2:** complete-contract spelling and propagation for abnormal non-return;
  blocks build-time abnormal-outcome admission.
- **Q3:** normalized argument-record/domain surface for `Respects`; blocks
  quotient lifting and `%` migration.
- **Q4:** Build entry-schema/implementation selection; blocks the generated
  program-storage entry bridge.
- **Q5:** witness-bearing proposition evidence-clause spelling; blocks the
  permanent source form and selected witness evidence.
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
