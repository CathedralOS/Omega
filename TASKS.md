# Tasks

Last pruned: 2026-08-03.

This file is an execution queue, not a changelog. A task should contain only:

- enough context for a cold agent to find the owning design and code;
- the remaining work;
- its real blocker, if any; and
- a concrete acceptance check.

Completed implementation history belongs in commits and design documents.
Remove completed tasks instead of appending a diary of landed slices.

Before taking work, fetch `main`, inspect the newest commits in that lane, and
avoid overlapping another active change. Commit and push coherent milestones.
Engineering difficulty is not a design blocker. Only an unresolved language or
architecture decision belongs in `OWNER_QUESTIONS.md`.

## Ownership firewall

Psi operates on Omega files and owns parsing plus all target-neutral language
semantics through terminal Psi. Omega consumes terminal Psi and owns provider
installation, optimization, ABI/storage realization, native emission, and
general execution machinery. Target backends own unavoidable ISA, ABI,
object-format, and relocation encoding. Cathedral owns OS data structures,
policies, protocols, and lifecycle.

If Cathedral cannot express a subsystem, identify the missing general Omega
primitive or mark the slice blocked. Do not implement the subsystem in Rust
inside the compiler as a shortcut. Page tables, descriptor tables, schedulers,
process tables, timer queues, and drivers remain Cathedral/package code.

Compiler validation and code generation may consume general plans. They must
not acquire customer-shaped semantic types, lifecycle states, writers,
scanners, or receipts.

## Assumed-but-unbuilt analysis register

Designs may depend on an analysis listed here only by naming the dependency.
They must not describe its result as something the checker already derives.

- **Terminal-Psi fuel and restricted fixed-work checking:** define terminal
  Psi; meter realized evaluation; and analyze whole hard-root
  or selected safe-point segments as `Bounded`, `Unknown`, or an attributed
  no-finite-guarantee result. The hard-root precursor is now denominated by an
  explicit, separately versioned fuel schedule: mixed schedules fail closed
  and the installed-root artifact publishes the schedule and provision. It is
  not yet terminal-Psi derivation, general parametric work, or WCET analysis.
- **Formal atomic-event model and target refinement:** define
  `sequenced_before`, `reads_from`, `modification_order`, `synchronizes_with`,
  `happens_before`, and `global_sequential_order`; mechanize the portable
  access/fence axioms; and prove the x86-64/AArch64 mappings. Existing ordering
  labels and instruction selection are implementation evidence, not this
  analysis.
- **Concurrent composition proof model (deferred; no current customer):** at
  final composition or deployment, assemble compiler-known activation classes,
  concrete resource identities, spawn/join and wait/wake edges, `invokes`,
  conserved bounds, core placement, priorities, and provider evidence into one
  sealed erased model consumable by ordinary proof machines. Retain
  implementation properties on selected conformances and whole-system
  deadlock, starvation, memory, and response properties on the composed
  artifact with premises and provenance. Do not build an ambient premise
  language, enrich `reaches`, or publish bounded exploration results. Implement
  only when a real protocol or deployment profile demands such a certificate.

## Priority queue

### P1 — Authority values and boundary evidence

The runtime model is specified in
`wiki/design_briefs/authority_values_and_boundary_evidence.md`.

#### P1a — routed establishment and admitted roots

**CLEAR DIRECTION.** Authority-bearing runtime values use ordinary data fields
plus routed domain facts.

Implementation checkpoint (2026-07-28): semantic facts distinguish evidence
origin from program-point origin; transfer
preserves evidence; admitted facts retain matching granted provider-plan
receipt identity; and `05_qualification_evidence.json` publishes the result.
Boundary admission now also retains the exact authorizing trait and state
signature, admits only an exact `ensures result in Domain` whose result carrier
matches the domain target, rejects direct accepted-machine membership claims,
and publishes the requirement identity beside the receipt. Transparent
predicate aliases now have independent syntax/resolved/typed records, expand
to their atomic conjunction before type and contract identity, compatibility,
admission, and executable predicate lowering, retain public/private publication
legality, reject unknown/cross-carrier/cyclic expansions, and report unmet
atomic facts. Establishment relationships now normalize independently as exact
checked- or boundary-requirement identities; alias
guarantees expand to atomic routes, typed constraints and snapshots preserve
them, and checked consumers no longer reconstruct authority from names.
Core `Extent` now exposes its ordinary `{ base: addr, length: u64 }` geometry
and carries authority through routed `Extent::Granted`; every declared-domain
parameter constraint becomes an implicit caller obligation, so matching runtime
geometry cannot cross a qualified boundary without evidence. Core now also
owns `ExtentRootProvider::grant`: a selected, build-admitted checked adapter may
originate exactly its qualified result with the authorizing requirement and
provider-plan receipt retained, while calling that adapter directly does not
mint `Granted`. Constrained parameters are scoped to their exact callable
state, so a graph may introduce a qualification after entry and must forward it
across each later transition without leaking that obligation onto the machine's
outer caller. Cathedral now imports the shared carrier, admits its checked
provider plan, obtains one `Granted` root after `ExitBootServices`, and carries
that linear value into owned idle. The remaining authority migrations and
composite claim-frontier work remain.

- **ENTRY-CONTENT-ROOTS — DESIGN CLEAR:** add one core-owned stable program-
  storage entry requirement whose exact qualified parameter positions identify
  the image and initial stack/storage roots. `Extent::Granted` authorizes that
  requirement as an alternative route; UEFI, process, and other target entry
  traits inherit the same semantic requirement while `Calling<C>` and target
  policy refine only its plan and ABI. Installation introduces the matching
  parameters. Derive sections and statics as subextents; allocate later frames
  and task stacks from existing roots. The startup provider admits only the
  handful of mappings it actually supplies, not each object independently.
  The core `ProgramStorageEntry::enter` declaration and its ordered image /
  initial-storage `Extent in Granted` positions are now live, and
  `Extent::Granted` retains that exact requirement as its second authorized
  route. Target entry schemas inherit the qualified positions without
  recognizing target names. The installation handoff now binds the selected
  schema's exact core requirement and calling-plan fingerprint to generated ABI
  captures at semantic positions 0 and 1. It validates both runtime geometries'
  `no_wrap` obligations before consuming either admitted grant, returns both
  grants on rejection, and imports the image and initial-storage roots only as
  one successful handoff. Installed image sections/statics now derive as
  borrowed subrange views under that one root; independently owned initial-
  storage allocations use a conserved partition that retains every prefix and
  suffix remainder, rejects invalid geometry without consuming the pool, and
  can recompose the exact parent lineage. Remaining integration is for concrete
  target startup providers to feed their emitted image/layout geometry into the
  handoff and allocate their later frames/task stacks from the returned pool.
  Cathedral now declares `UefiApplication: ProgramStorageEntry`, so the target
  package owns the exact inherited semantic schema without a look-alike root
  domain. Its current exported UEFI `Main::run(handle, table)` remains the
  boot-verified raw firmware callable, not yet the selected storage provider.
  Remaining integration is Build entry-schema selection plus the generated
  stub/geometry bridge that binds emitted image and initial-storage geometry to
  the inherited positions before forwarding the firmware invocation. The stub
  and geometry work is engineering, but production selection is **DESIGN
  BLOCKED by `OWNER_QUESTIONS.md` Q4** because the owning Build model still
  leaves its entry slot/discovery rule open. Do not substitute name recognition,
  silently choose a unique export, or pretend firmware supplied `Extent`
  parameters.

- `Task<T>` plus the interrupt mask guard and acknowledgement token are now
  ordinary linear data. The interrupt carriers expose the compact
  root/invocation/control-or-policy identities needed for exact settlement and
  use routed `Active`/`Pending` facts, so reconstructing identical fields
  cannot settle either obligation.
- Selected interrupt-root schemas now retain every linear routed entry
  qualification as a structured `accepts` row with its carrier-aware semantic
  domain identity and born-strict compiler carry policy. The selected provider
  receipt identity binds those rows, the external-root selection bridge
  preserves them, and `05_qualification_evidence.json` reports them. This
  closes the static admitted-entry contract for `Pending`; it does not
  manufacture a source fact from fields or substitute for a concrete
  invocation receipt.
- The installed-root ledger now binds the exact selected requirement and its
  canonical accepted-claim rows into root identity and installation records.
  A concrete interrupt-entry receipt mints `Pending` qualification evidence
  for the exact acknowledgement subject, retaining provider-plan, requirement,
  parameter, semantic-domain, carry-policy, invocation, and receipt identity;
  missing, duplicate, drifted, or replayed bindings fail closed.
- Core `InterruptMaskGuard::Active` now names the exact exclusive-receiver
  `InterruptMaskControl::save_and_mask` boundary requirement. Selected service
  schemas retain routed linear result claims structurally and include them in
  provider identity and `returns` authority-flow artifacts. The external-root
  ledger separately binds that selected mask-provider claim, and an exact mask
  transition receipt mints subject/invocation-bound `Active` evidence; explicit
  qualification, missing contracts, claim drift, and receipt substitution
  reject.
- `TaskRuntime` is now an ordinary boundary trait. Each concrete `start<M>` /
  `try_start<M>` Omega activation-sidecar fact binds the exact retained
  selected-provider plan and exact operation requirement, rejecting a missing
  provider or a provider that narrows the published machine-parameter
  contract. `CheckedTrees` retains the source/checking facts but no target
  layout, calling-plan, stack, or runtime-selection artifact. A dynamic
  provider-instance/invocation receipt now revalidates that exact static
  provider, requirement, operation, activation plan, and preservation evidence
  before lifecycle accounting; invocation and receipt identities are
  single-use within the instance. Continue into stack-resource authority,
  cancellation conformance, routed source establishment, and transactional
  custody under TR3–TR8.

Acceptance: reconstructing an authority carrier does not establish its facts;
an authorized route cannot satisfy a predicate-bearing result without proving
its predicates; an admitted provider cannot originate a fact outside a
requirement named by the domain; receipts and authority-flow reports identify every accepted
origin; and Cathedral obtains one qualified root at a time from its admitted
memory provider without split, merge, or an array of checked claims.

#### P1c — composite resource frontiers

**PARTIALLY CLEAR.** Implement path-indexed partial moves and normalized
multi-output claim transformations. Per-claim carry inheritance and
content-projection authoring have settled semantics. An exact qualification
selects content by publishing one owner-unique conformance to core
`Content<A>`; ordinary postconditions state backing, correspondence, and
authorized terminal outcomes. Do not infer content from multiplicity or
recognize domain, field, split, or merge names.

Implementation checkpoint (2026-07-28): transparent records now derive one
tracked claim per statically named contained linear field instead of requiring
an affine wrapper to reject or silently erase the debt. Local construction,
whole-record transfer, and field extraction preserve each claim's canonical
path, transfer-stable claim identity, and root-lineage provenance independently;
partial moves leave sibling obligations live; duplicate moves and sibling scope
loss reject; and the backend permission ledger retains complete path-indexed
event realizations with claim identity rendered separately from provenance.
Fresh linear resources and borrow loans mint deterministic identities;
destination bindings, aggregate fields, loan weakenings, and unique state-call
results preserve them. Multiple contained claims established at one state entry
share root lineage but retain distinct identities. Explicit `[linear]`
aggregates still contribute one nominal root.

Path-aligned checked state results now infer n-ary claim conservation by exact
relative output path: each uniquely matched callee claim keeps its identity and
lineage through caller binding, while ambiguous or non-path-aligned checked
multi-claim results reject instead of minting replacement claims. Direct record
constructors now contribute an explicit structural output map by nesting each
source claim under its named field. Checked bodies now publish complete
normalized outcome maps, and opaque n-ary result calls consume them to a fixed
point across expression calls and qualified tail transitions. Input-relative
entries rebind through the actual caller arguments; locally established entries
retain exact claim identity and provenance across multi-hop wrappers. Bodyless
or ambiguous targets remain fail-closed. The checked proof/debug surface retains
the structured maps in `05_claim_outcomes.json`.

Implementation checkpoint (2026-08-02): those outcome maps now drive the first
content-conservation inference consumer. For every exact input-relative mapping
whose entry and result places select the same semantic-domain/projection
fingerprint and algebra, checked facts retain a fingerprinted entry/current
equality together with the transfer-stable claim identity and structural input
and output paths. Direct forwarding, transparent record construction/extraction,
fixed indices, and ordinary qualification contracts participate. Each preserved
claim remains an individual rewrite row: the checker does not turn multiple
independent claims into `separate(...)`, because claim distinctness does not
prove projected-content disjointness. Fresh establishments,
projection-identity mismatches, and runtime indices infer nothing. Active sum
payloads retain distinct case-plus-field paths through the same inference.
`05_claim_outcomes.json` publishes these rows beside their outcome-map evidence.

Carry policy now follows the exact claim identity independently of the
carrier's structural policy. Each qualification-evidence origin begins strict,
its own positive permissions relax only that origin, and distinct origins and
distinct claims intersect. Because n-ary outcome-map rewrites preserve claim
identity, checked multi-output helpers cannot erase or exchange child carry
policies. Checked facts and the carry artifact retain the effective policy per
claim.

Literal-length fixed arrays now derive one canonical fixed-index frontier path
per contained claim. Array construction, literal-index extraction, partial
moves, claim identity/provenance, and n-ary result maps retain those paths;
moving one element leaves its siblings live, and duplicate extraction rejects.
Checked and backend artifacts render fixed indices structurally. Runtime-indexed
owned extraction remains deliberately fail-closed because it cannot name one
unique element.

Active sum payloads now derive canonical case-plus-field frontier paths.
Payload-field symbols are children of their declaring variants, so equal field
spellings in different cases cannot alias. Known case construction activates
only that case; extracting one payload leaves same-case siblings live and
retires impossible alternatives. Checked n-ary result maps retain live case
paths, prove statically inactive alternatives absent, and propagate that
liveness and exact claim identity through opaque calls. Borrow owner paths and
proof/debug artifacts retain the case identity structurally.

This is not full P1c: backing and conservation witnesses remain implementation
work. Symbol-keyed substitutions already retain
contained claims through nested generic transparent records.

Implementation checkpoint (2026-07-31): core now publishes `Content<A>`, the
transitional single-interval `Interval<CoordinateSpace>`, and
`CountedQuantity<Unit>`. The settled algebra carrier is now the canonical
`IntervalSet<CoordinateSpace>` because separated composition and residual
difference are not closed over one interval; migrate the implementation and
fingerprint before landing conservation consumers. A projection must be
one bodyful checked machine explicitly satisfying `Content<A>::project`,
attached to the exact atomic qualification whose carrier it reads. Foreign
homes, alias homes, and a second projection for the same exact qualification
reject. Projection machines are compiler-erased proof material, so their
proof-only algebra result does not acquire a runtime layout. Generic-data
instance rewriting also keeps conformance arguments aligned with rewritten
machine signatures. The next P1c checkpoint was closed-fragment normalization.

Implementation checkpoint (2026-07-31): accepted projection bodies now lower
to a compiler-owned symbolic expression over exact subject-field paths,
proof-natural literals/constructors, closed `+`/`-`/`*`, and the selected
`Interval` or `CountedQuantity` constructor. Checked facts retain that plan,
the normalized coordinate-space or unit identity, carrier/domain/machine
identity, and a stable fingerprint that deliberately excludes arena-local
symbols. Core algebra definitions remain structurally generic through checked
lowering.

Implementation checkpoint (2026-07-31): core now exposes compiler-erased
`embed<T>(value) -> Nat` for the closed content-projection fragment. Accepted
projections may embed exact `u8`/`u16`/`u32`/`u64`/`addr` subject-field paths;
checked facts retain the embedding distinctly from proof naturals and keep its
fingerprint independent of arena-local symbols. Signed, floating, boolean,
atomic, non-field, user-lookalike, and arbitrary call inputs remain fail-closed.
The backing and conservation consumers are still outstanding.

Implementation checkpoint (2026-07-31): checked callable signatures now use
those retained algebra identities for the first custody-conservation gate. A
content-bearing result rejects when it has compatible content-bearing inputs
but every compatible source is a shared or exclusive borrow; a by-value linear
input remains an eligible consumed source. This establishes the documented
  `submit(&buffer) -> PendingWrite` rejection without recognizing carrier,
  domain, parameter, or operation names. Full n-to-m equality, separation,
  backing, custody-exit, and ambiguity proofs remain outstanding.

Implementation checkpoint (2026-07-31): `05_claim_outcomes.json` now retains
every checked content projection beside the path-indexed outcome maps. Each row
keeps exact domain and machine identity, carrier identity, structured closed
algebra, the complete normalized symbolic expression (including runtime scalar
embeddings and arithmetic), semantic-domain identity, and the stable projection
fingerprint. It does not publish placeholder backing or conservation witnesses;
those rows remain absent until their actual checked proofs exist.

- **BUMP-ALLOCATOR-CANARY — DESIGN BLOCKED through
  `TERMINAL-CONTENT-CLAIMS`:** implement an
  ordinary package-level bump strategy over a consumed `Extent` once source
  content-conservation contracts can state
  its split, retirement, reset recomposition, and backing return. Keep
  allocatable tail, live extents, and retired extents distinct: release cleans
  `T` and returns authority but restores bump capacity only at reset. Exercise
  RAM and non-RAM placed access without adding an Arena primitive, interior
  mutability, or a new borrowing rule;
- **BOUNDARY-ISSUANCE — DESIGN CLEAR; depends on `CONSERVATION-CONTRACT`:**
  lower per-invocation geometry from ordinary
  postconditions over parameters, entry-version places, and result paths; do not add
  a receipt binder. Bound every newly established result claim through one
  n-ary relation, keep transferred input content in conservation, and reject
  algebra mismatch. Retain admitted external ownership and fresh issuance
  separately from derived geometry. Add stable backing identities, provider
  live-issuance ledgers, common-root custody delegation, explicit alias classes,
  and partitioned succession. A succession classifies preserved, reclaimable,
  retained, and excluded ranges; reclaimable ranges require a derived check
  that no live claim overlaps them. Runtime validation uses a declared
  `Outcome` or explicit trap; provider assertion failure remains a trust
  violation. Proof/debug artifacts
  retain geometry, issuer, custody lineage, alias class, succession history,
  and trust provenance;
- **CONSERVATION-CONTRACT — IMPLEMENTATION IN PROGRESS:** proof-only
  `entry(place)` and compiler-owned variadic `separate(...)` are now reserved
  call-shaped builtins. Validation admits them only in `ensures`, requires
  `entry` to select a parameter/`self` structural place, requires every
  projection subject to carry the exact qualification, and resolves each
  projection to the owner-unique `Content<A>::project` conformance. One
  equality per callable outcome and algebra normalizes entry/current
  structural places, fixed indices, flattened/sorted separation, exact
  projection identity, and a stable fingerprint; checked facts, machine
  contract identity, and `05_claim_outcomes.json` retain the result. Runtime
  use, algebra mixing, duplicate equations, arbitrary projection lookalikes,
  and unqualified subjects reject. Terminal Psi v9 now carries canonical
  structural-place declarations and exact content-conservation equalities
  without arena-local identity. Semantic format v1
  and proof format v8 encode field/fixed-index propositions canonically, while
  proof format v9 adds sum-case path segments; the verifier restricts the
  proposition to `ensures` and checks replaceable certificates. Checked
  lowering now derives exact one-to-one identity-reshuffle rows without
  manufacturing separation between independent claims. Terminal semantic v10
  carries canonical dense machine-local rows independently of the legacy
  checked representation. The
  terminal verifier requires a one-to-one parameter-entry/result-current map,
  rejects duplicate or prefix-overlapping paths and projection/algebra drift,
  and reconstructs one semantic content-equality axiom per exact projection for
  certificate use. Terminal semantic v11 adds a distinct stable sum-case path
  segment; the legacy checker precursor, verifier, semantic codec, and proof format v9
  retain case-plus-field identity without arena-local symbols or collision with
  equal field spellings in other cases. Checked lowering now also instantiates
  an authored partition equation through an exact direct returned call or one
  result staged through an exact local identity rewrite. Every
  source entry projection must substitute to a caller-parameter structural
  place whose transfer-stable claim identity reaches that exact call; the
  derived row retains the source theorem fingerprint, call site, all input
  claim identities, any staged/nested result rewrite rows, and the substituted
  equation in `05_claim_outcomes.json`. Composition copies the source
  `separate(...)`
  structure and cannot manufacture a partition. Eligible wrapper chains close
  to a fixed point. A staged-local chain or nested aggregate result composes
  only when every projected call-result claim is established at that exact
  call, survives the normalized outcome-map chain unchanged, and is published
  at one unique callable-result path. Each rewrite row binds that claim to its
  exact source and target structural places. Exact record, fixed-array, and
  active-case literal arguments now distribute each callee entry projection to
  one uniquely selected caller-parameter leaf whose claim reaches the exact
  call. Sum-payload paths retain their resolved case-plus-field identity, and a
  mismatched active-case literal fails closed. Multiple staged partition calls
  now retain one independent composition row per exact call when each
  call-established result claim reaches one unique returned-aggregate path.
  Fixed-point reuse retains the source theorem's wrapper-derivation depth so a
  derived theorem cannot masquerade as authored evidence. These
  non-direct rows remain
  checked/debug evidence because
  terminal semantic v12 rejects staged result rewrites and nonzero source
  derivation depth, deliberately carrying only direct depth-zero composition as its
  exact source theorem, source fingerprint,
  dense input-claim references, total structural-place substitution, and
  derived equation. The verifier requires the source to contain separation,
  binds every entry projection to one listed v14 entry-claim row, replays the
  substitution, and reconstructs only the exact derived theorem as a semantic
  axiom. The entry binding itself is not an axiom. Canonical bytes include the
  witness; existing proof format v9 already carries the resulting content
  proposition. Archived v1-v13 bytes retain their identities. The content
  producer now lives
  in `psi-checked-trees-to-terminal`: it revalidates and lowers checked
  conservation plans, exact identity reshuffles, and direct partition
  compositions into the existing v9-v14 terminal vocabulary, including dense
  claim identities and replayable place substitutions. The executable source
  canary remains content-free and fail-closed. Next connect the now-unblocked
  real content-bearing source slice, insert sealed introduction and
  custody-exit rows, and discharge or admit the exact frontier theorem. The
  real content-bearing executable source canary is currently blocked on
  implementation of aggregate terminal values/calls or the custody-exit
  producer needed to cross that boundary. This is not a language-design block.
- **TERMINAL-CONTENT-CLAIMS — DESIGN RESOLVED; implementation unblocked:** a
  real direct partition wrapper exposed a gap hidden by the synthetic terminal
  fixture.
  Checked composition correctly carries distinct entry claim identities but no
  identity reshuffles: aggregate conservation does not prove either input is
  individually equal to one output. Terminal v12 can name an input claim only
  through `ContentIdentityReshuffle`, and its verifier therefore required the
  stronger one-to-one equality. Terminal semantic v14 now carries an
  independent dense entry-claim binding with exact projection, algebra, and
  entry place, and the checked producer emits it for partition-only claims.
  The binding creates no equality axiom. Source integration and the dependent
  frontier work are now implementation tasks rather than language-design
  blockers.
  Contracts call the exact
  owner-unique `Content<A>::project` conformance machine; do not add
  `content(...)`, general `old(...)`, or `retired_via(...)`. Normalize one
  equation per content algebra and outcome row. Infer only claim-identity-
  preserving reshuffles; partition-changing primitives author the theorem and
  checked wrappers compose it. Insert sealed introduction and exact terminal
  custody-exit rows from the claim frontier. For a bodyless partial boundary,
  derive the residual from entry and result content and admit only provider
  custody acceptance. Add a closed, fingerprinted namespace-origin policy:
  `ProgramLocal` roots may be provisioned only by owner-authorized sealed
  declarations, while `ProviderBacked` roots require selected admitted
  issuance. Retain an internal canonical algebra account for every content-
  capable root even when no source projection is exposed. Report the policy
  and declaring package; treat correspondence from a local logical namespace
  to device reality as admitted evidence, and require all pools spending one
  hardware capacity to split or lease from one provider-issued root. Retain
  normalized obligations in semantic identity and
  replaceable proofs in proof/debug artifacts. Quantity-only projections never
  supply unit identity; report modeled identity coverage and require an
  identity-bearing or joint algebra when an operation uses such authority;
- keep domain facets, permission attenuation, carry, and root lineage as
  independent axes; recoverable or scarce authority uses a claim or loan rather
  than a discardable permission; and
- retain normalized projections, admitted backing, and the n-ary conservation
  witness beside the landed outcome maps in proof/debug artifacts.

Extent split/merge is triggered only when an independently owned subrange must
cross an ownership boundary; it is not required for the admitted-root,
placed-view, or subrange-loan slices. A package allocator that returns an owned
subextent does cross that boundary and must conserve the split. Virtual-to-physical
owned decomposition remains rejected until a compact canonical symbolic mapping
algebra with decidable containment, equality, restriction, and separated
composition is specified. Runtime-indexed extraction likewise remains a
monotone acceptance restriction until the frontier and prover can identify the
unique moved element.

Acceptance: partial moves preserve sibling obligations; duplicated or
overlapping children reject even when every child is individually contained and
their scalar measures add up; gaps reject unless an authorized custody exit
accounts for them; admitted backing and projected content normalize in the same
algebra; unrelated roots cannot merge merely because their intervals are
adjacent; mapped outputs inherit carry from their actual origins; related
virtual/physical quantities cannot be conserved independently; and ambiguous
mappings reject.

### P2 — Source-visible materialization and placed access

References:
`wiki/design_briefs/programmable_layouts.md`,
`wiki/design_briefs/os_memory_and_hardware_foundation.md`, and chapter 20.

#### L4/L5 — plan-laid views

- Finish source-visible validate/materialize establishment over owned storage.
- Complete materialization-backed non-scalar tiling and mutable-view
  establishment beyond checked recasts. Recursive aggregate representation-set
  checks are live for records, literal fixed arrays, complete-source aggregate
  slices, and exactly tiled interior slices.
- Direct ordinary-scalar projection through validated fixed `Bits` placements
  is live on x86-64 and AArch64, including masked writes and fragmented reads.
  Source-visible validation/materialization remains the establishment boundary.
- Keep validation as the route from raw bytes to established typed facts and
  derive public access from validated plans and field identities.

Acceptance: an Omega-authored compact-bit policy validates, materializes, and
projects a typed value on x86-64 and AArch64; malformed tiling and fact
establishment from raw bytes reject.

#### L6b — `AccessPlan` and placed views

**IMPLEMENTATION WORK.** Chapter 20 and
`wiki/design_briefs/os_memory_and_hardware_foundation.md` own the normalized
model. Qualified-Extent admission, placed-content establishment, and public
generic atomic operation requirements are settled. Target-specific lowering
remains implementation work.

- Derive `Placed<P, T>` projection and granular readable, destructive-read,
  writable, and atomic accessors. Stable ordinary writes require plan
  permission, exclusive current borrow, and exclusive source borrow; External
  and atomic operations follow their admitted operation contracts.
- Source derivation now retains the authoritative placement identity and exact
  per-field permissions in typed trees. Stable/external accessors expose only
  admitted trait methods; direct atomic syntax over `bool`, `u32`, and `u64`
  is checked per operation family, works through a shared view borrow, and
  cannot materialize an accessor as an ordinary scalar. Only the nominal
  placement package may directly name or issue a binding-private accessor;
  possession delegates the public operation requirements it conforms to generic
  code. Copyability, cross-activation sharing, and counted permits separately
  control durable, concurrent, and bounded delegation. Sealed primitive
  requests now retain the logical field fragments separately from the complete
  concrete transfer footprint; the foundation conflict judgment shares
  repeatable reads and exact same-container atomics while rejecting destructive,
  write/RMW, and mixed-width overlapping atomic events. Plan validation rejects
  overlapping atomic fields that select different transfer granularities,
  requires a destructive accessor to cover its whole transfer container, and
  rejects a second accessor overlapping that one-shot snapshot; generic
  External writes likewise reject when the logical field does not cover the
  complete transfer container. Typed validation rejects any recast whose source
  or target is a placed view, directing callers back through explicit admission
  over the underlying qualified extent instead of allowing permission
  escalation by representation compatibility. Qualified-borrow admission,
  placed-content establishment/retirement, and compiler view-set integration
  of cross-view footprint conflicts remain implementation work rather than
  language-design blockers.
- Publish one sealed `omega::core` requirement per atomic operation. Use shared
  receivers, the settled proof-static ordering vocabulary, exact derived
  conformance for core atomics and placed accessors, exact-forwarding wrapper
  derivation, and checked or admitted evidence for every other realization.
  Missing conformance makes an operation unavailable; arithmetic carrier
  bounds never manufacture hardware capability.
- Require one target/provider-supported atomic transfer at a fixed width and
  alignment for every operation, then apply operation-specific eligibility.
  Load duplicates, store discards the displaced value, swap conserves and may
  transfer an affine resident owned through Stable initialization, scalar CAS
  initially remains copyable, and each fetch operation proves its exact raw
  representation law over every provider-reachable representation. A cell may
  cross activations only when its resident type is transferable.
- Implement decisive and single-attempt compare-exchange separately. The
  single-attempt result distinguishes `Exchanged`, `Mismatched(observed)`, and
  `Uncommitted(observed)`; both failure arms use the read-compatible failure
  ordering, while success uses the read-modify-write ordering. Decisive CAS and
  target retrying fetch lowerings retain target-relative work attribution.
- Replace the bootstrap's source-facing `ExtentLoan` shape with ordinary
  `&`/`&mut` projections of `Extent in Granted`; keep any exact-loan carrier
  internal. Bind the selected provider's normalized profile receipt through
  the qualification rather than accepting a caller profile. Implement borrowed
  rejection by ending the loan and owned rejection by returning the moved
  extent.
- Implement `Stable` adopt/initialize/validate and `External` adopt, plus
  borrowed cleanup and owned `destroy -> Extent in Granted & Vacant`.
  Establishment must check read totality or stable validation, write encoding
  and value fit, and legal transfer derivation per field and operation. Do not
  synthesize fitting domains, External initialization, multi-transfer External
  reads, External RMW, or retrying atomic writes.
- Carry logical extents separately from physical effect footprints. Repeatable
  reads share; destructive reads and stable RMW reserve the whole affected
  transfer container; atomic operations retain their exact admitted conflict
  rule. A destructive unit derives one whole-snapshot `take`, never independent
  field takes.
- Retain schema correspondence separately from resource compatibility. Record
  its admitted source and optional runtime revision predicate, and bind the ID
  observation and full placement to the same stable provider/device instance.
- Connect target external/atomic emission. External transfers occur once at an
  admitted whole-container width; no generic external RMW or arbitrary-offset
  primitive is available.

Acceptance: UART/MMIO and shared-page IPC use the same extent/layout foundation
with different placement and resource profiles; ordinary owned RAM retains
normal lvalues; Stable-over-MMIO, External-over-insufficient-rights,
misaligned/inconsistent transfer plans, an unplanned offset, narrow external
write, destructive read through `Readable`, mixed-width overlapping atomics,
source-loan polarity upgrade, simultaneous overlapping views, view recast
escalation, and forged admission evidence all reject before code generation.
A binding-private accessor can be deliberately passed to an external generic
helper without exposing its nominal type; direct external projection still
rejects. Missing atomic-operation conformance, unsupported width/alignment,
affine swap over adopted contents, and cross-activation sharing of an
activation-bound resident reject at their respective derivation or crossing
sites. Single-attempt CAS distinguishes mismatch from an uncommitted attempt
without a second load.

#### L6c — symbolic materialization

- Carry symbolic data/entry sources and placement constraints through final
  artifacts.
- Carry immutable AOT post-handoff fragment bytes, exact footprint, and their
  symbolic invocation plan through final executable artifacts.
- Connect the final placed fragment to source-level provider invocation after
  P1 supplies runtime sealed handles and L4/L5 supplies materialization
  establishment. Provider preparation must not generate host code.
- Keep loader-consumed fields within native relocation vocabulary.
- Bind validation to final bytes and exact placement; compact fingerprints are
  report/cache identities, never authority.

Acceptance: one generic materializer handles a fragmented hardware descriptor
and an ordinary data relocation without learning either customer's semantics.

### P3 — Cathedral address translation

This is Cathedral work, not a compiler subsystem.

Cathedral already owns
`source/drivers/facts/x86_page_table_entry.omg`. Build its hierarchy,
validation states, installation protocol, and teardown in Cathedral by
composing general Omega primitives.

Prerequisites:

1. P1's sealed runtime `Extent`;
2. P2's source-visible materialization and placed access;
3. checked activation/invalidation operations (structured x86 CR3 access is
   live; additional TLB operations are catalog engineering); and
4. an ordinary allocator package over qualified `Extent` storage for dynamic
   hierarchy allocation.

A fixed bootstrap table may use pre-reserved storage before the dynamic
allocator exists. Do not restore `omega-page-tables` or any compiler-owned
page-table model.

Acceptance: Cathedral builds, installs, replaces, and tears down its own table
using Omega code; the compiler sees only general plans, extents, provider
contracts, and checked instructions.

### P4 — Cathedral exception roots and first timer

References:
`wiki/design_briefs/os_memory_and_hardware_foundation.md`,
`wiki/language_guide/chapter_23_inline_assembly.md`, and Cathedral's boot docs.

After P0/P1/P2:

- materialize a fatal/diagnostic entry for every architectural exception before
  enabling interrupts;
- provision dedicated per-CPU stacks for double fault, NMI, and machine check,
  plus one shared non-nesting maskable-IRQ stack class;
- keep final handler code transitively SIMD/x87-free under its `StatePlan`;
- use linear mask/restore and acknowledgement values;
- program PIT+PIC first, with LAPIC as the production provider;
- keep the hard root fixed-work: acknowledge, capture time, set a coalescing
  wake state, return. Timer fan-out belongs in an ordinary scheduled task; and
- treat a deferred acknowledgement as a lease on the interrupt root and its
  controller configuration. Reconfiguration, shutdown, CPU removal, relevant
  power transitions, and root retirement drain outstanding acknowledgements
  before proceeding; carry policy decides whether a bottom-half transfer is
  legal. Do not add revocation or a breakable pin.

Acceptance: QEMU boots, installs Cathedral-owned exception/IRQ structures,
reports timer ticks over owned serial output, and halts between ticks. Missing
or double acknowledgement, user-authored `iretq`/`lidt`, invalid stack/state
ceilings, and publication-before-ledger-record all reject.

## Active compiler lanes

### Calling plans and boundary artifacts

#### ENT2c — finish normalized ABI lowering

Migrate remaining compatibility call paths to evaluated `CallPlan + StatePlan`.
The major x86-64/AArch64 argument, result, aggregate, syscall, vtable, and
service-table paths are already plan-driven.

Remaining:

- remove residual hardcoded placement decisions;
- finish the retained foreign-storage path beyond the live safety gates.
  Reference-shaped ABI parameters already create call-scoped borrows; retained
  custody sourced only from a borrow rejects; an owned-input/linear-pending/
  terminal-redemption canary pins the content-conserving protocol shape.
  Preserve exact provider-era dependencies as compiler-owned claim metadata
  and connect protocol-correlated runtime completion receipts;
- add explicit provider-view claims where runtime protocol events end validity.
  The ordinary-borrow dual is pinned: provider-owned views whose every
  invalidator requires exclusive receiver access end at last use, and a live
  view rejects an overlapping exclusive invalidator;
- keep raw `addr` and `Ptr<T>` inert and non-dereferenceable; a calling plan may
  describe their ABI representation but cannot manufacture authority. The
  stale raw `Ptr::read`/`Ptr::write` bootstrap operators are retired, and a
  negative source canary pins that possession of `Ptr<T>` exposes no access;
- reject permanent foreign retention unless the consumed authority is
  transferred into an established static or process-lifetime root; do not
  invent a general permanent-custodian spelling without a concrete customer;
- record write-only views as a focused core-type follow-up rather than hiding
  write-only foreign access in a plan;
- extend differential checks wherever a compatibility encoder remains. The
  vtable-slot, vtable-field, and service-table emission and layout now require
  the selected binding's evaluated plan; their no-plan encoders, width oracles,
  AArch64 placement helpers, and target-policy compatibility tests are retired.
  The x86-64 ISA vtable/field/service-table encoder, width, and data-relocation
  APIs now require an authoritative plan directly as well; Win64 normalization
  remains only in test oracles, and the unused SysV no-plan entry points are
  retired. The shared SysV field-call layout also keeps production authority
  distinct from its explicitly named test oracle through final plan selection.
  Ordinary import encoding, width, AArch64 placement, and x86-64
  call/data relocation `with_plan` APIs now require `&CallPlan` rather than an
  optional plan; the no-plan route is a separately named differential oracle.
  The shared ordinary-import encoder now carries that distinction as an
  explicit authoritative-plan versus compatibility-oracle mode through both
  instruction selection and the x86-64 ISA layer, so an internal `None` cannot
  accidentally select hardcoded placement. The AArch64 ordinary, authored,
  vtable, and table-function normalizer carries the same explicit mode through
  its final plan validation/evaluation choice; optional plan context no longer
  selects the production-versus-oracle route there either. The x86-64 Win64
  and SysV ordinary-import marshallers and their relocation planners now carry
  that mode through their final plan validation/evaluation choice as well. The
  Win64 key-state, file-I/O, clock out-parameter, and runtime-text concrete
  adapters use it for their independently normalized native subcall plans too;
  no optional `CallPlan` remains as an authority selector in the x86-64 ISA
  layer. The x86-64 external/data relocation walker consumes the same explicit
  mode, keeping encoded bytes and relocation accounting on one authority
  route; a failed authoritative site lookup no longer retries through the
  no-plan oracle. Win64 selected-plan validation now checks exact argument
  arity before contextual shape comparison, so zip truncation cannot let a
  zero-parameter plan validate or relocate a one-argument call.
  The crate-local object call/data offset helpers enforce the same
  required-plan/named-no-plan split.
  AArch64 vtable/service-table plan normalization and field-call data
  relocation now require the retained AAPCS64 plan too.
  A malformed selected field binding without a plan fails emission explicitly
  and cannot reserve a compatibility width. Source-authored imports now have
  the same mandatory-plan encoder and width surface; their operation-key and
  no-plan fallback are retired across Microsoft x64, SysV AMD64, and AAPCS64.
  Linux statement,
  value-result, timespec-result, and timespec-argument syscall families now
  prove byte and width equality on x86-64/AArch64; result/argument relocation
  sites are differential-locked too. Their `with_plan` encoder/width APIs now
  require `&CallPlan`; value, timespec-result, and timespec-argument relocation
  helpers now expose the same mandatory-plan/named-no-plan split. Separately
  named no-plan functions retain only the differential oracle. Shared syscall
  normalization, encoding, relocation, and runtime-text consumers now carry an
  explicit authoritative-plan versus compatibility-oracle mode rather than an
  optional plan. The instruction-selection host module now projects only
  optional contextual value shapes; it never turns either explicit mode back
  into an optional borrowed `CallPlan`. Ordinary
  non-variadic scalar built-in imports now consume the binding-retained plan in
  emission, layout, and relocation accounting; their Windows x64/macOS arm64
  compatibility bytes and widths,
  plus Windows x64 relocation sites, are differential-locked. A selected
  built-in import with no retained plan now rejects in layout/emission instead
  of activating catalog-shaped compatibility placement. Void imports,
  pointer-result dereference imports, Windows key-state postprocessing, and
  AAPCS64 scalar-float returns now carry matching explicit-plan byte/width
  locks. Object relocation planning now independently validates every ordinary
  selected import and its exact operands through the retained plan before
  accepting call or data offsets; missing or incompatible plan evidence cannot
  fall back to catalog-shaped relocation arithmetic. A selected
  `HostOperation` with no retained binding now rejects at that independent
  relocation gate instead of reaching a no-plan data-offset path. Selected
  constant-result rows, which transfer no boundary control and own no
  `CallPlan`, use dedicated fixed non-boundary relocation geometry rather than
  entering that compatibility oracle. Runtime line
  and byte I/O host keys pass the same binding gate on every target before any
  Windows composite-subplan check or relocation collection. Every other selected
  host mechanism now requires its retained plan before data-address relocation
  too, so a syscall or indirect-table binding cannot activate the no-plan oracle.
  The dead clock out-pointer
  compatibility classifier is retired;
  composite encoders own their concrete subcall shapes. Authoritative AArch64
  built-in paths now derive result presence and scalar-float class from the
  retained plan through emission and relocation accounting; the operation
  catalog supplies those decisions only to the compatibility oracle. Void
  AArch64 imports also use the retained argument placements in both call and
  data relocation walks, including outgoing-stack drift. Composite Linux
  runtime byte-read, byte-write, and all three line-read target shapes now
  consume the binding-retained three-argument/result syscall plan in emission
  and layout, with x86-64/AArch64 compatibility bytes and widths locked to the
  explicit plan. Any selected syscall binding that loses its plan now rejects
  in layout/emission rather than reconstructing registers from the target;
  syscall result-versus-statement selection also comes from that plan rather
  than the operation catalog. Host layout now requires retained plan evidence
  for every selected mechanism before reserving bytes, so indirect table calls
  cannot collapse to a zero-width compatibility path either. The
  base `Stdin::read`, `Stdout::write`, and `Stderr::write`
  rows now actually retain those exact plans on both Linux targets rather than
  activating the no-plan compatibility path; `Process::exit_group` likewise
  retains its one-argument/no-result plan. The matching Darwin `Stdin::read`,
  `Stdout::write`, and `Stderr::write` rows retain their three-word/result
  AAPCS64 plan too, and `Process::exit` retains its exact I32/no-result plan.
  Darwin's fixed `lround`, `sqrt`, `hypot`, and `fma` rows likewise retain
  their exact F64 parameter and F64/I64 result plans instead of asking the
  encoder to reconstruct vector-register placement. Its `poll` sleep adapter
  now retains the three-word/no-result plan, and the calibrated monotonic/wall
  clock rows retain their one-word/result `clock_gettime_nsec_np` plans. The
  legacy Darwin `tick_count` alias now shares that retained plan and the
  data-driven clock-read operand path, so its declared clock ID 8 is no longer
  dropped; Windows' argument-free TickCount row remains result-only. Darwin's
  scalar Objective-C runtime cohort now retains exact word plans for class and
  selector lookup, two-/three-/six-argument message sends, runtime-byte-string
  sends, and autorelease-pool push/pop. The mixed rectangle/image-size message
  forms retain their exact interleaved word/F64 signatures, while CGRect max-X
  and max-Y retain four-F64/F64 plans; AAPCS64 independently assigns their X
  and V register streams from those selected shapes. The remaining scalar
  Core Graphics rows now retain exact zero-/one-/two-/seven-word parameter
  plans with their word or source-scratch results. Darwin `___error()` retains
  its fixed no-parameter/I32-stored-result plan. Every Darwin filesystem row now
  retains its typed libc signature. Integer literals take their selected
  parameter width at the ABI seam, compiler scratch remains result capacity
  rather than ABI type, and the retained result shape selects the actual store
  width. The creating `open` row retains its concrete Apple variadic plan and
  rejects loss of the fixed/anonymous parameter boundary.
  Checked AArch64 scalar arguments likewise take their exact retained parameter
  shape at the ABI seam when compiler scratch has greater capacity; a wider
  internal slot no longer overrides a proved-safe narrower call type.
  Windows' parameter-free `GetTickCount64`, `GetForegroundWindow`, `_errno`,
  and `GetLastError` rows now retain exact Microsoft x64 result plans. Every
  built-in Windows import row now retains its concrete native Microsoft x64
  plan at catalog construction, including the independently emitted
  GetStdHandle/ReadFile/WriteFile and time-adapter subcalls. DWORD literals and
  compiler-derived counts take the retained parameter width at the ABI seam
  instead of retyping that plan as an eight-byte scratch value. Unannotated
  compatibility external leaves likewise evaluate and retain the selected
  target's native plan from their declared recursive boundary signature during
  binding construction; explicit `Calling<C>` plans still take precedence,
  and compatibility syscalls retain the target's full-word syscall signature.
  Direct Win64 GetStdHandle, ReadFile/WriteFile, key-state, and time-out-pointer
  encoders plus their relocation walks now validate those retained concrete
  subcall plans; a semantic outer shape cannot replace the native adapter
  signature. The Windows runtime byte-read, byte-write, and all three line-read
  target shapes now consume the retained GetStdHandle plan together with the
  retained ReadFile/WriteFile plan in production layout and emission. Missing,
  partial, or incompatible composite evidence rejects; their shared production
  encoder/width surface now represents either one retained direct plan or the
  complete Windows plan pair, so a partial adapter cannot be constructed.
  Instruction selection preserves that sum after binding resolution as one of
  direct, complete Windows adapter, or explicitly named compatibility oracle;
  it no longer flattens the pair back into independently optional plans.
  AArch64 runtime-text import validation now consumes that resolved sum
  directly rather than projecting an optional direct plan.
  Its singular `with_plan` APIs require the direct plan, the ISA Windows-pair
  validator requires both plans, and explicitly named no-plan functions retain
  the differential oracle. Object
  relocation planning independently validates the same retained pair before
  recording either native call and rejects a missing GetStdHandle binding or
  plan instead of silently omitting that call record.
  The matching AArch64 direct-import composites now validate
  that same retained native signature and reject placement drift in lockstep
  with layout; Windows composites retain their independently normalized
  GetStdHandle/ReadFile/WriteFile subcall plans. The unused x86-64 relocation
  wrappers that silently selected Microsoft x64 are retired, along with the
  object walker's redundant second no-plan data-relocation lookup; remaining
  callers now name a policy or supply the retained plan. Concrete promoted
  variadic signatures now preserve their fixed/anonymous parameter boundary;
  the Darwin AAPCS64 planner places anonymous scalar arguments on the outgoing
  stack and pins the `open(path, flags, mode)` shape. The `open_create`
  encoder, layout width, call relocation, and data relocation now consume that
  complete plan through the shared explicit authoritative-plan versus
  compatibility-oracle mode; its concrete variadic normalizer no longer uses
  an optional plan to select that boundary. The duplicated `+8`/`+12`
  accounting and trailing-mode operation classifier are retired; and
- delete compatibility fields after their final consumer migrates. The
  vtable-field and service-table declared-parameter-count copies are retired,
  including the unused source-extraction copy on `ExternalBindingRow`;
  result presence now comes from the retained wire plan plus the service
  table's explicit dispatch-only operand topology. A selected `HostBinding`
  now structurally owns one complete `BoundaryEntryPlan`; unresolved external
  rows reject before entering the host ABI plan, and backend reports derive
  arity directly from the required call half.

Acceptance: changing a normalized plan changes lowering or rejects; changing
only policy source while producing the same canonical plan preserves contract
identity.

#### ENT3 — final state-footprint validation

**DESIGN CLEAR.** The exact final executable-region inventory, compiler-text
relocation envelope, checked-assembly validation, and format-owned import-thunk
validators are live. Emit one self-describing, versioned footprint certificate
bound to exact final bytes and placements. Admission replays its normalized
instruction/region rows against the closed target instruction specifications,
proves complete region coverage, and composes admitted leaves under their
separate provenance. Do not add a second whole-image decoder/admission path.

The existing single final-region artifact now carries the domain-separated
`omega.final-footprint-certificate` schema, format version 12, and a certificate
fingerprint over its final placement binding, compiler-text derivation, and
complete region inventory. Its explicit completeness flags remain false until
compiler-body footprint decoding and admitted-leaf evidence land. The envelope
is now a typed `omega-image` boundary with a closed class vocabulary,
strictly-normalized coverage rows, region-completeness/gap checks, and internal
identity replay; compiler output serializes only after that object validates.
Every encoded function now owns one exact contiguous instruction-row span.
Checked image emission replays the complete function/instruction partition
against relocated final text, rejects unowned rows or bytes, and binds counts
plus the final-byte fingerprint into the certificate as
`compiler_function_instruction_enumeration`. This closes instruction-boundary
enumeration without pretending the ordinary rows already carry decoded
register/state footprints. Each compiler function's retained first/last rows
also replay the architecture's exact fixed entry/return byte programs; this
supersedes the entry-symbol-only prefix/suffix check and covers every generated
function under `compiler_function_call_return_mechanics`.
Those final entry/return rows now also re-derive the architecture-owned
register/machine-state union, require exact equality with the earlier
`CallReturnMechanics` fragment admitted under `StatePlan`, and bind both the
footprint and boundary-contract identity into the certificate.
The first ordinary middle-row target-spec subset is also live: dispatch-loop
entry, case entry, state write/termination, forward arm skip, and case leave
retain their normalized indices and branch distances, regenerate the expected
x86-64/AArch64 program, and bind matching final bytes under
`compiler_function_body_specification_subset`. Storage-backed static guards now
retain their comparison recipe too; validation regenerates their encoded
program and composes exact storage-symbol relocation checks with the global
final-byte relocation envelope. Place-pair and place-vs-immediate guards now
retain the canonical `Place` operands and comparison recipe, replay the same
x86 materializer or direct AArch64 encoder, and require the exact place-derived
storage/index relocation set. Dedicated runtime-text literal-buffer and
descriptor-vs-literal guards now retain the canonical data-object symbol,
literal/length, storage source, operator, and normalized branch distances;
validation replays the target encoder, requires the exact data/storage
relocation set, and admits their `RuntimeTextGuardComparison` footprint.
Recursive runtime-value guards now retain their two roots into a single
canonical operand arena carried by encoded machine code. Final validation
regenerates the complete evaluator program, independently walks nested
binary/conversion/index/text-equality operands to require the exact relocation
set, and admits the target's closed `RuntimeValueGuardComparison` may-write
ceiling with operand-sensitive stack/control state. Ordinary writes/calls remain
incomplete. Direct exit-result materialization now replays immediate writes and
storage-to-result-register loads, including exact storage relocations, and
requires their derived clobber union to equal the retained
`ExitResultRegisters` fragment. The final validator now also
replays register-, stack-, and indirect-pointer entry argument copies plus the
entry `args` slice-descriptor write. Each row retains its normalized ABI/storage
recipe, requires the exact runtime-frame relocation site even when a stack-held
indirect pointer precedes it, and must reproduce the earlier `EntryStorage` or
`EntrySliceDescriptor` StatePlan fragment. The final validator now also
re-derives the dispatch/static-guard register and machine-state unions from
those successfully replayed rows, requires exact equality with their earlier
`StatePlan`-validated semantic fragments (including the complete place-guard
union), and binds the composed footprint
fingerprint plus the exact boundary-contract identity into the certificate.
Certificate construction rejects a different public ceiling. This is
footprint enforcement for the live target-spec subset, not a completeness
claim for the remaining body.

- Finish enumeration of compiler-generated entry/body regions.
- Validate final placed bytes after relocation, thunks, veneers, and generated
  stubs against `StatePlan`.
- Keep the public ceiling in requirement identity and private footprint
  evidence outside it.
- Extend general compiler-function body decoding; do not add an
  interrupt-specific validator.

Acceptance: forbidden register classes introduced anywhere in the final
transitive artifact reject, while two legal realizations with the same ceiling
retain one requirement identity.

#### ENT4 — registered callback lowering and Windows adapter canary

- Allow a foreign registration parameter declared by a callback requirement to
  select one named static boundary machine satisfying that requirement.
- Retain the evaluated `Calling<C>` relationship and `CallPlan + StatePlan`;
  emit the native thunk and relocation only in the selected binding lowering.
- Model durable registration as an ordinary linear package value with an
  explicit unregister consumer and optional code/component lease.
- Implement a Windows message-adapter canary using a generational state handle,
  provider-stack preflight, locally restricted synchronous handlers, and queued
  ordinary events. Add depth enforcement or owned-stack switching only if the
  concrete protocol forces them.
- Report callback entry plans and process-lifetime versus reclaimable
  registration roots without requiring a live ledger for statically linked
  process-lifetime code.
- Derive the same-context relation needed by
  `Atomic::interruption_fence` from the installed external-root route: selected
  handler, vector/signal source, execution context, and interruptible code.
  Reject the operation wherever that evidence is absent; source spelling is
  never an assertion of the relationship.

Acceptance: a `Calling<MicrosoftX64>` callback requirement accepts one matching
named machine, rejects signature/plan mismatch, emits no source-visible code
address, keeps state ownership in Omega, and cannot release a reclaimable code
lease before explicit unregistration. The Windows adapter demonstrates that
application-handler re-entry restrictions use ordinary local reach analysis.

### Frames, domains, reach, and trust

- **R5:** finish relational frame candidates and escaping mutation checks.
  Exact resolved statement/value-call frames preserve unrelated incoming range
  guards in the proof checker and transitive range-fact collector; opaque,
  overlapping, and unknown dynamic-dispatch frames remain conservative fences.
  Keep these summaries as inferred implementation metadata. Published
  `ensures` may state exact preservation when an interface needs it; prefer
  signatures exposing only the places a callee actually mutates.
  Relational loop-head inference now composes a finite, cycle-safe chain of
  authored machine-arrival upper bounds from the guarded counter to a
  collection length. At least one link must be strict, and every intermediate
  place plus the collection must remain stable across the whole machine;
  overlapping sibling-call frames reject the candidate. Direct/nested/indexed
  invariant-window consumption and dependent-loan witness pinning already
  close the known escaping-mutation paths. The 2026-07-30 ruling retired an
  authored boundary write clause: do not reintroduce `stores`; remaining R5
  work is further relational candidates, finer read-consumption precision, and
  broader exact inferred summaries for currently opaque implementations.
  Exact inferred frames now cross acyclic intra-machine state transitions:
  conditional arms union, shared tail states memoize, and non-`self` state
  parameters substitute positionally back into the source/caller namespace.
  Value-position calls nested anywhere in a state body now compose through the
  shared call-frame resolver before the statement or jump: initializers,
  assignment operands, statement-call arguments, transition subjects and
  arguments, and returned values all contribute their may-write paths.
  Recursion detection is shared across statement- and value-position calls. A
  direct bare-self transition preserves the same state namespace and therefore
  retains its finite collected frame. A named edge back to that exact state
  does too only when it forwards every state parameter by exact symbol
  identity. Multi-state cycles retain the same finite summary when every edge
  closing the cycle forwards the complete parameter namespace at the same
  ordinal; computed, projected, omitted, reordered, rebinding, and truly
  unresolved cycles stay opaque, so consumers fail closed rather than
  extrapolating across changing parameter bindings.
- **STR/EFX:** the source reach clause is now canonically `reaches`; the parser
  rejects legacy `effects` with directed migration guidance, and the Omega,
  canary, sample, and Cathedral source corpora use the new spelling. Syntax,
  symbol-resolved, and typed records/snapshots now name authored reach as
  service reach; termination decrease orders use independent arenas; and
  checked admissibility reports service reach, suspension, and blocking as
  separate dimensions. Finish independent service reach, `suspends`, `blocks`,
  termination, mutation, and trust publication/admission, then retire the
  remaining legacy internal umbrella names after their consumers migrate.
  Checked facts now name their grouped suspension/blocking/call-topology root
  `operational`, retiring the ambiguous `operations` umbrella field without
  merging it with service reach or any other semantic axis. Production
  consumers now name `OperationalPlan` bindings and parameters `operational`
  throughout inference, validation, checked-fact construction, capability
  analysis, and build-time admission; genuine executable and atomic operation
  collections retain their literal `operations` name.
  Imported transparent-refinement spelling must supply the narrowed
  operational envelope consumed by the completed exact call-acknowledgement
  checker.
- **TPR4/TPR6:** connect progress-profile grants and receipts without putting
  ranking witnesses into public identity.
- **GR6:** finish remaining qualification/trust consumers.

Acceptance: each contract axis normalizes independently; a wrapper cannot
launder reach; omission remains a strict public guarantee; private proof
improvements do not change public identity.

### Multiplicity, tasks, and allocation

- **CML4:** finish structural multiplicity migration. Implement checked
  `EdgeCleanupPlan` construction after outgoing-value materialization and
  transfer-map commitment; deterministic reverse-declaration cleanup;
  contextual cleanup-contract checking; structural partial-value cleanup;
  nominal-drop partial-move rejection; repeated-cycle resource composition;
  and conservation-witness/backend-ledger reporting. Composite resource
  frontier transformations follow P1c.
- **TR3–TR8:** finish whole-call-graph WCSU derivation for the fixed nonmoving
  `StackPlan`, `StackLease` reservation, selected-provider preservation and
  cancellation conformance, transactional argument custody, park/resume
  lowering, suspension-safe loans, and reference packages. The provider-
  independent plan schema, canonical crossings, activation-wide CPU/thread
  demands, and retirement of the generalized `TaskRuntimeContract` join are
  complete. The core runtime surface is now an ordinary boundary trait, and
  concrete activation facts retain the exact selected provider plan plus
  `start`/`try_start` requirement identity. Dynamic provider-instance and
  invocation receipts now bind those facts and their preservation evidence
  before the normalized lifecycle ledger can issue a claim. The normalized
  task-stack composer now validates exact local frame summaries and derives the
  maximum aligned live chain over acyclic same-stack calls; sibling calls share
  capacity, opaque same-stack leaves require explicit admissions, and malformed
  graphs reject. Opaque leaf admissions are now sealed results: their identity
  binds the exact selected provider plan, authored requirement, independent
  admission receipt, bytes, and alignment; provider/requirement drift and
  malformed demand reject before graph composition. Compiler call-graph
  collection, binding that evidence into the emitted `StackPlan`, fixed-stack
  reservation, cancellation conformance, transactional argument custody,
  routed task-claim establishment, and task-claim provenance remain.
- **PSIIR — IMPLEMENTATION WORK:** build the terminal Psi boundary settled in
  `wiki/architecture/pipeline/terminal_psi.md`. Psi owns Omega-file parsing
  through one concrete, post-instantiation semantic module; Omega starts at
  abstract-operation lowering. The Psi-owned workspace root, stable semantic
  identities, typed scalar proposition core, module value-typing context,
  small structural proof kernel, versioned certificate envelope, total truth /
  reflexive-equality / closed-integer judgments, and sealed exact admission
  validator are live; architecture tests forbid Psi dependencies on Omega.
  Semantic v13 conditional control is live: one already-defined Boolean value
  selects between ordered true/false successor records with independent stable
  `EdgeId`s, typed block-parameter bindings, scalar binding actions, and fuel
  sites. The verifier checks acyclic CFG reachability, value dominance, and both
  successor bindings; common-path proof reconstruction does not manufacture a
  proposition for the structural branch. Canonical bytes retain both ordered
  arms, interpretation charges only the selected edge, and Omega abstract
  lowering retains canonical block entries plus both successors. The first
  checked-source producer lowers an ordered positive-Boolean/fallback branch
  whose successors bind already-defined integer entry parameters to branch
  states that may compute recursively nested landed-literal/parameter
  add/subtract/multiply expressions in settled Wrapping or Saturating domains.
  The restricted fixed-work checker derives the maximum
  acyclic branch cost and complete safe-point graph partition. That exact
  three-block shape now retains both structural and return edges plus each
  branch's integer expression and operation provenance through independent
  assigned frames, then emits executable x86-64/AArch64 conditional returns.
  General target/native block programs remain implementation work.
  The first frontend-ownership migration slice is live: `psi-source` owns
  source identities, byte spans, and source-backed text; `psi-tokens` owns the
  token representation; `psi-arena` owns generic dense, paged, generational,
  and hierarchy arena storage needed by source representations;
  `psi-diagnostics` owns target-neutral diagnostics
  and phase snapshots; `psi-language-core` owns source-level multiplicity,
  data-supply, carry, domain-body, call-acknowledgement, atomic-ordering,
  cast-form, operator-spelling, and source-assembly contract vocabulary;
  `psi-language-semantics` owns resolved semantic identities, service/domain
  tables, termination/supply plans, establishment routes, byte predicates,
  canonical const-value atoms, content algebra/projection plans, built-in value
  domains, and normalized wire scalar ranges;
  `psi-extents`, `psi-layout-plans`, and `psi-access-plans` own target-neutral
  extent authority plus normalized author-selected layout and placed-access
  semantics; their old Omega package names and unused compatibility exports
  are retired. Omega orchestration,
  provider, installation, relocation, instruction-selection, and ISA consumers
  depend on the Psi owners directly;
  `psi-numerics` owns exact
  numerics, host-independent float semantics, arithmetic domains, and literal
  payloads; `psi-source-loader` owns root-file loading; and
  `psi-symbols` owns target-neutral symbol identities and hierarchy storage.
  The unused `omega-core` source/span, exact-bignum, const-value, content,
  built-in-value-domain, byte-predicate, normalized-wire, atomic-ordering,
  cast-form, operator-spelling, inline-assembly, arena storage, diagnostics, symbols,
  resolved-language-semantics, arithmetic-domain, float-semantics, and
  literal-payload aliases are retired; their
  remaining Omega consumers depend on the Psi owners directly. `omega-core`
  now contains only Omega-owned compiler/runtime infrastructure rather than a
  second target-neutral semantic owner; the unused generic trust grant/receipt
  carrier and its Psi-semantics dependency are retired as well.
  `psi-syntax-trees` owns the parsed source representation; the unused former
  Omega compatibility package is retired.
  `psi-symbol-resolved-trees` owns the source-shaped representation carrying
  resolved symbol identities; the unused former Omega compatibility package
  is retired.
  `psi-typed-trees` owns the target-neutral typed source representation; the
  unused former Omega compatibility package is retired, and legacy backend
  consumers depend on the Psi owner directly. Typed boundary identities retain
  semantic keys and canonical fingerprints only; concrete register/stack/ABI
  calling plans remain in Omega orchestration for selected native realization.
  `psi-facts` owns durable target-neutral places, contexts, propositions, and
  checked-fact plans; the unused former Omega compatibility package is retired,
  and legacy backend consumers depend on the Psi owner directly.
  `psi-checked-trees` owns the checked semantic representation, including
  proof, borrow, flow, reach, value-origin, and admissibility evidence;
  the state/control representations and transforms, artifact/backend
  orchestration, interpreter, and backend leaf consumers depend on the Psi
  owner directly. The unused former Omega compatibility package is retired.
  `psi-validation` owns target-neutral cross-semantic source validation. The
  unused former `omega-validation` compatibility package is retired; its
  integration tests remain in the architecture harness, where they also cover
  the separate Omega provider-admission subsystem.
  `psi-types` owns the unresolved source type-surface analysis; the unused
  former `omega-types` package is retired.
  `psi-proof` owns source proof-surface collection, obligation planning, and
  checking; the unused former `omega-proof` package is retired.
  `psi-typed-trees-to-checked-trees` owns semantic checking and checked-fact
  construction; the unused former Omega compatibility package is retired.
  Boundary provider approval now runs explicitly in Omega orchestration after the Psi
  check instead of entering the checker dependency graph.
  `psi-effects` owns operational ceilings, service reach, synchronous
  invocation inference, and capability-flow facts; target-neutral consumers
  depend on it directly. `omega-effects` no longer re-exports that vocabulary.
  Provider declarations,
  target/provider bindings, approval, installation, and the exact selected-plan
  carrier remain Omega-owned. `CheckedTrees` no longer embeds that concrete
  selection or target/layout-specific task activation plans; Omega
  orchestration threads both sidecars beside checked semantics until the
  terminal-Psi cut retires the legacy backend lane.
  `psi-source-files-to-tokens` owns Omega lexing and
  `psi-tokens-to-syntax-trees` owns unresolved parsing, both with no Omega
  dependency. `psi-syntax-trees-to-symbol-resolved-trees` owns name lookup,
  source-scope resolution, and stable symbol stamping;
  `psi-symbol-resolved-trees-to-typed-trees` owns type identity,
  compatibility, and signature normalization. The five former Omega-named
  source-to-checked pipeline packages have been retired after their last
  backend and validation harness consumers moved to Psi. `omega-compiler` invokes the Psi-owned lexer, parser,
  resolver, typer, checker, and source representations directly; it no longer
  reaches them through those compatibility packages. The driver still threads
  checked semantics into the legacy Omega state-graph lane until general
  terminal production replaces that early boundary.
  `psi-checked-trees-to-terminal` owns the first exact,
  fail-closed checked-to-terminal source slice. The reverse-named Omega package
  has been removed; general terminal production grows only in this Psi stage.
  Its independent content-evidence producer now revalidates checked
  conservation, reshuffle, and direct partition-composition facts into terminal
  v9-v14 rows. **TERMINAL-CONTENT-ENTRY-CLAIMS — COMPLETE:** terminal v14 adds a
  fingerprinted machine-local entry-claim binding row containing dense claim
  identity, projection, algebra, and entry structural place, with no output and
  no equality assertion. Partition-composition rows reference those bindings;
  `ContentIdentityReshuffle` remains exclusively the one-to-one equality case.
  The verifier checks unique canonical bindings and permits later content axioms
  to reference them independently; the proof adapter does not expose a binding
  itself as an axiom. The semantic module, codec, verifier, proof adapter,
  checked facts, producer, debug presentation, archived migration, and golden
  identities are versioned together. The executable content canary is now
  unblocked.
  The first in-memory executable slice is also live: stable machine/block
  topology, representable integer constants, v2 Boolean constants, v3
  exact-width wrapping integer addition, v4 exact-width saturating integer
  addition, v5 exact-width wrapping integer subtraction, v6 exact-width
  saturating integer subtraction, v7 exact-width wrapping integer
  multiplication, v8 exact-width saturating integer multiplication, v9
  proof-only structural-place/content-conservation propositions, v10 canonical
  identity-preserving claim reshuffles, v11 stable sum-case content paths, and
  v12 exact authored-partition substitution rows, v13 structural Boolean
  conditional edges, v14 independent entry-claim bindings, v15 total Boolean
  logical negation, v16 nominal proposition declarations/applications, and
  current-v17 total Boolean equality; the
  executable slice retains unconditional
  jump/return edges plus the ordered conditional,
  bodyful contracts, verifier-reconstructed semantic axioms, exhaustive proof-
  bundle checking, and direct execution of the verified module in
  `omega-interpreter`. Its validator rejects unreachable axiom sources and
  entry/postcondition references to internal values. The first Psi-owned
  checked-tree producer is also live and fail-closed: its linear integer form
  accepts any sequence of ordinary integer machine parameters, including none,
  and any sequence of at least two states. It lowers a recursively nested
  parameter/literal add/subtract/multiply expression in the settled Wrapping or
  Saturating domain into every unconditional jump argument and continues
  through the complete ordered sequence of ordinary integer parameters in each
  non-entry state. Every argument must exactly match its target parameter type.
  Optional compile-known propagation crosses every binding, so a closed chain's
  recomputed result must match its authored reflexive contract.
  A second exact form lowers any sequence of ordinary primitive-integer machine
  parameters, including none, and an exact literal, named parameter, or
  recursively nested parameter/literal expression using builtin
  add/subtract/multiply in a settled arithmetic domain. A third exact form
  lowers a recursively nested Boolean expression over literals, exact named
  parameters, builtin negation, builtin equality/inequality, and short-circuit
  `&&`/`||`, either
  directly or through a nonempty linear sequence of unconditional one-parameter
  Boolean state bindings. Optional compile-known
  propagation also requires its result to match the closed reflexive contract.
  A fourth exact form lowers an ordered positive-Boolean/fallback conditional.
  Each arm binds an ordered sequence of already-defined integer entry
  parameters to exactly typed branch parameters, then returns a recursively
  nested landed-literal/parameter add/subtract/multiply expression in a settled
  Wrapping or Saturating domain. A fifth exact form lowers the same ordered
  conditional shape for ordinary Boolean entry/branch parameters. Its positive
  guard and both branch returns accept the recursive Boolean vocabulary,
  including short-circuit control; guard decisions target the selected branch
  directly, and branch-local decision trees return only from the selected arm.
  It emits the module and
  proof bundle separately; real-source canaries cover all six versioned integer
  policy operations in constant-fed and runtime-fed forms, Boolean literal,
  ninth-parameter direct and three-state bound returns, a direct closed integer
  literal, a closed three-state integer chain, plus a ninth-parameter integer
  stack return after `CheckedTrees` are dropped. Constant-fed
  wrapping add, the direct stack return, and a register-plus-stack runtime
  wrapping add all reach emitted host machine code; Boolean literal, ninth-
  parameter direct return, and the three-state Boolean binding chain
  independently cover constant materialization, the host incoming-stack ABI,
  and source-independent jump binding. A nested wrapping
  add-then-multiply source expression does too. A parameterized two-state
  canary combines a register and ninth stack argument before an unconditional
  jump, continues from the bound block parameter, and agrees across fixed-fuel
  derivation, interpretation, and emitted host execution. A parameterized
  three-state companion repeats the computed binding, while a closed
  three-state companion begins from a literal; both agree at their eight-unit
  ceilings, and the closed twin with an unrelated contract rejects. A
  multi-binding three-state companion carries two independently computed values
  across both edges and agrees across its ten-unit certificate, interpretation,
  and emitted host execution.
  The source conditional survives frontend disposal, executes either arm with
  only its selected edge charged, retains both successors through the Omega
  abstract boundary, and reaches emitted host machine code for both arms. Its
  two-binding branch-local arithmetic paths each have a five-unit fixed-work
  certificate. A Boolean-literal selector now keeps the verified two-successor
  terminal graph but folds at Omega target lowering, retaining only the selected
  arm's operations and two edges in emitted provenance. The target/native
  continuation remains deliberately exact: runtime Boolean selection between
  two integer expressions is live, with each branch independently assigned and
  emitted. Each arm may traverse acyclic computed unconditional bindings and
  converge on a shared tail; provenance canonically retains every unique
  contributing operation and edge. A compile-known nested conditional inside
  either arm now folds to only its selected successor, including scalar-typed
  Boolean or integer edge bindings, while the outer runtime branch remains
  executable. Runtime-nested acyclic conditionals now lower as recursive
  target and assigned control, with each integer-returning leaf independently
  framed and emitted on x86-64 and AArch64. Conditional tests preserve all ABI
  inputs before leaf evaluation, including an AArch64 condition outside `x0`.
  The same acyclic block walker now starts at the actual entry, so any computed
  unconditional prefix before the first runtime branch retains its bindings,
  fuel edge, and canonical provenance through native emission. Boolean-result
  CFGs now use parallel recursive target/assigned control with immediate or
  ABI-parameter leaves and execute natively on both architectures. Cyclic
  semantics, reusable native block layout, and operations beyond the current
  scalar terminal vocabulary still fail closed.
  The real-source Boolean conditional crosses that complete verified, metered,
  interpreted, assigned, and native-emitted lane after the Psi frontend is
  dropped.
  Its replaceable debug map now pins generated operations/values to exact
  expression spans and every edge to either the explicit transition arrow or
  the implicit returned expression, rather than falling back to a whole state.
  Because the
  legacy exit prover cannot establish ordinary `result == literal` contracts,
  this bootstrap canary preserves a closed typed `requires`/`ensures` fact and
  asserts the returned `i32` independently; do not generalize that workaround
  into the target Psi frontend. The first source-independent Omega consumer is
  live too: it accepts only `VerifiedTerminalModule` and emits an owned stream
  of scalar materializations, explicit jump bindings, and return requirements
  with stable Psi provenance. Its representation and lowering crate contain no
  checked/typed-tree, `ExpressionHandle`, or legacy `StateKey` dependency. The
  first clean target/native continuation is live as well: it resolves the
  verified constant/jump chain to a provenance-retaining target
  return-immediate, emits AArch64 and x86-64 scalar-return bytes, rejects
  unsupported widths, and a linker-harness canary proves host execution matches
  terminal interpretation after producer/intermediate state is dropped. A v2
  Boolean canary now traverses that same verified/fueled/lowered path and
  executes native zero/one return bytes on the host. The first arithmetic
  vertical slice is live in v3: `WrappingIntegerAdd` requires two already
  defined operands of the exact result integer type, reduces unsigned values
  modulo the declared 1–128-bit width, and reinterprets signed reduced bits as
  two's complement. It is total and creates no overflow obligation. The
  verifier reconstructs its exact result-term axiom; closed wrapping terms are
  decided by the existing total integer-relation judgment; the interpreter and
  schedule-v1 meter execute it; and the clean Omega lane retains its operation
  provenance while reducing the current compile-known scalar slice. A canonical
  v3 `u8` 200+100 canary verifies, costs four units, lowers, emits, and executes
  as 44 through both the host linker harness and standalone Mach-O image after
  semantic/lowering state is dropped. The next arithmetic vertical slice is
  live in v4: `SaturatingIntegerAdd` has the same exact defined-operand/type
  requirements, clamps at the declared signed or unsigned bounds, is total,
  creates no overflow obligation, and reconstructs a distinct exact result-term
  axiom. Signed, unsigned, sub-width, and 128-bit semantic edges are covered. A
  canonical `u8` 200+100 canary verifies, costs four units, lowers with retained
  provenance, emits, and executes as 255 through the host linker harness and
  standalone Mach-O image after semantic/lowering state is dropped. The
  first runtime-parameter realization slice is live without a semantic-version
  change: machine parameters were already part of terminal Psi. The clean
  abstract function now retains declared parameter/result identities and
  scalar types; target lowering uses the established AAPCS64, System V AMD64,
  or Microsoft x64 call planner; and machine emission returns supported Boolean
  or 8/16/32/64-bit integer parameters from either their selected register or
  incoming stack slot. A frozen-v1 nine-`u8` canary forces the ninth argument
  through the host stack ABI, costs only its one return edge, and returns 77 in
  both the verified interpreter and a real C-linker invocation after semantic
  and lowering state are dropped. Parameter-fed runtime arithmetic is live as
  recursive target expressions without a semantic-version change. Compile-known
  subexpressions still fold; mixed immediate/register/stack wrapping and
  saturating additions emit for signed or unsigned 8/16/32/64-bit integers on
  AArch64 and x86-64. The first explicit terminal assigned-target stage now
  validates selected parameter registers against the exact architecture,
  rejects repeated-parameter home drift, and assigns referenced AArch64
  argument registers to stable aligned frame spills before evaluation into
  `x0`. Terminal machine emission accepts only that assigned representation;
  scratch-conflicting x86-64 inputs likewise receive stable frame spills, and
  `rsp` rejects as an expression-parameter home. Both emitters retain the
  original incoming-stack base across recursive evaluation. A nested v4
  `u8` canary consumes the ninth stack argument, wraps to 4, saturates to 255,
  and matches interpretation through a real C ABI call; a signed `i64` canary
  independently reaches both saturation bounds. General value liveness, spill
  reuse, and non-scalar assignment remain later implementation work. The next
  arithmetic slice is live in v5:
  `WrappingIntegerSubtract` requires two defined operands of the exact result
  integer type, reduces `left - right` modulo the declared 1–128-bit width, and
  reinterprets signed reduced bits as two's complement. Its verifier axiom,
  closed proof term, interpreter, schedule-v1 charge, abstract/target lowering,
  constant folding, and AArch64/x86-64 runtime emission are live. Canonical
  semantic v5 and minimal proof format v4 have distinct golden identities while
  archived v1–v4 semantic and v1–v3 proof identities remain frozen. A
  parameter-fed `u8` canary round-trips, verifies, costs one operation plus one
  return edge, and agrees with real C ABI execution at 5-10 = 251. The
  next arithmetic slice is live in v6: `SaturatingIntegerSubtract` has the same
  exact defined-operand/type requirements, clamps `left - right` at the
  declared signed or unsigned bounds, is total, and creates no overflow
  obligation. Its verifier axiom, closed proof term, interpreter,
  schedule-v1 charge, abstract/target lowering, constant folding, and
  AArch64/x86-64 runtime emission are live. Canonical semantic v6 and minimal
  proof format v5 have distinct golden identities while older identities
  remain frozen. A parameter-fed signed `i64` canary round-trips, verifies,
  costs one operation plus one return edge, and reaches both saturation bounds
  through real C ABI calls. The
  next arithmetic slice is live in v7: `WrappingIntegerMultiply` requires two
  defined operands of the exact result integer type, reduces their product
  modulo the declared 1–128-bit width, and reinterprets signed reduced bits as
  two's complement. Its verifier axiom, closed proof term, interpreter,
  schedule-v1 charge, abstract/target lowering, constant folding, and
  AArch64/x86-64 runtime emission are live. Canonical semantic v7 and minimal
  proof format v6 have distinct golden identities while older identities
  remain frozen. A parameter-fed `u8` canary round-trips, verifies, costs one
  operation plus one return edge, and agrees with real C ABI execution at
  20*13 = 4. The next arithmetic slice is live in v8:
  `SaturatingIntegerMultiply` has the same exact defined-operand/type
  requirements, clamps the product at the declared signed or unsigned bounds,
  is total, and creates no overflow obligation. Its verifier axiom, closed
  proof term, interpreter, schedule-v1 charge, abstract/target lowering,
  constant folding, and AArch64/x86-64 runtime emission are live. Canonical
  semantic v8 and minimal proof format v7 have distinct golden identities
  while older identities remain frozen. A parameter-fed signed `i64` canary
  round-trips, verifies, costs one operation plus one return edge, and agrees
  with real C ABI execution across positive overflow, negative overflow,
  `MIN * -1`, and an ordinary negative product. The first unary Boolean slice
  is live in v15: builtin `!` lowers to total `BooleanNot` operations and exact
  recursive proposition terms, rejects custom operator resolution and
  non-Boolean operands/results, reconstructs its semantic axiom, costs one
  schedule-v1 unit, interprets directly, and lowers through the clean Omega
  abstract/target/assigned stages. Canonical semantic v15 and minimal proof
  format v10 have distinct golden identities while archived semantic v1-v14
  and proof v1-v9 identities remain frozen. A real checked-source canary
  round-trips, verifies, costs one operation plus one return edge, and returns
  the complemented canonical Boolean through real C ABI execution; exact
  AArch64 and x86-64 encodings are pinned independently. The
  next Boolean slice is live in v17: builtin `==` over two Boolean operands
  lowers to total `BooleanEqual` operations and exact recursive proposition
  terms, reconstructs its semantic axiom, costs one schedule-v1 unit, and
  interprets directly. Canonical semantic v17 and minimal proof format v11 have
  distinct golden identities while archived semantic v1-v16 identities remain
  frozen. A checked-source canary compares a runtime parameter with `false`,
  round-trips and verifies the terminal module, and agrees with native C-ABI
  execution after clean Omega lowering folds the literal comparison to the
  existing canonical Boolean target forms. A second canary compares two runtime
  Boolean parameters through recursive target/assigned Boolean expressions;
  AArch64 and x86-64 emission preserve both ABI inputs and return canonical
  zero/one equality results. Builtin Boolean `!=` canonically composes the same
  equality operation with `BooleanNot`, verifies and meters both semantic
  sites, and exercises nested target/assigned Boolean emission without adding a
  redundant terminal opcode. Direct-return `&&`/`||` now use the required
  control lowering: Psi emits an acyclic terminal decision tree, the deciding
  left operand bypasses the right subtree with a smaller measured fuel path,
  and recursive Boolean target/assigned control executes both operators on
  AArch64 and x86-64 without adding eager logical opcodes. Recursive Boolean
  target expressions may now drive those control nodes or appear at their
  return leaves, so comparisons such as `(a == b) && (b == c)` preserve
  short-circuit fuel and execute natively. Linear Boolean state chains now use
  the same decision trees for both carried jump bindings and final returns;
  canonical Boolean leaves converge through ordinary block-parameter bindings.
  Explicit Boolean conditionals compose the same control in both their guard
  and return arms while preserving branch bindings and selected-path fuel.
  Value-producing decision trees now also admit short-circuit expressions as
  either operand of equality/inequality: each selected leaf retains the
  explicit v17 `BooleanEqual` operation (and canonical `BooleanNot` for
  inequality), so its proof axiom and fuel unit are not erased by control
  lowering. The
  initial vocabulary now has canonical semantic bytes and a domain-separated
  semantic fingerprint as well: decoding rejects alternate encodings, invalid
  modules, and trailing data, while a golden identity test freezes the format.
  Canonical proof-bundle bytes and a separate golden proof fingerprint are live
  too, covering kernel, certificate, and admission evidence with independent
  proof-system versions. Proof format v1 remains byte-frozen for the original
  vocabulary; minimal format v2 adds recursive wrapping-add scalar terms;
  minimal format v3 adds recursive saturating-add scalar terms; minimal format
  v4 adds recursive wrapping-subtract scalar terms; minimal format v5 adds
  recursive saturating-subtract scalar terms; minimal format v6 adds recursive
  wrapping-multiply scalar terms; minimal format v7 adds recursive
  saturating-multiply scalar terms; minimal format v8 adds content-conservation
  propositions and their structural-place terms; minimal format v9 adds
  sum-case structural paths; minimal format v10 adds recursive Boolean-not
  terms; and minimal format v11 adds recursive Boolean-equality terms. All reject over-deep or
  unnecessarily newer encodings. A role-domain-separated
  manifest binds semantic,
  proof, optional installation, and optional debug section identities; proof,
  provider-record, or debug replacement changes container identity without
  changing semantic identity. The real-source canary encodes both semantic and
  proof sections, discards the complete producer output, decodes and validates
  their manifest, and then compares interpreted/native behavior. Semantic v1
  remains decodable, verifiable, executable, and frozen to its integer
  vocabulary; v2 adds `BooleanConstant`; v3 adds `WrappingIntegerAdd`; v4 adds
  `SaturatingIntegerAdd`; v5 adds `WrappingIntegerSubtract`; v6 adds
  `SaturatingIntegerSubtract`; v7 adds `WrappingIntegerMultiply`; v8 adds
  `SaturatingIntegerMultiply`; v9 adds proof-only structural places and
  content-conservation propositions; v10 adds canonical identity-preserving
  claim reshuffles; v11 adds stable sum-case content-path segments; v12 adds
  exact authored-partition substitution rows; v13 adds structural Boolean
  conditional control; v14 adds independent dense entry-claim bindings;
  v15 adds total `BooleanNot`; v16 adds proposition declarations and normalized
  applications; current v17 adds total `BooleanEqual`; and explicit validated
  migration preserves an older semantic graph while producing a new v17 fingerprint.
  Archived v1 through v16 identities
  remain frozen. The clean
  lane now also constructs an owned, semantic-identity-bound object artifact
  with canonical function spans and retained Psi provenance, emits the Omega
  object container plus ELF/AArch64, ELF/x86-64, Mach-O/AArch64, and PE/x86-64
  standalone images, and rejects altered final text or unclassified executable
  gaps. Source and Boolean canaries emit after producer/intermediate state is
  dropped; on the macOS host they directly execute the emitted Mach-O image. A
  canonical typed installation-record v1 now separately binds the terminal
  identity, exact target facts, PE subsystem when applicable, profile decision,
  sorted selected-provider-plan identities, emitted-image SHA-256, and compiler
  text-validation evidence. Its hostile-input decoder rejects alternate order,
  malformed target facts, unknown tags/versions, truncation, and trailing data;
  the source canary binds those exact bytes into the role-separated artifact
  manifest after producer and intermediate lowering state is dropped. The
  current scalar slice honestly records an empty provider closure; later
  call/boundary slices must populate it from their selected plans. Canonical
  typed debug-map v1 now binds exact terminal semantic identity to ordered
  source origin/path/length/digest rows and bounded spans over stable semantic
  subjects; wrong-module attachment, unknown subjects/files, invalid spans,
  alternate order, and hostile bytes reject. The checked-source producer now
  publishes retained authored declaration spans for machines, blocks, edges,
  operations, values, contracts, and obligations; the real-source canary
  round-trips those bytes after checked trees are dropped and binds them into
  the manifest's debug role without changing semantic identity. Psi expression
  tables now preserve authored integer/Boolean-literal and operator-token spans
  through checked trees, and terminal operation/result-value rows use those
  exact sites. Authored transition arrows likewise survive into terminal jump
  edges; synthesized return edges retain their source-state declaration
  fallback. Terminal contract and obligation subjects now use the exact
  authored `ensures` fact site instead of the enclosing machine declaration.
  Broader register assignment, further closed arithmetic variants,
  and migration of the legacy backend remain.
  Move or rename the current target-neutral `omega-*` frontend crates under Psi
  ownership as each slice migrates; do not leave parsing or checking on an
  Omega-to-Psi path. With the initial interpreter, lowering customers, and
  canonical semantic codec live, next add operations in vertical slices
  containing execution
  semantics, generated obligations, sound proof rules, interpreter behavior,
  Omega lowering requirements, and canonical encoding. Merge the useful
  `StateGraph`/`ControlFlowPlan` topology, replace every `ExpressionHandle` with
  lowered values, predicates, structural places, operations, and edge actions,
  and keep author-declared hardware geometry while excluding target-selected
  ABI/storage realization. Re-root the interpreter and then abstract-operation
  construction, moving binding substitution and concrete instantiation above
  the boundary. Freeze serialization and fingerprints only after the in-memory
  vocabulary passes both paths. Keep semantic module, proof bundle,
  installation record, and debug/source maps separate; carry certificates for
  any non-total search needed by portable verification. Acceptance: one
  integer/control/contract canary serializes canonically, verifies after source
  and producer state are discarded, and produces identical interpreted and
  native behavior; no Omega-side lowering crate used by that path depends on
  `TypedTrees` or `ExpressionHandle`. **Initial acceptance canary complete;**
  continue vocabulary and ownership migration in the Psi producer rather than
  reintroducing a reverse bridge. The producer exposes one exact executable
  integer/control/contract `lower_machine` entry plus independent checked
  content-plan, identity-reshuffle, and partition-composition translators. An
  architecture test pins exactly one `lower_machine` entry and rejects return
  of the deleted Omega-to-Psi package.
- **IRFUEL — IMPLEMENTATION WORK:** implement the settled
  `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md` sequence:
  versioned terminal Psi and fuel schedule, evaluator/interpreter metering,
  restricted fixed-work checking over entries and safe-point segments,
  attributed response outcomes, and trusted native block metering. Keep target
  WCET and wall-clock conversion separate. The external-root precursor already
  has schedule-keyed provider summaries and provisions, rejects mixed
  schedules, and reports logical fuel rather than structural work; continue
  from terminal Psi and its interpreter meter rather than treating that
  provider-authored precursor as a Psi proof. The terminal-Psi
  v1/v2/v3/v4/v5/v6/v7/v8/v9/v10/v11/v12/v13
  schemas, serialization, migration, and verifier/lowering boundary are owned
  by PSIIR.
  The current TypedTrees evaluator now publishes an explicitly versioned deterministic
  step-usage record for interpreted and
  build-time outcomes; it is telemetry precursor evidence, not terminal-Psi
  fuel. The first terminal-Psi fuel slice is now live: a `psi-core`-owned
  nonzero schedule identity independently versions a v1 one-unit-per-operation/edge
  table, the verified interpreter returns checked deterministic totals and
  stable operation/edge attribution, and an optional sponsor allowance fails
  atomically before an unpaid semantic site. The serialized source canary costs
  four units without changing its semantic fingerprint. Schedule v1 also
  charges v3 wrapping-add, v4 saturating-add, v5 wrapping-subtract, v6
  saturating-subtract, v7 wrapping-multiply, and v8 saturating-multiply
  operations one unit. The closed v3/v4 arithmetic canaries each total four
  units; the parameter-fed v5/v6/v7/v8 arithmetic canaries total two.
  Explicit in-memory
  execution state preserves the exact cursor and values across exhaustion;
  checked allowance replenishment resumes at the unpaid edge without replaying
  or double-charging earlier work. The first restricted checker now derives an
  exact, producer-independent entry-to-return certificate for the current
  acyclic single path, keyed by canonical semantic identity, entry, return
  edge, and schedule; validation recomputes every field, and the source
  canary's four-unit certificate equals measured usage. Exact selected
  block-to-edge segment certificates also recompute against that identity and
  schedule, include the endpoint charge, and reject an edge not reached before
  return. A memoized acyclic CFG walk derives the maximum conditional path
  without summing mutually exclusive arms. The current-vocabulary semantic
  safe-point selector partitions the complete reachable graph at every explicit
  jump, conditional, or return edge in canonical block/edge order; validation
  rejects omitted or reordered segments. Build-time migration, loop
  certificates, response outcomes, and trusted native metering remain. Honest
  attributed response outcomes are implementation-sequenced after terminal Psi
  gains wait/foreign-edge variants and their finite-response contract field;
  the current total jump/return vocabulary can derive `Bounded` but has no
  semantic edge from which to validate `NoFiniteGuarantee(edge)`. Do not expose
  a producer-authored attribution carrier before the verifier can recompute it.
  External-root provider summaries and provisions now use the Psi-owned fuel
  schedule identity directly. Local summary evidence now separates sealed
  terminal-Psi entry/segment certificates from admitted opaque-provider unit
  claims; certificate units and schedule are derived, provider receipts exclude
  them, and the external-root report retains exact terminal identity. The
  real-source four-unit canary now crosses the generic installation ladder and
  composes only after a sealed binding proves exact relocation-free frozen
  bytes, architecture, installed-code context, and selected function offset.
  External-root installation rechecks that the root summary is a whole-entry
  certificate for the exact root code/stub; a segment-only root fails closed.
  Migrate Cathedral's hard-root graph; opaque provider leaves remain admitted
  summaries. Because this carrier joins selected installation state with
  terminal semantic certificates, `omega-external-roots` lives in Omega
  orchestration rather than foundation.
- **FFIVAL:** validate the settled boundary model before adding any new
  construct. The returned-custody-from-borrow rejection canary now lands
  through content-algebra facts. The provider-independent executor-selection
  gate now consumes exact per-axis checked/admitted evidence identities,
  rejects a CPU- or host-thread-affine activation when the selected executor
  lacks the matching axis, and retains the validated selection in task
  lifecycle custody; the source activation now binds the exact selected
  `TaskRuntime` plan and operation requirement, while per-invocation evidence
  remains under TR3–TR8. Then
  implement a narrow Windows `user32` slice:
  `RegisterClassEx`, `CreateWindowEx`/`WM_NCCREATE`, `GetMessage`,
  `DispatchMessage`, `DefWindowProc`, `DestroyWindow`, and
  `UnregisterClass`. It must express bootstrap-to-steady callback recovery,
  pinned-thread blocking, registration custody, thunk calling/state plans, and
  cycle breaking with existing machinery.
- **TCBMANIFEST:** derive executable TCB metadata from selected-provider closure
  rather than source reach. Retain exact provider/executable/plan identity,
  implementation evidence, static-selection versus Omega-runtime-admission
  origin, execution scope, and independently evidenced memory, termination,
  fault, and resource containment guarantees. Report known entries separately
  from `Complete(scope, evidence)` or attributed `Incomplete(scope, causes)`;
  an uncontained opaque in-process provider forces incompleteness. Make
  platform baselines ordinary profile allowlists, preserve fixed package
  selections transitively, and let profiles permit-and-mark or reject before
  installation. Add canaries showing that a checked wrapper cannot launder the
  entry, static import adds no loader reach, explicit runtime loading does, and
  the runtime ledger claims only Omega-mediated admissions. The first artifact
  slice now derives known checked-machine and compiler-intrinsic entries from
  the exact selected-plan closure, keeps completeness separate, and attributes
  every unpinned in-process import/vtable/table row as an opaque cause instead
  of treating its loader spelling as executable identity. Pinned opaque
  admissions now bind the exact selected provider-plan, method, requirement,
  and opaque binding before contributing a stable artifact identity plus
  independent implementation and containment receipts. Pinning alone retains
  the attributed incompleteness cause; only a separate executable-closure
  receipt removes it, and closure evidence survives when another row keeps the
  overall scope incomplete. Selected closures can now be assigned a nonzero
  isolated execution-scope identity before opaque admissions. A validated
  manifest set retains an exact admitted endpoint in the parent manifest and a
  separately addressable child manifest, rejects scope drift and duplicate
  scope attachment, and keeps parent/child completeness and profile evaluation
  independent. The JSON artifact surface preserves that separation and the
  manifest admission receipt. The append-only runtime ledger now admits pinned
  executable identities only through an
  Omega-mediation receipt, rejects receipt replay and duplicate artifact rows,
  and unions its canonical entries with the static manifest under the exact
  same scope. Runtime entries retain their distinct origin, implementation and
  containment evidence; a missing executable-closure receipt contributes an
  attributed runtime incompleteness cause, while valid closure evidence remains
  visible even beside another cause. Repeated union is idempotent, and the JSON
  artifact surface renders runtime entries, causes, and evidence distinctly.
  The compiler now accepts a deployment-owned TCB build-policy carrier
  separately from source syntax, binds its opaque admission candidates against
  the exact selected rows, evaluates the selected profile, and carries the
  sealed acceptance to the filesystem installation gate. The legacy compile
  entry remains source-compatible and delegates with no selected profile. The
  normalized profile gate now allows static current-artifact checked bodies
  only through an explicit class rule; every compiler-known or opaque entry
  otherwise needs an exact provider/plan/executable/evidence/origin/scope
  allowance and any required containment axes. Incomplete scopes either reject
  or produce a sealed acceptance marked with the original causes, retaining
  the exact manifest and profile against replay. Source/package selection of
  named build profiles remains ordinary Build API work; the exact method names
  are not yet designed.
- **REPLACE-OPAQUE:** extend component acceptance tests with selected-provider
  manifest union across coexisting eras, process-static service handover
  contracts, and mapping reuse only after proof that no live authority reaches
  the cohort. Proven quiescence permits ordinary reuse; an incomplete drain or
  a possible opaque holder reserves an unmapped/trapping quarantine
  with attributed capacity loss. A stale call must fault without being reported
  as discharged, and an opaque callback into replaceable code must use a
  process-lifetime gateway or an accepted unregister/quiescence contract. The
  executable-installation ladder now implements the fail-closed mapping half:
  an exact replacement receipt must prove execute removal, unmapping/trapping,
  and continued range reservation before consuming installed code into
  quarantine. The retained record attributes incomplete drain or possible
  opaque custody, reports exact virtual-address capacity loss, and produces
  only a non-discharging stale-entry fault. Quiescent retirement remains the
  sole route that returns reusable W+NX placement. A profile-accepted live
  manifest set now retains the process-static baseline separately from sorted,
  exact component-era identities and unions their executable subjects with
  source attribution. It rejects scope drift and duplicate/zero eras, makes
  any incomplete source weaken the live report without fabricating a combined
  selected-provider closure identity, and counts a containment axis only when
  every contributing row carries independent evidence. Process-static
  services now publish and enforce one exact duplicate-key, versioned-key, or
  atomic-transfer policy. Failed registrations return their candidate; exact
  version pairs reject; and atomic handover requires a non-replayed receipt
  binding service contract, old/new era and registration identities, atomic
  publication, old-registration retirement, and explicit obligation transfer.
  Completion records that obligations moved rather than disappeared.
  Opaque callback admission now has only the two settled replacement-safe
  routes. A process-lifetime gateway exact-binds installed code and entry,
  proves the foreign target and current-era dispatch contract, and retains a
  code borrow with no retirement operation. A direct reclaimable root instead
  owns the installed external-root handle until an exact provider unregister
  receipt and the existing independent unreachability/quiescence receipt both
  succeed; either failed gate returns all linear inputs. The profile-gated era
  ledger now binds one exact binding/entry contract and admitted entry plan,
  atomically publishes a new current era while closing the previous era to new
  entry, retains already-entered old invocations, rejects invocation/publication
  replay, and enforces the live-era limit. Only zero active entries, zero
  residual cohort holds, complete dispositions, and a fresh release receipt
  make a noncurrent era quiescent and remove its TCB manifest. This scoped
  REPLACE-OPAQUE acceptance slice is complete; runtime-specific entry algorithms
  and broader component artifact/migration encoding remain under the separate
  compilation work.
- **BLOCKEXEC:** provide an ordinary package-level blocking executor for
  codec-style native calls using activations, bounded queues, moved custody,
  linear completion claims, suspension, and provider selection. It is not a
  language call kind or plan axis. Document that an in-process worker cannot be
  killed safely, an orphan pins its worker/storage/provider era, and bounded
  recovery from a hung call requires process isolation.
- Build the package-level bump-allocation canary after
  `CONSERVATION-CONTRACT`. Core supplies qualified `Extent`, placement, and
  conservation; it does not bless Arena, bump, slab, pool, buddy, or heap
  strategy semantics.
- Implement owned `Vec<T>` and then `Vec<u8>::Utf8` through ordinary data and
  domain qualification. Before growable containers select an allocator,
  specify separately whether their requirement needs cleanup/retirement,
  authority return, or immediate capacity reuse; reusable fragmented
  allocation remains a container/backend dependency rather than a new owner
  question.

Acceptance: linear debt cannot disappear through aggregation or bulk reclaim;
CPU/thread-restricted activations require selected preservation evidence; task
and allocation handles expose no compiler-owned stack/control storage.

### Mathematical and float libraries

- **N6:** implement law-bearing relations and quotient evidence in the ordered
  sequence fixed by
  `wiki/design_briefs/law_bearing_relations_and_quotients.md`:
  1. **PROP-FAMILY-SURFACE — DESIGN CLEAR:** land the dedicated
     `proposition` declaration and binder kind, primitive and witness-bearing
     forms, transparent `=` aliases, proposition application in fact
     positions, and normalized proposition identity. Proposition braces name
     the one canonical carrierless evidence interface; they are not executable
     machine bodies. Transparent aliases expand before terminal identity and
     survive only in source/debug maps. Source slice landed 2026-08-03:
     primitive, single-evidence, and transparent declaration forms now have
     dedicated syntax nodes, deep-copy/source-identity/snapshot retention, and
     reject runtime return or executable/ambiguous body shapes. The syntax to
     resolved boundary deliberately fails closed until its dedicated
     proposition symbol and proof-static binder telescope land. Resolved slice
     landed 2026-08-03: proposition declarations now receive a distinct root
     `Proposition` symbol, value parameters and type/const binders retain
     lexical symbols, and machine-index binders receive a deliberately
     non-callable `PropositionMachineParameter` identity. Witness types and
     direct transparent proposition expansions resolve in that telescope;
     typed slice landed 2026-08-03: declarations retain their proof-only body
     classification, direct proposition applications become a distinct proof
     fact, machine-index and value arity are checked, runtime-value use rejects,
     monomorphization deep-copies the application, and checked fact payloads
     preserve its proposition identity without coercing it to `bool`.
     Normalization slice landed 2026-08-03: transparent proposition chains
     expand before identity (including Boolean-backed aliases), cycles reject,
     fact-only versus witness-bearing classification enters the canonical
     identity, call/operator substitutions retain caller terms, and requires
     discharge uses exact normalized proposition matching rather than the
     legacy unknown-payload fallback. Type/const proposition application,
     generic proposition binders, selected witness evidence, and
     self-contained terminal-Psi declaration/application identity remain in
     this rung. Checked-introduction slice landed 2026-08-03: proposition
     arguments receive declaration-type validation; ordinary checked machines
     may forward established facts or cite a checked/accepted contract after
     discharging its normalized proposition requirements, while an empty
     ordinary body can no longer invent a primitive proposition ensure;
     generic-binder source slice landed 2026-08-03: trait headers accept the
     distinct `<proposition Relation>` kind only with one mandatory authored
     `where proposition Relation(...)` application signature; source copying,
     identity, snapshots, duplicate/missing-contract diagnostics, and
     non-trait rejection are covered. Symbol resolution deliberately fails
     closed until the signature receives a dedicated resolved/typed kind and
     proposition applications can target it. Generic-binder semantic slice
     landed 2026-08-03: proposition-family parameters and their value
     signatures now retain dedicated resolved/typed kinds, lexical symbols,
     parameter type resolution, source/typed identity, and proof-fact
     normalization; trait requirement contracts can apply the abstract family
     with arity and value-type validation while runtime use remains rejected.
     Concrete-substitution slice landed 2026-08-03: trait applications resolve
     proposition slots contextually (without admitting propositions as value
     types), validate a concrete declaration's value signature after ordinary
     type-parameter substitution, reject category/signature mismatches, and
     forward abstract proposition parameters through composed traits. Indexed
     telescope slice landed 2026-08-03: a relation over an indexed proof
     carrier must instantiate one fresh ordered copy of the carrier's complete
     static-parameter telescope per representative, with matching binder kind,
     const type, and exact binder use; accidental index reuse rejects.
     Nullary proposition-law conformance slice landed 2026-08-03: a
     single-requirement proof conformance substitutes the selected concrete
     proposition family into the trait law and requires an exact normalized
     proven `ensures`; proving another proposition no longer passes merely
     because proposition facts sit outside the legacy equality-law matcher.
     Type/const application slice landed 2026-08-03: proposition calls retain
     category-tagged type, const, and machine arguments; named types, bounded
     integer const literals, and same-typed lexical forwarding resolve against
     the authored telescope, enter normalized identity and snapshots, survive
     monomorphization, and instantiate generic proposition value signatures.
     Cross-category, wrong-width/wrong-type const, and concrete value-signature
     mismatches reject. Indexed-law synthesis slice landed 2026-08-03:
     proposition-law conformance expands one complete fresh carrier telescope
     per representative parameter, validates each proof-machine group against
     the carrier's binder kinds and contracts, treats a bare indexed carrier
     only as its family identity, and reconstructs the concrete proposition's
     binder labels from the proof machine's representative types before exact
     normalized-law matching. Missing, reused, or swapped representative
     packs reject. **SELECTED-WITNESS-EVIDENCE — DESIGN BLOCKED
     (`OWNER_QUESTIONS.md` Q1):** a witness-bearing proposition fixes its
     carrierless evidence interface, but selecting and reopening one concrete
     evidence term requires the unresolved complete requirement-to-satisfier
     map for a named conformance. The current owned-`dyn` surface also has no
     carrierless evidence constructor/open form; do not infer a witness from
     same-named machines or retain only the proof-irrelevant proposition.
     Terminal-Psi identity slice landed 2026-08-03: semantic v16 owns
     canonical nominal proposition declarations, ordered type/const/machine
     binder telescopes, value-parameter type identities, fact-only versus
     witness-bearing classification, and normalized application rows. The
     checked-source producer expands transparent aliases to their nominal
     endpoint, omits alias declarations, assigns dense deterministic IDs, and
     emits no frontend arena handle; codec, verifier, migration, and archived
     v1-v16 compatibility are covered;
  2. **DESIGN BLOCKED (`OWNER_QUESTIONS.md` Q1):** add the proof stratum to
     selected-conformance projection and permit by-value `dyn` only when the
     complete normalized value has no runtime carrier. The carrierless runtime
     rule is settled, but the projection cannot retain or reopen an exact proof
     term until named conformances have a complete requirement-to-satisfier
     association; this is the same blocker as selected witness evidence above;
  3. independent `Reflexive`, `Symmetric`, `Transitive`, and `Antisymmetric`
     requirements plus `Equivalence`, `Preorder`, and `PartialOrder`
     composition landed in core on 2026-08-03. Each base trait owns only its
     law, composites inherit without redeclaration, and the permanent canary
     covers concrete proposition substitution plus heterogeneous machine-
     indexed reflexive/symmetric/transitive telescope synthesis;
  4. **DESIGN BLOCKED (`OWNER_QUESTIONS.md` Q3):** add `Respects` over
     normalized argument records, checking both representative-invariant
     semantic preconditions and related results. The semantic clauses are
     settled, but the source/identity surface for the synthesized record,
     receiver/parameter projections, and derived callable-domain proposition
     is not; do not promote the legacy flattened pair-of-calls scan;
  5. **DESIGN BLOCKED (`OWNER_QUESTIONS.md` Q1 and Q3):** migrate `%` from
     executable-`bool` relations and suffix-based law discovery to proposition
     evidence plus explicit selected conformances. Exact equivalence selection
     needs Q1's complete conformance map, and lifted operations depend on Q3's
     `Respects` surface.
  Preserve the existing generic quotient canaries as migration coverage for
  heterogeneous machine-indexed representatives; add a decidable rational
  relation, existential Cauchy evidence, a total lifted operation, and a
  partial lifted operation as acceptance drivers. Proposition-surface canaries
  cover a checked fact-only relation, witness-bearing evidence and a transparent
  alias reopening the same witness, rejection when a proof supplies no required
  evidence, rejection of a literally bodyless ordinary theorem machine,
  explicit admitted-axiom provenance, and `%` rejecting any equivalence whose
  closure depends on admitted evidence. Equality propositions canonicalize
  operand orientation; transitivity composes through the shared endpoint on
  either side without adding a separate trusted symmetry judgment.
- **N8:** expand the construction corpus and proof-engine support needed by
  layouts, quotients, and `Real`.
- **F7:** replace hardcoded float lowering with the settled ordinary
  boundary-operator/provider-plan architecture in this strict remaining order:
  1. implement executable per-operation `FloatSemantics` functions and make
     build-time folding plus the interpreter consume them;
  2. implement checked arithmetic-policy adapters, including result-checked
     `Trapping` and overflow-only `Saturating`;
  3. add explicit target satisfiers and selected `ProviderPlan` realization for
     x86-64, AArch64, and checked software fallbacks, including canonical
     floating-control-state preconditions and foreign-boundary restoration; and
  4. ship differential validation evidence for every admitted hardware
     realization.
  Keep `f32` and `f64` permanently bound to binary32 and binary64. Keep
  multiply-then-add, fused multiply-add, directed-rounding variants,
  comparisons, conversions, and classification as distinct named requirements.
  A representation-sensitive consumer of a possibly-NaN result must prove
  non-NaN, canonicalize, or demand an exact raw-NaN refinement; the base
  arithmetic contract exposes only `FloatMeaning`.

  Implementation checkpoint (2026-07-28): the shared host-independent semantic
  engine now owns exact decimal landing, binary32/binary64 decode and rounding,
  base add/subtract/multiply/divide/negate, partial comparisons, the settled
  min/max choice, distinct multiply-then-add versus fused-multiply-add,
  classification, correctly rounded square root, exact/saturating/trapping
  float-to-integer conversion, integer-to-float rounding from exact integers,
  format conversion, and explicit directed-rounding variants.
  Anonymous-constant landing and the interpreter's landed arithmetic consume
  that engine, including per-operation binary32 rounding, square root,
  conversion policy edges, full unsigned-64 conversion, and proof-level NaN
  payload erasure. Backend guard-constant folding now consumes the same engine
  instead of host `f64` arithmetic plus narrowing. The compatibility x86-64
  and AArch64 conversion lowerings now distinguish signed from unsigned sources
  through the full 64-bit range. The compatibility `Math::fused_multiply_add`
  interpreter call now also consumes `FloatSemantics::fused_multiply_add`; its
  native/interpreter edge canary distinguishes the positive fused residual from
  the zero produced by multiply-then-add. The source-visible core surface now
  publishes pure `FloatSemantics` identities plus contracted f32/f64 boundary
  requirements for primitive arithmetic/comparison spellings, distinct
  multiply-then-add/FMA, classification, and directed rounding; checked
  operator evidence pins the primitive spelling selections. Named
  F32/F64 FMA, classification, and directed-rounding value calls now resolve
  through one ambiguity-checked operator rule shared by validation, checked
  flow, and the interpreter; arguments and declared result formats remain
  type-checked, unknown requirements fail closed, and the interpreter consumes
  the shared semantics rather than host arithmetic. One zero-argument semantic
  machine now executes in both a fixed-array-length build-time position and at
  runtime, covering f32/f64 rounding boundaries, subnormal underflow, overflow,
  signed zero, infinities, NaN comparisons and min/max, classification, square
  root, directed add/subtract/multiply/divide/sqrt/FMA, and fused-versus-unfused
  behavior. Hermetic core float requirements no longer trip the interpreter's
  host-boundary purity backstop; compatibility imports still do.
  Rung 1 is complete for the settled operation surface.
  The first rung-2 slice centralizes result-checked `Trapping` and overflow-only
  `Saturating` in the shared semantic engine. The interpreter now consumes
  those adapters, including trapping propagated NaN/infinity; checked spelled
  binary uses of the imported normalized float surface retain the exact
  binary32/binary64
  `FloatTrappingNonFinite` or `FloatSaturatingOverflowOnly` adapter selected by
  the operand policy. Native and interpreter canaries pin propagated
  non-finites, finite overflow, division by zero, and invalid results.
  Named F32/F64 calls now retain their selected requirement identity and the
  same adapter in a distinct checked named-use arena. Float-returning unary,
  binary, ternary, and directed operations apply the shared adapter in the
  interpreter; classification results carry no float adapter, and mixed
  explicit operand policies reject statically. Checked adapter evidence now
  rides state-graph, control-flow, and abstract value facts, including nested
  operations, and normalized table lowering consumes that evidence rather than
  reconstructing float policy from type domains. Format mismatches and
  contradictory carried evidence fail closed; the legacy type-domain fallback
  remains only for compatibility operations that have no checked operator
  evidence. Native x86-64 and AArch64 realization now applies the result-only
  `Trapping` verdict to spelled and named float operations, including propagated
  NaN/infinity, while `Saturating` retains its overflow-only operand-aware
  adapter. Stage-copy tests and a sentinel-safe native canary pin the path.
  Rung 2 is complete; continue with rung 3's explicit target satisfiers and
  selected `ProviderPlan` realization.
  The first rung-3 slice now reifies each exact overloaded boundary-operator
  signature as an independent one-row provider slot. Explicit f32/f64
  satisfiers for all four primitive arithmetic and six primitive comparison
  requirements exist on `windows_x64`, `linux_x64`, `linux_arm64`, and
  `macos_arm64`, selecting the corresponding compiler-known intrinsic.
  Selection validates the exact binding even when the requirement is unused,
  rejects mislabeled intrinsics or an absent exact selection, and retains the
  selected plan identity on checked spelled/named operator uses. The identity
  rides state graph, control flow, and abstract-operation facts; instruction
  selection resolves it through the retained selected-plan set and rejects
  zero, missing-plan, or contradictory evidence. Cross-target selection for
  all twenty exact slots per target, used-operation identity, stage-copy,
  backend fail-closed, malformed-binding, and native pipeline canaries pin the
  slice. Primitive spellings have completed target-plan migration. The first
  named-operation cohort now adds exact F32/F64 `minimum`, `maximum`,
  `square_root`, `negate`, `is_nan`, `is_finite`, `is_infinite`, `is_normal`,
  `is_subnormal`, and `multiply_then_add` satisfiers on all four native targets.
  Checked named-use evidence authorizes an execution-only rewrite in both
  engine pipelines while source proof identity remains attached to the
  boundary requirement. Negate becomes a root-preserving multiply by a landed
  negative one; `is_nan` becomes an unnameable unary compiler builtin, so its
  operand is evaluated once and its binary32/binary64 width is retained through
  nested lowering. Native/interpreter canaries cover NaN operand order, equal
  signed-zero choice, exact-square roots, signed-zero/infinity negation, and
  NaN/non-NaN predicates in both widths; x86-64/AArch64 cross-target output and
  exact-binding rejection pin the new paths. The remaining bool-valued
  classification predicates share that exactly-once unary path. Interpreter
  execution uses `FloatSemantics`; both native backends classify raw signless
  IEEE patterns, with an internal 4/8 metadata marker retaining the source
  format across direct bool writes whose operand folds to an immediate. Zero,
  normal, subnormal, infinity, and NaN edges plus native width-lockstep tests
  pin both formats. Enum-valued `classify` uses format-specific unnameable
  builtins and returns the declared eight-byte `FloatClass` carrier directly:
  source-order i32 tag at byte zero and the overlaid sign payload at byte four.
  Layout assertions plus every tag/sign edge pin interpreter, native, and
  cross-target execution. Multiply-then-add preserves its
  distinct two-rounding contract through an unnameable format-specific ternary
  compiler call that survives state-local expression copying. Both native
  backends retain all three authored operands, emit a separate multiply and
  add, and adapt policy only at the final result; cancellation and finite-
  overflow canaries prove unfused and operand-aware Saturating behavior in both
  engines. The nearest-even F32/F64 FMA pair now selects exact provider slots
  on both AArch64 targets. A distinct unnameable ternary compiler call retains
  all three operands and their format; the interpreter consumes
  `FloatSemantics::fused_multiply_add`, while native AArch64 emits one scalar
  `FMADD` and applies result policy only after the fused result. Cancellation
  edges prove the positive residual in both formats and reject substitution of
  the two-rounding provider. Generic x86-64 remains an SSE2 baseline, so its
  FMA slot deliberately remains unselected until a feature-qualified or checked
  software realization exists. The first canonical-control-state slice is now
  live in every generated callable frame. x86-64 saves the incoming MXCSR,
  installs masked exceptions plus nearest-even and gradual underflow, and
  restores the caller's complete value; AArch64 does the same for FPCR with its
  zero-valued canonical controls. Call-return footprint evidence now retains
  `ControlState`, and validation authorizes that state only for prescribed
  `CallReturn` mechanics. This establishes checked state for ordinary generated
  entry and callbacks and restores the foreign caller on exit. Returning
  foreign crossings now take the conservative general-binding path: imports
  and indirect vtable/table calls receive aligned MXCSR/FPCR save/restore
  envelopes around their existing target call programs, while direct syscalls
  receive none. Layout, emission, and relocation rebasing consume the same
  mechanism classification. A hostile AArch64 native canary changes FPCR via
  `_fesetround(FE_UPWARD)` and proves the following half-ULP addition still
  ties nearest-even. A later admitted preservation proof may remove a redundant
  envelope without changing call layout. The first directed-rounding provider
  cohorts now select exact F32/F64
  add/subtract/multiply/divide/square-root-toward-zero/positive/negative slots
  on all four native targets. Each baseline ISA realization saves its complete
  MXCSR/FPCR, installs the requested direction for one scalar operation, and
  restores the prior state before result-policy adaptation; midpoint dual-engine
  canaries also prove subsequent ordinary arithmetic remains nearest-even.
  The six directed F32/F64 FMA slots now select exact providers on both AArch64
  targets. Interpreter lowering consumes the matching directed
  `FloatSemantics` identity; native lowering balances the requested FPCR
  direction around exactly one scalar `FMADD`, then restores control before
  result-policy adaptation. Half-ULP dual-engine canaries distinguish all three
  directions and prove a following ordinary FMA remains nearest-even. Remaining
  rung-3 work includes x86-64 FMA realization, checked software fallbacks, and
  rung-4 differential evidence. The first rung-4 slices now retain
  `omega.float.hardware.macos_arm64.directed-add.v1` /
  `0xeb87c478c8a1e513` and
  `omega.float.hardware.macos_arm64.directed-subtract.v1` /
  `0xc014cab348eb363c`, plus
  `omega.float.hardware.macos_arm64.directed-multiply.v1` /
  `0xec7e7bae35b056cb` and
  `omega.float.hardware.macos_arm64.directed-divide.v1` /
  `0xb6dc18215e0c4019`, plus
  `omega.float.hardware.macos_arm64.directed-square-root.v1` /
  `0x8b87625fd5e9f1b7`. Each identity binds the family's six exact selected plans,
  binary32/binary64 rounding edges, all three directions, control-state
  restoration, interpreter/native results, and Linux x86-64/AArch64 cross-build
  results. The nearest-even FMA slice separately retains
  `omega.float.hardware.macos_arm64.nearest-fma.v1` /
  `0xa1b8c9cb16855a61`, binding its two exact plans to binary32/binary64
  cancellation edges, one fused rounding, interpreter/native results, and Linux
  AArch64 cross-build success. The multiply-then-add slice retains
  `omega.float.hardware.macos_arm64.multiply-then-add.v1` /
  `0x8b5fa3afbbf00653`, binding its two exact plans to binary32/binary64
  cancellation edges, two distinct roundings, binary32 finite-overflow
  saturation, interpreter/native results, and both Linux cross-builds. The
  minimum/maximum/square-root cohort retains
  `omega.float.hardware.macos_arm64.minimum-maximum-square-root.v1` /
  `0x8b3cf5ec26298fed`, binding its six exact plans to both-format NaN operand
  order, the settled signed-zero choices, exact square roots,
  interpreter/native results, and both Linux cross-builds. The negate/`is_nan`
  cohort retains `omega.float.hardware.macos_arm64.negate-is-nan.v1` /
  `0x57aa3468298305e9`, binding its four exact plans to both-format signed-zero
  and infinity negation, NaN/infinity/finite predicate separation, selected-root
  unary evaluation shape, interpreter/native results, and both Linux
  cross-builds. The bool-valued classification cohort retains
  `omega.float.hardware.macos_arm64.classification-predicates.v1` /
  `0xb89ec4b21c43f9a8`, binding its eight exact plans to both-format boundaries
  between finite/infinite, infinite/NaN, normal/subnormal, and subnormal/zero,
  exactly-once unary evaluation shape, interpreter/native results, and both
  Linux cross-builds. The enum-valued classification cohort retains
  `omega.float.hardware.macos_arm64.classify-enum.v1` /
  `0xf63a865e9bbb85f2`, binding its two exact plans to the eight-byte source-order
  `FloatClass` carrier, sign payload at byte four, every tag and signed payload
  in both formats, exactly-once unary evaluation shape, interpreter/native
  results, and both Linux cross-builds. The format-conversion cohort retains
  `omega.float.hardware.macos_arm64.format-conversion.v1` /
  `0xeb1e22fdac585936`, binding its two exact directional plans to the
  binary64-to-binary32 halfway and just-above edges, exact widening, infinity
  preservation, interpreter/native results, and both Linux cross-builds. The
  integer-to-float cohort retains
  `omega.float.hardware.macos_arm64.integer-to-float.v1` /
  `0x279651cb7ccd80ee`, binding all sixteen exact source/destination plans to
  narrow signed/unsigned extension, binary32/binary64 precision-boundary ties,
  maximum unsigned64 conversion, interpreter/native results, and both Linux
  cross-builds. The float-to-integer cohort retains
  `omega.float.hardware.macos_arm64.float-to-integer.v1` /
  `0x297cb8ce8d1adc1c`, binding all twenty exact source/destination/domain plans
  to both-format truncation toward zero across every integer width, in-range
  Trapping dispatch, signed/unsigned/NaN saturation, interpreter/native results,
  and both Linux cross-builds. The primitive arithmetic/comparison cohort
  retains `omega.float.hardware.macos_arm64.primitive-arithmetic-comparison.v1` /
  `0xab789e8539fe9f96`, binding all twenty exact operation/format plans to finite
  add/subtract/multiply/divide and all six equality/ordered comparisons in both
  formats, interpreter/native results, and both Linux cross-builds. The policy-
  adapter cohort retains `omega.float.hardware.macos_arm64.policy-adapters.v1` /
  `0x72c8984fc8703b9b`, binding all eight primitive arithmetic plans to both
  result adapters in both formats, finite and nested success paths, overflow-
  only saturation, unclamped division by zero, every Trapping non-finite class,
  interpreter/native observations, and both Linux builds for every case. The
  directed-FMA slice retains
  `omega.float.hardware.macos_arm64.directed-fma.v1` /
  `0x75be2c4963f3f15a`, binding its six exact plans to binary32/binary64 half-ULP
  edges, all three directions, one fused rounding, control-state restoration,
  interpreter/native results, and Linux AArch64 cross-build success. The
  aggregate semantic-edge twin retains
  `omega.float.hardware.macos_arm64.semantic-edge-twins.v1` /
  `0xa6cd3291982e12a1`, binding all 56 exact plans selected by one zero-argument
  build/runtime machine to both-format rounding, subnormal/overflow, signed
  zero, infinity, NaN partial ordering, min/max, classification, square root,
  directed arithmetic/FMA, and fused-versus-unfused edges. Its retained result
  includes build-time evaluation, interpreter/native exit agreement, and Linux
  AArch64 cross-build success. This is cross-family coherence evidence for the
  macOS AArch64 realization, not a substitute for a target/family-specific
  result. Every other admitted hardware realization still needs an equally
  target-specific retained suite result.
  The first checked-software provider slice is now live independently of a
  float algorithm: an ordinary body may satisfy one exact named boundary
  operator without `via`, provided its machine-checked equality/`&&` ensures
  cover every requirement guarantee under positional parameter substitution
  and it adds no stronger requires. The
  compiler derives and selects a one-row `CheckedAdapter` plan, retains the
  exact plan identity on the general named-operator use fact, and redirects
  interpreter/native execution to the checked body while preserving the public
  operator as proof identity. Positive dual-engine and missing-guarantee/
  stronger-premise rejection canaries pin the path. The actual binary32/
  binary64 software FMA implementation (or feature-qualified x86 provider)
  remains required before the generic x86 FMA slots can be selected.
  The public float/integer and float-format conversion requirement family is
  settled. Result-domain overload resolution and provider/artifact lowering are
  now live. The public float-format pair is published and pinned across all
  native target plans plus both execution engines. The integer-to-float matrix
  is likewise published and pinned. The float-to-integer matrix now publishes
  `from_f32`/`from_f64` on every signed and unsigned fixed-width destination,
  with unqualified, `Trapping`, and `Saturating` result-overload slots selected
  independently on every native target. Exact proof reuse, toward-zero runtime
  results, target-width saturation and NaN-to-zero, NaN/overflow traps, no
  `Wrapping` candidate, and named-requirement duplicate dispatch rejection are
  pinned. The checked-result operation remains separately design-blocked on
  its public result carrier.

Keep `Real` proof-only and core-level. Do not lower it as a runtime float or
move it to a convenience library.

### Lifetimes and remaining source surfaces

- Finish general outlives constraints, persistent owners, and remaining
  aggregate borrow propagation. Program-static literal views (including folded
  literal joins), nested static aggregate views, static-only machine results,
  and exact persistent-place copies within one state are admitted without
  manufacturing a source loan. Whole persistent fields with that provenance
  now cross named graph-state edges through a predecessor-intersection
  must-analysis keyed by stable field identity rather than each state's
  receiver parameter. Stable nested field, sum-case, and fixed-index borrow
  frontiers cross the same way, can accumulate over several states, and survive
  disjoint sibling mutation; overlapping or dynamically indexed mutation, a
  missing predecessor fact, or an opaque statement call clears the affected
  shortcut. Runtime-indexed static provenance now crosses a named edge when an
  immutable state-parameter index is forwarded unchanged and rebased to the
  target parameter; rewritten, omitted, or ambiguous index transport still
  clears it. Complete R5 statement/value-call frames now preserve paths proven
  disjoint or read-only and invalidate only overlapping paths; unresolved
  frames remain opaque. Parameter-backed storage, broader runtime-indexed
  expressions, broader exact R5 summaries, and general state-parameter
  loan-root rebasing remain.
- Implement local dynamic traits as two-word borrowed descriptors selecting one
  complete nominal conformance. Derive the per-requirement dynamic surface,
  lower checked adapters, retain compile-time operational envelopes, add
  transparent trait refinements and complete generic-bound consumption, and
  prototype envelope/effect-row inference before committing the full lowering.
  The first nominal-dispatch correction is live: typed/checked dynamic
  candidate discovery now consults only explicit whole-trait data conformance
  items, excludes unbound generic conformances, and never infers an edge from
  same-named attached machines. Existing interpreter/native dispatch canaries
  now declare their conformance edges. Psi type validation also rejects a
  boundary trait as a local dynamic value and rejects a bare generic trait
  whose parameters cannot yet be bound by the `dyn` source form. The first
  signature-derived per-requirement surface is also live in typed/checked Psi:
  a requirement is absent when it lacks a borrowed receiver, carries
  requirement-local generics, or mentions `Self` outside that receiver,
  without hiding eligible siblings; source-call validation consumes that same
  canonical judgment. Per-requirement eligibility/adapters and envelope
  inference remain. Standalone `Type satisfies Trait as Name;`
  declarations now retain the name through checked Psi as a stable child
  symbol and reject duplicate `Type::Name` paths. Generic machine `where T
  satisfies Trait<Args>` bounds now retain their subject and specialization
  through checked Psi, while `T satisfies Type::Name` resolves to that exact
  child conformance symbol; unknown subjects, traits, arity mismatches, and
  named selections reject instead of disappearing at the parser. Generic trait
  headers now retain and resolve the same conformance-bound carrier, including
  exact named selections, with the same declaration diagnostics. Generic
  machine bodies now consume every bound's statically known requirement
  surface: unconstrained, absent, and ambiguous calls reject, trait generic
  arguments instantiate requirement parameters, and exact named bounds expose
  only their selected trait. Concrete specialization requires one matching
  nominal conformance (or the exact named carrier), checks conformance
  arguments, pins the obligation in the template fingerprint, rewrites the
  selected attached-state symbol, and erases the discharged bound. Generic
  trait-header obligations are now enforced at every static application site:
  standalone data conformances, machine conformances, trait parents, and
  generic bounds all discharge nested or exact-named obligations from nominal
  conformances or the enclosing generic evidence, and ambiguous or absent
  evidence rejects at the authored relationship. The Psi interpreter and
  native backend both carry a mutable generic receiver's exact
  caller-field base through receiverless helpers; the static-dispatch canary
  runs without a dictionary in either engine. The first checked dynamic-
  coercion selection rung is live for a direct place bound to a borrowed local:
  a bare `&T as &dyn Trait` consumes the unique complete nominal conformance,
  retains the exact data/trait/optional named-conformance symbols in checked
  facts, and rejects missing or ambiguous conformances. Exact
  `&dyn Type::Conformance` targets now retain their carrier and stable child
  symbol through typed identity, derive the dispatch trait from that declared
  edge, resolve otherwise-ambiguous conformances, and reject unknown or
  wrong-carrier selections. Omega's runtime ABI now distinguishes the borrowed
  `{ instance, selected-conformance table }` carrier from the byte-identical
  slice descriptor, and layout/runtime-storage descriptors retain the exact
  trait and authored named-selection metadata. **DYNAMIC-CONFORMANCE-SATISFIERS
  — DESIGN BLOCKED (`OWNER_QUESTIONS.md` Q1):** the source model does not yet
  settle how one named whole-trait edge binds its complete set of attached
  requirement satisfiers; do not derive adapter rows by state-name coincidence.
  Descriptor materialization, private table emission, requirement adapters,
  and envelope inference remain after that ruling.
  Local descriptors must not cross replaceable component boundaries. Add owned
  erased **runtime** values only after general storage ownership,
  size/alignment metadata, and cleanup contracts can support them; N6's
  carrierless proof-only owned-`dyn` case has no runtime carrier and is ordered
  separately above.
- Complete the hermetic semantic-evaluation contract in
  `wiki/design_briefs/build_time_evaluation.md`: check every normalized
  admission axis at the concrete invocation, add the sealed target-semantic
  capsule, split semantic result keys from canonical usage records, publish
  deterministic live progress, and add constant/runtime equivalence canaries
  led by `f32`/`f64`. Add optional root-controlled warning and hard-ceiling
  policy only after the meter/reporting path exists; unlimited terminating
  evaluation remains legal. The common gate now also requires ordinary checked
  termination across the concrete typed call closure, using the same pure
  graph/ranking judgment as checked facts; machine-entry-symbol backedges are
  normalized to the entry state, and unmeasured recursive callees reject before
  evaluation. The evaluator boundary also closes escaping mutation by using a
  fresh machine instance, fresh owned argument graphs, and snapshot-only
  results. Canonical usage schema v1 is now distinct from the evaluator-step
  schedule and records checked recursive `result_cells`; logical-word,
  aggregate-construction, and peak-live-cell telemetry remain. Declared linear
  runtime carriers in reachable machine storage, signatures, locals, and
  opaque callable contracts now reject by structural multiplicity until an
  exact proof/build-admission exists. Authored `requires` premises anywhere in
  the reachable machine/callable closure now reject before evaluation because
  the pre-check evaluator has no concrete checked invocation proof; a later
  invocation-sensitive gate must supply the ordinary proof rather than invent
  a build-time-only rule. Cyclic layout/access policy helpers and their runtime
  canaries now carry ordinary checked fuel rankings, so enabling the common
  termination floor no longer leaves those semantic-evaluation consumers on
  stale unmeasured loops. **BUILD-TIME-ABNORMAL-OUTCOME — DESIGN
  BLOCKED (`OWNER_QUESTIONS.md` Q2):** the failure/control axis is settled, but
  its complete-contract and terminal source surface are not. Contextual `trap`
  currently erases to an ordinary terminal transition and is not usable
  admission evidence; do not invent an abnormal-outcome summary until the
  distinct normalized control row and propagation rule are settled.
- Extend `build.omg` provider plans with the normalized
  `Hermetic | Receipted | Volatile` observation ceiling. Publish static ceiling,
  realized class, replay receipts, `ReplayableFromRecord`, and transitive
  `RebuildableFromSource`, with the first failed provenance edge. Build-host
  services remain explicit capabilities and do not enter the hermetic semantic
  evaluator.
- Implement separate compilation and replaceable-realization artifacts without
  new replacement syntax. A component is a selected provider realization plus
  its compiler-validated code/state/resource closure, not a package. Calls
  crossing that closure name requirements; concrete calls remain internal.
  Keep candidate resource demand separate from stable semantic identity unless
  policy explicitly fixes a budget. Emit target/runtime stack-provision needs,
  mapping lifetime cohorts, and two-sided import/export validation. Implement
  boundary-trait binding multiplicity, `BindingEntryCeiling`/plan validation,
  origin/custodian claim metadata, custody transfer receipts, compiler root
  maps, enumerable component-state roots, and named-path retention reports.
  Runtime binding-era algorithms, drain/coexistence policy, migration
  scheduling, and resource provisioning remain consumer/runtime work. Stable
  object identity and the ObjectTable migration bundle wait for a deployment
  that requires replacement without holder cooperation.
- Implement serialized capability attenuation/revocation.
- **ATOMIC-EVENT-MODEL — DESIGN BLOCKED:** implement normalized
  `Atomic::fence` for `Receive | Publish | ReceivePublish`; define
  the formal atomic-event model and complete target-refinement proofs before
  enabling protocol verification. Keep the strong receive baseline on
  AArch64; a weaker acquire instruction requires protocol-scoped proof and
  measured justification. A global-order fence requires the completed global
  atomic semantics rather than a conservative backend guess.
- Add sealed provider requirements for DMA publication/acquisition, cache
  maintenance, MMIO notification, and posted-write completion without
  strengthening `reaches` or adding boundary-signature clauses. A checked
  driver may derive a complete submission contract from those primitives; an
  opaque OS provider may satisfy it with admitted evidence when policy permits.
  Every emitted requirement must be discharged or reject.
- Tie publication evidence to exact range and write state so intersecting write
  frames invalidate it. Require acquisition to consume request- and
  instance-bound completion evidence, and restore Stable CPU observation only
  when custody returns. Terminal Psi must retain scoped ordering events; erased
  evidence and generic call effects are not lowering barriers.
- Implement registered callback lowering and the Windows adapter canary under
  ENT4 without introducing a general source-visible code-address value.

### Wire runtime

The language model is settled in guide chapters 21-22 and the programmable
layouts brief. Complete the implementation in dependency order:

- extend repeated encode/decode to `Vec<T>` once its allocator obligations are
  available; packed scalar decode into `&[T]` remains intentionally
  unsupported because varints cannot form a zero-copy scalar view;

Keep `compact_binary` strict while extending its normalized plan and generated
realizations. Additional native or ecosystem codec families are ordinary
policy packages over the same schema and requirement surface.

### Admitted executable installation

The typestate and placement foundation is live. Remaining work:

- connect retained semantic artifacts to loader/provider execution;
- implement trusted/PCC and final-footprint validators;
- complete target W^X/coherence reporting and uninstall/replacement joins; and
- keep arbitrary runtime bytes-to-code and JIT unsupported.

Acceptance: only an admitted reusable artifact plus consumed placement authority
can produce installed code; validation binds exact final bytes and placement;
ordinary code never receives a raw executable address.

## Owner-blocked index

- **ATOMIC-EVENT-MODEL:** blocked on the open formal portable atomic-event
  axioms and x86-64/AArch64 refinement choices recorded in
  `wiki/language_guide/appendix_open_questions.md`. This blocks portable fences
  and protocol verification, not placed atomic accessors, checked ISA barriers,
  or installed-root same-context evidence.
- **CHECKED-RESULT-ARITHMETIC:** blocked on whether failure-returning checked
  arithmetic earns a distinct public library carrier beyond exact-by-default
  obligations and the existing explicit policy families, as recorded in
  `wiki/language_guide/appendix_open_questions.md`.
- **BUILD-TIME-ABNORMAL-OUTCOME:** blocked on `OWNER_QUESTIONS.md` Q2's
  complete-contract spelling and normalized propagation model for nuclear
  abort, trap-capable operations, and other abnormal non-return. The legacy
  contextual `trap` parser route erases to ordinary termination and cannot
  support build-time admission.
- **DYNAMIC-CONFORMANCE-SATISFIERS:** blocked on `OWNER_QUESTIONS.md` Q1's
  binding between one named whole-trait edge and its complete requirement
  satisfier set. This blocks dynamic table adapter emission, not static trait
  dispatch.

## Vertical acceptance slices

- **Termination firewall:** cyclic components strictly decrease one joint rank;
  private witnesses never enter public contract identity.
- **Contract/admission split:** service reach, suspension, blocking,
  termination, mutation, and trust normalize independently. Candidate resource
  demand and installed provision admit separately; a fixed resource ceiling is
  contract identity only when policy deliberately publishes one.
- **Allocator substrate:** implement a package-level bump strategy over one
  qualified `Extent`. Two allocations coexist; release cleans and returns an
  exact retired subextent without restoring tail capacity; reset rejects while
  a live claim remains and succeeds after full recomposition; finish returns
  the original backing; RAM and non-RAM placements expose their ordinary access
  views.
- **OS gauntlet:** UART/MMIO, Cathedral-owned address translation, DMA,
  hostile/trusted shared-page IPC, Cathedral-owned exception/timer entry, and
  SMP AP bringup. A new customer-shaped compiler concept fails the slice.
- **Control-state negatives:** checked asm cannot hide stack/control mutation;
  provider exits must match their plan; external loans cannot reach outside
  their extent; parked continuations remain non-addressable.

## Platform-gated verification

- Run the Linux host/time/filesystem rows natively on AArch64. x86-64 WSL
  coverage exists. `mkdirat`/`fchmodat` now normalize path creation and
  permission changes with plan-owned `AT_FDCWD`; direct `unlink_at` and
  plan-prefixed `readlinkat` are normalized too. Plain-path removal now injects
  both `AT_FDCWD` and Linux's `AT_REMOVEDIR` through retained plan data.
  `renameat`/`linkat`/`symlinkat` retain their directory descriptors and flags
  through the same plan surface. **Linux metadata is DESIGN CLEAR:** extend the
  closed `FieldPlan` vocabulary with an integer placement carrying byte offset,
  stored width, and signed/unsigned interpretation. Projection sign- or zero-
  extends into the portable semantic carrier; mutable views require total
  encoding or a concrete fit proof plus ordinary legal-transfer evidence.
  `IntegerAt` is now live through source evaluation, normalized plan/access
  geometry, typed plan-laid layouts, and concrete Omega layouts: byte offset,
  whole-byte stored width through 64 bits, and interpretation survive into a
  field-keyed backend record; invalid carriers and non-total decode ranges
  reject. Direct owned and reference-backed scalar projection now loads the
  exact stored width and uses the retained interpretation to sign- or
  zero-extend into the portable carrier; a raw-byte native canary distinguishes
  both rules for direct and runtime-indexed projections and cross-compiles for
  Windows x64 and Linux AArch64. Descriptor-backed, inline-frame, machine-owned,
  and reference-backed runtime indices retain their ordinary address geometry
  while loading only the stored field width. The concrete layout now retains
  total-write evidence derived from the field's admitted semantic range; direct
  stable-owned mutation consumes it to store exactly the physical width. Direct
  guards also reselect the stored projection and compare its widened semantic
  value rather than reading the carrier width from raw storage. Concrete
  proved-fit mutation is now live for exact compile-time integers and runtime
  assignment values whose Psi-proved inclusive range wholly fits the stored
  encoding. Every resolvable assignment participates in range analysis without
  becoming a new language obligation; checked values retain both their use-site
  type reference and the BigInt discharge interval, including stable incoming
  guards and boundary witnesses. Omega consumes only that checked fact rather
  than reconstructing proof from layout shape. An unconstrained runtime value
  remains fail-closed. Read-only interpreter record views now perform the same
  stored-width decode. The portable filesystem stat record
  has wide semantic carriers, while Darwin, Linux x86-64, Linux AArch64, and
  Windows target policies retain their physical widths; both Linux kernel
  layouts validate through the cross-target fstat canary. Native Linux runtime
  confirmation remains platform-gated.
  Mutable raw-byte record recasts now retain `IntegerAt` metadata through
  validation, interpreter projection and write-back, native pointee lowering,
  and relocation. Each assignment still requires total-write evidence or a
  Psi-proved fitting value; an unconstrained recast write rejects. Typed
  aggregate aliases still require identical representations. By-value boundary
  classification now derives every stored-integer leaf's physical width and
  alignment from the validated encoding metadata; a native and cross-target
  canary preserves that physical representation through a by-value state pass.
  The target-neutral ordinary scalar materializer now fit-checks concrete
  signed and unsigned values, writes only the stored width in either byte
  order, and sign- or zero-extends through the matching decoder; rejection is
  atomic. A compiler/provider-resolved symbolic value uses the same fit check;
  unresolved loader-consumed `IntegerAt` stays fail-closed. A post-handoff
  writer retains the exact source/stored widths and interpretation as
  invocation evidence, privately resolves the sealed target once, and rejects
  a non-fitting value before opaque context publication or destination writes.
  Linux `read_dir` now retains the real three-argument `getdents64`
  plan, omits the Darwin-only cursor at selection, and decodes the Linux record
  offsets in both target packages. Direct syscall failures now flow as explicit
  `-errno` results into target-package classification; Linux does not acquire a
  hidden libc-style error slot. Every POSIX directory wrapper now drains
  complete 512-byte record fills through EOF: count and stats retain their
  tallies, indexed lookup retains its global record number, and the fd-relative
  core rewinds once before carrying the cursor across fills. Both interpreter
  filesystem providers paginate the same record stream; native Darwin and
  Linux x64/AArch64 structural canaries pin the routes.
- Keep unavailable hosts structurally tested; do not claim runtime verification
  without the host.
- Build the Windows GUI callback canary through the settled callback-requirement
  path; do not pass a raw code address or add a Win32-only callback escape.

## Deferred until a real customer

- richer measured-recursion guards and multi-subject lexicographic cycles;
- reduced-rational divisibility theory beyond current quotient work;
- asynchronous extent revocation beyond provider quiescence;
- non-blocking executable-visibility tokens;
- runtime-generated host code, JIT, and arbitrary self-modifying code;
- independent final-byte CFI certificates and optional CET/PAC/shadow-stack
  hardening;
- universe levels before a full math-library replay goal;
- reusable fragmented allocation until a growable-container/backend customer
  can state its retirement, authority-return, and immediate-reuse demands; and
- an optimizing SSA/register-allocation/SIMD backend beyond current correctness
  requirements.
