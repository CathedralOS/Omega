# Tasks

Last pruned: 2026-08-14.

This file is the current execution queue, not a changelog. Git retains completed
implementation history; architecture pages and design briefs describe the
current model and only the implementation state needed to explain remaining
work. A task belongs here only when it names:

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

- **ENTRY-CONTENT-ROOTS.** Complete the physical entry bridge and explicit-entry
  corpus migration. Bind the exact mapped, zeroed receiver reservation and its
  exclusive activation loan through installation to the selected physical
  provider and generated native bridge. Pure language/checker fixtures stop at
  checked artifacts; deployable/provider/artifact/ABI/layout/native fixtures
  select an exact target-owned `ProgramEntry`; temporary ABI probes name their
  explicit fixture seam. Sample refresh and native execution must use authored
  roots and never invent one, while targetless checking selects none.

  The CLI corpus is rooted on all hosted targets except the four GUI samples,
  which currently select Windows x64 and macOS arm64. Linux needs an ordinary
  source-level `Gui`/`Input` provider plus its general call/result realization;
  that is engineering work, not a language-design blocker. Proof-only and
  deliberately trapping fixtures remain targetless. Final firmware composition
  of `ImageHandle`/`SystemTable` inputs with semantic roots is design-blocked on
  owner Q2; the remaining physical bridge and corpus work is not.
- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Take one real
  content-bearing source program through terminal Psi. Add sealed introduction
  and custody-exit frontiers, derive residual geometry at partial bodyless
  boundaries, and admit only provider custody. Infer identity-preserving
  reshuffles; partition changes require an authored theorem. Before emitting an
  introduction or exit, checked facts must bind the exact content subject and
  geometry to the selected provider plan, invocation receipt, backing/root
  lineage, installed occurrence, and route; a generic established-claim identity
  is insufficient. Checked source already derives exact identity-reshuffle and
  authored-partition composition rows, and terminal Psi independently validates
  their canonical replay. The exact root-only source passthrough now produces a
  structural result/return carrier with claim transfer, exit-time content
  replay, interpretation, and fuel. Omega preserves that carrier through the
  exact one-fragment native ABI path and all artifact/install layers, with claim
  identity retained as zero-runtime metadata. The remaining work is real sealed
  introduction, custody exit, residual geometry, and provider binding—not
  another passthrough representation.
- **ROOT-INTRODUCTION-AND-BACKING — design blocked on owner Q3.** Provider-issued
  and compiler-provisioned origins must preserve complete evidence and reject
  cross-origin composition and replay. Once the sealed local-capacity source
  form is settled, lower it into compiler-owned origin, terminal-evidence, and
  artifact rows and add a source-level conservation canary.
- **BOUNDARY-ISSUANCE** (after conservation): derive invocation geometry from
  parameters, entry places, and results. Keep ownership, issuance, custody,
  aliasing, and partition succession distinct. Providers may attest custody,
  never computable interval arithmetic.
- Under **TR3-TR8**, finish routed task claims, stack authority, cancellation,
  and transactional custody. Deferred acknowledgements lease the interrupt root
  and controller configuration; reconfiguration drains them.

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

- Finish source-visible materialization over owned storage, including
  non-scalar tiling and mutable views beyond current record/array/slice checks.
  Raw bytes establish no typed fact without a selected validated plan and exact
  field identities. The accepted fixed subset reflects primitive arrays and
  recursively fixed arrays/records as one `Repeated` or `Nested` field. View
  paths retain one whole `At` extent; owned materialization also admits an outer
  fixed array tiled by exactly one compiler-sized element `At` at one validated
  constant destination stride. Compiler-derived strides and offsets drive the
  interpreter and all three native target paths. Mutable fact-free byte views
  write and reread through those same extents. Typed owned materialization
  derives complete bytes from the exact schema (or a checked zero-argument Psi
  evaluator) while Omega supplies byte order, zeroes padding, and validates
  completely before mutation.
  Erased terms remain semantically mandatory but add no bytes, including nested
  records and fixed arrays whose entire runtime shape is erased. Scalar
  placement/access semantics remain fenced for aggregates. Continue beyond
  this fixed subset. Sum materialization is design-blocked on the unsettled
  tagged-case placement vocabulary.

#### L6b — `AccessPlan` and `Placed<P, T>`

- Derive borrowed/owned `Placed<P, T>` establishment and retirement from
  `Extent in Granted`, using ordinary subrange borrows. Implement `Stable`
  adopt/initialize/validate and `External` adopt; owned destruction returns
  `Granted & Vacant` before allocator integration.
- Derive readable, destructive-read, writable, and atomic field accessors while
  keeping logical extents distinct from whole-transfer footprints. Enforce
  total decode/encode, exact provider width/alignment, and operation-specific
  atomic laws. Continue rejecting External initialization, multi-transfer
  reads, and synthesized RMW.
- Keep alias-exclusion admission separate from access rights; `&mut` does not
  claim exclusivity against a device. Connect admitted whole-container External
  and atomic transfers to both native backends through one sealed core
  requirement per atomic operation.
- Retain schema/device correspondence, runtime revision evidence, and provider
  identity separately from storage compatibility.

#### L6c — symbolic materialization

- Carry symbolic sources, placement constraints, immutable post-handoff bytes,
  exact footprint, and invocation plan through final artifacts. Connect placed
  fragments to source-level provider invocation after establishment; provider
  preparation generates no host code. Validate exact bytes and placement;
  fingerprints remain report/cache identity, never authority.

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

- **PSIIR.** Extend the accepted terminal vocabulary as complete vertical
  slices. The accepted carrier includes scalar direct calls and guarded crash
  continuations, canonical Unit structural/content calls, literal fixed-array
  custody with one typed fixed-index projection, structural results, exact
  affine cleanup on Unit/scalar returns and structural edges, bounded scalar and
  local Boolean computation, and acyclic control with two decisions plus one
  equal-frontier diamond. Whole-root nominal cleanup supports finite mixed
  claim-free affine/scalar parameter lists, bounded helper calls, contextual
  direct-Boolean obligations, shared cleanup targets, and edge-specific ordered
  action streams. One claim-free affine record may transfer pairwise
  prefix-disjoint all-field paths and clean every maximal residual subtree.
  Codec, independent verification, interpretation, fixed fuel, and all Omega
  artifact/install paths agree on these carriers.

  The nominal-cleanup Boolean slice source-distributes one short-circuit local
  through a finite chain of single-use branch-free continuation locals. Add
  complete slices for value reuse, repeated short-circuit stages, explicit
  shared convergence, nested decisions, calls and effects,
  wider partial-value cleanup, nested nominal ownership, returned transfer,
  loops, suspension, scoped ordering, and ranked tail recursion. Dynamic or
  nested indexing, wider projections/signatures, content-bearing splits, and
  unsupported contracts remain fenced until independently verifier-owned.
  Retire checked/source-tree consumers with each slice; nothing below terminal
  Psi may depend on typed/source trees, `ExpressionHandle`, source rendering, or
  an Omega-to-Psi bridge. Bind partition-composition replay to the exact
  operation and verifier-selected callee guarantee; fingerprints are identity,
  never authority.
- **CRASH-CONTRACT.** Extend guarded implication beyond the accepted acyclic
  scalar slice. Direct and staged calls retain invocation-specific substitutions
  and verifier-reconstructed continuations. Canonical Boolean and fixed-integer
  member paths rebase across whole-root, fixed-index, and all-field-projected
  structural calls. The proposition carrier covers Boolean composition,
  relevant-record equality, fixed-width bitwise terms, policy-distinct integer
  arithmetic, evidence-bounded division/remainder, and exact or wrapping shifts;
  codecs, verification, fuel, and interpretation reject missing or redirected
  premises. Continue with case-payload paths and aggregate equality over text,
  floats, sums, and erased fields. Trapping predicate arithmetic is
  design-blocked on owner Q10; imported crash capsules remain blocked on
  artifact identity and certificate binding.
- **PROOF-CERTIFICATION-BRIDGE.** Emit kernel-checkable certificates from source
  automation. One recursive certificate owns one SCC, cites its ranking and
  well-foundedness evidence once, and proves every internal edge decreases;
  ordinary calls remain contract applications. Normalization cites exact
  conformance/law evidence and preserves transitive trust. Thread recursive
  components and cited laws through the accepted trust record and synopsis.

  Acceptance: perturbing any recursive edge decrease, component
  well-foundedness reference, normalized-law identity, or cited premise
  rejects or changes the recorded trust closure; measured mutual proof
  recursion checks while an unmeasured cycle rejects; an admitted law makes
  every dependent normalization admission-dependent.
- **PCC verifier closure — design blocked on owner Q7.** The artifact determines
  obligations; bundles only discharge them. The Rust verifier already
  reconstructs the exact obligation set and invokes `psi-proof-kernel`; connect
  that certificate calculus to the
  independent low-rung kernel route and record one auditable reconstruction
  closure: low reference verifier, checked derivation, or explicitly trusted
  Psi verifier. A Psi kernel port alone is insufficient.
- **IRFUEL.** Extend entry/segment certificates to loops and build-time use;
  the generic terminal inspection path now independently verifies a selected
  source closure and publishes its recomputed acyclic entry certificate, with
  Cathedral's first timer root pinning that evidence. Add attributed response
  outcomes only when terminal wait/foreign edges can derive them. Inserted native
  metering must consume the installed exact-site
  attribution rows, but is design-blocked on the sponsor counter, exhaustion
  transfer, and resumable continuation ABI in owner Q6. Keep WCET and wall-clock
  conversion separate.
- **PROOF-RELEVANCE-MIGRATION.** Finish binding-level `[erased]`, checked
  noninterference, erased-stripped layout, and obligation preservation across
  the remaining consumers. Explicit relevance remains in semantic/proof
  identity while supported runtime carriers recursively omit erased storage,
  initialization, topology, bytes, tags, and ABI transfer; runtime use rejects
  and omitted evidence remains a required semantic term.

  Continue moving any remaining target-neutral generic/build-time probe
  sequencing out of `omega-compiler`; Psi owns those services and normalized
  plan carriers, while Omega owns target filtering and ABI/provider realization.
  This is engineering, not a language-design blocker. Unsupported computed,
  chained, dynamic-receiver, unresolved-generic, non-checked-supply, and
  unresolved-machine-parameter shapes keep failing closed. `Placed<P, T>`
  erased-evidence establishment is design-blocked on owner Q8. Relevance does
  not invent a runtime carrier or public ABI for otherwise non-layoutable types.
- **EFFECTFUL-TYPED-COMPUTATION:** specify the value/computation judgments
  connecting effectful machines to the future typed proof calculus. Treat both
  migrations as staged semantic work, not prerequisites for extending the
  existing terminal vocabulary.

Acceptance: a canonical terminal artifact can be verified after source and
producer state are discarded; the verifier independently reconstructs every
obligation and rejects missing/extra/mismatched evidence; interpretation and
native execution consume that same verified artifact; proof replacement does
not change semantic identity. Crash sites are never represented as ordinary
terminal transitions or absent cleanup, and concrete safe invocations can
disprove all crash routes.

### P4 — Calling plans, final footprints, and callbacks

Owners:

- `wiki/design_briefs/calling_plans.md`
- `wiki/design_briefs/os_memory_and_hardware_foundation.md`
- `wiki/language_guide/chapter_23_inline_assembly.md`

#### ENT2c — normalized ABI lowering

- Finish foreign-storage custody and provider-view invalidation. Borrowed
  custody ends at return; durable retention consumes an owned claim and ends
  through a receipt. Successful bodyless terminal boundary calls now retain the
  exact verifier-derived completion-receipt set through canonical encoding,
  interpretation, native lowering, machine-code evidence, and installation;
  provider rejection records no receipt and leaves custody live. The checker
  accepts only one compatible consumed input
  for inferred post-return custody and rejects borrow-only sources. Ambiguous
  multiple-owned sources are accepted only when an exact authored equality
  relates one whole input entry projection directly to the whole current result
  projection in the same content algebra; partition/subplace equations and
  borrowed selections remain fail-closed. Extend this closure to result-bearing
  boundary calls and provider-view invalidation.
- **WRITE-ONLY-MEMORY-VIEW — design blocked on owner Q4.** Once its core
  representation and initialization transition are settled, carry the exact
  view through foreign signatures, calling plans, borrow checking, and both
  execution paths without widening it to read/write authority.

#### ENT4 — registered callbacks

- **CALLBACK-PARAMETER-REQUIREMENT — design blocked on owner Q5.** The source
  operation must nominally bind one static machine-parameter position to one
  exact callback requirement; callable-shape coincidence and unique conformance
  are insufficient. Once settled, retain a checked per-use row and exact
  call/state plan, then emit its thunk only from selected binding lowering.
  Registration is linear, explicitly unregisters, and retains required code/
  component leases.
- Implement the narrow Windows `user32` canary without exposing a raw code
  address. Derive `Atomic::interruption_fence` same-context evidence from the
  installed external-root route and reject it elsewhere.

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
- Completing the x2APIC acknowledgement transition is design-blocked on owner
  Q9: the provider-neutral `InterruptAcknowledgement::complete` requirement
  currently hardcodes `PortIo`, while x2APIC correctly uses `MachineControl`.
  Do not grant false port-I/O reach to the x2APIC provider as a workaround.

Acceptance: QEMU installs Cathedral-owned memory/interrupt structures, reports
timer ticks over owned serial output, and halts between ticks. No customer-shaped
compiler concept is introduced.

## Parallel compiler and language lanes

### Frames, reach, and trust

- **R5:** continue exact inferred may-write summaries and relational candidates.
  Exact frames compose through transparent returns/helpers, caller-isolated
  scratch locals, statement/value positions, stable mutable aliases, and direct
  alias replacement; rebinding leaves earlier reborrows intact. The bounded
  non-reference direct-call expression class is complete through depth two,
  including member projection and one or more independently bounded indexes;
  typed non-reference assignment-value call trees extend through depth four.
  One top-level concrete primitive-only record or selected-case literal may
  likewise contain an independently bounded non-reference call tree in each
  direct common or payload field while publishing every write. One direct
  field may instead contain a second concrete primitive-only record or
  selected-case literal whose direct fields obey the same rule; this aggregate
  depth-two rail does not widen the depth-four call budget. A declared
  primitive field at either level may wrap independently bounded call operands
  in up to two nested scalar-computation shells made from unary/binary
  operators, primitive value casts, member projections, or indexing without
  widening that budget.
  Indexing irreversibly coarsens to the nearest backing collection while
  preserving independent index-call writes. Finite named-state SCCs accept only
  bijective write-capable parameter permutations. Primitive-only concrete
  record/sum locals remain isolated through nested fixed arrays.

  Continue with representable relational candidates. Boundary,
  beyond-per-position-budget, binding-reborrow, reference-valued/opaque,
  escaped, non-bijective, generic, recursive or reference-bearing aggregate
  literals, third aggregate or computed shells, other computed field shapes,
  and out-of-isolated-root shapes remain conservative fences. Do not restore
  authored `stores` clauses or treat lifetime elision as evidence; Git carries
  individual evidence cohorts.
- **STR/EFX:** finish independent normalization/publication of machine supply,
  service reach, suspension, blocking, termination, mutation, and trust; remove
  remaining umbrella carriers after their consumers migrate.
- **TPR4/TPR6:** connect progress-profile grants and receipts without putting
  private ranking witnesses into public identity.
- **GR6:** finish qualification/trust consumers and their artifact rows. The
  retained selected-provider rows already bind exact plan, overload, grant,
  subject, authority-flow, semantic-domain, carry, predicate, and root-selector
  identity across lock/report/runtime admission. Continue with consumers that
  still lack exact blast-radius rows. Selected schemas, adapter dispatch, and
  calling-plan lookup require nonempty overload identities; name-only singleton
  matching remains forbidden.

Acceptance: contract axes normalize independently, wrappers cannot launder
reach or trust, and private proof improvements do not change public identity.

### Multiplicity, tasks, and execution

- **CML4:** construct the complete `EdgeCleanupPlan` after outgoing-value
  materialization and transfer-map commitment. Current Unit/scalar and bounded
  acyclic slices retain reverse-declaration cleanup, partial-record transfer of
  prefix-disjoint all-field paths, maximal-residual disposal, nominal helper
  calls, shared targets, edge/action ownership, and direct-Boolean contextual
  obligations through terminal verification, interpretation, fuel, and all
  native artifact paths. Extend contextual cleanup beyond the current
  receiver-independent Boolean subset and finite continuation chains; add typed
  shared convergence, wider structural partial values, repeated-cycle resource
  composition, and conservation/backend-ledger reporting. This is not yet a
  general conditional CFG, complete cleanup plan, or conservation witness.
- **TR3-TR8:** finish whole-call-graph WCSU derivation, bind exact `StackPlan`
  evidence, reserve fixed nonmoving `StackLease`s, validate preservation and
  cancellation conformances, transfer arguments transactionally, lower
  park/resume, and implement the suspension-safe-loan subset. Current Unit,
  scalar, and acyclic conditional shapes retain exact frame/link/temporary,
  call, crash-terminal, and target-generated division-diamond evidence from
  instruction selection through decoded installation and artifact-wide closure
  composition. One depth-independent conditional-tree carrier accounts nested
  decisions and mutually exclusive source-distributed convergence calls.
  Extend that accounting to actual shared native joins and general affine
  cleanup rather than claiming convergence from duplicated leaves.
  Provider-sized external adapter/arrival state is design-blocked on
  `OWNER_QUESTIONS.md` Q11: stack-domain ownership across interrupted and
  switched entry must be settled before this can become a complete root
  `StackPlan`. Zero-byte internal closures remain inadmissible until that
  adapter demand exists.
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

- **SELECTED-WITNESS-EVIDENCE.** Bind a privately selected named conformance to
  one carrierless term at named `ensures`; consume its normalized requirement
  map. Named `requires` terms are positional erased inputs, passed explicitly
  after `;` and projected as `term.member`. Never infer evidence from visible
  facts or attached state names.

  The implemented front half retains optional single-proposition bindings on
  machine/state contracts through resolved and typed snapshots, then mints one
  exact erased checked term identity with independent requires/ensures lane
  position and normalized proposition application. Calls and named transitions
  carry separate erased argument lanes and bind them positionally after runtime
  substitution; missing, extra, unknown, non-nominal, or mismatched terms reject.
  Enclosing terms remain live without retransmission. Bare-name forwarding is
  proof-only and retains an exact source/output handle pair; direct ordinary
  returns assign every named output exactly once, while crash-only paths assign
  none. Continue with private complete-conformance selection, generated output
  packages, and terminal evidence identity.
- Extend named-ensures definite assignment to every ordinary outcome and emit
  compiler-generated nominal output packages. `value` is the runtime result;
  evidence erases, destructuring is complete or explicitly `_`, and guarded
  fields exist only in the matching refinement. Keep proposition, evidence-term,
  and provenance identities separate.
- Finish name-owned generic telescopes and explicit binders:
  `Name<Telescope>: [Subject] satisfies Trait { ... }` declares an
  implementation; `Evidence: Subject satisfies Trait` binds one. Identity
  retains declared name, telescope, optional subject, instantiated trait, and
  normalized rows. No visibility-, priority-, or specificity-based selection.
- Project carrierless evidence from the complete conformance map. Projection is
  stable per retained term and forwarding preserves it; separate introductions
  may differ. Evidence cannot eliminate into runtime computation.
- Add `Respects` over compiler-derived positional call telescopes, deriving its
  dependent domain, pointwise input relations, and lifted result relation.
- Add exact-pair-selected heterogeneous constructor lifts. Dependent records
  lift in order and generate checked transport obligations for coarser earlier
  fields. Extend R6 carrier-family binders for reusable proposition-valued
  relators; add no global carrier role or default relator.
- Gate runtime deciders whose lifted relation depends on erased `Type` content;
  require determination by the runtime projection or report the component.
- Then migrate `%` and suffix law discovery to propositions plus explicit
  conformances, and expand the checked `Nat`/`Int`/`Rat`/Cauchy/approximation
  corpus. `Real` remains proof-only and core-level.

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

- Finish outlives constraints, persistent/parameter-backed owners, aggregate
  borrow propagation, runtime-index expressions beyond exact immutable
  local/state-parameter forwarding, loan-root rebasing, and exact R5 facts.
- Materialize dynamic descriptors for pass-through, rebound, and escaping
  borrows from the retained exact conformance rows and declaring-trait symbol.
  Bodyless/bare requirements do not license `dyn`; ambiguous same-carrier
  boundaries name the exact complete conformance.
- Complete hermetic evaluation with crash refinement, target capsule, separate
  result/usage identities, deterministic progress, and runtime equivalence.
  Publish `Hermetic | Receipted | Volatile` ceilings and realized provenance.
- Finish member reflection (`Self::fields` and field/case splices), constant
  positions, and proof checking of generator-expanded bodies.
- Complete the ordinary `Build` API/executor with exact dependency aliases,
  package-scoped providers, no ambient filesystem escape, and generated-source
  rechecking under consumer ceilings.
- Harden resolution with content/revision checks, archive containment, limits,
  scoped writes, receipts, and one dependency/build/trust lock. Any imported
  claim-set diff invalidates root acceptance; release providers are hermetic or
  receipted, and volatile observations cannot pass source-rebuildable release.

### Components and executable trust

- **FFIVAL:** run the narrow Windows `user32` boundary-coherence slice after
  ENT4, using existing activation, custody, registration, stack, and reach
  machinery.
- Extend component artifacts with stack needs, mapping cohorts, two-sided
  import/export checks, boundary multiplicity, custody receipts, and enumerable
  roots. Drain/coexistence, scheduling, and provisioning remain runtime work.
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
- The retained selected provider plan, sealed provider execution, exact
  installed entry, and post-handoff writer context now join to the matching AOT
  fragment. That bound invocation now consumes one exact activated mapping plus
  a provider receipt for nonempty write rights, pinning, and non-publication,
  and returns a written-but-still-unpublished destination while failed linear
  transitions return every input. Implement consumer semantic validation and
  publication, physical AOT invocation, trusted/PCC and final-footprint
  validators, target W^X/coherence reporting, and uninstall/replacement joins.
- Keep arbitrary runtime bytes-to-code, JIT, and raw executable addresses
  unsupported.

Acceptance: only an admitted reusable artifact plus consumed placement authority
can produce installed code; validation binds exact final bytes and placement.

## Blocked index

These entries are pointers, not duplicate specifications.

- **FIXED-OPERATOR-SURFACE-BINDING:** blocked on the source form in owner Q1;
  named operator identities and operand-directed semantics remain settled. The
  parser, core/std sources, and canaries still carry temporary `spelling`
  clauses solely to bootstrap those semantics; they are not a compatibility
  surface and must migrate with the Q1 decision.
- **UEFI-PHYSICAL-SEMANTIC-ENTRY-COMPOSITION:** the Q2 portion of
  `ENTRY-CONTENT-ROOTS` is blocked on how platform-private handoff values
  compose with the portable semantic root requirement.
- **SEALED-LOCAL-CAPACITY-SOURCE-FORM:** the source-facing remainder of
  `ROOT-INTRODUCTION-AND-BACKING` is blocked on owner Q3.
- **WRITE-ONLY-MEMORY-VIEW:** the Q4 portion of `ENT2c` is blocked on its core
  representation, source form, and transition to readable initialized content.
- **CALLBACK-PARAMETER-REQUIREMENT:** the Q5 portion of `ENT4` is blocked on the
  source form and checked identity for one exact static callback requirement.
- **SUM-MATERIALIZATION:** blocked on the tagged-case placement vocabulary in
  `wiki/language_guide/appendix_open_questions.md`.
- **ATOMIC-EVENT-MODEL:** blocked on the portable atomic axioms and target
  refinement choices in `wiki/language_guide/appendix_open_questions.md`.
- **CHECKED-RESULT-ARITHMETIC:** blocked on whether failure-returning checked
  arithmetic earns a distinct public carrier beyond exact-by-default
  obligations and existing policy families.
- **TRAPPING-CONTRACT-ARITHMETIC:** blocked on owner Q10's definition of a
  potentially trapping arithmetic subterm inside a specification predicate.
- **IMPORTED-CRASH-CAPSULES:** blocked on the separately compiled realization
  artifact, import/export identity, and certificate-binding model in
  `wiki/language_guide/appendix_open_questions.md`.
- **NATIVE-LOGICAL-FUEL-METERING:** blocked on the sponsor-owned counter,
  exhaustion transfer, and unpaid-site continuation ABI in owner Q6. Attribution
  provenance and installation binding are implemented and do not settle that
  runtime contract.
- **PCC-VERIFIER-CLOSURE:** blocked on choosing the deployment-authoritative
  obligation-reconstruction assurance route in owner Q7. The Rust verifier and
  proof kernel remain usable, but kernel acceptance alone does not close trust
  in the reconstructed obligation set.
- **PLACED-ERASED-EVIDENCE-ESTABLISHMENT:** blocked on the source contract and
  checked representation in owner Q8.
- **PROVIDER-NEUTRAL-INTERRUPT-ACKNOWLEDGEMENT:** blocked on selecting how the
  semantic pending-to-completed transition publishes the exact PIC or
  LAPIC/x2APIC realization effect in owner Q9.

## Platform-gated verification

- Run the Linux host/time/filesystem and `IntegerAt` metadata paths natively on
  AArch64. x86-64 WSL and cross-target structural coverage already exist; do
  not claim runtime verification without the host.
- Build and run the Windows GUI callback canary through ENT4; do not pass a raw
  code address or add a Win32-only escape.
- Keep unavailable hosts structurally tested and report the missing runtime
  leg explicitly.

## Deferred until a real customer

- fault-tolerant component restart: define closed-custody component closure,
  explicit owner-death protocols for shared resources, external device reset or
  transaction obligations, and target-supplied isolation evidence together;
  abandonment-frontier reports alone must never license survivors;
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
