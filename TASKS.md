# Tasks

Last pruned: 2026-07-31.

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

Omega owns language semantics and general compiler machinery. Target backends
own unavoidable ISA, ABI, object-format, and relocation encoding. Cathedral
owns OS data structures, policies, protocols, and lifecycle.

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

- **Canonical IR fuel and restricted fixed-work checking:** define the
  versioned portable IR; meter realized evaluation; and analyze whole hard-root
  or selected safe-point segments as `Bounded`, `Unknown`, or an attributed
  no-finite-guarantee result. The hard-root precursor is now denominated by an
  explicit, separately versioned fuel schedule: mixed schedules fail closed
  and the installed-root artifact publishes the schedule and provision. It is
  not yet canonical-IR derivation, general parametric work, or WCET analysis.
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
- connect the concrete installed-root invocation and mask-transition receipts
  to source `Pending`/`Active` establishment, carry those receipt identities
  into the resulting checked facts, and migrate the
  `TaskRuntime` handle through the ordinary selected-provider behavior evidence,
  stack-resource, and custody work tracked under TR3–TR8.

Acceptance: reconstructing an authority carrier does not establish its facts;
an authorized route cannot satisfy a predicate-bearing result without proving
its predicates; an admitted provider cannot originate a fact outside a
requirement named by the domain; receipts and authority-flow reports identify every accepted
origin; and Cathedral obtains one qualified root at a time from its admitted
memory provider without split, merge, or an array of checked claims.

#### P1b — domain establishment and exact coercion surface

**DESIGN SETTLED; IMPLEMENTATION IN PROGRESS.** Move predicate propositions from
domain bodies to ordinary `requires`. Domain bodies enumerate exact
trait-requirement identities authorized to establish provenance; every
predicate is proved at an authorized route's qualified return. An empty domain
has no establishment obligations and permits explicit qualification from its
bare carrier. Remove ambient package-owner minting and retire the legacy core
qualification relationship from domain establishment.

Implement `as` as one compiler-derived surface: qualified targets preserve
denotation across numeric widening, proved exact narrowing, and direct
qualification; explicitly bare targets erase non-owning semantic meaning. It
invokes no arbitrary user code and never discovers or invokes a unit conversion.
Predicate-only atoms may weaken implicitly; semantic and non-owning provenance
atoms erase only through explicit `as`; owned claims require consumption or
transfer.

Implementation checkpoint (2026-07-31): explicit `as` now qualifies directly
into an empty domain, including a transparent alias only when every expanded
atom has neither predicates nor establishment routes. The legacy core
`RepresentationQualification` trait, its privileged semantic roles, selected
satisfier field, erased named-call lowering, and canonical-use artifact are
retired. `05_qualification_evidence.json` instead reports each exact
`vacuous_qualification` origin. Predicate-bearing and routed atoms remain
fail-closed on this path. Exact integer `as` now preserves mathematical value:
all widening accepted by the source carrier range, while narrowing and
signedness changes require a declared-range or dominating-guard proof.
Unproved casts reject and direct authors to visible named policies; same-carrier
policy erasure remains exact by bounding inferred facts with the source carrier.
Per-atom weakening is now enforced at assignments, initializers, arguments,
returns, struct fields, and array elements: predicate-only atoms may disappear
implicitly, while semantic meaning, routed provenance, and non-Exact arithmetic
policy require an explicit bare `as`. Same-data-carrier `as` supplies the
zero-runtime-work provenance-erasure surface. Unit conversion remains an
ordinary named library operation. Proof-static indexed domains follow the
staged implementation below.

The source/IR route migration is complete: domain predicate `requires` and exact
`Trait::requirement` body entries now parse into independent records; authored
routes resolve once to ordinary checked or boundary requirement symbols and
reject unresolved or result-mismatched citations. Checked conformers originate
`authorized_route_establishment` evidence only through the exact cited slot,
and a routed domain carrying predicates still requires those propositions to
be proved. Core `Extent::Granted` now authors its `ExtentRootProvider::grant`
route directly. Domain predicates across the core, canary, sample, lattice,
and embedded-test corpora now use ordinary `requires`; predicate-in-body syntax
rejects with directed migration guidance. Domain operators now use ordinary
top-level declarations: an exact `operator Type::Domain::operation ...` name or
one unique domain-qualified operand tuple supplies the semantic home, nested
declarations reject with directed migration guidance, and operator ownership
no longer grants domain establishment. Owner-machine and ambient boundary
contract placement no longer infer routes; every checked or admitted
establishment path comes from an exact requirement authored by the domain.

Acceptance: a look-alike trait cannot establish another domain; owner code has
no establishment privilege outside named routes; checked and admitted
conformances retain exact route identity and receipts; `i32::Km` qualifies
freely; `Extent::Granted` does not; exact `Km`/`M` coercions share operator
normalization; lossy coercions reject; and no semantic domain disappears
implicitly.

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

Implementation checkpoint (2026-07-31): core now publishes `Content<A>`,
`Interval<CoordinateSpace>`, and `CountedQuantity<Unit>`. A projection must be
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
backing, retirement, and ambiguity proofs remain outstanding.

Implementation checkpoint (2026-07-31): `05_claim_outcomes.json` now retains
every checked content projection beside the path-indexed outcome maps. Each row
keeps exact domain and machine identity, carrier identity, structured closed
algebra, the complete normalized symbolic expression (including runtime scalar
embeddings and arithmetic), semantic-domain identity, and the stable projection
fingerprint. It does not publish placeholder backing or conservation witnesses;
those rows remain absent until their actual checked proofs exist.

- **BUMP-ALLOCATOR-CANARY — LANGUAGE-DESIGN BLOCKED on
  `OWNER_QUESTIONS.md` #5:** implement an ordinary package-level bump strategy
  over a consumed `Extent` once source content-conservation contracts can state
  its split, retirement, reset recomposition, and backing return. Keep
  allocatable tail, live extents, and retired extents distinct: release cleans
  `T` and returns authority but restores bump capacity only at reset. Exercise
  RAM and non-RAM placed access without adding an Arena primitive, interior
  mutability, or a new borrowing rule;
- **BACKING-RECEIPT — LANGUAGE-DESIGN BLOCKED on `OWNER_QUESTIONS.md` #4:**
  require admitted roots to carry backing receipts denominated in the same
  algebra and prove projected content is within that backing through ordinary
  postconditions. Provider selection and receipt identity are live, but no
  source/IR binder yet supplies the receipt's per-invocation algebra value;
- **CONSERVATION-CONTRACT — LANGUAGE-DESIGN BLOCKED on
  `OWNER_QUESTIONS.md` #5:** prove all consumed content equals the separated
  composition of produced content plus any remainder retired through an
  authorized route. The equation and closed algebras are settled, but the
  documented `content(...)`/`old(...)` forms remain schematic: no source or IR
  surface binds projection selection, entry snapshots, separated composition,
  or route-authorized retirement. The same decision must conserve every
  independent content-bearing claim kind and require one joint projection when
  correspondence between quantities carries authority meaning;
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
their scalar measures add up; gaps reject unless an authorized retirement
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

**PARTIALLY DESIGN BLOCKED.** Chapter 20 and
`wiki/design_briefs/os_memory_and_hardware_foundation.md` own the normalized
model. The source-visible loan/profile admission surface is blocked on
`OWNER_QUESTIONS.md` #1, and public generic atomic accessor requirements are
blocked on #2. Target-specific lowering remains implementation work.

- Derive `Placed<P, T>` projection and granular readable, destructive-read,
  writable, and atomic accessors. Ordinary writes require plan permission,
  exclusive current borrow, and exclusive source loan.
- Source derivation now retains the authoritative placement identity and exact
  per-field permissions in typed trees. Stable/external accessors expose only
  admitted trait methods; direct atomic syntax over `bool`, `u32`, and `u64`
  is checked per operation family, works through a shared view borrow, and
  cannot materialize an accessor as an ordinary scalar. Binding-private
  accessors are restricted to machines authored in the nominal placement
  policy's canonical package, including statement-position calls whose
  generated target symbol is absent. Generic atomic-family helper contracts
  are blocked on owner question #2. Admitted source-loan construction is
  blocked on owner question #1.
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
- program PIT+PIC first, with LAPIC as the production provider; and
- keep the hard root fixed-work: acknowledge, capture time, set a coalescing
  wake state, return. Timer fan-out belongs in an ordinary scheduled task.

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
  operation blocked only on the checked-result arithmetic carrier.
- Generalize named-machine/requirement overload identity beyond the current
  path-and-parameter rule: normalize the result's dispatch-bearing domain set,
  reject duplicate sets at declaration, select the empty set without an
  expected result, require set equality otherwise, and prove predicate-only
  refinements after selection. Include the set in checked/artifact/symbol
  identity. Replace the current return-only-overload rejection canaries with
  positive result-domain cases plus duplicate-predicate and semantic-weakening
  rejection canaries. Fixed operator spellings remain operand-directed.
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
- add differential checks where a compatibility encoder remains; and
- delete compatibility fields after their final consumer migrates.

Acceptance: changing a normalized plan changes lowering or rejects; changing
only policy source while producing the same canonical plan preserves contract
identity.

#### ENT3 — final state-footprint validation

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
- **DOM1/DOM2/DOM3/DOM5:** exact integer `as`, per-atom
  weakening/explicit erasure, operator ownership, predicate `requires`, and
  exact route bodies are complete. Keep unit conversion in ordinary library
  machines and operators.
- **PDI1:** generalize `const` parameters to structured values with decidable
  structural equality and one canonical form. Reject noncanonical index values
  at the index site; current `Rat` values must have a positive denominator,
  cancelled signed coordinates, and gcd-reduced numerator magnitude and
  denominator.
- **PDI2:** implement closed indexed erased domains using
  `domain<T, const U: Unit> T::Quantity<U>;`. The first units package uses named
  combinations, spans carriers from one declaration, supports a destination
  index parameter in generic conversion, and uses ordinary per-pair operators.
- **PDI3:** only after PDI2, add computed open result indices, exact selected
  algebra-instance normalization, established-local-fact compatibility, and
  retained verification-condition evidence. Do not add a special citation
  surface; unresolved equality rejects.
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
  complete. Authority-value declarations follow P1a.
- **IRFUEL — PARTIALLY ARCHITECTURE BLOCKED:** implement the settled
  `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md` sequence:
  versioned canonical IR and fuel schedule, evaluator/interpreter metering,
  restricted fixed-work checking over entries and safe-point segments,
  attributed response outcomes, and trusted native block metering. Keep target
  WCET and wall-clock conversion separate. The external-root precursor already
  has schedule-keyed provider summaries and provisions, rejects mixed
  schedules, and reports logical fuel rather than structural work; continue
  from canonical IR and its interpreter meter rather than treating that
  provider-authored precursor as an IR proof. The v1 canonical IR schema,
  serialization, and verifier/lowering boundary are blocked on
  `OWNER_QUESTIONS.md` #3. The current TypedTrees evaluator now publishes an
  explicitly versioned deterministic step-usage record for interpreted and
  build-time outcomes; it is telemetry precursor evidence, not canonical-IR
  fuel.
- **FFIVAL:** validate the settled boundary model before adding any new
  construct. The returned-custody-from-borrow rejection canary now lands
  through content-algebra facts. The provider-independent executor-selection
  gate now consumes exact per-axis checked/admitted evidence identities,
  rejects a CPU- or host-thread-affine activation when the selected executor
  lacks the matching axis, and retains the validated selection in task
  lifecycle custody; source selected-provider evidence wiring remains under
  TR3–TR8. Then
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
  `OWNER_QUESTIONS.md` #5. Core supplies qualified `Extent`, placement, and
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
  engines. Remaining rung-3 work includes FMA, directed-rounding families,
  checked software fallbacks, canonical floating-control-state
  preconditions/restoration, and rung-4 differential evidence.
  The public float/integer and float-format conversion requirement family is
  settled. Remaining work is source/core publication, result-domain overload
  resolution, provider selection/lowering, and canaries. The checked-result
  operation remains separately design-blocked on its public result carrier.

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
- Implement the settled retained-storage and provider-view canaries under
  ENT2c; keep `addr`/`Ptr<T>` inert and require protocol-correlated redemption.
- Implement registered callback lowering and the Windows adapter canary under
  ENT4 without introducing a general source-visible code-address value.

### Wire runtime

The language model is settled in guide chapters 21-22 and the programmable
layouts brief. Complete the implementation in dependency order:

- [done] extend the live repeated-field encoder (exact `[T; N]`, bounded
  `FixedVec<T, N>`, and borrowed byte slices) to general borrowed scalar
  slices, retaining runtime length, two-pass work, and exact output-capacity
  obligations in the normalized generated encode plan;
- extend repeated encode/decode to `Vec<T>` once its allocator obligations are
  available; packed scalar decode into `&[T]` remains intentionally
  unsupported because varints cannot form a zero-copy scalar view;
- [done] expose strict, projecting, and preserving decode requirements;
  implement the preserving package carrier `Relayed<T>` with an opaque
  round-trip remainder;
- [done] represent published historical formats as ordinary immutable data and
  select checked migration machines through format-lineage packages;
- [done] make compatibility checks consume channel/store demands and report
  directional readability, writability, unknown preservation, canonicality,
  and migration coverage; and
- [done] retain realization origin separately from trust, classifying the
  current generated `compact_binary` codec as compiler-admitted until its
  generated body is independently checked against the public codec
  requirement.

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

The question document owns the context and alternatives. This table only routes
blocked work.

| Question | Unblocks |
|---|---|
| #1 placed-storage admission surface | source Extent loans, profile receipts, placement admission, and Placed construction |
| #2 generic atomic accessor requirements | generic helpers over exact placed/core atomic operation families |
| #3 canonical portable IR contract | portable artifact schema, interpreter boundary, IR fuel schedule, and IR proof/PCC identity |
| #4 algebra-denominated backing | source-visible admitted backing receipts and containment obligations |
| #5 content-conservation contracts | normalized n-to-m content equations, correspondence, allocator canaries, inference, and retained proof evidence |

## Vertical acceptance slices

- **Termination firewall:** cyclic components strictly decrease one joint rank;
  private witnesses never enter public contract identity.
- **Contract/admission split:** service reach, suspension, blocking,
  termination, mutation, and trust normalize independently. Candidate resource
  demand and installed provision admit separately; a fixed resource ceiling is
  contract identity only when policy deliberately publishes one.
- **Indexed domains, rung 2:** implement structured const parameters plus one
  erased domain family over closed named unit indices. Prove one declaration
  spans `f64` and `i64` carriers, carrier layout and SIMD shape remain unchanged,
  arithmetic-policy composition stays independent, and predicate facts derive
  through `ensures`. Computed `A / B` result indices belong to rung 3.
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
  coverage exists; remaining Linux work is path/stat/directory/errno adapters.
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
