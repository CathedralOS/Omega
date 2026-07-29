# Tasks

Last pruned: 2026-07-28.

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

- **Normalized `WorkPlan` (#4):** required by bounded interrupt work,
  work-to-semantic-safe-point reporting, deterministic build-evaluation
  accounting, and complete foreign/callback work attribution. The current
  provider-local fixed-work composer is an implementation precursor, not the
  general analysis.
- **Higher-order callback operational summaries:** a future direct raw-callback
  checker may need summaries stating which callback parameters an operation may
  synchronously invoke, then SCC composition over those summaries. Registered
  callback safety does not currently depend on this analysis: platform adapters
  define restricted handler requirements and check ordinary application reach
  locally. Promote this item only when a concrete non-adapter customer requires
  modular raw-callback cycle analysis.
- **Formal atomic-event model and target refinement:** define
  `sequenced_before`, `reads_from`, `modification_order`, `synchronizes_with`,
  `happens_before`, and `global_sequential_order`; mechanize the portable
  access/fence axioms; and prove the x86-64/AArch64 mappings. Existing ordering
  labels and instruction selection are implementation evidence, not this
  analysis. Modular package premises remain owner-blocked on #9.

## Priority queue

### P1 — Authority values and boundary evidence

The runtime model is specified in
`wiki/design_briefs/authority_values_and_boundary_evidence.md`.

#### P1a — bodyless establishment and admitted roots

**CLEAR DIRECTION.** Authority-bearing runtime values use ordinary data fields
plus bodyless domain facts.

Implementation checkpoint (2026-07-28): semantic facts distinguish evidence
origin from program-point origin; checked carrier owners may establish their
own bodyless result facts without bypassing bodyful predicates; transfer
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
owner-machine, domain-operator, or boundary-requirement identities; alias
guarantees expand to atomic routes, typed constraints and snapshots preserve
them, and checked consumers no longer reconstruct authority from names.
Core `Extent` now exposes its ordinary `{ base: addr, length: u64 }` geometry
and carries authority through bodyless `Extent::Granted`; every declared-domain
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
  use bodyless `Active`/`Pending` facts, so reconstructing identical fields
  cannot settle either obligation.
- connect installed-root entry receipts to those interrupt facts, add their
  compiler-owned carry policy and authority-flow rows, and migrate the
  `TaskRuntime` handle through the ordinary selected-provider behavior evidence,
  stack-resource, and custody work tracked under TR3–TR8.

Acceptance: reconstructing an authority carrier does not establish its facts;
an owner machine cannot satisfy a bodyful result without proving its body; an
admitted provider cannot originate a fact outside an owner-authorized
requirement; receipts and authority-flow reports identify every accepted
origin; and Cathedral obtains one qualified root at a time from its admitted
memory provider without split, merge, or an array of checked claims.

#### P1b — delegated canonical qualification

**DESIGN BLOCKED — OWNER_QUESTIONS #3.** Decide the owner-authored source and
package-identity relationship that delegates canonical qualification authority.
Until then, only the domain-owning package may publish an implicitly eligible
`RepresentationQualification<Q>` satisfier; all third-party conformers fail
closed.

Acceptance: an explicit delegation opens exactly the named bodyless domain for
exactly the delegated package, survives separate compilation by normalized
identity, is visible in qualification artifacts, and is non-transitive unless
the settled design says otherwise. Imports, aliases, matching names, and
ordinary trait visibility do not delegate authority.

#### P1c — composite resource frontiers

**PARTIALLY CLEAR.** Implement path-indexed partial moves and normalized
multi-output claim transformations. Per-claim carry inheritance has settled
semantics. The source surface that marks a qualification as content-bearing and
authors its projection, admitted backing, retirement, and conservation contract
is **DESIGN BLOCKED —
`OWNER_QUESTIONS.md` #6**; do not infer content from multiplicity or invent a
declaration spelling.

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
aggregates still contribute one indivisible nominal root.

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

This is not full P1c: content projections/backing and conservation witnesses
remain. Symbol-keyed substitutions already retain contained claims through
nested generic transparent records. Content authoring remains blocked on owner
question #6.

- make content-bearing qualified claim kinds publish one normalized projection
  into a compiler-owned partial composition algebra;
- implement the initial closed normalized vocabulary
  `Indivisible | Interval<Scalar>` once owner question #6 settles how an
  authored content clause selects it; never default ordinary linear claims into
  that vocabulary;
- require admitted roots to carry backing receipts denominated in the same
  algebra and prove projected content is within that backing;
- prove all consumed content equals the separated composition of produced
  content plus any remainder retired through an authorized route;
- conserve every independent content-bearing claim kind and require one joint
  projection when correspondence between quantities carries authority meaning;
- keep domain facets, permission attenuation, carry, and root lineage as
  independent axes; recoverable or scarce authority uses a claim or loan rather
  than a discardable permission; and
- retain normalized projections, admitted backing, and the n-ary conservation
  witness beside the landed outcome maps in proof/debug artifacts.

Extent split/merge is triggered only when an independently owned subrange must
cross an ownership boundary; it is not required for the admitted-root,
placed-view, subrange-loan, or borrow-backed Arena slices. Virtual-to-physical
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

**READY.** Chapter 20 and
`wiki/design_briefs/os_memory_and_hardware_foundation.md` own the source model.

- Add the ordinary `PlacementPlan`, `AccessPlan`, `ResourceProfile`,
  `BoundaryReach`, transfer-rule, exposure, observation, and operation records
  to `omega::core`.
- Add build-time `Placement::plan(schema)` and
  `Access::plan(schema, validated_layout)` evaluation. Construct each access
  plan from an all-inaccessible seed using compiler-issued schema field keys.
- Migrate the normalized Rust model from name-keyed vectors, per-entry reach,
  generic RMW, `ProviderPrivate`, and reusable placed-view grants to exact
  schema cardinality, reach per placement, derived RMW legality,
  `BindingPrivate`, and an admission token owning the exact Extent loan.
- Implement offset-keyed admitted resource profiles, profile restriction on
  subrange loans, consumer-demand/provider-supply compatibility, and the
  build-time base-congruence plus runtime-base alignment split.
- Derive `Placed<P, T>` projection and granular readable, destructive-read,
  writable, and atomic accessors. Ordinary writes require plan permission,
  exclusive current borrow, and exclusive source loan.
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
4. ordinary Arena/Allocation support for dynamic hierarchy allocation.

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

- Finish the ordinary core float/integer and float-format conversion machines,
  with explicit narrowing and rounding policy, then retire the legacy numeric
  `as` spelling after its last compatibility-only consumer.
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
  result family merely to mirror another language. Remaining implementation
  work is float/integer and float-format named operations, their corpus
  migration, and retirement of numeric compatibility `as` after its last
  internal conversion consumer.
  Keep `arithmetic/runtime_integer_casts_exit` on compatibility `as` until that
  retirement: its authored cast-initializer/transition-parameter shape is the
  regression for legacy sign/zero extension and truncation lowering, while the
  named surface has separate coverage.

Acceptance: qualification `as` preserves carrier, payload, and runtime work;
numeric and unit conversions are visible calls; bodyful and bodyless domains
take their respective proof and establishment routes; and normalized identity
does not depend on the transitional facet projection.

### Calling plans and boundary artifacts

#### ENT2c — finish normalized ABI lowering

Migrate remaining compatibility call paths to evaluated `CallPlan + StatePlan`.
The major x86-64/AArch64 argument, result, aggregate, syscall, vtable, and
service-table paths are already plan-driven.

Remaining:

- remove residual hardcoded placement decisions;
- keep foreign-pointer lifetime work blocked on owner question #1 rather than
  inventing implicit retention;
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

### Provider plans and retirement of `provides`

Reference: `wiki/design_briefs/extern_boundary_and_format_domains.md`.

- **PRV4b:** finish checked Console adapters over selected native leaves.
  Adapter dispatch now consumes the exact checked-adapter rows retained in the
  selected whole-provider plan; a free compatibility adapter cannot overlay a
  selected nominal or external realization. Receiverless statement and value
  calls retain the selected entry symbol and full nominal target. The remaining
  Console slice is to replace its no-selected-plan free-adapter fallback with
  complete target provider closures over the native byte leaves.
- **PRV4c:** finish target defaults and type-per-slot overrides.
- **PRV4e:** migrate remaining foreign offsets/flags into format/layout policy.
- **PRV4f:** delete compatibility `provides`, `call_shape`, and host-operation
  chains after the final consumer moves.
- Retire `Binding::Instruction` as parsed checked-assembly coverage lands.

Acceptance: provider plans derive from declarations and selected conformances;
no source-authored row builder or duplicate requirement-to-implementation table
remains.

### Compile-time machine parameters and generics

Compile-time machine parameters are live; do not cite them as a generic
blocker. Distinguish them from runtime reification of machine identity.

- **MP6:** finish consuming `Seq::map`/`filter` and remaining concrete generic
  collection slices.
- Complete backend monomorphization and cache identity for generic data and
  machine instantiations.
- Keep general `Entry::of<H>`-style runtime relocation reification separate
  from callback lowering. A callback registration parameter provides the exact
  requirement context needed to select a named static machine and emit its
  private relocation; type-parameter invocation alone does not.

Acceptance: a declared `<machine F>` with its required `where machine F(...)`
contract monomorphizes and calls directly; omitted contracts reject even when
current consumers happen to align.

### Frames, domains, effects, and trust

- **R5:** finish relational frame candidates and escaping mutation checks.
  Boundary write-frame spelling is owner-blocked on #2. Exact resolved
  statement/value-call frames now preserve unrelated incoming range guards in
  the proof checker and transitive range-fact collector; opaque and overlapping
  frames remain conservative fences.
- **DOM1/DOM2/DOM3/DOM5:** finish operator ownership and weakening
  certificates. Delegated package authority is owner-blocked on #3.
- **STR/EFX:** finish independent service reach, `suspends`, `blocks`,
  termination, mutation, and trust publication/admission. Remove legacy mixed
  rows after migration. Imported transparent-refinement spelling must supply the
  narrowed operational envelope consumed by the completed exact call-
  acknowledgement checker.
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
- **WORKPLAN:** after owner question #4, implement one deterministic
  abstract-work algebra for interrupt roots, work-to-next-safe-point queries,
  and build-evaluator metering. Preserve maximum/unbounded path attribution and
  keep external wait plus wall-clock conversion in separate trust-bearing
  columns.
- **FFIGATE:** after owner question #5, implement the hosted-FFI gateway as an
  ordinary bounded native-worker provider with explicit queue admission,
  stack provision, cancellation disposition, retained-loan custody, and
  shutdown/quiescence. Registered callback lowering is ENT4; retained pointer
  lifetime remains blocked on #1.
- Replace ambient allocation with `Arena`/`Allocation`; connect Arena backing
  to qualified `Extent` after P1.
- Implement owned `Vec<T>` and then `Vec<u8> in Utf8` through ordinary data and
  domain qualification.

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
  **Language-design blocked:** the public float/integer and
  float-format conversion requirement names and signatures are not settled
  anywhere in the owning brief; only the negative ruling that compatibility
  `as` is not that surface is settled (`OWNER_QUESTIONS.md` #10).

Keep `Real` proof-only and core-level. Do not lower it as a runtime float or
move it to a convenience library.

### Lifetimes and remaining source surfaces

- Finish general outlives constraints, persistent owners, and remaining
  aggregate borrow propagation.
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
- Migrate atomic source orderings to
  `NoOrdering | Receive | Publish | ReceivePublish | GlobalOrder`; implement
  normalized `Atomic::fence` for `Receive | Publish | ReceivePublish`; define
  the formal atomic-event model and complete target-refinement proofs before
  enabling protocol verification. Keep the strong receive baseline on
  AArch64; a weaker acquire instruction requires protocol-scoped proof and
  measured justification. A global-order fence requires the completed global
  atomic semantics rather than a conservative backend guess.
- Foreign retained-pointer lifetimes are owner-blocked on #1.
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
| #1 retained foreign pointer | asynchronous/retained FFI borrows |
| #2 boundary write frame | R5 boundary mutation clauses |
| #3 delegated canonical qualification | third-party bodyless-domain qualification |
| #4 normalized bounded-work plan | interrupt bounds, safe-point response, evaluator cost algebra |
| #5 hosted-FFI gateway | reusable native-worker execution and backpressure |
| #6 claim-content projection and backing | P1c content algebra and conservation |
| #7 opaque in-process executable trust | root TCB declaration and profile rejection |
| #8 contained execution failure | obligation poison, recovery, and reclamation |
| #9 modular concurrency premises | separately compiled protocol verification |

## Vertical acceptance slices

- **Termination firewall:** cyclic components strictly decrease one joint rank;
  private witnesses never enter public contract identity.
- **Contract/admission split:** service reach, suspension, blocking,
  termination, mutation, and trust normalize independently. Candidate resource
  demand and installed provision admit separately; a fixed resource ceiling is
  contract identity only when policy deliberately publishes one.
- **Units:** implement two units in one dimension with canonical bodyless
  qualification, explicit conversion, arithmetic-policy composition, generic
  preservation, and operator coherence.
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
- universe levels before a full math-library replay goal; and
- an optimizing SSA/register-allocation/SIMD backend beyond current correctness
  requirements.
