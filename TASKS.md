# Tasks

Last pruned: 2026-08-12.

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
  corpus migration. The recorded installation handoff now binds an exact mapped
  receiver reservation, zeroes it, and carries its one exclusive activation
  loan without releasing receiver-bound roots through an unchecked path. Bind
  that handoff and the installer to the selected physical provider and generated
  native bridge. Classify the remaining semantic canaries out of deployable
  compilation unless they assert provider, artifact, ABI, layout, or native
  behavior; pure language/checker fixtures use the checked-only harness. Migrate
  the remaining `Main::main` corpus to exact target-owned `ProgramEntry`
  bindings. Three single-target hosted canaries
  cover Windows x86-64, Linux x86-64, and Linux AArch64; target-scoped build
  selection now accepts a matrix of exact per-target root bindings, and the
  first four-target hosted canary plus the byte/line console replay matrices
  and initial cross-architecture guard/copy, double-indexed, and indexed-pair
  binary-write, text/bounded-buffer, wire, and storage/guard footprint artifact
  cohorts plus the primitive-float differential matrix have migrated. Hosted
  import, runtime-exit, fixed-array, plan-laid, and wire-policy cohorts now
  select exact roots too. Five intentional result-bearing entry-ABI probes and
  one checked-only filesystem-conversion fixture now use an explicitly named
  legacy test entry; the trust-report/lock cohort is entry-agnostic checked-only
  compilation, while the hosted build-evaluation/native staging cohort selects
  an exact host-owned entry root. The native runtime-END/domained-subslice and
  positive recast/layout cohorts select exact roots across the four hosted
  targets too; negative recast diagnostics and the entry-agnostic checked/
  artifact canary cohort plus the broad sample source-compatibility sweep use
  production checked-only compilation. Backend `Main::main` discovery is
  deleted. The macOS native filesystem, generated-filesystem, float, ObjC/GUI,
  and directly executed sample cohort now stages each complete source project
  into an isolated tree, merges an exact `macos_arm64::ProgramEntry` binding
  into its build root, and uses production compilation while preserving project
  assets and GUI subsystem configuration.
  Production and development interpreter callers require Omega's
  exact entry choice; checked-only semantic compilation no longer invents
  `Main::main`, while legacy test harnesses name that temporary fixture entry
  explicitly. Implicit Psi entry discovery and the former `Main::run` entry
  convention are retired. Final composition of firmware
  `ImageHandle`/`SystemTable` inputs with semantic roots is design-blocked on
  owner Q2; the remaining bridge and corpus work is not.
- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Take one real
  content-bearing source program through terminal Psi. Add sealed introduction
  and custody-exit frontiers, derive residual geometry at partial bodyless
  boundaries, and admit only provider custody. Infer identity-preserving
  reshuffles; partition changes require an authored theorem. Before emitting an
  introduction or exit, checked facts must bind the exact content subject and
  geometry to the selected provider plan, invocation receipt, backing/root
  lineage, installed occurrence, and route; a generic established-claim identity
  is insufficient.
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
  field identities.

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

- **PSIIR.** Extend terminal Psi beyond general blocks, positional scalar direct
  calls, guarded in-module call-crash continuations, and the accepted Unit
  structural/content call slice. Add aggregate and result-bearing custody,
  cleanup, returned transfer, loops, suspension, and scoped ordering as complete
  vertical slices. Ranked
  tail-recursive call graphs stay rejected until their tail-position and
  ranking evidence is terminal and verifier-owned. Retire checked/source-tree
  consumers as each slice moves. Nothing below terminal Psi may depend on
  typed/source trees, `ExpressionHandle`, source rendering, or an Omega-to-Psi
  bridge. Canonical partition-composition rows remain replay evidence, not
  semantic axioms. Their source fingerprint is independently reconstructed;
  bind each row to an exact operation and verifier-selected callee guarantee
  before making the theorem available on success. Fingerprints are identity,
  never authority.
- **CRASH-CONTRACT.** Extend guarded implication beyond the accepted acyclic
  scalar slice. Source-produced direct calls now consume checked
  invocation-specific rows, preserve parameter and computed direct-local
  substitutions, and emit verifier-reconstructed guarded continuations.
  Positional calls stage short-circuit arguments left-to-right; guarded staged
  calls bind their continuations from the fingerprint-pinned, parameter-relative
  callee contract to exact terminal argument values. Add wider aggregate/member
  predicates. Imported crash capsules remain design-blocked on artifact identity
  and certificate binding.
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
- **PROOF-RELEVANCE-MIGRATION.** Implement binding-level `[erased]`, checked
  noninterference, erased-stripped layout, and obligation preservation. The
  field-binding surface preserves explicit relevance through syntax, resolved,
  and typed trees plus their snapshots. The executable slice supports
  non-generic transparent records, sums, and mixed common-field/case shapes,
  plus closed synthesized generic-record instances selected by explicitly typed
  local initializers, direct exact assignments, unique exact call parameters,
  or exact returns (including nested concrete record fields), and pure or mixed common-field/case generic sums with multiple
  exact closed instances per generic base (including nested concrete payload
  records and constructor/pattern uses selected by exact local, assignment,
  parameter, or self-field context), and
  non-generic plain `CheckedShape` records whose attached machines are closed
  ordinary checked bodies:
  construction elaborates an omitted exact common or selected-case erased
  binding only when a currently resolved/nameable non-generic evidence type has
  exactly one payloadless constructor and no common fields, while checked
  validation forbids runtime reads/writes/effects through it; proof,
  multiplicity, exhaustiveness, case identity, and semantic identity retain it;
  the legacy proof-only classifier ignores erased containment; and the
  checked/runtime expression boundary plus record/case layout and eligible
  attached-machine layout omit the field and its initializer subtree without
  renumbering variants; erased attached fields also create no runtime
  contained-machine topology. Ambiguous, absent, generic, and otherwise
  ineligible evidence remains explicit-term-required.
  Expand this without compatibility scaffolding to ambiguous/overloaded or
  attached-call generic record/sum construction contexts and attached machines
  over generic
  or case-bearing data, non-checked supply modes, or unresolved machine
  parameters. Plan-laid values now omit erased bindings from physical Schema
  reflection, validated plan entries, native/interpreter layouts, recasts, and
  latent ABI classification while the synthetic semantic value and normalized
  schema identity retain their terms, obligations, and relevance. `Placed<P,
  T>` erased-evidence establishment is design-blocked on owner Q8. All
  unsupported shapes continue to fail closed at use sites.
  Numbered wire data preserves erased bindings and identities through syntax,
  resolved/typed trees, snapshots, and semantic data while normalized plans,
  validation, interpreter/native encode, and decode omit their tags and bytes.
  Explicit relevance supersedes
  “recursive means proof-only”; non-layoutable `Type` values remain erased-only.
  Do not infer carrier relation roles from relevance.
  Fixed non-generic record boundary values now retain erased fields in semantic
  identity, source-policy input, and terminal-Psi structural rows while omitting
  them recursively from native argument/result layout and transfer. Direct and
  nested SysV AMD64 and AAPCS64 entry/import canaries pin the erased-stripped
  classifications. Case-bearing values and unresolved generic aggregates still
  fail through their existing public ABI-shape limits rather than a relevance
  fence; this slice does not manufacture an ABI for shapes the calling-policy
  vocabulary cannot express.
  Result-domain overload families with one identical closed parameter signature
  now provide the same exact generic record/sum literal context as a unique free
  call; parameter-distinct overloads remain resolver-owned and fail closed.
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
  through a receipt.
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

Acceptance: QEMU installs Cathedral-owned memory/interrupt structures, reports
timer ticks over owned serial output, and halts between ticks. No
customer-shaped compiler concept is introduced.

## Parallel compiler and language lanes

### Frames, reach, and trust

- **R5:** continue exact inferred may-write summaries and relational candidates.
  Complete statement/value frames and transitive boundary wrappers preserve
  facts outside their writes; opaque frames remain conservative fences. Named
  state SCCs solve finite exact frames when write-capable parameters traverse
  bijective permutations, including edges forwarded through structurally
  transparent returned places. Stable local mutable aliases substitute exact
  `self`/parameter origins through acyclic graphs and SCC equations, including
  direct stable rebinding: the rebound name takes the replacement origin while
  earlier reborrows keep their established origin. Member suffixes remain exact;
  indexing coarsens to the nearest collection and that coarsening is absorbing.
  A validated mutable recast local retains its source storage origin when that
  source expression is effect-free; effectful recast sources remain opaque.
  A free or attached helper may relate a returned `&mut` place to one
  mutable-reference parameter, including an attached helper's actual receiver;
  its terminal result may follow a prefix of effect-free caller-isolated scratch
  locals and direct local `&mut` aliases, including mutable bindings and results
  of other structurally transparent helpers. A caller-isolated scratch local
  may also be initialized by one direct value call when its inferred frame is
  complete and empty. The same exact returned-place
  relation composes when such a result is supplied directly as a statement-call
  argument.
  Value-shaped assignments with effect-free right-hand sides may write through
  those origins, including exact transparent call-produced targets, without
  changing the relation, while their ordinary frames remain published;
  effect-free discarded expressions are neutral. A direct helper-local alias
  rebind updates that name's origin without redirecting prior reborrows; a
  structurally transparent helper result may supply the replacement through the
  same origin algebra. Other computed rebinding, discarded/statement calls,
  opaque or recursive result producers, effectful index computations, other
  computed helper initializers, and nontrivial results remain opaque. Opaque,
  recursive, or write-capable scratch initializer calls remain fences.
  Primitive-only concrete record/sum locals are caller-isolated through nested fixed arrays;
  that isolation survives structurally transparent helper forwarding.
  Recursive, generic, reference-bearing, or other computed local roots remain
  fences.
  Escaped aliases, unrepresentable rebinding, and non-bijective cyclic transport
  also remain opaque. Signature lifetime elision alone establishes no relation.
  Continue with other representable candidates; do not restore authored
  `stores` clauses.
- **STR/EFX:** finish independent normalization/publication of machine supply,
  service reach, suspension, blocking, termination, mutation, and trust; remove
  remaining umbrella carriers after their consumers migrate.
- **TPR4/TPR6:** connect progress-profile grants and receipts without putting
  private ranking witnesses into public identity.
- **GR6:** finish qualification/trust consumers and their artifact rows. The
  durable trust report now copies each routed provider entry/result claim with
  exact plan fingerprint, requirement, subject, authority flow, semantic
  domain, carry policy, predicate-discharge requirement, and grant provenance;
  granted rows also retain the exact authored root-grant selectors that
  activated the selected plan, while unselected candidates retain none;
  the qualification artifact also retains the canonical requirement overload
  identity and predicate-body status beside its readable label; vacuous-use
  rows, safe-point and activation-wide carry rows, and machine-contract rows
  retain the exact owning machine overload identity. Continue with
  consumers that still lack exact blast-radius rows. Provider-slot grants now
  resolve through the selected closure, so lock/report/runtime admission bind
  the same plan and leave unselected candidates dev-active. Selected provider
  schemas, rows, adapter dispatch, and calling-plan lookup now require exact
  nonempty overload identities; no name-only singleton compatibility remains.

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
  park/resume, and implement the suspension-safe-loan subset. The terminal
  Unit-body native slice now retains exact code-positioned stack/link evidence;
  object construction validates the instructions, derives local and caller-live
  peaks, and composes an acyclic closure demand. The branch-free scalar slice
  likewise retains and replays exact frame and temporary-stack mutations,
  validates typed direct-call outbound/link evidence against those mutations,
  derives caller-live bytes with pending temporaries, and joins the same
  acyclic closure. One bounded scalar CFG shape is also sealed: a top-level
  Boolean-parameter or linear Boolean-expression condition with two direct
  linear integer return arms. Object validation distinguishes the condition
  branch form, requires exact x86 flag-preserving frame release and AArch64
  `B.EQ` evidence for expression conditions, replays the balanced condition
  prefix and both arms independently, and takes their maximum. Typed scalar
  calls in the prefix or either arm reuse exact call evidence and closure
  composition. Extend that accounting to nested/reconvergent conditionals,
  crashes in arms, division/remainder expressions, the external entry adapter,
  and installed-root/provider admission before treating it as a full root
  `StackPlan`.
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
- Add named-ensures definite assignment per outcome and compiler-generated
  nominal output packages. `value` is the runtime result; evidence erases,
  destructuring is complete or explicitly `_`, and guarded fields exist only
  in the matching refinement. Keep proposition, evidence-term, and provenance
  identities separate.
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
- **TCBMANIFEST:** derive executable TCB metadata from the selected-provider
  closure and build profile. Separate known entries from proved completeness;
  retain provider/executable/plan identity, scope, origin, implementation
  evidence, and independent containment axes.
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
- **ATOMIC-EVENT-MODEL:** blocked on the portable atomic axioms and target
  refinement choices in `wiki/language_guide/appendix_open_questions.md`.
- **CHECKED-RESULT-ARITHMETIC:** blocked on whether failure-returning checked
  arithmetic earns a distinct public carrier beyond exact-by-default
  obligations and existing policy families.
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
