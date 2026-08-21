# Tasks

Last pruned: 2026-08-17.

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

  The function spine now retains a sealed compiler-private identity from
  abstract operations through target/assigned operations, machine
  instructions, encoded bytes, and object-entry selection. `Source(StateKey)`
  and `ProgramStorageEntryWrapper(continuation StateKey)` are distinct: object
  planning selects one exact symbol/identity pair, and synthetic wrapper gates
  retain the source continuation's symbol and text interval separately rather
  than relabeling it. Duplicate, redirected, missing-continuation, and
  interval-drift claims reject. Object planning now publishes every encoded
  function through a compiler-private identity-to-text-symbol linkage table;
  duplicate identities, canonical link-name collisions, overlapping or out-of-
  bounds intervals, and tampered table bindings fail closed. A future wrapper
  call relocation can therefore target the exact retained source continuation
  without rediscovering it by name. For the currently admitted
  `ProgramStorageApplication`/`ImageAndInitialStorage` schema, the bridge also
  retains an address-free wrapper transfer plan that maps both physical root
  ordinals to their exact source-visible parameter, frame byte range, and
  disjoint capture-instruction rows, plus free-versus-borrowed activation-loan
  receiver behavior. It deliberately does not call the physical arrival plan a
  source-call ABI. The retained bridge plan also owns a sealed platform
  executor gate: only the exact selected physical-provider
  installation and mapped, zeroed receiver activation can construct its
  borrowed continuation handoff, and the executor runs before that activation
  is finished. This gate intentionally does not claim that native bytes
  executed. The main backend now also owns a generic compiler-private direct-
  call operation from abstract operations through target/assigned operations,
  machine instructions, real x86-64 `call rel32` / AArch64 `bl imm26`
  placeholders, exact-identity object relocations, and final-image byte,
  relocation, and footprint replay. Missing, duplicate, redirected, wrong-
  width, and opcode-tampered call claims fail closed. No production builder
  emits that operation yet, and it deliberately carries no invented argument
  or receiver placement. Production builds still emit and select
  `Source(entry StateKey)`. The native bridge now retains an address-free
  outbound continuation ABI, distinct from the physical arrival
  `BoundaryEntryPlan`. The bridge separately retains the exact checked
  source declaration signature captured before typed ownership moves into the
  backend: target slot, machine/state symbols and names, canonical normalized
  callable identity, Unit result, free-versus-mutable receiver identity, and
  ordered receiver-excluded visible parameter type/mode/shape rows. Those facts
  are rechecked against the exact lowered continuation, selected slot, arrival
  parameter identities, and checked receiver layout. For the currently
  admitted UEFI x86-64 schema only, the compiler-private Microsoft x64 policy now derives
  one complete `CallPlan` over the optional receiver followed by Image and
  InitialStorage, with Unit result, and validates every placement against the
  sealed declaration shape. A future SysV/AAPCS schema remains fenced until the
  structural classification graph is retained. The production executor gate
  binds the attached receiver placement to the exact mapped address and live
  activation loan; identity, shape, alignment, and loan-length drift reject.
  The free form has a complete layout but no production executor traversal
  because the current gate requires receiver activation. The ABI plan carries
  no runtime root value, `Extent`, root authority, wrapper body, emitted call,
  or callee inbound realization. After activation ends, installed roots can now
  move into a sealed authority-disposition carrier that revalidates the exact
  initial-storage geometry, lineage, rights, provenance, mapping era, origin,
  receiver selection, and complete partition coverage. A receiver-free
  disposition may release Image and whole InitialStorage as two owned root
  authorities. An attached disposition keeps the receiver's
  `OwnedExtentPartition` intact, exposes its potentially noncontiguous before
  and after residuals only by borrow, and fails closed while returning the
  intact carrier if asked for two whole roots. This is not an outbound source
  argument: it cannot move residual authority during the live receiver loan or
  make two separated remainders satisfy one `Extent in Granted` formal. The
  receiver-free whole-root form can now bind to the exact emitted bridge's
  retained free continuation ABI. That sealed carrier owns both `Extent`
  authorities and retains ordered Image/InitialStorage declaration indices,
  call indices, nominal identities, shapes, and address-free placements;
  bridge-binding, target-slot, source-continuation, callable, role/order,
  type/shape/placement, receiver, and Unit-result drift reject while returning
  the intact authority. It does not materialize operand bytes, populate
  registers or stack, emit the call edge, or claim native execution. Attached
  and zero-sized-receiver forms remain deliberately excluded. The
  receiver-free path now has one sealed transition from the recorded
  production installation through validated root disposition and whole-root
  authority into that argument carrier. A borrowed preflight rejects binding,
  source/ABI, receiver, role/order, type/shape, or placement drift without
  consuming the recording; fail-closed errors after ownership starts moving
  retain the highest successfully constructed authority carrier for recovery.
  This linkage still creates no runtime operand bytes, wrapper body, call edge,
  or native-execution evidence. The selected source signature now also retains
  the exact checked `Extent` record graph—`base: addr` at byte 0 and
  `length: u64` at byte 8—and replays its data/field symbols, names, primitive
  types, offsets, aggregate size/alignment, and absence of alternate storage
  encodings against the backend layout. The receiver-free argument carrier can
  move into one sealed non-clone logical-value carrier that keeps Image and
  InitialStorage authority intact while binding their exact base/length
  observations to those declaration and call rows. Structural, role/index,
  target-layout, or wrapping-geometry drift returns the intact prior carrier.
  These are logical values only: no bytes, registers, stack locations, wrapper
  body, call, or execution evidence is produced.
  Those receiver-free logical values can now move into a sealed indirect-
  operand image carrier for the exact admitted UEFI/Microsoft x64 ABI. It
  retains little-endian `{base,length}` bytes beside each immutable
  `ValuePlacement`, requires Image through RCX with caller copy `32..48` and
  InitialStorage through RDX with caller copy `48..64`, and rejects role,
  index, field-layout, shape, pointer, range, size, alignment, overlap, or
  target drift while returning the intact authority-bearing logical carrier.
  The byte images are geometry, not authority. This slice deliberately does
  not allocate or write the caller-copy stack area, populate RCX/RDX, emit a
  wrapper body or call edge, realize the callee inbound ABI, claim native
  execution, or admit attached/zero-sized-receiver entries.
  Production builds therefore still lack a source-compatible attached-root
  value/authority carrier (or separate hidden supply), generated wrapper body,
  and source-function inbound realization; defining that disposition at the
  source schema, emitting the body, physically realizing receiver-free root
  values, adding the exact native call edge to the retained operation, and
  retaining resulting native bridge evidence remain before this slice is
  complete.

  The CLI corpus is rooted on all hosted targets except the four GUI samples,
  which currently select Windows x64 and macOS arm64. Linux needs an ordinary
  source-level `Gui`/`Input` provider plus its general call/result realization;
  that is engineering work, not a language-design blocker. Proof-only and
  deliberately trapping fixtures remain targetless. Final firmware composition
  of `ImageHandle`/`SystemTable` inputs with semantic roots is design-blocked on
  owner Q2; the remaining physical bridge and corpus work is not. The native
  differential RUN corpus now routes every host-authored fixture through
  production entry selection (including bounded outer-job/single-worker native
  compiles) instead of silently retaining the legacy test-entry seam. Eight
  result-as-process-exit probes now keep their value-returning logic in ordinary
  helpers while target-rooted Unit entries consume those results through the
  explicit exit provider; that migration also closed named unsigned-conversion
  signedness and logical-NOT helper-result lowering gaps. Six additional
  residual scalar-result and host-deployable probes now use the same authored
  four-host root and Unit-entry discipline. Eight indexed-array and slice-loop
  native probes now also route their existing Unit entries through authored
  four-host roots without weakening their bounds, mutation, or conversion
  regression shapes. Nine further indexed-access, mutable-slice, subslice, and
  two-pointer native probes retain their exact regression programs while using
  the same authored production roots. Ten direct/dispatched slice reads,
  element copies, frame aliases, and bounded or dynamic subslice probes now
  likewise compile and run only through authored production roots. The tracked
  nested-window, parameter-subslice, runtime-end, and descriptor-pointer probes
  add ten more unchanged Unit-entry programs to that rooted native cohort.
  Eight linear ownership handoff, transfer, and transparent-record frontier
  fixtures now preserve their ownership and transition programs in direct Unit
  entries with explicit exit providers. Ten named float provider and conversion
  matrices now also use authored roots for native and cross-target differential
  execution. Ten indexed string-concat, bounded-carrier, slice-alias, and guard
  probes now consume the same checked-in four-host roots in their native and
  cross-target artifact tests. Ten further fixed-index, pointee, mutable-
  parameter, copied-struct, and lookup-driven text probes now use authored
  roots in their native and cross-target artifact coverage. Ten array
  reduction, indexed-write, indexed-guard, and stack-algorithm probes now also
  run their unchanged Unit entries through authored four-host roots. Four
  nested-loop/index probes and six dependent range, ordering, subtraction, and
  product-index probes likewise retain their exact programs under authored
  four-host roots. Ten dungeon reentry, Boolean/ordered dispatch, and
  string-field lookup regressions now also run their unchanged Unit entries
  through authored roots. Eight atomic operation probes and two structural
  dispatch/nested-field probes now share authored roots across native execution
  and the existing AArch64 opcode checks. Ten aggregate construction, nested-
  field, and value-copy probes now likewise run unchanged Unit entries through
  authored roots. Ten call-result, machine-owned storage, sum-payload, and
  subslice-window probes now likewise use authored production roots. Ten text-
  storage, string-reference, room-dispatch, and tuple-matrix probes now also
  use authored production roots. Ten domain-membership, address-value, finite-
  matrix, and static generic-dispatch probes now likewise consume authored
  roots. The tracked corpus audit leaves 67
  legacy fixtures without an authored `build.omg` root.
  Continue migrating those fixtures through production entry
  selection; replace result-as-process-exit probes with ordinary Unit entries
  and explicit exit providers rather than preserving the legacy entry seam.
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
  identity retained as zero-runtime metadata. The remaining work is real
  authorized introduction, custody exit, residual geometry, and provider binding—not
  another passthrough representation.
- **INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION.** Implement the settled
  domain-route/installation model without a provision declaration. A
  content-bearing domain remains the sole source-level authority for one exact
  requirement. Its statically enumerable installed parameter positions may
  introduce fresh program-local lineages; ordinary calls and result routes with
  no parent lineage reject. Reconstruct exact per-occurrence capacity from the
  requirement instance, qualification, and owner-unique `Content<A>` projection,
  including owner-constrained const families. Join that schema to finite slot
  cardinality and lifecycle epoch during installation verification and derive,
  rather than trust, the aggregate for one installed artifact instance.

  Migrate the current `ExtentCompilerProvisioning`/`sealed_declaration`
  implementation carrier to route-position, capacity-schema, occurrence,
  cardinality, and epoch identities. Preserve provider issuance as a distinct
  admitted origin. Add source, terminal, artifact, and installation canaries for
  a one-root introduction, a finite multi-instance aggregate, an ordinary-call
  mint attempt, an unbounded installation shape, understated producer totals,
  cross-origin composition, stale epoch replay, and coexistence-peak reporting.
  A shared cap is one aggregate parent root divided among children; another
  child without supply rejects. Cross-epoch limits require persistent authority.
- **BOUNDARY-ISSUANCE** (after conservation): derive invocation geometry from
  parameters, entry places, and results. Keep ownership, issuance, custody,
  aliasing, and partition succession distinct. Providers may attest custody,
  never computable interval arithmetic.
- Under **TR3-TR8**, finish routed task claims, stack authority, cancellation,
  and transactional custody. Deferred acknowledgements lease the interrupt root
  and controller configuration; reconfiguration drains them.

Acceptance: reconstructed carriers mint no authority; every introduced content
claim traces to a verifier-reconstructed installed program-local occurrence or
admitted provider issuance; artifact-instance aggregates are derived for an
exact epoch and Cathedral can compose coexistence peaks; external effects have
an exact root-to-provider backing chain;
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
  completely before mutation. Fully specialized generic records now participate
  through their synthesized concrete `CheckedShape` symbol and substituted
  member types, including specializations nested under fixed-array wrappers;
  spelling is never layout authority, and distinct specializations retain
  distinct symbols and widths.
  Erased terms remain semantically mandatory but add no bytes, including nested
  records and fixed arrays whose entire runtime shape is erased. Scalar
  placement/access semantics remain fenced for aggregates. Continue beyond
  this fixed subset. Sum materialization is design-blocked on the unsettled
  tagged-case placement vocabulary.

#### L6b — `AccessPlan` and `Placed<P, T>`

- Implement the settled borrowed/owned `Placed<P, T>` establishment and
  retirement model from `Extent in Granted`, using ordinary subrange borrows
  and no source-visible admission intermediate. Add the distinct core `view`,
  `initialize`, and `validate` operations; provider-specific adopt/open wrappers
  establish their external domains before `view`. Derive unconditional
  non-runtime Type inputs by canonical declaration path, keep proposition
  inputs and outputs in the `;` proof lanes, reject generic case-dependent Type
  custody, and emit the canonical per-outcome disposition table. The Type-only
  rejection payload returns exactly the formal inputs marked `returned`;
  embedded inputs become retirement debt and consumed inputs cite their exact
  consumer. Finish owned destruction/move-out evidence before returning
  `Granted & Vacant`.
- Derive readable, destructive-read, writable, and atomic field accessors while
  keeping logical extents distinct from whole-transfer footprints. Enforce
  total decode/encode, exact provider width/alignment, and operation-specific
  atomic laws. Continue rejecting External initialization, multi-transfer
  reads, and synthesized RMW.
- Keep alias-exclusion admission separate from access rights; `&mut` does not
  claim exclusivity against a device. Sealed primitive events now specialize
  linearly into Stable read/take/write/swap, External read/take/write, or one
  exact Atomic operation and ordering while preserving the original authority
  on pre-event rejection. Carry the settled address-free placed-occurrence,
  resident-claim, loan, mapping/revision, exact footprint, and boundary-reach
  identities through Terminal Psi, installation, the interpreter, and both
  native backends without replaying source layout. Emit claim-local
  introduction, forwarding, transformation, exit, and loan rows.
- Implement `Extent::Resident<P, T>` as the owned exact-range dormant-content
  qualification, including invariant type indices, mutual exclusion with
  `Vacant`, split/merge rejection, borrow versus owned-view continuity,
  resident-preserving retirement, partial-view retirement fences, and explicit
  migration through `Vacant`. Carry non-runtime custody in the resident claim.
  The concrete foundation now seals a provider-issued nonzero
  `ResidentClaimId` into dormant owned Stable content; explicit view consumes
  that carrier into a fresh nonzero `PlacedOccurrenceId`, field/access/lowering
  requests retain both identities, and resident-preserving retirement returns
  the same claim and receipts for a later fresh view. Ordinary borrowed views
  retain neither identity. Source-visible domain establishment, borrowed
  resident loans, `Vacant` transitions, partial moves, Terminal propagation,
  and installation remain.
- Complete the atomic 2x2 compare-exchange family: existing observing strong
  and weak forms require copyable residents; new non-observing strong and weak
  forms return the proposal on failure and may transfer affine or linear
  custody using one copyable comparison key and exact selected encoding law.
- Close generic `ResidentContentTransfer<P, T>` applications at final
  composition from concrete and symbolic artifact demand, verify one selected
  provider covers the reconstructed application set, and bind exact issuance
  occurrences at installation. Do not create a slot per monomorph.
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
  A finite nonempty same-source-carrier exact-divide/remainder chain may now
  feed the same partial cast when verifier-owned toward-zero division and
  dividend-sign remainder interval-hull replay maps the full source carrier
  wholly inside the target. Every arithmetic prefix and the cast retain
  independent evidence; guard-sensitive nonconvex preimages remain fenced.
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
  The same direct partial-cast root may now feed a finite nonempty
  left-associated same-target-carrier exact-divide/remainder chain. Every
  prefix retains independent evidence derived only from its own landed safe
  divisor; cast evidence, prior operation proofs, value definitions, and
  quotient/remainder algebra supply no authority.
  The direct-root and post-cast exact-divide/remainder chain families now share
  one runtime-divisor widening when at least one right sibling is a direct
  same-carrier machine parameter. Every runtime divisor retains an independent
  positive or at-most-`-2` proposition. The joint signed `-1` exception remains
  restricted to the first direct-root operation when its dividend bound is
  independently available; computed and post-cast dividends import no prior
  proof authority. Literal-only chains remain on their existing paths.
  One direct machine-parameter root may now feed any finite left-associated
  same-carrier chain containing both exact-left and exact-right shifts, with
  independently landed heterogeneous legal counts. Every left prefix maps its
  carrier-tight safe interval backward through all prior canonical mixed-shift
  definitions, intersecting the carrier after each inverse left or right step;
  every right proof remains its own legal-count proposition. No prior shift
  proof supplies authority, so later right shifts cannot erase unsafe prefixes.
  The same finite mixed exact-shift chain may now feed one validator-legal
  partial fixed-native exact cast. The cast starts from the target/source
  carrier intersection and independently maps that interval backward through
  every ordered mixed-shift definition; mathematical emptiness reconstructs
  falsehood, while checked interval-arithmetic failure admits nothing. Every
  shift prefix and the cast retain separate mandatory evidence.
  Conversely, one validator-legal direct partial fixed-native cast may root a
  finite nonempty same-target-carrier chain containing both exact-left and
  exact-right shifts. Each left prefix independently replays the ordered
  canonical post-cast definitions back to the cast, intersects its surviving
  target interval with the source carrier, and reconstructs source-root bounds;
  the cast and every shift retain separate evidence. Mathematical emptiness is
  falsehood, while checked transfer failure admits no family.
  A unified finite exact-arithmetic prefix may now feed a finite exact-shift
  suffix on the same fixed-native carrier when the arithmetic prefix is a
  left-associated add/subtract/nonnegative-multiply literal chain and the
  suffix contains at least one exact-left shift. Every left prefix maps its
  safe interval backward through prior shifts and the checked affine form
  `A * root + B`; every arithmetic operation, count, and left prefix retains
  independent evidence. `A == 0` decides only the current left obligation,
  mathematical emptiness is falsehood, and checked replay failure admits no
  family.
  Conversely, a finite exact-shift prefix may now feed a finite exact-
  arithmetic suffix on the same fixed-native carrier. Each arithmetic prefix
  maps the carrier backward through checked `A * shifted_root + B`, then
  replays the complete ordered shift prefix to the direct root. Every count,
  left overflow, and arithmetic obligation remains independently mandatory;
  `A == 0` decides only the current proposition after full shape validation,
  mathematical emptiness is falsehood, and checked replay failure admits no
  family.
  A unified finite affine/cast/affine sandwich may now cross one validator-legal
  partial fixed-native exact cast. Both sides are nonempty left-associated
  add/subtract/nonnegative-multiply literal chains. The cast independently maps
  its target/source interval through the checked source form; every target
  prefix maps the target carrier through its own checked form, intersects with
  the source carrier, then maps through the complete source form. Every source
  arithmetic, cast, and target arithmetic obligation remains independently
  mandatory. A zero coefficient on either side decides only the current
  proposition after full ordered shape validation; mathematical emptiness is
  falsehood and checked replay failure admits no family.
  A unified finite exact-shift/cast/exact-shift sandwich may likewise cross one
  validator-legal partial fixed-native cast, with nonempty left-associated
  shift chains on both sides and independently landed heterogeneous legal
  counts. Every source shift, cast, and target shift keeps separate mandatory
  evidence. Each target-left prefix replays its target definitions to the cast,
  intersects the surviving interval with the source carrier, then replays the
  complete source chain to the direct parameter. Mathematical emptiness is
  falsehood; checked transfer failure admits no family.
  The two heterogeneous affine/shift cast sandwiches are retained as one
  consolidated family. A nonempty source affine chain may cross one partial
  fixed-native exact cast into a nonempty target shift chain, or a nonempty
  source shift chain may cross the cast into a nonempty target affine chain.
  Each side uses its established landed-literal/count rules and ordered
  canonical replay. Every source operation, the cast, and every target
  operation keeps separate mandatory evidence; zero coefficients decide only
  the current obligation after full shape validation. Mathematical emptiness
  is falsehood, while checked composition or interval-transfer failure admits
  no family. Empty-sided shapes remain on their narrower existing paths.
  A consolidated divide/remainder cross-cast family now covers all four
  compositions between one nonempty landed-literal exact-divide/remainder
  chain and one nonempty affine or shift chain. When divide/remainder precedes
  the cast, the existing carrier-total quotient/remainder hull must fit the
  target carrier; each target prefix is true when that full hull lies inside
  its reconstructed safe interval, false when disjoint, and otherwise remains
  unadmitted rather than inventing a guard-sensitive nonconvex preimage. In the
  converse direction, the source affine or shift chain and cast reconstruct by
  their existing rules while every target divisor proof stays independent.
  Every source operation, cast, and target operation retains separate evidence.
  The corresponding direct same-carrier family now retains all four nonempty
  divide/remainder-to-affine/shift compositions without a cast. A leading
  divide/remainder chain supplies its complete verifier-owned carrier hull to
  each following affine or left-shift safe interval: containment is true,
  disjointness is false, and partial overlap remains unadmitted. In the
  converse direction, affine or shift proofs use their established direct-root
  reconstruction while each following divide/remainder proof depends only on
  its own landed safe divisor. Every operation retains independent evidence.
  A finite nonempty exact-divide/remainder chain may also cross one
  validator-legal partial fixed-native exact cast into another finite nonempty
  exact-divide/remainder chain. The cast replays the complete carrier-total
  source hull and is admitted only when that hull wholly fits the target
  carrier; it does not manufacture a partial-overlap or falsehood case. Every
  source divisor proposition, the cast, and every target divisor proposition
  keeps separate mandatory evidence, and each target operation depends only on
  its own independently landed safe divisor.
  The three homogeneous exact-multiply placements—direct, feeding one partial
  exact cast, and rooted at one direct partial exact cast—also admit finite
  signed-carrier chains containing at least one negative independently landed
  factor. A checked sign/magnitude cumulative product reverses negative
  preimages, handles the signed minimum without host negation, and keeps zero
  local to only the current proposition. Every multiply prefix and cast
  remains independently mandatory; mathematical emptiness is falsehood while
  checked product or interval failure admits no family.
  A separate signed-affine family now covers three placements: a direct
  signed-carrier chain, the same chain feeding one partial fixed-native exact
  cast, and one direct partial cast feeding the chain. Each chain is finite,
  nonempty, left-associated, rooted at one direct machine parameter, contains
  at least one add/subtract offset and at least one negative multiply, and has
  an independently landed same-carrier literal on every right edge. The
  verifier replays every shrinking prefix as checked sign/magnitude
  `A * root + B`; a negative `A` reverses the interval preimage, `MIN` never
  uses host negation, and `A == 0` decides only the current obligation. Every
  arithmetic prefix and cast retains independent evidence. Mathematical empty
  preimages are falsehood, while checked coefficient, offset, division, or
  interval failure admits no family. Homogeneous signed products,
  nonnegative-affine chains, two-sided sandwiches, and conversion-chain forms
  remain on their existing or fenced paths.
  A consolidated two-sided signed-affine sandwich now crosses exactly one
  validator-legal partial cast between signed fixed-native carriers. The
  source and target are finite nonempty left-associated add/subtract/multiply
  chains with independently landed same-carrier signed literals. Either the
  source itself contains an offset and a negative multiply, permitting any
  target affine prefix, or the source remains on the established nonnegative
  affine algebra and the current target prefix contains both an offset and a
  negative multiply. The verifier replays checked sign/magnitude `(As, Bs)`
  and `(At, Bt)`, reverses either negative preimage, intersects the exact cast
  carriers, and reconstructs only the current target obligation. Zero on
  either side remains local after full shape validation; every source prefix,
  cast, and target prefix keeps separate evidence. Mathematical emptiness is
  falsehood, while checked coefficient, offset, division, or interval failure
  admits no family. The all-nonnegative sandwich, one-sided signed-affine and
  homogeneous signed-product paths, thin product/offset permutations, and
  conversion-spine forms retain their existing priority or fence.
  A finite chain of at least two validator-legal partial fixed-native exact
  casts may now start at one direct integer machine parameter. For each cast
  prefix, the verifier follows only ordered shrinking cast definitions and
  intersects the root carrier with every carrier reached so far. The resulting
  canonical root bounds prove only that cast; every earlier cast retains its
  own mandatory evidence. A mathematical empty intersection is falsehood,
  while malformed definitions or carrier reconstruction failure admit no
  family.
  The same finite cast core may now follow one nonempty already-admitted
  computed prefix: landed-literal affine arithmetic (including the homogeneous
  signed-product path), an exact-shift chain, or a carrier-total exact-
  divide/remainder chain. For every cast prefix, the verifier intersects every
  carrier reached so far and reuses only that computed family's verifier-owned
  inverse algebra to reconstruct the direct root. Every computed-prefix and
  cast obligation remains independently evidenced. Empty affine/product/shift
  preimages are falsehood, checked replay failure admits no family, and the
  divide/remainder hull remains admissible only by complete containment.
  Conversely, a finite chain of at least two partial exact casts may feed one
  nonempty already-admitted target-carrier affine, homogeneous signed-product,
  exact-shift, or landed-safe-literal divide/remainder suffix. The verifier
  first validates and intersects the complete ordered cast chain without
  importing any cast evidence, then reuses only the selected post-cast
  family's existing inverse algebra for the current suffix prefix. Every cast
  and suffix operation remains independently evidenced; mathematical empty
  preimages are falsehood and checked replay failure admits no family.
  The two directions compose into one unified nonempty computed-prefix,
  at-least-two-partial-cast, nonempty computed-suffix sandwich across the same
  affine, homogeneous signed-product, exact-shift, and carrier-total landed-
  divisor families. Each source prefix, each shrinking cast prefix, and each
  target prefix is reconstructed independently from ordered canonical
  definitions. The verifier intersects every cast carrier, applies only the
  selected target inverse and source inverse/hull algebra, and never imports
  another operation's evidence. Zero coefficients remain local to the current
  target obligation; a mathematical empty interval is falsehood, while
  malformed shape or checked transfer failure admits no family.
  A separate wider-arithmetic composition now permits one nonempty admitted
  computed prefix to pass through a finite nonempty chain of strict valid
  fixed-native integer widenings and feed one nonempty admitted computed
  suffix. Both sides independently select affine, homogeneous signed-product,
  exact-shift, or landed-safe-literal divide/remainder algebra. Every widening
  definition is retained and validated in order; each target interval pulls
  back through numeric-identity widening by intersecting the source carrier,
  then reuses only the selected source inverse or carrier-total hull. Every
  exact operation retains independent evidence. Mathematical emptiness is
  falsehood, divide/remainder partial overlap and checked replay failure admit
  no family, and zero coefficients remain local to the current obligation.
  A heterogeneous conversion-spine sandwich now composes the same nonempty
  computed prefixes and suffixes across a finite contiguous word containing at
  least one strict valid fixed-native integer widening and at least one
  validator-legal partial fixed-native exact cast. Every adjacent carrier and
  shrinking definition is validated in order. Each cast prefix independently
  intersects all preceding conversion carriers and replays only the selected
  source inverse or complete-hull algebra; each target prefix walks the entire
  conversion word before the same source replay. Widenings remain retained
  numeric-identity operations without invented evidence, while every source
  operation, partial cast, and target operation keeps separate evidence. Pure
  widening, pure cast, one-edge, direct, and narrower sandwich shapes retain
  their existing dispatch priority. Mathematical emptiness is falsehood,
  source divide/remainder casts require complete hull containment without a
  partial or falsehood admission, and checked replay failure admits no family.
  A same-root affine fork/join now admits one exact add or subtract whose two
  operands are disjoint, nonempty, independently admitted landed-literal
  affine branches over the same fixed-native carrier and the exact same direct
  machine parameter. The verifier replays each branch separately as checked
  sign/magnitude `Al * root + Bl` and `Ar * root + Br`, then reconstructs the
  join from `(Al + Ar, Bl + Br)` or `(Al - Ar, Bl - Br)`. A zero combined
  coefficient decides only the join after both complete ordered branch walks;
  every operation in both branches and the join retains separate evidence.
  Mathematical empty preimages are falsehood, while checked coefficient,
  offset, division, or definition-walk failure admits no family. The branch
  definition walks must be disjoint and source ordered apart from their common
  root. Distinct-root joins, one empty side, outer operations other than add or
  subtract, conversions, runtime siblings, locals, members, calls, effects,
  and stale or redirected definitions remain fenced.
  A distinct-root signature-bounded affine fork/join separately admits the
  same outer exact add or subtract when the two disjoint, nonempty, ordered
  landed-literal affine branches end at different direct machine parameters
  of one fixed-native carrier. For each root, the verifier selects only the
  tightest landed unary lower and upper bounds appended by its signature,
  intersects them with the carrier, and maps the resulting interval forward
  through that branch's checked signed affine form. The join range is the
  Minkowski sum or difference of the two independent branch ranges. Complete
  containment in the join carrier yields the canonical conjunction of the
  selected root bounds; a wholly disjoint range yields falsehood; partial
  overlap admits no family. Relational cross-root premises, absent or
  one-sided unary bounds, shared roots, overlapping or reordered branch walks,
  computed roots, carrier drift, conversions, and unchecked arithmetic remain
  fenced. Every operation in both branches and the join retains independent
  evidence.
  A same-root signature-bounded signed affine quadratic product join now
  admits one outer exact multiply whose two disjoint, nonempty, ordered
  landed-literal affine branches end at the same direct signed fixed-native
  parameter with nonzero coefficients. The verifier selects the tightest
  landed unary lower and upper signature bounds, composes the correlated
  integer quadratic, and evaluates its exact discrete range at both endpoints
  and the in-range floor/ceiling lattice points adjacent to the rational
  vertex. Complete carrier containment yields the canonical two-bound
  conjunction; a wholly disjoint range yields falsehood; partial overlap or
  checked coefficient, vertex, or evaluation failure admits no family. Every
  branch operation and the outer multiply retains separate evidence. Constant
  collapse, distinct roots, relational premises, one-sided bounds, unsigned
  carriers, malformed walks, computed roots, conversions, and stale evidence
  remain fenced.
  A same-root signature-bounded signed affine divide/remainder safety join now
  admits one outer exact divide or remainder whose two disjoint, nonempty,
  ordered landed-literal affine branches end at the same direct signed
  fixed-native parameter with nonzero coefficients. The verifier selects the
  tightest landed unary lower and upper signature bounds and solves the exact
  integer-lattice equations for divisor zero and divisor `-1`. A `-1` root is
  forbidden only when the correlated dividend evaluates to the carrier
  minimum at that exact root. No forbidden root emits the canonical two-bound
  conjunction; forbidden roots covering the whole integer interval emit
  falsehood; a partially unsafe interval or checked equation/evaluation
  failure admits no family. Every branch operation and the outer divide or
  remainder retains separate evidence. Distinct roots, constant collapse,
  relational premises, one-sided bounds, unsigned carriers, malformed walks,
  computed roots, conversions, and stale evidence remain fenced.
  A distinct-root signature-bounded signed affine product join now admits one
  outer exact multiply whose two disjoint, nonempty, ordered landed-literal
  affine branches end at different direct signed fixed-native parameters. The
  verifier requires and selects the tightest landed unary lower and upper
  signature bounds for both roots, maps each interval forward through its
  checked signed affine form, and takes the exact hull of all four rectangle
  corner products. Complete carrier containment yields the canonical
  four-bound conjunction; a wholly disjoint hull yields falsehood; partial
  overlap or checked corner overflow admits no family. Every branch operation
  and the outer multiply retains separate evidence. Same-root quadratic
  correlation, relational premises, one-sided bounds, unsigned carriers,
  overlapping walks, computed roots, conversions, and stale definitions remain
  fenced.

  Next engineering frontiers are other proof-bearing results feeding another
  proof-bearing operation, wider multivariate and other computed-sibling joins,
  other computed exact-cast and wider exact-arithmetic
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
  premises. Genuinely empty-record equality normalizes to the existing Boolean
  constant carrier through calls and terminal verification; all-erased records
  remain distinct and fenced. The distinct `addr` carrier is also explicitly
  excluded from both direct structural-member predicates and whole-record leaf
  expansion, and Terminal lowering rejects the retained source contract rather
  than encoding address equality as fixed-integer evidence. Built-in IEEE
  `f32`/`f64` equality now retains one atomic, format-annotated proposition per
  relevant structural leaf, including whole-record expansion and projected-call
  rebasing. Direct structural-field `!=` uses the same atomic carrier with an
  explicit comparison kind. The verifier resolves both exact paths and formats
  independently; the carrier preserves IEEE NaN and signed-zero behavior rather
  than laundering either operator through mathematical `Equal`. Whole-record
  float `!=` canonically negates the already-sorted equality conjunction as
  `P -> Falsehood`; projected calls rebase every leaf below that implication.
  Aggregate equality now also retains byte-sequence fields as one content atom
  over two nonempty canonical structural paths. The checked and Terminal
  carriers distinguish borrowed views from bounded owned storage (including
  the exact owned capacity) without admitting native pointer/descriptor layout
  into semantic identity; equality itself is live length plus the exact live
  byte prefix, never pointer, capacity, or unused-byte equality. Both roots are
  independently resolved and rebased through structural calls. Borrowed
  `&[u8] in Domain` and bounded `[u8; N] in Domain` fields participate in
  synthesized `Equatable` record equality, while literals and direct text `!=`
  remain outside this slice. Payload-less sums now retain a closed structural
  sum shape with exact case identities. Intrinsic `==` lowers to the flat
  canonical conjunction of both membership implications for every declared
  case; `!=` is that equality proposition implying falsehood. The verifier
  independently resolves both structural subjects and every case identity.
  Payload-bearing pure sums now retain exact case-payload field identities and
  direct relevant Boolean, fixed-integer, IEEE, and byte-sequence leaf types.
  Their intrinsic `==` is the canonical disjunction of per-case conjunctions:
  matching membership for both roots plus that case's exact payload-leaf
  equalities. `!=` is that complete equality proposition implying falsehood.
  Unknown or redirected case/field identities reject independently. Mixed
  common-field/case shapes, nested or recursive payload expansion, address and
  erased payload equality, and runtime sum layout remain fenced. Semantic codec
  v18, proof-bundle v12, and installation-record v24 retain the structural
  shapes, case-payload paths, and proposition. Continue with the fenced mixed,
  nested, recursive, and erased aggregate cases. Concrete machine/state
  contracts plus domain/data predicates, trait invariants and signatures,
  machine-parameter requirements, and root/domain operator contracts now reject
  direct binary and named-float `Trapping` arithmetic plus direct Trapping
  conversions. Comparisons, bitwise inspection, float classification,
  Wrapping/Saturating operations, and non-reserved custom float calls remain
  total; proof expressions do not create crash sites. Wrapping/Saturating
  division and remainder now form in concrete and direct abstract Prop only
  when an independently accepted prior fact proves the divisor interval
  nonzero; carrier-overflow policy does not define division by zero, and the
  fact containing a partial term cannot justify that term's own formation.
  Exact and Saturating shifts in direct abstract Prop now retain the same
  independently-prior `[0, operand_width)` count obligation already enforced
  for concrete machine/state contracts; `Saturating` defines value overflow,
  not an invalid count, while `Wrapping` continues to define every count by
  modulo reduction. A count bound inside the proposition containing the shift
  cannot authorize that shift's formation.
  Exact division and remainder in concrete machine/state and direct abstract
  Prop now retain the catalog's complete primitive-definedness judgment: an
  independently accepted prior fact must exclude a zero divisor and, for
  signed carriers, the `MIN / -1` primitive pair (including remainder's shared
  hardware-definedness edge). A guard in the proposition containing the
  operation supplies no authority for that operation's own formation.
  Finish the settled total-specification arithmetic slice: retain explicit
  fixed-integer/address embeddings into proof `Int` with their derived carrier-
  range facts. This remains blocked on a real vocabulary dependency: the
  shipped core `embed` is a transitional ordinary machine returning structural
  `Nat`, while typed primitive expressions and Terminal `ScalarTerm` have no
  unbounded proof-`Int` embedding that can retain and independently verify the
  source carrier identity/range; do not substitute dead checked evidence.
  Explicit same-carrier policy-erasure `as` coercions now retain the
  ordinary Exact representability obligation in concrete machine/state Prop
  and across the direct abstract signature form; only independently accepted
  prior `requires` facts discharge it, so the proposition containing an
  operation cannot justify that operation's own formation. Add the per-
  primitive Exact, Wrapping, Saturating, and Trapping denotation bridges;
  compiler-derived Trapping guards remain executable crash-site facts rather
  than predicate effects. Imported crash capsules remain blocked on artifact
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
- **PCC-CANONICAL-SEMANTIC-LEDGER.** Replace the current trusted Rust fusion of
  artifact traversal and algebraic reduction with the settled two-part closure.
  A total low-rung generator consumes canonical terminal-Psi bytes, validates
  the exact structure, directly denotes each primitive operation, and emits one
  ordered canonical ledger of goals plus local premise introductions. Clever
  interval, affine, shift, quadratic, and divide/remainder analysis becomes an
  untrusted certificate producer that proves the unchanged canonical goal.

  First expose the migration state honestly: add exact trust-graph nodes for the
  Rust decoder/verifier, each sufficient-form reduction family, the ledger
  framework, each unproved leaf denotation schema, and each unproved call-
  composition row. Every dependency must resolve
  through an acyclic graph to a registered root with kind, semantic subject,
  digest/version, owner, scope, rationale, and accepting policy; unknown leaves
  reject. Do not encode an uncertified reducer as an admitted program premise.

  Current migration inventory: the canonical proof synopsis now publishes one
  validated source-bound trust graph for the exact Rust decoder, proof kernel,
  verifier, eight sufficient-form reduction families, the current unproved
  ledger framework, 35 closed leaf-schema rows, and three separate call-
  composition rows covering all 38 `OperationKind` variants. Node
  digests bind the exact deciding Rust/specification bytes and explicit
  versions; the graph identity also binds every canonical dependency edge.
  Unknown, cyclic, unreachable, duplicate, malformed-root, and noncanonical
  graphs reject, and the current artifact closure reports `fully-derived false`.
  The first production Rust ledger slice now has one closed
  `psi-terminal-semantics` table covering all 38 operation kinds and preserving
  the 35 leaf / 3 call-composition custody split. Twenty goal-free scalar leaves
  carry explicit result, operand, denotation, goal, fact, crash, fuel, and
  frontier axes and reconstruct their local equations through one generic
  interpreter. Exact lookup rejects missing or duplicate rows. The terminal
  verifier and codec trust graph consume that shared inventory instead of
  maintaining independent operation matches. Structural/effect rows and
  call/control composition remain separate and are not promoted into the
  goal-free scalar table. The second production Rust ledger slice
  now mirrors Gamma's separate exact-unique three-row structural/effect table:
  Boolean field reads, port writes, and trivial affine-local establishment keep
  result, custody, action, external-effect, fuel, and place-frontier axes
  explicit. One generic interpreter emits distinct fact, effect, or frontier
  observations; the verifier consumes its Boolean equation instead of
  reconstructing that row independently. The trust graph consumes the same
  table as 32 direct-denotation plus three structural/effect nodes while
  preserving the 35 leaf / three call-composition operation-custody split.
  The modular verifier source split is also fully rebound into trust identities:
  evidence provenance, integer foundations, proof-bundle custody,
  reconstruction, and substitution bytes can no longer change outside the
  registered verifier/ledger dependency digests.
  The third production Rust ledger slice now mirrors Gamma's exact-unique
  three-row call-composition table. Scalar, structural Unit, and boundary calls
  retain independent target, result, argument, requirement, transfer, outcome,
  crash-route, evidence-lifetime, fuel, and frontier policies. The verifier's
  contract composition moved out of general operation reconstruction into one
  focused table-selected module; existing module validation still proves the
  concrete signature, movement, coverage, substitution, outcome, crash, and
  evidence invariants before composition. Call policy and implementation bytes
  are both bound into the same three call trust nodes.
  The fourth production Rust ledger slice now owns the twelve proof-bearing
  scalar leaves in a separate exact-unique table. Exact cast, left/right shift,
  exact add/subtract/multiply, and exact/wrapping/saturating divide/remainder
  retain declared result and operand shape, direct denotation, one of six
  canonical goal shapes, normal-successor result equation, crash policy, fuel,
  and frontier policy as independent axes. One generic interpreter emits a
  typed canonical-goal carrier and the post-discharge result equation; malformed
  type/row custody rejects before reduction. Artifact reconstruction consumes
  that observation instead of rebuilding twelve result equations. The current
  sufficient-form algorithms remain trusted migration dependencies and select a
  reduced proposition through one isolated dispatcher; neither the table nor
  the dispatcher falsely claims a kernel derivation of the canonical goal. The
  trust graph binds the table to exactly those twelve denotation nodes and the
  dispatcher to every affected reducer.
  The bounded Gamma spike is complete. It canonical-decodes four exact current
  `PSITERM\0` v18 fixtures and audits a 54-row scalar ledger covering constants,
  Boolean not/equality, integer equality/order, bitwise operations, strict
  i8-to-i16 widening, partial i16-to-i8 exact cast, exact/wrapping shifts with
  independently typed counts, and the complete
  exact/wrapping/saturating add/subtract/multiply and divide/remainder cohorts,
  signed toward-zero division, `MIN / -1`, conditional
  equations, branch-local scope/invalidation, all-predecessor merge rejection
  and acceptance, exact call-clause enumeration/substitution, and strict
  justification ranks. A separate 3-row structural/effect ledger covers exact
  relevant-Boolean field custody, affine-local establishment and retirement,
  published port-service authority, the observable port-write effect, and the
  three distinct place-frontier policies. Matching/asymmetric/malformed cases
  agree between the Beta-written reference interpreter and the independent
  Python evaluator.
  The 1,983-byte fixture yields a 3,607-byte modeled ledger and 2,984-byte
  prospective certificate; the 695-byte structural/effect fixture yields a
  185-byte modeled ledger and 164-byte prospective certificate. A separate
  697-byte fixture canonical-decodes exact `CallUnit` and `BoundaryCall`
  custody, including qualified affine resources, structural requirements,
  claim transfer, completion receipt, and boundary identity. The assembled
  typed core is 4,982 lines / 198,971 bytes / 423 functions, with maximum
  source nesting 25. Its PSITERM-neutral byte cursor, checked
  `u8`/little-endian `u16`/`u32`, and exact low/high-half `u64` primitives are
  now a separately gated 109-line reusable layer, including exact unsigned
  `u64` order and an exact four-limb `u128` carrier. A separately gated
  592-line terminal-codec layer owns the exact current
  magic/format/vocabulary envelope
  plus canonical Boolean, optional and required full-width semantic-ID carriers,
  exact identity equality/order, and length-prefixed UTF-8 grammar, together
  with the complete Boolean/fixed-signed/
  fixed-unsigned/address scalar-type grammar and exact widths `1..=128`, plus
  exact signed/unsigned 128-bit integer-value payloads; it
  rejects header/scalar/type/value drift plus overlong, surrogate, out-of-range,
  isolated-continuation, and truncated encodings. Its separate v18 structural-
  leaf module additionally owns exact IEEE kind/format, byte-sequence carrier,
  full-width canonical paths with exact case segments, and atomic proposition
  tags `11`/`12`/`13`, including nonempty paths and canonical operand order. All
  three bounded decoders consume
  only the 302-line header/scalar/type/value subset; the structural-leaf module
  remains independently gated and outside their claimed semantics. Scalar
  declarations and boundary results now retain
  the complete decoded type grammar; the bounded operation rows still admit
  only Boolean/i8/i16. Integer-constant operations retain exact signed/unsigned
  128-bit payloads until that row policy selects and narrows signed i8. The
  bounded spike narrows identities to a zero high half only in explicit adapters
  after complete decoding;
  remaining recursive vocabulary and monomorphic type-specific results remain
  spike-owned. The
  bounded thirty-two-kind scalar leaf slice now resolves through five composed,
  exact-unique policy-cohort schema tables: each row owns result shape,
  denotation, goal, post-discharge fact, crash policy, fuel, and frontier
  behavior, while calls remain separate coverage/substitution algebra.
  Missing, duplicate, and altered table rows reject end to end without changing
  either canonical ledger. The generator's known-value environment now
  retains exact typed declarations rather than IDs alone: duplicate result
  identities, operand-type drift, duplicate declarations, join-parameter
  overlap, and call argument-type drift reject before row publication. The
  structurally owned `EstablishTrivialAffineLocal` and
  `BooleanStructuralField` plus effectful `PortWrite` now resolve through their
  own exact-unique schema table and separate decoder/evaluator modules rather
  than scalar-row permutations. Erased relevance, field/service/port drift,
  cleanup drift, establishment-target drift, and missing affine retirement all
  reject. The three call-composition definitions now live in their own
  exact-unique table and one generic axis checker rather than three more
  evaluator branches. Target/result custody, positional binder shape,
  requirement coverage, capture-free substitution, claim/receipt transfer,
  guarded outcomes, crash routes, evidence lifetime, fuel, and frontier policy
  remain independently visible. The canonical scalar call consumes its row end
  to end; canonical-byte Unit and boundary sites exercise the same checker.
  Missing,
  duplicate, cross-kind, weakened-evidence, wrong-requirement,
  weakened-frontier, signature, state-version, move/reborrow, coverage,
  substitution, outcome, crash, evidence-lifetime, raw identity, target,
  argument, transfer, receipt, truncation, and trailing-byte drift reject. The
  first Rust producer-modularity checkpoint is also complete. Structural Unit
  planning no longer owns the shared Boolean/integer convergence classifier
  body: that sufficient-form family and its forty focused tests live in
  dedicated `shared_convergence` modules. The six exact binary families and
  exact-cast family select their existing ordered recognizers through seven
  declarative registries and one generic dispatch path rather than repeated
  `or_else` permutations. The production parent shrank from 10,926 to 5,626
  lines; shared convergence is now a 493-line orchestration module plus four
  responsibility modules for cast chains, affine forms, products/divisors, and
  shifts/cross-family composition, none larger than 1,317 lines. The test
  parent shrank from 10,785 to 2,915 lines, and its forty classifier cases are
  separated into chain, affine-join, and nominal-cleanup modules, none larger
  than 3,411 lines. The remaining structural parent is now a 250-line
  orchestrator over return analysis, control/boundary construction, cleanup,
  call closure, and type/shape custody modules, none larger than 1,356 lines;
  its fifty-three tests are separated into cleanup and call-closure modules
  behind a 57-line test root. The downstream checked-to-terminal producer no
  longer embeds its 3,891-line shared runtime-parameter classifier in the
  23,735-line crate root. That classifier now has a 706-line orchestration and
  registry module over Boolean, conversion, affine, product/divisor, and
  shift/cross-family responsibilities, none larger than 1,083 lines. Six exact
  binary cohorts plus exact cast consume named ordered registries through two
  generic dispatchers rather than maintaining another set of repeated
  `or_else` permutations. Structural scalar-return custody is now separate as
  a 553-line lowering/orchestration module over a 258-line expression-shape
  responsibility and a 1,431-line nominal-cleanup specialization behind one
  parent-facing entry point. The distinct structural Unit control path is a
  601-line module. Structural Unit cleanup is a 733-line nominal
  lowering/orchestration module over separate 828-line ordered-nominal and
  352-line partial-affine responsibilities. Attached Unit closure assembly is
  an 826-line orchestrator over 132-line provider discovery, 203-line exact
  call closure, 487-line type/domain/service catalog, and 168-line parameter
  transfer responsibilities. Result-bearing boundary custody and general
  structural-result transfer are separate 393-line and 314-line modules over a
  shared 246-line structural-type retention responsibility. Scalar-graph
  terminal-module assembly is now a separate 1,061-line responsibility behind
  one parent-facing builder. Content conservation, identity reshuffling, and
  partition composition now form one 788-line lowering module with only three
  public APIs and two explicit internal contracts. The root-level regression
  corpus is now a separately compiled 334-line fixture/orchestration parent over
  isolated 767-line Unit-cleanup, 179-line scalar-graph, 506-line content-ledger,
  957-line structural-control, 457-line attached-Unit, and 852-line
  structural-return families instead of a second responsibility embedded in
  production. The 9,597-line `nominal_affine_source` regression file is now a
  31-line root over five focused responsibilities with all 33 tests retained;
  its remaining 6,238-line integer-comparison case is one atomic cross-layer
  mutation matrix, not a completed decomposition target. Proposition
  vocabulary, evidence-term identity, contract lanes,
  proof-output invocations, and producer provenance now form one 906-line evidence
  module behind a single lower-and-install API. Scalar and structural crash
  routes, checked crash-site/frontier custody, argument-root substitution, and
  canonical proposition construction now form one 1,727-line module with
  eleven explicit internal contracts.
  Terminal operation emission and proof finalization now form one 597-line
  module with five explicit entry points. Short-circuit Boolean decisions and
  terminal control emission now form one 734-line module, while replaceable
  debug-map presentation is a separate 188-line module. Scalar-graph
  preparation, validation, partial evaluation, and lowering now form one
  1,297-line module with fourteen explicit internal contracts. Reachable
  scalar-call discovery and multi-machine assembly now form one 158-line
  module with two explicit entry points. The crate root is now 1,017 lines.
  The verifier's former 9,239-line sufficient-form reconstruction test parent
  is now a 15-line root over fifteen cast, conversion, add/subtract,
  multiply/affine, join, shift, and divide-policy responsibilities. All 76
  cases remain, and no family module exceeds 1,248 lines.
  Terminal proof replay now has the same production boundary. Its former
  2,233-line root is a 256-line public verification orchestrator over a
  44-line canonical proof-bundle model, 1,036-line executable-site/path-fact
  reconstruction, exact evidence-producer provenance (139), integer arithmetic
  foundations (337), and proposition/value/place substitution (494). The
  existing public proof and substitution surfaces remain explicitly
  re-exported, while sufficient-form selection retains its specialized owners
  instead of being merged into one generic permutation dispatcher.
  Exact-shift reduction now follows that same boundary. Its former 2,376-line
  production file is a 237-line precedence/orchestration parent over a
  944-line direct-chain/foundation responsibility and a 1,254-line
  cross-family cast/arithmetic/divide composition responsibility. The existing
  public reducer surface and precedence are unchanged, and the integer-shift
  trust node binds all three exact source files.
  Exact conversion reduction now has the same split. Its former 2,219-line
  file is a 243-line cast-precedence/direct-fallback parent over a 977-line
  conversion-chain and interval-foundation responsibility and a 1,063-line
  divide/product/affine/offset composition responsibility. Existing reducer
  contracts and ordering remain unchanged, and the integer-conversion trust
  node binds every implementation source.
  The checked-lowering regression file that had accumulated ranking,
  operational-contract, write-frame, crash-route, and data-fact verification
  is now a 23-line root over eight exact test families. All 67 tests and the
  shared exact-symbol helper remain, and no family exceeds 3,614 lines.
  Checked-tree visualization has also separated view production from its
  regression corpus: the former 11,465-line file is now a 5,092-line
  production module with a 609-line shared fixture parent over eleven exact
  behavior, content, qualification, carry, and machine-contract test families.
  All 188 embedded tests and 215 test/helper functions remain, and no family
  module exceeds 1,043 lines.
  The checked interpreter now follows the same responsibility boundary. Its
  former 9,938-line evaluator is a 1,205-line state/model parent over separate
  execution, statement/call, wire-codec, host-dispatch, filesystem, console,
  expression/value-call, name/place, cast/recast, record-view, type-metadata,
  scalar-operation, and typed-program-lookup modules. The complete function
  and declaration inventories remain; cross-responsibility collaboration is
  narrowly exposed through `pub(super)`, local helpers remain private, and no
  child responsibility exceeds 1,408 lines. This is a semantics-preserving
  split; exact host-service grant custody remains a separate authority task
  rather than being hidden inside the refactor.
  Profiling the differential corpus also ruled out a wholesale Arena-to-
  `PagedArena` migration as a concurrency fix: `PagedArena` provides stable
  paged storage, not concurrent mutation, and the existing sound parallel
  pattern remains worker-local `Arena`s followed by deterministic ordered
  merge. Checked lowering now builds one call-frame/incoming-guard index for
  all range, contract, crash, and multiplicity consumers, reducing the
  helper-rich Mandelbrot canary's checked phase from about 15.6s to 9.5s.
  Backend state-value planning now builds the separate exact value-call
  dependency closure required before pruning. Runtime-flow and required-call
  states seed canonical `(machine symbol, state symbol)` identities; the index
  transitively visits local initializers, transition guards and values,
  terminal expressions, and nested call receivers and arguments through the
  simplifier's shared symbol-based resolver. Known-state resolution ambiguity
  conservatively retains the full program. Collection omits only states outside
  that closure: `StateValueUse.required` remains an independent emission fact,
  and the `runtime_nested_named_conversion_alias_exit` regression still exits
  70 with its off-flow nested value-machine expansion retained.
  On the exact warmed Mandelbrot stress canary, state-value planning fell from
  the documented 27.9s to 2.062ms while state storage took 1.360ms and the
  complete backend plan took 30.451ms. A full artifact-producing compile took
  7.470s, of which checked lowering was 7.207s; the exact one-canary
  interpreter/native differential run took 21.29s end to end, down from the
  documented 69.4s profile, with identical output and exit status. The
  dependency slice preserves worker-local `Arena`s and deterministic ordered
  merge; neither `PagedArena` nor cross-worker mutation is involved. The old
  hotspot remains useful history: a 10-second sampled profile landed in the
  `simplify_call_expression` / `helper_state_model` recursion; its two hottest
  leaf stacks are source-provenance `Arc` clone and drop (2,639 and 2,629
  samples), reflecting repeated reconstruction of expression trees and their
  identifiers. Prefer memoized normalized helper models or an indexed
  expression recipe over changing the backing arena. A linear structural
  helper-model cache was prototyped and rejected: on the exact warmed
  `text/runtime_mandelbrot_render_exit` differential canary, disabling it took
  60.21s wall/385.84s aggregate CPU while enabling it took 75.44s wall/76.77s
  CPU. It removed duplicated parallel work but moved the critical path onto one
  worker doing linear cache scans, increasing latency by 25%. No cache code was
  retained; the next attempt needs an indexed canonical key/recipe.
  Two later sampled Stage-05 fixes keep that rule. Default-domain analysis now
  builds one invocation-local call-frame resolver instead of reconstructing it
  for every fixpoint state visit, reducing the warmed checked phase from 6.754s
  to 5.338–5.386s. Named result-overload resolution now builds one source-
  ordered machine-family index keyed by exact normalized path and parameters,
  with direct entry-symbol lookup; operator and trait semantics are unchanged
  and the index is not retained. On the same canary that reduced Stage 05 again
  to 4.743–4.817s and full compile wall time to 5.51–5.64s.
  Checked-fact loop-invariant analysis now also reuses that pass's existing
  immutable call-frame resolver instead of rebuilding it below indexed-access
  checking. On a fresh exact profile this removed the former 1,056-sample
  construction stack and reduced warmed Mandelbrot Stage 05 from 4.810s to
  3.421s, with byte-identical output and native/interpreter exit 70.
  A subsequent eager complete-state write-summary prototype was rejected rather
  than retained: against a fresh 3.505s warmed baseline it moved 170 samples
  into resolver construction while targeting a 130-sample recursive summary
  stack, and regressed Stage 05 to 3.606–3.648s. Output remained byte-identical,
  but the remaining cyclic summaries need a genuinely lazy memo or SCC/fixpoint
  design; no cache code remains.
  Corpus-level bounded parallelism is viable at the harness boundary: the
  differential runner now defaults to four independent jobs with one native
  backend worker each, retains deterministic corpus-order reporting, and
  exposes `DIFF_JOBS`, `DIFF_LIMIT`, and exact `DIFF_CANARY` controls. On this
  14-core host the first eight canaries fell from 8.30s to 4.00s; a 32-canary
  concurrency probe passed completely, while eight outer jobs improved only
  101s to 97s over four and therefore is not the default. The native leg now
  selects output-only artifact emission because it consumes only the certified
  executable, not pipeline viewers or diagnostic reports; the same cached
  eight-canary probe fell again from 4.00s to 2.86s with all pairs matching.
  Semantic validation, trust policy, and final-footprint certification remain
  enabled. The disposable `omega-run` probe now follows that same policy while
  preserving full reports under explicit `--keep`. On the exact warmed
  Mandelbrot compile, suppressing reports that were immediately deleted reduced
  median wall/CPU from 4.36s/4.43s to 4.05s/4.12s and retired instructions from
  73.4B to 67.6B; a small unary-entry probe fell from 0.05s/654M instructions to
  0.02s/292M. `--keep` still produced the complete 35-file, 1.8 MiB inspection
  directory, and native/interpreter results remained identical. The native leg
  uses each original source's authored target-owned `ProgramEntry` when present
  and the bounded legacy `Main::main` seam only for the remaining unrooted
  corpus; the former generated target wrapper discarded value-returning entry
  codes and produced false mismatches.
  The compiler pass/fail corpus umbrellas now use the same deterministic outer
  scheduler for checked-only, cross-target, rooted-target, Windows-host, and
  active backend members instead of leaving the large backend registries
  serial. Each no-output backend compile defaults to two inner workers, with
  `OMEGA_CANARY_JOBS` and `OMEGA_CANARY_INNER_WORKERS` retained as explicit
  profiling controls. On an eight-program heavyweight probe, four outer jobs
  and two inner workers reduced wall time from the one-outer/one-inner 65.42s
  to 31.12s. The dominating float total-order canary fell from 58.61s with one
  inner worker to 29.63s with two; four and fourteen inner workers provided no
  material wall improvement (29.11s and 29.28s) while increasing aggregate CPU
  from 78.29s to 112.16s and 226.53s. This measured ceiling is why the harness
  does not inherit unrestricted host parallelism inside every outer job. After
  repairing the corpus drift this broader gate exposed, the complete active
  pass umbrella finishes in 234.92s and the complete active fail umbrella in
  21.05s on the same host; both collect the whole registry rather than stopping
  at the first failure. Dedicated native-canary helpers now use the same
  two-worker ceiling instead of multiplying Rust test-thread concurrency by a
  host-wide compiler pool. The exact float total-order test fell from 128.30s
  wall/998.98s aggregate CPU with fourteen inner workers to 91.83s/256.84s with
  two; production compiler defaults remain unchanged.
  The compiler canary integration suite is no longer a 48,301-line permutation
  file. Its shared compile helpers, exact corpus registries, and umbrella
  orchestration now form a 3,277-line root over twenty-one responsibility
  modules for target artifacts, reports, content, ranges, arithmetic, providers,
  calls, ABI, proofs, layouts, and runtime families. All 1,241 tests and 1,272
  functions remain; the sole cross-family float differential helper is imported
  explicitly, and no family module exceeds 3,795 lines.
  Development and test profiles now both omit full DWARF by default, with an
  explicit `CARGO_PROFILE_{DEV,TEST}_DEBUG=2` escape hatch for debugger
  sessions. On the same macOS host, rebuilding the development CLI after the
  profile change reduced the executable from 140,687,560 to 118,904,896 bytes;
  the semantic canaries remained 0.01-second work once compiled. This reduces
  codegen/link and artifact-I/O pressure without weakening compiler diagnostics
  or semantic validation.
  A later apparent Rust frontend regression was build-cache accumulation, not
  compiler execution: `target/debug/deps` had grown to 1,359,819 entries and a
  rustc sample spent its startup in `SearchPath::new`/`readdir` before parsing
  the crate. `cargo clean` removed 1,483,970 derived files (195.1 GiB). The same
  proof-codec target then fell from 58.8s incremental to 2.86s cold and 0.68s
  after touching `psi-core`. Treat a uniform per-crate pre-parse delay as Cargo
  cache hygiene first; it is not evidence for Arena concurrency, test sharding,
  linker changes, or disabled semantic gates.
  The real-source terminal-Psi differential suite now applies the same boundary:
  its former 10,520-line file is an 852-line artifact/native execution harness
  over ten contract, call/control, exact-arithmetic, scalar-operation, and
  crash/admission families. All 115 tests and 137 functions remain, and no
  family exceeds 2,030 lines.
  Terminal native machine emission has undergone the same split: its
  12,922-line crate root is now an 891-line production orchestrator with the
  complete 58-case, 5,028-line regression corpus compiled separately.
  Unit-body and calling-policy emission, per-target parameter homes, aggregate
  argument staging/copying, and Unit stack/fuel/effect evidence form a separate
  1,301-line responsibility. Scalar-return and Boolean-control cleanup,
  nominal-cleanup admission, exact residual partitioning, and cleanup
  stack/fuel/call evidence form a separate 1,120-line responsibility. Scalar
  control/expression emission now has a 31-line orchestration/re-export root
  over distinct 1,861-line x86-64 encoding, 1,775-line AArch64 encoding, and
  1,067-line shared conditional-shape/stack-evidence responsibilities. All
  eighty-five implementation functions retain their exact architecture or
  shared owner, and the parent-facing surface remains explicit rather than
  becoming one permutation dispatcher.
  Terminal-module validation has begun the same split: its parent shrank from
  7,498 to 282 lines, with structural/service foundation (956 lines),
  structural/boundary operation custody (822), public error vocabulary (803),
  structural ownership/frontier cleanup (750), per-machine
  registration/orchestration (716), scalar crash/frontier and Boolean-predicate
  custody (674),
  content-conservation validation/replay (534), operation operand/type custody
  (522), partial/nominal affine cleanup custody (473), evidence/proposition
  custody (410), control-flow/dominance validation (301), proposition-root
  projection (146), contract proposition scope (120), and call-graph acyclicity
  (68) in separate responsibilities. Public validation types remain
  re-exported from the crate boundary.
  Final-image validation has begun the same responsibility split: its parent is
  down from 22,945 to 712 lines. Its regression corpus is a 25-line root over
  separate 701-line final-validation, 1,197-line place-replay, and 1,037-line
  guard/assembly families instead of a second responsibility embedded in
  production. Imported-call replay now has a 1,335-line parent, while table and
  vtable indirect calls form a separate 549-line responsibility. Runtime
  byte/line/text-boundary replay is a separate 504-line responsibility,
  and syscall replay plus exact relocation-target derivation is a separate
  507-line responsibility. Compiler footprint
  derivation now has a 509-line composition/partition parent over a declarative
  four-family registry: 249-line control/entry, 621-line storage/place,
  866-line outbound-call, and 512-line buffer/wire/text responsibilities. A
  separate 1,547-line
  module owns assembly footprints, operand-loader semantics, exact instruction
  bytes, and retained relocation checks behind two parent entry points; and a
  1,598-line module owns exact compiler relocation sets, symbol custody, and
  unchanged instruction-bit validation. Compiler atomic-operation replay and
  recursive runtime-operand storage-site derivation now form a separate
  752-line responsibility. The closed place-copy shape vocabulary and exact
  classifier form a 1,218-line responsibility; indexed and pointee offset
  decomposition is a separate 946-line responsibility. Place-pair and
  place-copy shapes map to exact
  architecture-specific relocation sites in a separate 505-line module. The
  closed place-write shape vocabulary and its exact classifier family form a
  separate 304-line responsibility. Retained place-write encoding plus exact
  register and relocation-site derivation form a separate 1,039-line
  responsibility. The closed compiler instruction-relocation recipe vocabulary
  and exact final-byte/site replay form a separate 1,539-line responsibility.
  Exhaustive expected-byte, class, position, and relocation-recipe
  reconstruction now has a 55-line specification-family dispatcher behind a
  single typed entry point; fixed mechanics, guards, return transport, and
  entry and dispatch transport form a separate 477-line family, while compiler atomics,
  place copies and writes, and storage results form a separate 1,083-line
  family. Imported calls, runtime I/O, indirect calls, and syscalls form a
  separate 858-line family. Bit fields, bounded buffers, wire encoding, and
  text materialization form a separate 1,480-line family. Binary arithmetic
  and scalar conversion writes form a separate 478-line family. The separate
  native-refinement lane now applies the same engineering boundary to x86-64
  byte encoding: the public root is down from 19,412 to 89 lines and
  re-exports 106-line function-frame, 591-line entry/result ABI,
  662-line privileged-effect, 578-line Linux-syscall, and 760-line atomic
  responsibilities with their focused byte/width tests. Compact Binary wire
  append/read, scalar, byte-slice, nested, repeated-field, predicate, and UTF-8
  encodings now form a separate 1,880-line responsibility. Stored/literal text
  append, materialization/comparison, Win64/Linux line reads, and bounded text
  carriers form a separate 1,580-line responsibility. Generic host dispatch,
  authored imports, normalized Win64/System V argument and result placement,
  direct/vtable/table calls, byte I/O, and exact relocation-site replay form a
  separate 4,399-line production responsibility; its 2,005-line ABI regression
  corpus is separately compiled. Runtime value comparison, operand replay,
  binary arithmetic, conversion, and text equality form a separate 4,340-line
  scalar responsibility; integer/bit-field/indexed place writes and copy-layout
  contracts form a separate 675-line responsibility. Their 652-line arithmetic
  and conversion regression corpus is separately compiled. Dispatch-loop,
  case-entry, state-write, case-leave, and static-guard encoding now form a
  separate 176-line responsibility. Shared register moves, loads/stores,
  displacement checks, copy-chunk iteration, and atomic byte helpers form one
  explicit 1,114-line crate-internal primitive layer. These
  are semantics-preserving responsibility splits, not trust promotions: the
  full low generator, row proofs, and composition bridges remain open, and no
  trust-graph node becomes derived from the spike. The corresponding AArch64
  cleanup has begun: its former 14,216-line runtime-storage parent is now an
  810-line address/load/store orchestration parent. Runtime operand replay,
  text equality, integer and floating arithmetic, arithmetic-domain policy,
  classification, and their exact width contracts form a separate 2,172-line
  runtime-value responsibility. Atomic load/store, read-modify-write, ordering,
  result-site, and width policy form a separate 697-line responsibility, and
  scalar conversion, placement, trap, saturation, and width policy form a
  separate 746-line responsibility. Direct place-pair, place-value,
  computed-value, register, machine-state, and exact failure-branch comparison
  contracts form a separate 356-line responsibility. Recursive-operand
  register/state contracts plus
  immediate integer, bit-field, direct binary, pointee binary, saturation, and
  trapping writes form a separate 1,048-line scalar-write responsibility.
  Direct, pointee, indexed, and double-indexed bounded-buffer writes plus
  literal and source appends form a separate 935-line responsibility. Direct,
  pointee, indexed, and double-indexed string-descriptor writes plus their
  register/state ceilings form a separate 392-line responsibility. Direct,
  pointee, frame-indexed, machine-indexed, and double-indexed place-address
  writes plus exact clobber/state ceilings form a separate 440-line
  responsibility. Descriptor, pointee, frame, and machine single- and
  double-indexed integer/binary writes form a separate 897-line responsibility.
  Direct, pointee, single-/double-indexed, cross-region, and indexed-pair copy
  encoders plus exact chunk and clobber contracts form a separate 2,597-line
  responsibility. The 3,361-line byte/width/policy regression corpus remains
  separately compiled, and the exact public, function, and test inventories
  remain preserved.

  Define a closed typed schema language with no opaque callbacks. One row per
  leaf operation owns well-formedness, direct mathematical denotation, canonical
  goals, post-discharge facts, crash behavior, and local fuel/frontier effects.
  Missing operation rows reject mechanically. A change to control, validity,
  effect, or frontier machinery is a visible ledger-algebra revision rather than
  an ordinary row addition. Schema and artifact identities pin the exact state
  model, mathematical definitions, operational clauses, and semantics version.

  Before committing the full low implementation, build a Gamma spike that
  canonical-decodes bytes and covers Exact and Wrapping arithmetic, signed
  divide/remainder with toward-zero behavior and `MIN / -1`, one conditional
  result equation, one branch-local premise, an asymmetric join that rejects,
  the positive all-predecessor merge dual, exact call-requirement enumeration
  and substitution, justification ranking, dominance, and invalidation. Measure
  Gamma/specification size, audit complexity, Beta-reference runtime and memory,
  ledger size, and prospective reconstruction-certificate size. Difficulty does
  not weaken the endpoint; an inability to express the total definition cleanly
  triggers a rung-design correction.

  The production ledger records premise origin, prerequisites, establishment
  point, value/place versions, validity scope, invalidating events, and an
  acyclic logical-justification rank. Rank prevents circular evidence but does
  not replace dominance and all-path availability. Ordinary merge evidence is
  acyclic and requires valid matching facts on every predecessor; cyclic control
  requires invariant establishment and preservation. Partial-operation result
  equations become available only on the proved normal successor. Calls check
  clause coverage separately from capture-free positional instantiation across
  arity, binder kinds/types, state versions, moves/reborrows, outcome guards,
  crash routes, and evidence lifetimes.

  Establish every deployed ledger by direct low-rung evaluation or a low-kernel-
  checked derivation of the same total definition. Rust agreement is a
  differential oracle whose disagreement rejects and whose agreement grants no
  authority. Convert reduction families incrementally: a converted family emits
  a certificate; an unconverted family remains an exact versioned trusted-
  judgment dependency.

  Prove separate composition bridges. Safety/partial correctness combines
  exhaustive derivation, sound schema rows, valid premises, and checked goals.
  Progress/total correctness combines well-founded measures, per-edge descent,
  complete SCC/call closure, and explicit environmental progress premises. Fuel
  is sponsor scheduling and discharges neither. Row proofs are universally
  quantified low-rung metatheory, with derived status computed from an accepted
  proof and exact dependencies. Conservative semantic extensions need checked
  transport; relevant changes require reproof, while old artifacts retain their
  pinned semantics identity. Native ISA/hardware refinement remains a separate
  trust closure.

  Acceptance: byte mutation, omitted sites, extra premises, stale versions,
  one-arm-only join facts, post-write stale facts, circular justification,
  wrong call substitution, premature result equations, altered schema rows,
  unknown roots, and changed semantic dependencies all reject or change the
  recorded closure. A reducer cannot replace the canonical goal with its
  sufficient preimage. An artifact report lists every remaining trusted
  implementation/row and cannot appear fully derived until both applicable row
  proofs and the relevant global composition bridge are accepted. A Psi-hosted
  kernel port alone emits no ledger and supplies no reconstruction assurance.
- **IRFUEL.** Extend entry/segment certificates to loops and build-time use;
  the generic terminal inspection path now independently verifies a selected
  source closure and publishes its recomputed acyclic entry certificate, with
  Cathedral's first timer root pinning that evidence. Add attributed response
  outcomes only when terminal wait/foreign edges can derive them. Inserted native
  metering must consume the installed exact-site
  attribution rows, but is design-blocked on the sponsor counter, exhaustion
  transfer, and resumable continuation ABI in owner Q3. Keep WCET and wall-clock
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
  unresolved-machine-parameter shapes keep failing closed. Carry the settled
  `Placed<P, T>` non-runtime-field input paths and per-outcome dispositions
  through checked and terminal representations. Relevance does not invent a
  runtime carrier or public ABI for otherwise non-layoutable types.
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
- **WRITE-ONLY-BORROW — implement the settled `&write T` access mode.** Parse
  and resolve the third borrow kind; preserve its exclusive loan and restricted
  operation set through type checking, reborrows, projections, calls, provider
  selection, canonical plans, Terminal Psi, both execution engines, native ABI
  lowering, and diagnostics. Permit explicit `&mut T` attenuation only. Reject
  observation, readable reborrows, take/swap/read-modify-write, content-driven
  projection, non-discardable displacement, and invariant restoration that
  depends on reading the referent. Retain exact per-outcome write footprints so
  untouched ranges and their facts survive. Checked implementations prove the
  restriction over their call closure; opaque providers publish an admitted,
  implementation-pinned non-observation judgment. Migrate byte-output boundary
  surfaces only after the borrow kind is executable; do not reinterpret
  `&write` as vacant storage or typed construction.

#### ENT4 — registered callbacks

- **CALLBACK-PARAMETER-REQUIREMENT — implement the settled nominal binder.**
  Parse and resolve `where machine Selected satisfies Trait::requirement`,
  deriving the complete callable contract from one uniquely resolved
  requirement row. Centralize that exact resolver for domain route lists and
  every other signature-free requirement site. Reject overloaded paths,
  structural coincidence, and visible-unique selection. Retain a checked
  per-use row with call site, static-machine ordinal, selected
  machine/satisfaction row, exact requirement overload, separate published and
  actual envelopes plus their refinement proof, and the target thunk-placement
  recipe. Emit the private
  relocation only from validated binding lowering. Registration is linear,
  explicitly unregisters, retains required code/component leases, and keeps
  selected identity in provenance without importing narrower facts unless an
  API contract forwards them. Add declaration-side compatibility reporting
  when a new overload makes signature-free references ambiguous, with pass/fail
  canaries covering both callback binders and domain routes.

  The declaration and admission slice is implemented. Syntax, resolved, and
  typed trees retain a discriminated structural-or-nominal contract; nominal
  binders normalize to one exact trait/requirement symbol pair through the
  shared signature-free resolver. Selection requires one explicit satisfaction
  row for that exact requirement, rejects structural coincidence and a row for
  another trait, and keeps nominal and structural specializations distinct in
  template identity. Checked-only filesystem pass/fail canaries now pin unique
  and overloaded signature-free paths for both nominal callback binders and
  authored domain routes. Declaration-side compatibility reporting is also
  implemented: after symbols are assigned and before authored paths are
  normalized, one diagnostic names each overloaded declaring-trait family and
  source-ordered diagnostics name every affected nominal binder or domain
  route. The checked identity spine is also implemented:
  every admitted nominal use retains its exact statement/expression site,
  static-machine ordinal, registration operation, selected machine and entry,
  unique satisfaction trait/requirement, and canonical requirement-overload
  identity. Validation captures that authority before specialization consumes
  the authored arguments and again after each fixed-point cloning round, while
  structural machine parameters publish no nominal row. Each row now also
  pins the normalized published-requirement contract identity separately from
  the selected machine's normalized declared contract identity and retains an
  explicit admission-refinement receipt binding those endpoints. Requirement
  capsules now also retain canonical published service reach and synchronous
  invocation rows plus suspension, blocking, termination, and crash ceilings.
  A separate exact-machine realized envelope aggregates effective checked
  reach/invocation, transitive suspension/blocking, checked termination/crash,
  mutation frames, and capability-flow evidence without relabeling any of it as
  public contract identity. Crash evidence is refreshed after path-conditioned
  checked validation, rather than snapshot before that pass. Resource ceilings
  remain independent until their checked representation exists. The
  checked row now also retains an optional nonzero evaluated boundary-calling
  plan fingerprint. Ordinary nominal binders retain no callback placement;
  boundary callback uses gain the exact join key that target lowering must use
  to recover its already-evaluated `BoundaryEntryPlan`, without exposing a
  runtime code address. Both check-only and native orchestration immediately
  consume that key, revalidate the retained target plan, and reject missing,
  duplicate, invalid, or fingerprint-drifted realizations before backend
  lowering. That join now materializes one target-owned callback-placement row
  containing the exact nominal-use site and ordinal, selected machine/entry,
  satisfaction identity, fingerprint, and validated `BoundaryEntryPlan`.
  Checked-only compilation exposes those rows and native compilation retains
  them on `BackendPlan`, so no later thunk pass may replace the recipe with a
  convention oracle or silently discard it. Native backend planning now also
  resolves each selected machine/entry pair to one exact `ControlFlow`
  `StateKey`, rejects a lost entry before instruction selection, and assigns a
  deterministic compiler-private thunk symbol joined by placement-row index.
  That identity includes every source/selected handle generation and duplicate
  private identities reject before instruction selection.
  The symbol is planned object identity only and never an Omega value. Function
  identity now survives assigned operations, machine instructions, encoded
  bytes, and exact object-entry selection. Image emission requires a valid
  selected-entry `StateKey`; the private symbol must globally name exactly one
  encoded function with that key and exactly one matching private text symbol
  with the same interval. Missing, duplicate, redirected, or interval-drifted
  identities reject, so a plan row cannot be mistaken for emitted thunk
  evidence. The remaining slices are resource-ceiling aggregation,
  multi-entry/re-entrant target instruction lowering, and the
  private registration relocation (whose binding placement is design-blocked
  on `OWNER_QUESTIONS.md` Q7),
  registration leases/unregister,
  and cross-target registered-callback canaries.
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
- **BOUNDED-INSTALLATION-REACH-ROWS.** Implement bounded installation reach
  rows spelled `reaches <= Bound` on installation-bound boundary requirements.
  Give each row the exact normalized
  requirement path as identity, retain its `+`-union upper bound and internal
  dependency closure, reject escape through ordinary callable package or
  component contracts, expose unresolved rows and bounds in preselection
  manifests, substitute the selected provider row throughout the root closure,
  and reject final admission with any unresolved row. Do not add effect
  negation, subtraction, lower bounds, exclusive-or, named row variables, or
  cross-requirement correlation.
- Migrate both `InterruptEntry::enter` and
  `InterruptAcknowledgement::complete` from the temporary hardcoded `PortIo`
  ceiling to distinct bounded installation rows beneath
  `MachineControl + PortIo`. Bind entry/completion coherence through the exact
  installed provider execution, acknowledgement policy, operation, and token
  lineage rather than row equality. PIC completion resolves to `PortIo`;
  LAPIC/x2APIC completion resolves to `MachineControl`. Checked and terminal
  artifacts retain the selected provider, operation, bound, resolved row, and
  refinement evidence without granting authority from reach.

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
  unchanged; a third aggregate level remains fenced. The same primitive
  assignment may directly index a fixed-array literal whose eagerly evaluated
  elements contain bounded calls or use the one remaining scalar-computation
  shell; all element and index writes publish even when the selected index is
  constant. One outer scalar shell may instead consume that remaining budget;
  combining both remains a third-shell fence.
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
  umbrella. Static-machine selection validation now projects transient
  inference into separate machine-keyed suspension and blocking rows before
  threading callable-shape judgments; service reach is inferred separately and
  neither operational axis can supply the other's fallback. Checked fact
  construction likewise projects separate machine-keyed rows before publishing
  the suspension and blocking roots, including mixed-axis checked bodies. The
  published checked operational root is retired; its plan remains only as a
  transient validation and independent-fact construction input.
  Continue removing umbrella carriers after their remaining consumers migrate.
- **TPR4/TPR6 — design blocked on owner Q5.** Choose how an ordinary domain or
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
  `OWNER_QUESTIONS.md` Q4: stack-domain ownership across interrupted and
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
  requires/ensures lanes, public proof-output selectors, opaque member projections,
  and separate producer provenance; codec and verification reject identity,
  interface, lane, field-name, row, producer, and orphan drift. The detailed
  accepted carrier is stated in
  [`law_bearing_relations_and_quotients.md`](wiki/design_briefs/law_bearing_relations_and_quotients.md).

  Replace the implemented immediate generated-output-package syntax with the
  settled proof-output lane. Ordinary results retain their declared Type;
  `let (value; public_slot: local_term) = call()` captures selected named
  guarantees, while omitted selectors contribute facts but mint no local term.
  Evidence-only binding keeps the empty Type lane. Checked and terminal Psi
  retain the exact call, callee lane, public selector, caller-local term,
  proposition/interface, outcome guard, and producer provenance without any
  generated package identity. Proposition terms remain copyable and add no
  runtime work, cleanup, or fuel.

  Generic producer conformances use the same nested static application form as
  other name-owned telescopes. Type, const, and static-machine arguments are
  explicit; only ordinary lifetime elision applies. Runtime Type results retain
  their own multiplicity independently of the proof lane.
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
  callable static-machine arguments. Implement nested call-site applications
  such as `SequenceEncoding<u8, Message>` inside the enclosing machine
  telescope. Require every type, const, and static-machine argument owned by
  the conformance; apply only the ordinary lifetime-elision rules, rejecting an
  ambiguous or otherwise unresolved lifetime. Expected subject/trait shape
  validates the closed application but never fills its non-lifetime arguments.
  Nested generic calls already forward an exact closed evidence binder bare
  through specialization. Identity retains declared name, complete normalized
  telescope including resolved elided lifetimes, optional subject, instantiated
  trait, and normalized rows. No visibility-, priority-, specificity-, or
  expected-shape-based selection.

  The nested-application closure slice is implemented. Static applications
  remain recursively delimited through syntax, resolved, and typed Psi; checked
  specialization requires every non-lifetime argument, rejects a bare generic
  conformance name, validates the instantiated subject/trait shape, and retains
  an argument-sensitive identity containing the exact declaration, explicit
  lifetime/type/const/static-machine lanes, subject, trait application, and
  closed row map. Different members of one conformance family now produce
  different specialization identities. Inline and default realization machines
  now close over the conformance telescope, and binder-member rewriting carries
  the selected application's type arguments into the ordinary fixed-point
  monomorphizer. Distinct family applications produce distinct executable row
  instances. Literal const and concrete static-machine arguments now enter the
  same specialization tuple, including calls through a captured static-machine
  parameter inside the row body. Specializing an enclosing generic machine now
  recursively substitutes type, const, and static-machine arguments inside a
  forwarded conformance application before the next fixed-point pass. Generic
  carriers retain their full instantiated subject identity (for example
  `Box<Card>`) while executable attached-row lookup uses the carrier's declared
  base namespace. Terminal Psi now retains each used closed application under
  its concrete terminal machine ID as one ordered, category-tagged telescope,
  exact declaration/subject/trait identities, normalized row map, and canonical
  application commitment. Format-13/vocabulary-18 codec round trips preserve the
  table, and a dedicated verifier module rejects unknown owners, malformed or
  duplicate applications, and redirected rows. Erased conformance lifetimes now
  resolve only from the enclosing call's parameter-position ordinary borrow
  constraints. The closer retains the resolved region in checked and terminal
  identity, rejects missing or conflicting constraints, and checks an explicitly
  supplied region against the same call-site constraint; declaration arity alone
  never selects a lifetime.
- Add `Respects` over compiler-derived positional call telescopes, deriving its
  dependent domain, pointwise input relations, and lifted result relation.
  The proof obligations and positional telescope are settled; the explicit
  source location that selects one named conformance for an ordinary lifted
  operation is design-blocked on `OWNER_QUESTIONS.md` Q6.
- Add exact-pair-selected heterogeneous constructor lifts. Dependent records
  lift in order and generate checked transport obligations for coarser earlier
  fields. Extend R6 carrier-family binders for reusable proposition-valued
  relators; add no global carrier role or default relator.
- Gate runtime deciders whose lifted relation depends on erased `Type` content;
  require determination by the runtime projection or report the component.
- Continue total specification arithmetic. Concrete and abstract Prop owners
  reject direct Trapping arithmetic and conversion while preserving total
  comparison, bitwise, classification, Wrapping, and Saturating terms. Same-
  carrier policy erasure retains Exact formation against prior facts without
  self-justification. Fixed-width integer and address
  `embed` returns proof `Int` and contributes exact source-carrier range facts;
  proof `Int as Nat` requires nonnegativity. Make ordinary `Nat - Nat` Exact
  with `right <= left` discharged at formation, rename the bootstrap monus
  operation and its dependent order/metric corpus to
  `Nat::saturating_sub`, and keep clamping unavailable through bare `-`.
  Migrate `Granted::content` and the content-projection examples to explicit
  `as Nat` conversions while retaining `IntervalSet<Nat>` as their public
  nonnegative algebra. Add the integer-policy bridge catalog and the separate
  `FloatMeaning` projection rules described by
  [`total_specification_arithmetic.md`](wiki/design_briefs/total_specification_arithmetic.md).
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
  Direct helper-call results and moved/projected borrow-carrying aggregates now
  retain exact source loans, enclosing field/fixed-index paths, and polarity
  when nested inside another literal. Same-carrier denotation-preserving value
  casts also retain those loans at root and nested positions across moved,
  helper-produced, and literal operands. Validated shared/mutable recasts over
  whole name/member places now publish the exact source loan too; indexed
  byte-region recasts remain conservative until their complete target footprint
  can enter overlap facts. Remaining computed aggregate expression forms still
  need the same propagation law.
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

- **EXTERNAL-ENTRY-STACK-DOMAIN:** owner Q4.
- **FIXED-OPERATOR-SURFACE-BINDING:** owner Q1.
- **UEFI-PHYSICAL-SEMANTIC-ENTRY-COMPOSITION:** owner Q2.
- **SUM-MATERIALIZATION:** tagged-case placement vocabulary in
  `wiki/language_guide/appendix_open_questions.md`.
- **ATOMIC-EVENT-MODEL:** portable atomic axioms and target refinement choices
  in `wiki/language_guide/appendix_open_questions.md`.
- **CHECKED-RESULT-ARITHMETIC:** public carrier ruling for failure-returning
  checked arithmetic.
- **IMPORTED-CRASH-CAPSULES:** realization/import/certificate identity in
  `wiki/language_guide/appendix_open_questions.md`.
- **NATIVE-LOGICAL-FUEL-METERING:** owner Q3.
- **PROGRESS-PROFILE-CLASSIFICATION:** owner Q5.

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
