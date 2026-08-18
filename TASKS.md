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
  Source establishment and owned retirement are design-blocked on owner Q6:
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
  backends is design-blocked on owner Q13: the canonical installed placed-root
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
  premises. Continue with case-payload paths and aggregate equality over text,
  floats, sums, and erased fields. Trapping predicate arithmetic is
  design-blocked on owner Q8; imported crash capsules remain blocked on
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
  The bounded Gamma spike is complete. It canonical-decodes four exact current
  `PSITERM\0` v11 fixtures and audits a 54-row scalar ledger covering constants,
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
  typed core is 4,977 lines / 198,803 bytes / 423 functions, with maximum
  source nesting 25. Its PSITERM-neutral byte cursor, checked
  `u8`/little-endian `u16`/`u32`, and exact low/high-half `u64` primitives are
  now a separately gated 97-line reusable layer, including an exact four-limb
  `u128` carrier. A separately gated 309-line
  terminal-codec layer owns the exact current magic/format/vocabulary envelope
  plus canonical Boolean, optional and required full-width semantic-ID carriers,
  exact identity equality/order, and length-prefixed UTF-8 grammar, together
  with the complete Boolean/fixed-signed/
  fixed-unsigned/address scalar-type grammar and exact widths `1..=128`, plus
  exact signed/unsigned 128-bit integer-value payloads; it
  rejects header/scalar/type/value drift plus overlong, surrogate, out-of-range,
  isolated-continuation, and truncated encodings. All three bounded decoders
  consume the shared header result and structural consumers use the shared
  scalar/type/value results. Scalar declarations and boundary results now retain
  the complete decoded type grammar; the bounded operation rows still admit
  only Boolean/i8/i16. Integer-constant operations retain exact signed/unsigned
  128-bit payloads until that row policy selects and narrows signed i8. The
  bounded spike narrows identities to a zero high half only in explicit adapters
  after complete decoding;
  tags, recursive vocabulary, and monomorphic type-specific results remain
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
  production. Proposition vocabulary, evidence-term identity, contract lanes,
  package invocations, and producer provenance now form one 906-line evidence
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
  transfer, and resumable continuation ABI in owner Q5. Keep WCET and wall-clock
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
  erased-evidence establishment is design-blocked on owner Q6. Relevance does
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
- **WRITE-ONLY-MEMORY-VIEW — design blocked on owner Q3.** Once its core
  representation and initialization transition are settled, carry the exact
  view through foreign signatures, calling plans, borrow checking, and both
  execution paths without widening it to read/write authority.

#### ENT4 — registered callbacks

- **CALLBACK-PARAMETER-REQUIREMENT — design blocked on owner Q4.** The source
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
  Q8: the provider-neutral `InterruptAcknowledgement::complete` requirement
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
- **TPR4/TPR6 — design blocked on owner Q14.** Choose how an ordinary domain or
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
  `OWNER_QUESTIONS.md` Q9: stack-domain ownership across interrupted and
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
  `OWNER_QUESTIONS.md` Q12. Generic package application is design-blocked on
  Q10. Explicit-discard packages are design-blocked on Q11.
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
  permitted inference are design-blocked on `OWNER_QUESTIONS.md` Q10. Nested
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

- **EXTERNAL-ENTRY-STACK-DOMAIN:** owner Q9.
- **FIXED-OPERATOR-SURFACE-BINDING:** owner Q1.
- **UEFI-PHYSICAL-SEMANTIC-ENTRY-COMPOSITION:** owner Q2.
- **WRITE-ONLY-MEMORY-VIEW:** owner Q3.
- **CALLBACK-PARAMETER-REQUIREMENT:** owner Q4.
- **SUM-MATERIALIZATION:** tagged-case placement vocabulary in
  `wiki/language_guide/appendix_open_questions.md`.
- **ATOMIC-EVENT-MODEL:** portable atomic axioms and target refinement choices
  in `wiki/language_guide/appendix_open_questions.md`.
- **CHECKED-RESULT-ARITHMETIC:** public carrier ruling for failure-returning
  checked arithmetic.
- **TRAPPING-CONTRACT-ARITHMETIC:** owner Q8.
- **IMPORTED-CRASH-CAPSULES:** realization/import/certificate identity in
  `wiki/language_guide/appendix_open_questions.md`.
- **NATIVE-LOGICAL-FUEL-METERING:** owner Q5.
- **PLACED-ERASED-EVIDENCE-ESTABLISHMENT:** owner Q6.
- **PROVIDER-NEUTRAL-INTERRUPT-ACKNOWLEDGEMENT:** owner Q7.
- **GENERIC-CONFORMANCE-APPLICATION:** owner Q10.
- **EVIDENCE-PACKAGE-DISCARD:** owner Q11.
- **GENERATED-EVIDENCE-OUTPUT-PACKAGES:** owner Q12.

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
