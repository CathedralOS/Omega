# Tasks

Last pruned: 2026-08-02.

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

- **ENTRY-CONTENT-ROOTS — DESIGN BLOCKED (OWNER_QUESTIONS Q1):** add
  entry-provisioned content roots for the loaded image and initial
  stack/storage to the typed boot handoff. Derive sections and statics as
  subextents; allocate later frames and task stacks from existing roots. The
  startup provider admits only the handful of mappings it actually supplies,
  not each object independently. The missing decision is the stable semantic
  owner and identity of those routed root domains/entry requirement; core
  `Extent::Granted` cannot name a UEFI/Cathedral-specific route, while arbitrary
  target-owned domains give generic compiler derivation no portable identity.

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
  layout, calling-plan, stack, or runtime-selection artifact. Continue from
  that static binding into per-invocation behavior receipts,
  stack-resource authority, and custody under TR3–TR8.

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
  mismatched active-case literal fails closed. States with multiple candidate
  partition calls remain fail-closed. These
  non-direct rows remain
  checked/debug evidence because
  terminal semantic v12 deliberately carries only direct composition as its
  exact source theorem, source fingerprint,
  dense input-claim references, total structural-place substitution, and
  derived equation. The verifier requires the source to contain separation,
  binds every entry projection to one listed identity-reshuffle claim, replays
  the substitution, and reconstructs only the exact derived theorem as a
  semantic axiom. Canonical bytes include the witness; existing proof format v9
  already carries the resulting content proposition.
  Archived v1-v11 bytes retain their identities. The content producer now lives
  in `psi-checked-trees-to-terminal`: it revalidates and lowers checked
  conservation plans, exact identity reshuffles, and direct partition
  compositions into the existing v9-v12 terminal vocabulary, including dense
  claim identities and replayable place substitutions. The executable source
  canary remains content-free and fail-closed. Next compose multiple-call
  structural rewrites around authored-partition calls,
  connect a real content-bearing source slice after its separately recorded design
  blocker is resolved, insert sealed introduction and custody-exit rows, and
  discharge or admit the exact frontier theorem.
- **TERMINAL-CONTENT-CLAIMS — BLOCKED on language/IR design:** a real direct
  partition wrapper exposes a gap hidden by the synthetic terminal fixture.
  Checked composition correctly carries distinct entry claim identities but no
  identity reshuffles: aggregate conservation does not prove either input is
  individually equal to one output. Terminal v12 can name an input claim only
  through `ContentIdentityReshuffle`, and its verifier therefore requires the
  stronger one-to-one equality. Do not synthesize that unsound evidence. Settle
  and version an independent entry-claim binding (or enrich partition input
  rows with claim, projection, algebra, and structural place without an output
  equality) before adding the content-bearing source canary. This blocks only
  source integration and the dependent frontier work; the Psi-owned checked
  plan translators, canonical v9-v12 bytes, and verifier remain live.
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
  control durable, concurrent, and bounded delegation. Qualified-borrow
  admission, placed-content establishment/retirement, and transfer-footprint
  conflict checking remain implementation work rather than language-design
  blockers.
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

### Domain theory and numeric conversion

- Implement the settled ordinary core float/integer and float-format
  conversion requirements. Destination-owned names such as `F32::from_f64`
  and `I32::from_f64` use general result-domain overload lookup for
  unqualified, `Trapping`, and `Saturating` same-shape results. Exact
  denotation-preserving coercions remain the ordinary `as` surface; directed
  one-step rounding remains separately named. Keep the failure-returning
  operation blocked only on the checked-result arithmetic carrier. The
  float-format slice is now published as total nearest-even
  `F32::from_f64`/`F64::from_f32` requirements with exact provider selection on
  all four native targets; interpreter/native rounding-boundary and infinity
  canaries pin execution. The complete `F32`/`F64` integer-source matrix is
  also published for signed and unsigned 8/16/32/64-bit carriers, with exact
  provider selection on all targets and dual-engine nearest-even/signedness/
  upper-half-`u64` canaries. The complete float-to-integer matrix is now
  published for both float sources and every signed/unsigned 8/16/32/64-bit
  destination. Unqualified calls reuse the established finite/range proof,
  while same-path `Trapping` and `Saturating` result overloads select distinct
  exact provider plans on all four targets. Dual-engine matrix, NaN/clamp,
  trapping, unproven-Exact, and absent-Wrapping canaries pin the family.
- Generalize named-machine/requirement overload identity beyond the current
  path-and-parameter rule: normalize the result's dispatch-bearing domain set,
  reject duplicate sets at declaration, select the empty set without an
  expected result, require set equality otherwise, and prove predicate-only
  refinements after selection. Include the set in checked/artifact/symbol
  identity. Replace the current return-only-overload rejection canaries with
  positive result-domain cases plus duplicate-predicate and semantic-weakening
  rejection canaries. Fixed operator spellings remain operand-directed. The
  shared typed-tree result-set normalizer is now live with alias expansion and
  the settled predicate/semantic/routed/empty partition. Declaration duplicate
  checking and expected-result lookup now use that identity for actual overload
  families across locals, assignments, returns, arguments, fields, arrays, and
  no-context calls; singleton declarations retain ordinary compatibility after
  lookup. Runtime and rejection canaries pin empty/qualified dispatch,
  predicate collapse, and exact-set semantic weakening. Requirement collection,
  provider-plan coverage/fingerprints, adapter dispatch, authored host bindings,
  and platform-call lowering now retain exact result-overload identity; a
  dual-engine provider canary pins ordinary versus `Saturating` requirement
  dispatch. The public conversion families remain.
- All fixed-width integer pairs are now available from
  `core::numeric_conversion`. Widening is named only where the complete source
  range fits; every other pair—including a signed-to-wider-unsigned conversion
  that excludes negatives—is range-narrowing and explicitly selects Exact,
  Wrapping, Saturating, or Trapping behavior. Exact narrowing publishes and
  enforces its representability precondition; every conversion result returns
  to ordinary Exact arithmetic. The first corpus cohort now exercises the named
  surface across indexed operands, guard subjects, comparisons, bitwise
  operands, entry results, 16-bit conversions, and signed/unsigned extension.
  A second cohort covers real text algorithms: decimal and binary formatting,
  decimal parsing, FNV hashing, CRC-32, direct indexed byte writes, and explicit
  trapping versus wrapping narrowing choices. Seven user-facing samples now
  use the named surface for widening and trapping conversion: `width_mixer`,
  `array_sum`, `format_number`, `print_number`, `multiplication_table`,
  `prime_sieve`, and `maze_flood`. The follow-on sweep migrated every remaining
  runtime integer width/signedness conversion in `samples/`, including byte
  parsing/rendering, hashing, indexed image decode, descriptor counts, PRNG
  word extraction, and signed-byte reinterpretation. Integer-looking `as T`
  spellings that remain in samples are same-carrier arithmetic qualification
  (or the wire policy's same-type compatibility spelling), not hidden numeric
  conversion; float conversion remains its own F7 lane.
- Proof-directed exact integer `as` is now enforced across the compiler and
  corpus. The checker retains tighter flow facts only when the intrinsic source
  carrier contains them and otherwise falls back to that carrier's full range,
  so widening and same-carrier arithmetic-policy erasure are exact by
  construction without manufacturing an empty proof interval. Narrowing and
  cross-signed coercion require complete target containment from a declared
  range or dominating guard. Unproved casts reject with policy guidance.
  Positive/negative canaries pin widening, declared-range narrowing,
  guard-derived narrowing, and rejection, while former truncation and
  reinterpretation fixtures now name Wrapping or Trapping explicitly.
- Call-result normalization now materializes a value-machine call directly
  beneath a value cast or qualification through the ordinary synthetic local
  route. Inline named conversion, subsequent arithmetic-policy qualification,
  and enclosing arithmetic therefore preserve the delivered result; the
  indexed narrow/widen canary pins the formerly mislowered shape.
- Dominating range guards now survive resolved pure or disjoint value-call
  frames in both the proof-obligation and transitive indexed-range paths.
  Exact R5 paths invalidate only overlapping evidence; opaque frames still
  fail closed. Positive/negative regressions pin pure conversion calls versus
  calls that mutate the guarded place.
- The active PRNG canary cohort now uses named wrapping high-word extraction.
  Runtime branch alias substitution descends through binary/member arguments
  and replaces their bare parameter roots, so a named conversion nested inside
  a mutating value machine reads the caller's aliased state instead of an
  unused cloned parameter slot. A focused native regression pins the formerly
  crashing shape beside the existing dungeon-derived call/dispatch tests.
- Filesystem metadata consumers now use named `u8` widening for raw stat-record
  byte decoding across 15 native macOS canaries, both filesystem-to-time
  interop legs, the Windows SetFileTime round trip, and the raw byte-assembly
  setup of the cast-field payload regression. A checked-tree cohort covers all
  19 and the complete native filesystem/GUI suite still passes.
  Guarded nonnegative filesystem host counts now use exact `i64`-to-`u64`
  narrowing in the portable facade and every target implementation. Incoming
  guard proof now instantiates the nested callee contract and rebinds target
  state parameters through the transition arguments; a focused proof regression
  and Linux x64/AArch64 plus Windows checked-tree cohort pin the route.
  Backend-safe enum construction materializes each converted count under a
  distinct local name; a dynamic native conversion canary and all 88 native
  filesystem/GUI tests pin the delivered payload. Target `set_times` encoders
  now name wrapping signed-to-unsigned epoch conversion and byte truncation;
  POSIX directory walkers name host-count wrapping, byte widening, and
  cross-signed result conversion; Windows attribute decoding names its byte
  widening; and portable stat consumers name every width extension. The
  four-target checked-tree rows and native timestamp/directory workflows pin
  those policies. Residual filesystem `as` spellings are same-carrier Wrapping
  qualifications, target-owned boolean-to-foreign-bit encoding, or
  compatibility-specific casts whose authored shape pins legacy lowering. The
  cast-field migration exposed and
  now pins a compiler fix for terminal branch substates with several
  assignment-value calls in one local initializer: branch storage reserves
  every result ordinal, leaf-only nested call trees execute, top-level
  `Machine::entry` calls resolve by machine identity, and the full enclosing
  initializer materializes after its operands. Its final `mode as u32` remains
  intentionally because that cast-valued payload field is the regression's
  subject; the raw byte widening no longer waits on compiler work.
- `std::time` now uses the named integer surface for every runtime width or
  signedness conversion. Its remaining integer-looking casts only select or
  forget a same-carrier arithmetic policy. Store-enforced Exact ranged locals
  now discharge nested conversion preconditions; a broader declared range
  remains insufficient. Flattened nested calls expand compiler-elided scalar
  local initializers before applying enclosing parameter aliases, including
  cast/call structure and `min`/`max`/`clamp` scalar classification. Aggregate
  locals and value-call-result locals retain their dedicated runtime
  materialization instead of being mistaken for elided aliases. The constructor,
  totals/divide, clock, sleep, cross-target, and filesystem-time canaries pin
  both proof and native delivery.
- `std::macos_gui` now names its integer payload conversions: framebuffer
  dimensions widen from `u32` to the Core Graphics `i64` ABI fields, and the
  raw Objective-C liveness result narrows to `u32` with the existing wrapping
  policy. Its residual numeric casts are the six integer-to-float conversions
  that belong to F7; the native provider and GUI sample cohort pins the
  integer migration.
- Checked-result narrowing is design-blocked on the open arithmetic-library
  question in `wiki/language_guide/appendix_open_questions.md`; do not invent a
  result family merely to mirror another language. Remaining numeric-conversion
  implementation work is float/integer and float-format policy operations;
  exact integer `as` is complete. Keep
  `arithmetic/runtime_integer_casts_exit` as coverage for sign/zero extension,
  proved truncation, and cast-valued transition lowering; the named policy
  surface has separate coverage.

Acceptance: qualified `as` targets preserve denotation, bare targets make
non-owning semantic erasure explicit, arbitrary user code is never invoked,
and every proof used for exactness is retained; lossy or policy-bearing conversions are
visible calls; domain predicates and routes take their respective proof and
establishment paths; and normalized identity does not depend on the
transitional facet projection.

### Calling plans and boundary artifacts

#### ENT2c — finish normalized ABI lowering

Migrate remaining compatibility call paths to evaluated `CallPlan + StatePlan`.
The major x86-64/AArch64 argument, result, aggregate, syscall, vtable, and
service-table paths are already plan-driven.

Remaining:

- remove residual hardcoded placement decisions;
- implement the settled foreign-storage lifetime model: derive ordinary
  call-scoped borrows from reference-shaped ABI parameters; require storage
  used after return to move into an ordinary linear protocol claim; infer the
  consumed-input-to-produced-claim mapping through resource conservation; and
  preserve exact provider-era dependencies as compiler-owned claim metadata;
- add the provider-view dual for foreign-owned storage, using ordinary borrows
  where all invalidators require exclusive access and explicit claims where
  runtime protocol events end validity;
- keep raw `addr` and `Ptr<T>` inert and non-dereferenceable; a calling plan may
  describe their ABI representation but cannot manufacture authority;
- reject permanent foreign retention unless the consumed authority is
  transferred into an established static or process-lifetime root; do not
  invent a general permanent-custodian spelling without a concrete customer;
- record write-only views as a focused core-type follow-up rather than hiding
  write-only foreign access in a plan;
- extend differential checks wherever a compatibility encoder remains. The
  current vtable-slot, vtable-field, and service-table cohort now proves exact
  byte equality between compatibility selection and an explicit evaluated
  native plan on Microsoft x64, SysV AMD64, and AAPCS64; both result-bearing
  widths are pinned too. Authored scalar imports now carry the same byte/width
  lock on Microsoft x64 and both AAPCS64 targets, while SysV AMD64 explicitly
  fails closed without its required plan. Linux statement, value-result,
  timespec-result, and timespec-argument syscall families now prove byte and
  width equality on x86-64/AArch64; result/argument relocation sites are
  differential-locked too. Ordinary non-variadic scalar built-in imports now
  consume the binding-retained plan in emission, layout, and relocation
  accounting; their Windows x64/macOS arm64 compatibility bytes and widths,
  plus Windows x64 relocation sites, are differential-locked; and
- delete compatibility fields after their final consumer migrates.

Acceptance: changing a normalized plan changes lowering or rejects; changing
only policy source while producing the same canonical plan preserves contract
identity.

#### ENT3 — final state-footprint validation

**DESIGN BLOCKED (OWNER_QUESTIONS Q2).** The exact final executable-region
inventory, compiler-text relocation envelope, checked-assembly validation, and
format-owned import-thunk validators are live. Completing the general
compiler-function decoder and composing admitted leaves requires the canonical
certificate/decoder trust boundary for static and dynamically loaded artifacts.

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
  reachable state-transition cycle or truly unresolved frame stays opaque, so
  consumers fail closed rather than extrapolating from one observed route.
- **STR/EFX:** the source reach clause is now canonically `reaches`; the parser
  rejects legacy `effects` with directed migration guidance, and the Omega,
  canary, sample, and Cathedral source corpora use the new spelling. Syntax,
  symbol-resolved, and typed records/snapshots now name authored reach as
  service reach; termination decrease orders use independent arenas; and
  checked admissibility reports service reach, suspension, and blocking as
  separate dimensions. Finish independent service reach, `suspends`, `blocks`,
  termination, mutation, and trust publication/admission, then retire the
  remaining legacy internal umbrella names after their consumers migrate.
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
  invocation receipts, fixed-stack reservation, cancellation conformance,
  transactional argument custody, and task-claim provenance remain.
- **PSIIR — IMPLEMENTATION WORK:** build the terminal Psi boundary settled in
  `wiki/architecture/pipeline/terminal_psi.md`. Psi owns Omega-file parsing
  through one concrete, post-instantiation semantic module; Omega starts at
  abstract-operation lowering. The Psi-owned workspace root, stable semantic
  identities, typed scalar proposition core, module value-typing context,
  small structural proof kernel, versioned certificate envelope, total truth /
  reflexive-equality / closed-integer judgments, and sealed exact admission
  validator are live; architecture tests forbid Psi dependencies on Omega.
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
  second target-neutral semantic owner.
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
  v9-v12 rows; a content-bearing executable source canary remains.
  The first in-memory executable slice is also live: stable machine/block
  topology, representable integer constants, v2 Boolean constants, v3
  exact-width wrapping integer addition, v4 exact-width saturating integer
  addition, v5 exact-width wrapping integer subtraction, v6 exact-width
  saturating integer subtraction, v7 exact-width wrapping integer
  multiplication, v8 exact-width saturating integer multiplication, v9
  proof-only structural-place/content-conservation propositions, v10 canonical
  identity-preserving claim reshuffles, v11 stable sum-case content paths, and
  current-v12 exact authored-partition substitution rows; the executable slice
  retains unconditional jump/return edges,
  bodyful contracts, verifier-reconstructed semantic axioms, exhaustive proof-
  bundle checking, and direct execution of the verified module in
  `omega-interpreter`. Its validator rejects unreachable axiom sources and
  entry/postcondition references to internal values. The first Psi-owned
  checked-tree producer is also live and fail-closed: it lowers one exact typed
  integer-constant/unconditional-jump source slice whose return is either the
  matching literal or a builtin parameter-plus-literal add/subtract/multiply
  in the settled Wrapping or Saturating domain. A second exact form lowers any
  nonempty sequence of ordinary primitive-integer machine parameters and an
  exact recursively nested parameter/literal expression using builtin
  add/subtract/multiply in a settled arithmetic domain. A third exact form
  lowers a Boolean literal or one exact named parameter from ordinary Boolean
  parameters. It emits the module and
  proof bundle separately; real-source canaries cover all six versioned integer
  policy operations in constant-fed and runtime-fed forms, Boolean literal and
  ninth-parameter returns, plus a ninth-parameter integer stack return after
  `CheckedTrees` are dropped. Constant-fed
  wrapping add, the direct stack return, and a register-plus-stack runtime
  wrapping add all reach emitted host machine code; Boolean literal and ninth-
  parameter returns independently cover constant materialization and the host
  incoming-stack ABI. A nested wrapping
  add-then-multiply source expression does too. Because the
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
  AArch64 and x86-64. AArch64 preserves referenced argument registers in an
  aligned spill frame before evaluating into `x0`, and both emitters retain the
  original incoming-stack base across recursive evaluation. A nested v4
  `u8` canary consumes the ninth stack argument, wraps to 4, saturates to 255,
  and matches interpretation through a real C ABI call; a signed `i64` canary
  independently reaches both saturation bounds. General register assignment
  remains later implementation work. The next arithmetic slice is live in v5:
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
  `MIN * -1`, and an ordinary negative product. The
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
  propositions and their structural-place terms; all reject over-deep or
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
  claim reshuffles; v11 adds stable sum-case content-path segments; current v12
  adds exact authored-partition substitution rows; and explicit validated
  migration preserves an older semantic graph while producing a new v12
  fingerprint. Archived v1 through v11 identities remain frozen. The clean
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
  call/boundary slices must populate it from their selected plans. A typed
  debug/source-map payload schema, general register assignment, further closed
  arithmetic variants, and migration of the legacy backend remain.
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
  v1/v2/v3/v4/v5/v6/v7/v8/v9/v10/v11/v12
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
  return. Semantic safe-point selection, build-time migration, branch/loop
  certificates, response outcomes, and trusted native metering remain.
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
  the runtime ledger claims only Omega-mediated admissions.
- **REPLACE-OPAQUE:** extend component acceptance tests with selected-provider
  manifest union across coexisting eras, process-static service handover
  contracts, and mapping reuse only after proof that no live authority reaches
  the cohort. Proven quiescence permits ordinary reuse; an incomplete drain or
  a possible opaque holder reserves an unmapped/trapping quarantine
  with attributed capacity loss. A stale call must fault without being reported
  as discharged, and an opaque callback into replaceable code must use a
  process-lifetime gateway or an accepted unregister/quiescence contract.
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
  1. land the proof-side Prop-family/index-telescope fragment;
  2. add the proof stratum to selected-conformance projection and permit
     by-value `dyn` only when the complete normalized value has no runtime
     carrier;
  3. add transparent proposition aliases plus independent `Reflexive`,
     `Symmetric`, `Transitive`, and `Antisymmetric` requirements, with
     `Equivalence`, preorder, and partial-order composition;
  4. add `Respects` over normalized argument records, checking both
     representative-invariant semantic preconditions and related results; and
  5. migrate `%` from executable-`bool` relations and suffix-based law
     discovery to proposition evidence plus explicit selected conformances.
  Preserve the existing generic quotient canaries as migration coverage for
  heterogeneous machine-indexed representatives; add a decidable rational
  relation, existential Cauchy evidence, a total lifted operation, and a
  partial lifted operation as acceptance drivers.
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
  software realization exists. Remaining rung-3 work includes that x86-64 FMA
  realization, directed-rounding families, checked software fallbacks,
  canonical floating-control-state
  preconditions/restoration, and rung-4 differential evidence.
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
  manufacturing a source loan; opaque statement calls clear that provenance.
  Parameter-backed storage, cross-state propagation, call mutation summaries,
  and state-parameter root rebasing remain.
- Implement constant data parameters after their identity/coherence rules are
  pinned by existing generic machinery.
- Implement local dynamic traits as two-word borrowed descriptors selecting one
  complete nominal conformance. Derive the per-requirement dynamic surface,
  lower checked adapters, retain compile-time operational envelopes, add
  transparent trait refinements and named-conformance generic bounds, and
  prototype envelope/effect-row inference before committing the full lowering.
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
  evaluation remains legal.
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
- Implement normalized `Atomic::fence` for
  `Receive | Publish | ReceivePublish`; define
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
- Implement the settled retained-storage and provider-view canaries under
  ENT2c; keep `addr`/`Ptr<T>` inert and require protocol-correlated redemption.
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

- **ENTRY-CONTENT-ROOTS:** blocked on `OWNER_QUESTIONS.md` Q1's stable semantic
  owner/identity for image and initial-storage handoff roots.
- **ENT3:** blocked on `OWNER_QUESTIONS.md` Q2's final artifact-footprint
  certificate format, trusted decoder surface, and admitted-leaf composition
  boundary.
- **LINUX-METADATA:** blocked on `OWNER_QUESTIONS.md` Q3's choice between a
  width-adapting integer layout placement and target-owned foreign-record
  normalization.
- **TERMINAL-CONTENT-CLAIMS:** blocked on a versioned terminal-Psi entry-claim
  binding that identifies a partition theorem's consumed inputs without
  asserting the stronger, false one-to-one input/output equality. This also
  blocks the dependent bump-allocator canary and frontier work.

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
  through the same plan surface. **Linux metadata is DESIGN BLOCKED
  (`OWNER_QUESTIONS.md` Q3):** the real x86-64/AArch64 `struct stat` fields
  require target-width normalization that the settled layout vocabulary cannot
  express. Linux `read_dir` now retains the real three-argument `getdents64`
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
