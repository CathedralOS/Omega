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

- **ENTRY-CONTENT-ROOTS.** Complete the generated native entry bridge and
  explicit-entry corpus migration. The production-facing installation carrier
  now joins the exact selected provider plan, arrival requirement, calling-plan
  fingerprint, physical provider/invocation, and both roots before consuming
  either grant. Its physical path maps and zeroes the exact receiver reservation
  and returns one exclusive activation loan; the separate local seam rejects
  provider-issued roots. Connect an emitted target entry stub to that carrier,
  consume the activation loan while invoking the selected source continuation,
  and retain the resulting generated-bridge evidence. Pure language/checker
  fixtures stop at
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
  recursively fixed arrays/records as whole `Repeated` or `Nested` fields, and
  one plan may place multiple independently keyed aggregate fields. View paths
  retain one whole `At` extent; owned materialization also admits an outer
  fixed array tiled by exactly one compiler-sized element `At` at one validated
  constant destination stride. Compiler-derived strides and offsets drive the
  interpreter and all three native target paths. Mutable fact-free byte views
  write and reread through those same extents, including two runtime indices
  through a gapped outer fixed array of recursively fixed arrays while
  retaining the plan-derived outer stride and compiler-derived inner stride,
  and through a gapped outer fixed array of fixed records whose interior fixed
  array retains the compiler-derived member offset between those indices.
  Typed owned materialization
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
  `Granted & Vacant` before allocator integration. Permission-stage borrowed
  admission can already withdraw the exact loan before content establishment.
  Source establishment and owned retirement are design-blocked on owner Q8:
  the admitted intermediate, failure signatures, and erased evidence are
  unsettled, and retirement lacks the checked destruction or move-out receipt
  needed to establish `Vacant`. Continue independent internal authority work.
- Derive readable, destructive-read, writable, and atomic field accessors while
  keeping logical extents distinct from whole-transfer footprints. Enforce
  total decode/encode, exact provider width/alignment, and operation-specific
  atomic laws. Continue rejecting External initialization, multi-transfer
  reads, and synthesized RMW.
- Keep alias-exclusion admission separate from access rights; `&mut` does not
  claim exclusivity against a device. Sealed primitive events now specialize
  linearly into Stable read/write, External read/take/write, or one exact
  Atomic operation and ordering while preserving the original authority on
  rejection. Connecting those admitted events to Terminal Psi and both native
  backends is design-blocked on owner Q15: the canonical installed placed-root
  authority and read/take/write/atomic value-custody contract are unsettled.
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

- **PSIIR.** Extend terminal Psi only as complete vertical slices: canonical
  encoding, independent obligation reconstruction and verification,
  interpretation, fixed fuel, Omega lowering, native evidence, artifact/image
  custody, and installation must move together. The detailed accepted
  vocabulary and current fences live in
  [`terminal_psi.md`](wiki/architecture/pipeline/terminal_psi.md); do not
  duplicate its operation-by-operation ledger here.

  The accepted baseline covers bounded scalar/direct and structural/content
  calls, guarded crash continuations, structural results, fixed-array custody,
  exact affine cleanup and partial transfer, bounded acyclic control, selected
  provider catalog/dispatch, and verified Boolean/integer shared cleanup
  convergence. Integer leaves retain the documented policy arithmetic, casts,
  shifts, division/remainder, exact-operation evidence, bounded nesting, and
  independent exact leaves in distinct proof-free subtrees across interpreter
  and every native target. A finite same-carrier exact-add chain may have a
  landed literal sibling at each link, while a finite exact-subtract chain may
  continue only through its left operand and must have a landed literal right
  operand at each link. A finite same-carrier chain may mix exact addition and
  subtraction when both kinds occur, every link continues through its left
  operand, and every right operand is a landed same-carrier literal. The
  verifier combines additions and mathematical negations of subtrahends in a
  checked sign/magnitude offset and derives every carrier-tight prefix bound
  from the direct root. A finite same-carrier chain may also mix exact divide
  and remainder while continuing only through its left operand, with a landed
  nonzero unsigned divisor or signed divisor other than `0` and `-1` at every
  link. A finite same-value-carrier exact-right-shift chain may likewise
  continue only through its left operand, with an independently landed fixed
  native integer count satisfying `0 <= count < value width` at every link.
  A finite same-value-carrier exact-left-shift chain may likewise continue only
  through its left operand, with independently landed fixed native integer
  counts satisfying `0 <= count < value width`; count carriers may differ.
  The verifier accumulates those counts with checked arithmetic and derives the
  carrier-tight bound on the direct root for every prefix, including the
  zero-only root when the cumulative count reaches the value width.
  A finite same-carrier exact-multiply chain may continue only through its left
  operand, with an explicitly landed same-carrier nonnegative literal factor at
  every link. All seven forms require a direct machine-parameter root. The
  verifier walks ordered definitions for addition, subtraction, their mixed
  chain, multiplication, and left shift and reconstructs every retained
  operation's safety obligation independently;
  multiplication accumulates nonnegative factors with checked arithmetic and
  derives carrier-tight root bounds, while divide/remainder and right-shift
  links need no producer-definition authority because each safe landed divisor
  or count reconstructs independently. One direct
  fixed-integer parameter may also pass through a finite chain of valid
  widenings and then exactly narrow back to its original carrier; Terminal
  retains every operation and independently derives the narrowing obligation
  from the ordered, uniquely defined widening chain. Separately, one exact
  fixed-native cast may consume a finite nonempty left-associated same-carrier
  exact-add/subtract literal-offset chain rooted at a direct machine parameter.
  The verifier retains every prefix proof, accumulates the checked offset, and
  independently derives target-range-minus-offset bounds intersected with the
  source carrier, including signed and cross-sign conversions. One
  validator-legal partial fixed-native cast may also consume a finite nonempty
  left-associated same-source-carrier exact-multiply chain rooted at a direct
  machine parameter, with independently landed nonnegative literal factors.
  Every multiply prefix keeps its own evidence; the cast uses the checked
  cumulative product to reconstruct the inverse target interval and intersect
  it with the source carrier. Product zero makes only the cast obligation true,
  product one uses the ordinary target/source intersection, and larger products
  divide the signed or unsigned target bounds without erasing earlier proofs. A
  validator-legal partial fixed-native cast may likewise consume a finite
  nonempty left-associated same-source-carrier exact-left-shift chain rooted at
  a direct machine parameter, with independently landed legal fixed-native
  counts whose carriers may differ. Every shift prefix keeps its own evidence;
  the cast uses the checked cumulative count to shift the target interval right
  and intersect it with the source carrier. Count zero uses the ordinary
  target/source intersection, a sub-source-width count uses signed or unsigned
  inverse target bounds, and a source-width-or-larger count makes only the cast
  true because any successfully produced exact source result is zero.
  A finite nonempty same-source-carrier exact-right-shift chain may feed the
  same partial cast under the same direct-root and heterogeneous landed-count
  fences. The cast independently reconstructs the arithmetic/zero-fill shift
  preimage of the target interval; at or above source width, unsigned roots
  yield zero while signed roots yield `-1` or `0` and therefore require a
  nonnegative root only when the target is unsigned.
  Conversely, one
  validator-legal partial fixed-native cast of a direct parameter may root a
  finite nonempty left-associated same-target-carrier exact-add/subtract chain
  with independently landed literal right siblings. The cast keeps its own
  direct representability evidence; every arithmetic prefix keeps distinct
  evidence for the target interval shifted by its checked cumulative offset and
  intersected with the source carrier. Cancellation cannot erase an earlier
  prefix obligation. The same direct partial-cast root may instead feed a
  finite nonempty left-associated same-target-carrier exact-multiply chain with
  independently landed nonnegative literal factors. Every multiply prefix
  keeps distinct evidence for the target interval divided by its checked
  cumulative product and intersected with the source carrier; zero and one
  produce a true current-prefix obligation without erasing earlier proofs. The
  direct partial-cast root may also feed a finite nonempty left-associated
  same-value-carrier exact-left-shift chain with independently landed in-range
  fixed-native counts whose carriers may differ. Every prefix keeps distinct
  evidence for the target interval shifted right by its checked cumulative
  count and intersected with the source carrier; a cumulative count at least
  the target width admits only the zero root.
  One direct fixed-native parameter may now also root a finite left-associated
  same-carrier affine chain that contains both an exact add/subtract offset and
  an exact multiply. Every right sibling is an independently landed
  same-carrier literal, multiply factors are nonnegative, and every ordered
  prefix retains independent evidence. The verifier replays each prefix as
  `A * parameter + B` with checked nonnegative `A` and checked signed `B`, then
  derives the carrier preimage; constant prefixes are true or false from `B`
  alone. A later zero factor or offset cancellation cannot erase an earlier
  proof. The same unified mixed affine chain may now feed one validator-legal
  partial fixed-native exact cast. The cast independently reconstructs the
  target interval through `(A, B)` and intersects it with the source carrier;
  `A == 0` decides only the cast from target representability of `B`, while all
  earlier arithmetic-prefix evidence remains mandatory.
  The converse unified family is now retained as well: one validator-legal
  partial fixed-native exact cast of a direct parameter may root a finite
  nonempty same-target-carrier affine chain containing both offset and multiply
  operations. The verifier independently reconstructs every prefix through
  checked `(A, B)` composition and the target/source interval intersection;
  `A == 0` decides only the current prefix from target representability of `B`,
  while cast and earlier arithmetic evidence remain mandatory.
  The direct partial-cast root may now also feed a finite nonempty
  left-associated same-value-carrier exact-right-shift chain with independently
  landed heterogeneous legal counts. The cast proof remains independent, and
  every shift prefix is reconstructed from only its own `0 <= count < width`
  fact without cumulative count, value-definition, or evidence import.

  Next engineering frontiers are other proof-bearing results feeding another
  proof-bearing operation, other computed exact-cast and wider exact-arithmetic
  premises, member/comparison mixtures, calls and effects, wider partial-value
  cleanup, nested ownership, returned transfer, loops, suspension, scoped
  ordering, and ranked tail recursion. Dynamic/nested indexing, wider
  projections and signatures, content-bearing splits, and unsupported contracts
  remain fail-closed until independently verifier-owned.

  Retire checked/source-tree consumers with each slice. Nothing below terminal
  Psi may depend on typed/source trees, `ExpressionHandle`, source rendering, or
  an Omega-to-Psi bridge. Partition replay binds the exact operation and
  verifier-selected callee guarantee; fingerprints are identity, never
  authority.
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
  borrowed selections remain fail-closed. Primitive result-bearing calls now
  carry exact whole-root receipts from source checking through terminal Psi
  encoding, verification, and retry-safe interpretation. Omega retains the
  result in its abstract plan and rejects the old metadata-only settlement path
  rather than dropping it. An admitted x86-64 `u8` port-read provider now has
  an exact result-returning native realization whose arguments, receipts,
  instruction interval, and provider identity survive installation. Other
  result shapes and targets remain fail-closed. Explicit provider views now
  borrow one linear validity claim: consuming invalidation is accepted after
  the view's last use and rejected while the view remains live. Projected/
  content-bearing result calls remain fail-closed.
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
  allocation waits for the package allocator. Cathedral now represents one
  512-entry page candidate and validates its complete exact-zero starting
  state with a checked bounded scan. Physical backing, address-space-profile
  hierarchy, mappings, installation, and teardown remain. Do not restore a
  compiler-owned page-table model.

#### Exception roots and first timer

- Materialize fatal/diagnostic entries for every architectural exception before
  enabling interrupts. Cathedral now has a checked fixed-work internal leaf
  that records one normalized 0–31 vector in preallocated atomic state,
  publishes its validity, and unconditionally aborts. Generated per-vector
  stubs, admitted internal-state binding, physical entry plans, stacks, gates,
  and IDT installation remain.
- Provision dedicated per-CPU double-fault/NMI/machine-check stacks and one
  non-nesting maskable-IRQ stack class; preserve the selected `StatePlan`.
  Cathedral now authors and validates the complete four-role class/IST policy;
  WCSU-derived byte sizing, a source-level `StackLease`, storage provisioning,
  and installed-root binding remain.
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
  A direct primitive scalar assignment value may wrap complete caller-isolated
  call producers in up to two unary, binary, primitive-cast, member-projection,
  or indexing shells without widening that call budget.
  One top-level concrete primitive-only record or selected-case literal may
  likewise contain an independently bounded non-reference call tree in each
  direct common or payload field while publishing every write. One direct
  field may instead contain a second concrete primitive-only record or
  selected-case literal whose direct fields obey the same rule; this aggregate
  depth-two rail does not widen the depth-four call budget. A declared
  primitive field at either level may wrap independently bounded call operands
  in up to two nested scalar-computation shells made from unary/binary
  operators, primitive value casts, member projections, or indexing without
  widening that budget. Literal-length caller-isolated fixed-array assignment
  values preserve the same relation through one nested array level; every
  element retains the same call and primitive-computation budgets. Within that
  same two-level aggregate budget, fixed arrays may contain concrete record or
  selected-case literals, and concrete record or selected-case fields may
  contain literal fixed arrays. A primitive scalar assignment value may also
  select one direct member from a concrete caller-isolated record or
  selected-case literal whose effectful primitive fields are bounded
  direct-call trees or use one scalar-computation shell around those calls;
  every field publishes its writes. One additional outer scalar shell is
  admitted only when the fields do not consume that remaining shared
  computation-depth-two budget. The literal receiver may use the existing
  two-level aggregate budget while carrying that reduced computation budget
  unchanged; a third aggregate level remains fenced.
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
  service reach, suspension, blocking, termination, mutation, and trust. The
  state graph and checked-tree visualization now consume suspension and blocking
  independently from exact flow-state and machine-contract facts while service
  reach stays on its dedicated facts. Provider approval now consumes exact
  checked-flow call coordinates directly and no longer replays the operational
  umbrella. The published checked operational root is retired; its plan remains
  only as a transient validation and independent-fact construction input.
  Continue removing umbrella carriers after their remaining consumers migrate.
- **TPR4/TPR6 — design blocked on owner Q16.** Choose how an ordinary domain or
  routed requirement is classified and attached as a progress premise before
  connecting progress-profile grants and receipts. Generic routed/domain
  requirements must not be treated as progress merely because they are
  predicate-free or provider-backed; private ranking witnesses remain outside
  public identity.
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
  native artifact paths. Nominal scalar cleanup admits finite continuation
  chains whose stages contain arbitrarily nested finite short-circuit Boolean
  decisions. One finite parameter/constant decision tree, including Boolean
  equality against a constant, can instead feed a typed shared terminal-Psi
  convergence value and one native cleanup tail. Extend contextual
  cleanup beyond the current receiver-independent Boolean subset, finite
  continuation trees, and that narrow shared-convergence shape; add
  wider structural partial values, repeated-cycle resource composition, and
  conservation/backend-ledger reporting. This is not yet a general conditional
  CFG, complete cleanup plan, or conservation witness.
- **TR3-TR8:** finish whole-call-graph WCSU derivation, bind exact `StackPlan`
  evidence, reserve fixed nonmoving `StackLease`s, validate preservation and
  cancellation conformances, transfer arguments transactionally, lower
  park/resume, and implement the suspension-safe-loan subset. Current Unit,
  scalar, and acyclic conditional shapes retain exact frame/link/temporary,
  call, crash-terminal, and target-generated division-diamond evidence from
  instruction selection through decoded installation and artifact-wide closure
  composition. One depth-independent conditional-tree carrier accounts nested
  decisions and mutually exclusive source-distributed convergence calls.
  One bounded Boolean carrier additionally accounts ordered actual
  unconditional native join branches plus the final fallthrough into one
  affine-cleanup tail. Extend that accounting to general shared native joins
  and general affine cleanup rather than claiming convergence from duplicated
  leaves.
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

  The accepted front half carries exact erased terms through resolved, typed,
  checked, call/transition, and finite-state definite-assignment paths. It
  supports positional input lanes, forwarding, concrete subjectless producer
  selection, normalized direct/inherited requirement rows, and exact generic
  evidence interfaces. Terminal Psi retains canonical term declarations,
  requires/ensures lanes, public output-field names, opaque member projections,
  and separate producer provenance; codec and verification reject identity,
  interface, lane, field-name, row, producer, and orphan drift. The detailed
  accepted carrier is stated in
  [`law_bearing_relations_and_quotients.md`](wiki/design_briefs/law_bearing_relations_and_quotients.md).

  The immediate generated-output-package rung now destructures the complete
  nonempty set of unconditional evidence fields from a concrete zero-input
  checked machine. Source field order may vary; checked and terminal Psi
  canonicalize by callee lane, mint one distinct fresh caller-local term per
  field, and require each term to be forwarded exactly once. A proof-only call
  remains fully erased. A scalar-result call additionally requires exactly one
  contextual `value` field, synthesizes one ordinary caller local/call, and
  links the grouped proof row to that exact canonical terminal call operation;
  proof metadata adds no runtime work or fuel beyond the ordinary call.

  Retained/projection and guarded complete-package forms are design-blocked on
  `OWNER_QUESTIONS.md` Q14. Generic package application is design-blocked on
  Q12. Explicit-discard packages are design-blocked on Q13.
  Keep proposition, evidence-term, and provenance identities separate; neither
  provenance nor display spelling is a term identity oracle.
- Finish generic conformance instantiation and explicit binders. The declaration
  front half now parses `Name<Telescope>: [Subject] satisfies Trait { ... }`,
  retains lifetime/type/const/static-machine parameters through resolved and
  typed Psi, resolves its contracts and trait arguments in that name-owned
  scope, and gives every named conformance a package-scoped symbol. Machine
  telescopes retain a distinct proof-static `Evidence: Subject satisfies Trait`
  binder with its own lexical symbol. A concrete call now binds the exact named
  closed conformance, validates its instantiated subject/trait shape, exposes
  direct and inherited requirements in the binder scope, substitutes the
  selected normalized rows, and commits the map identity separately from
  callable static-machine arguments. Still instantiate generic conformance
  declarations over their own telescopes; the call-site application form and
  permitted inference are design-blocked on `OWNER_QUESTIONS.md` Q12. Nested
  generic calls already forward the exact evidence selection through
  specialization. Identity retains declared name, telescope, optional subject,
  instantiated trait, and normalized rows. No visibility-, priority-, or
  specificity-based selection.
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

These are pointers to the owning question or open design item, not duplicate
specifications:

- **EXTERNAL-ENTRY-STACK-DOMAIN:** owner Q11.
- **FIXED-OPERATOR-SURFACE-BINDING:** owner Q1.
- **UEFI-PHYSICAL-SEMANTIC-ENTRY-COMPOSITION:** owner Q2.
- **SEALED-LOCAL-CAPACITY-SOURCE-FORM:** owner Q3.
- **WRITE-ONLY-MEMORY-VIEW:** owner Q4.
- **CALLBACK-PARAMETER-REQUIREMENT:** owner Q5.
- **SUM-MATERIALIZATION:** tagged-case placement vocabulary in
  `wiki/language_guide/appendix_open_questions.md`.
- **ATOMIC-EVENT-MODEL:** portable atomic axioms and target refinement choices
  in `wiki/language_guide/appendix_open_questions.md`.
- **CHECKED-RESULT-ARITHMETIC:** public carrier ruling for failure-returning
  checked arithmetic.
- **TRAPPING-CONTRACT-ARITHMETIC:** owner Q10.
- **IMPORTED-CRASH-CAPSULES:** realization/import/certificate identity in
  `wiki/language_guide/appendix_open_questions.md`.
- **NATIVE-LOGICAL-FUEL-METERING:** owner Q6.
- **PCC-VERIFIER-CLOSURE:** owner Q7.
- **PLACED-ERASED-EVIDENCE-ESTABLISHMENT:** owner Q8.
- **PROVIDER-NEUTRAL-INTERRUPT-ACKNOWLEDGEMENT:** owner Q9.
- **GENERIC-CONFORMANCE-APPLICATION:** owner Q12.
- **EVIDENCE-PACKAGE-DISCARD:** owner Q13.
- **GENERATED-EVIDENCE-OUTPUT-PACKAGES:** owner Q14.

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
