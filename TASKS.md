# Tasks

Last pruned: 2026-08-13.

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
  provider and generated native bridge. Finish classifying `Main::main`
  fixtures: pure language/checker fixtures stop at checked artifacts, while
  deployable/provider/artifact/ABI/layout/native tests select an exact
  target-owned `ProgramEntry`; temporary legacy ABI probes name their fixture
  entry explicitly. The CLI basics cohort and the five deployable proof
  samples, the eight CLI algorithm samples, the six CLI interpreter samples,
  eight deployable CLI game samples, all eleven CLI text samples, all thirteen
  CLI collection samples, eleven deployable CLI rendering samples, and twelve
  deployable CLI simulation samples now author all four hosted roots; the two
  proof-only samples remain targetless.
  The formerly staged `bouncing_ball_2d` and `particle_sim` samples now select
  the required core float-operation providers explicitly; their direct and
  nested mutable floating-point machine-field writes lower on all four hosted
  targets.
  Sample refresh names the exact host and never invents an entry;
  the native sample oracle selects authored roots directly and stages only
  unrooted legacy sources. The complete basics cohort, including
  `temperature_convert`, now lowers directly from its authored host entry. The
  fourteen deployable plan-laid runtime canaries likewise author all four
  hosted roots, and the active pass umbrella exercises them through production
  entry selection rather than its explicit legacy fixture seam. Their direct
  native runtime tests select the authored host root as well. The
  active pass-canary umbrella uses its explicit legacy fixture entry and asserts
  that state-graph lowering occurred, so it cannot silently collapse into
  checked-only coverage. Production and development interpreter execution
  likewise requires an exact choice, while checked-only compilation selects no
  storage root. Restored explicit legacy-entry coverage exposed and fixed lost
  checked float-operator identity across inlined calls, generic float builtins,
  desugared `abs`, nested field/literal arithmetic, and anonymous literal casts.
  The sqrt, min/max/abs/clamp, running-fold, literal-cast, classification, and
  named-provider runtime cohorts now lower and execute with exact retained
  provider evidence. Final firmware composition of
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
  slices. Current support covers scalar direct calls and guarded crash
  continuations; canonical Unit structural/content calls; exact affine cleanup
  on Unit/scalar returns and structural control edges; a bounded scalar/local
  Boolean computation subset; and attached acyclic structural control with up
  to two Boolean decision states plus one exact equal-frontier diamond. Codec,
  independent verification, interpretation, fuel, and Omega consumption agree
  on each accepted carrier. Unsupported claims, cleanup, calls, topology, or
  computation fail closed rather than falling back to source trees.

  Literal fixed-array custody retains complete dense sibling claims and one
  typed literal-index projection through either boundary settlement or an exact
  one-parameter ordinary Unit call. Verification rebases the selected claim,
  interpretation keeps unselected siblings live, and Omega realizes the
  internal call on all five targets, including indirect Windows homes. Exact
  path/type/layout/copy/claim custody survives object, image, and canonical
  installation validation. Dynamic or nested indexing, wider call signatures,
  contracts over fixed-index projections, content-bearing splits, and partial
  returns remain fenced.

  The structural partial-value cleanup slice is complete for one claim-free
  affine record: source-ordered ordinary Unit calls may transfer a finite
  nonempty set of pairwise prefix-disjoint, nonempty all-field paths, provided
  at least one residual subtree remains. Return disposes every maximal live
  residual subtree in recursive reverse declaration order; a partially moved
  ancestor is never discarded whole.
  Checked plans, canonical terminal format, independent verification,
  interpretation/fuel, and all five Omega artifact/install paths preserve the
  exact root, field paths, and leaf types without a runtime bitmap or cleanup
  bytes. Claims/content, non-crash contracts, wider crash predicates,
  arrays/cases, and nominal `drop` remain fenced for partial-record cleanup.

  The whole-root nominal cleanup slice is complete for a finite nonempty list
  of claim-free, unqualified affine parameters whose records are empty or
  contain only relevant Boolean/integer fields. One attached `T::drop` may be
  empty or make a finite source-ordered list of calls to distinct exact-empty helpers.
  Multiple attached drops execute in reverse parameter order and may share one
  cleanup target. Every body in the action list may use the same bounded
  helper-call form, including a shared cleanup target or helper. Checked production, canonical
  terminal encoding, independent verification, interpretation, and fixed fuel
  preserve each whole receiver, charge the root edge once, and count every
  cleanup invocation. Omega carries one ordered cleanup-action stream through
  all five object/image/install paths. Empty drops add no call; each accepted
  executable form emits a call owned by its exact edge/action ordinal before
  return teardown and retains source-ordered operation-owned helper custody.
  The bounded empty/helper-call bodies additionally admit the finite
  direct-Boolean contextual contract subset described under CML4 across one or
  more cleanup roots.
  Shared targets reuse one proof receiver while each root receives distinct
  edge obligations. Nested/erased receivers, wider body shapes, locals,
  claims, qualifications, other contract forms, and non-root edges remain
  fenced.

  Scalar return edges now use the same ordered cleanup-action vocabulary. The
  source-produced nominal branch accepts a finite nonempty list of direct,
  claim-free affine parameters that may freely mix no-code and nominal roots,
  a finite set of direct primitive scalar inputs interleaved at authored
  parameter positions, a finite source-ordered prefix of immutable branch-free
  primitive locals, and either one branch-free scalar result or one top-level
  Boolean `&&`/`||` whose operands are branch-free over those inputs and locals,
  and the same empty or bounded zero-argument helper-call `drop` body for each nominal
  root. Checked plans retain the complete authored parameter partition while
  terminal Psi gives scalar values and structural places independent dense
  namespaces. Local and result operations materialize in source order before
  every cleanup action. Every action then runs in reverse authored structural
  root order through verification, interpretation, fixed fuel, and all five Omega
  object/image/install paths; nominal targets may be distinct or shared and
  no-code actions retain their exact positions without emitting instructions.
  Native lowering preserves the computed ABI result and, where required, the
  return link across executable cleanup calls with byte-validated stack
  evidence.
  The finite mixed list additionally admits the same direct-
  Boolean contextual requirements as Unit cleanup: checked production retains
  root-specific caller premises (including supported premises on no-code
  roots), terminal Psi reconstructs and verifies every nominal action
  obligation, and Omega projects proof-only receivers/obligation identities
  after verification while preserving the complete runtime action order
  through all five targets. The bounded Boolean form retains two decisions and
  three distinct return edges; every leaf owns the same complete cleanup
  stream, and native artifacts retain three edge-specific cleanup intervals.
  One final top-level short-circuit Boolean local may be consumed once by a
  branch-free return suffix, including one intervening branch-free Boolean
  continuation local returned directly; either form is source-distributed into
  the same three proof-bearing cleanup leaves. Value reuse, a second
  continuation local, repeated stages, explicit convergence to one cleanup
  return, nested decisions, calls, effects, nested nominal ownership, and wider
  scalar bodies remain to be added as complete vertical slices.

  The root-only structural result carrier now reaches exact one-fragment Omega
  native realization and installation, including a finite claim-free affine
  parameter tail and a finite consecutive prefix of established empty-record
  affine locals. Both clean up in canonical reverse order with no emitted
  cleanup code; register and stack ABI homes survive installation. Next add
  wider partial-value cleanup,
  remaining edge kinds and conservation,
  returned transfer, loops, suspension, and scoped ordering. Cycles, divergent
  or wider joins, reordered custody, computed structural guards/successors,
  wider projections, and incomplete evidence remain fenced until their entire
  slice is verifier-owned.
  One-state Unit/effect bodies also accept a finite leading run of immutable,
  unqualified, empty-record affine locals. Establishment is source ordered;
  return cleanup is reverse-local then reverse-parameter order. Complete
  custody and fuel attribution survive all five native artifact pipelines as
  zero-byte runtime work. Nonempty, mutable, qualified, content-bearing,
  nominal-cleanup, or post-effect locals remain fenced.
  Ranked tail-recursive call graphs remain rejected until tail position and
  ranking evidence are terminal and verifier-owned. Retire checked/source-tree
  consumers with each slice; nothing below terminal Psi may depend on
  typed/source trees, `ExpressionHandle`, source rendering, or an Omega-to-Psi
  bridge. Bind canonical partition-composition replay rows to an exact operation
  and verifier-selected callee guarantee before exposing the theorem; their
  independently reconstructed fingerprints are identity, never authority.
- **CRASH-CONTRACT.** Extend guarded implication beyond the accepted acyclic
  scalar slice. Source-produced direct calls now consume checked
  invocation-specific rows, preserve parameter and computed direct-local
  substitutions, and emit verifier-reconstructed guarded continuations.
  Positional calls stage short-circuit arguments left-to-right; guarded staged
  calls bind their continuations from the fingerprint-pinned, parameter-relative
  callee contract to exact terminal argument values. Nonempty paths to relevant
  Boolean fields of record parameters retain every canonical field identity,
  rebase across whole-root and all-field-projected structural Unit calls,
  round-trip through both codecs, and are checked independently by the
  verifier. Fixed-index argument prefixes and built-in Boolean member equality,
  inequality, negation, and conjunction now compose and rebase every retained
  path. Same-typed relevant fixed-integer members now retain canonical paths
  through equality, inequality, and ordered comparisons, including conjunction,
  whole-root and all-field-projected structural calls, both codecs, and
  independent leaf-type checking. Terminal proposition disjunction now retains
  distinct canonical branches, rebases every nested member path across
  all-field-projected calls, and is independently reconstructed by the verifier.
  Continue with whole-aggregate equality, arithmetic over members, and
  case-payload paths. Imported crash capsules remain design-blocked on artifact
  identity and certificate binding.
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
timer ticks over owned serial output, and halts between ticks. No
customer-shaped compiler concept is introduced.

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
  depth-two rail does not widen the depth-four call budget.
  Indexing irreversibly coarsens to the nearest backing collection while
  preserving independent index-call writes. Finite named-state SCCs accept only
  bijective write-capable parameter permutations. Primitive-only concrete
  record/sum locals remain isolated through nested fixed arrays.

  Continue with representable relational candidates. Recursive, boundary,
  beyond-per-position-budget, binding-reborrow, reference-valued/opaque,
  escaped, non-bijective, generic, recursive or reference-bearing aggregate
  literals, third aggregate shells, calls beneath aggregate-field operators or
  other computed field shapes, and out-of-isolated-root shapes remain
  conservative fences. Do not restore authored `stores` clauses or treat
  lifetime elision as evidence; Git carries individual evidence cohorts.
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
  materialization and transfer-map commitment. Accepted terminal slices carry
  exact reverse-declaration affine cleanup through Unit/scalar returns and
  bounded acyclic structural control, including short-circuit Boolean stages and
  one equal-frontier diamond. Partial-record transfer accepts a finite
  source-ordered set of pairwise prefix-disjoint, nonempty all-field paths. It
  preserves and disposes every maximal residual subtree in recursive reverse
  declaration order through interpretation and all five native artifact paths.
  One whole
  affine parameter whose record is
  empty or contains only relevant Boolean/integer fields now invokes an exact
  attached nominal cleanup through interpretation, fuel, and all five native
  artifact paths on Unit return and on the bounded branch-free scalar-return
  branch.
  The cleanup may be empty or make a finite source-ordered
  zero-argument calls to mutually distinct exact-empty helpers; native
  artifacts retain the cleanup edge and helper operations as distinct call
  owners. Finite nominal cleanup lists run in reverse parameter order through
  interpretation, fuel, and every native artifact path. They may share a
  target, and every action may use the bounded executable body, including a
  shared cleanup target or helper; native calls retain exact edge/action
  ordinals. One contextual subset additionally accepts a finite
  canonical set of direct relevant Boolean receiver-field `requires` clauses
  in either polarity
  across a finite cleanup-root list when the caller's canonical Boolean fact
  set proves every one at the Unit return edge; unrelated supported caller
  facts remain available. Shared cleanup targets retain one target-local proof
  receiver while each action gets distinct positional obligations. Terminal Psi
  retains every proof-only receiver substitution and positional obligation,
  independently verifies the
  source-produced semantic/proof artifact, and removes proof metadata before
  all five Omega runtime carriers. Missing premises reject with an edge- and
  cleanup-specific diagnostic. Contextual scalar cleanup distributes one
  bounded short-circuit stage through at most one single-use branch-free
  continuation local; unlike the claim-free scalar lane, it does not yet retain
  a typed convergence block or shared cleanup edge. Extend contextual cleanup
  beyond this direct-Boolean, receiver-independent-body subset; add wider
  structural partial-value cleanup,
  repeated-cycle resource composition, and
  conservation/backend-ledger reporting. The accepted slices are not yet a
  general conditional CFG, complete cleanup plan, or conservation witness.
- **TR3-TR8:** finish whole-call-graph WCSU derivation, bind exact `StackPlan`
  evidence, reserve fixed nonmoving `StackLease`s, validate preservation and
  cancellation conformances, transfer arguments transactionally, lower
  park/resume, and implement the suspension-safe-loan subset. Accepted Unit,
  branch-free scalar, and one two-arm scalar conditional native shape retain and
  replay exact code-positioned frame/link/temporary evidence and compose an
  acyclic closure demand. The canonical terminal installation record now seals
  those per-function and per-call facts, and a decoded record reproduces the
  same internal closure demand. Extend accounting to nested/reconvergent
  conditionals, crashes in arms, division/remainder, and the external entry
  adapter; then compose the decoded demand with selected-provider admission in
  the installed-root report before calling it a complete root `StackPlan`.
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
  facts or attached state names. The optional contract binding now parses only
  on machine `requires`/`ensures`, requires exactly one proposition, and remains
  distinct through resolved and typed trees plus snapshots. Checked trees now
  reject Boolean, membership, fact-only, and non-nominal bindings, and mint one
  exact erased term identity per witness-bearing binding with its independent
  requires/ensures lane position, normalized proposition application, and
  carrierless interface. Continue with erased call-lane arguments,
  assignment/forwarding, private complete-conformance selection, generated
  output packages, and terminal evidence identity.
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
