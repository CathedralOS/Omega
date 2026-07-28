# Tasks

Last pruned: 2026-07-26.

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
mint `Granted`. Compiler-owned carry atoms and the remaining authority
migrations remain.

- migrate task-runtime handles, interrupt guards, and acknowledgement tokens to
  ordinary data declarations with their required fields;
- connect provider receipts, linearity, carry policy, and authority-flow
  reporting; and
- migrate Cathedral's temporary Extent model onto the shared declaration.

Acceptance: reconstructing an authority carrier does not establish its facts;
an owner machine cannot satisfy a bodyful result without proving its body; an
admitted provider cannot originate a fact outside an owner-authorized
requirement; receipts and authority-flow reports identify every accepted
origin; and Cathedral obtains one qualified root at a time from its admitted
memory provider without split, merge, or an array of checked claims.

#### P1b — delegated canonical qualification

**DESIGN BLOCKED — OWNER_QUESTIONS #16.** Decide the owner-authored source and
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

**CLEAR DIRECTION.** Implement path-indexed partial moves and content-conserving
multi-output transformations.

- give every establishment a fresh claim identity while retaining root lineage
  independently from its current canonical place;
- infer one-to-one and unambiguous aggregate outcome mappings, preserve sibling
  obligations on partial moves, and reject ambiguous mappings;
- make content-bearing qualified claim kinds publish one normalized projection
  into a compiler-owned partial composition algebra;
- implement the initial closed vocabulary `Indivisible | Interval<Scalar>`,
  with indivisible as the default;
- require admitted roots to carry backing receipts denominated in the same
  algebra and prove projected content is within that backing;
- prove all consumed content equals the separated composition of produced
  content plus any remainder retired through an authorized route;
- conserve every independent content-bearing claim kind and require one joint
  projection when correspondence between quantities carries authority meaning;
- keep domain facets, permission attenuation, carry, and root lineage as
  independent axes; recoverable or scarce authority uses a claim or loan rather
  than a discardable permission; and
- retain normalized projections, admitted backing, outcome mappings, and the
  n-ary conservation witness in proof/debug artifacts.

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

- Define ordinary core numeric conversion machines, with explicit narrowing
  policy, then migrate width and float/integer conversion away from the legacy
  `as` spelling.

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
- keep foreign-pointer lifetime work blocked on owner question #14 rather than
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

### Provider plans and retirement of `provides`

Reference: `wiki/design_briefs/extern_boundary_and_format_domains.md`.

- **PRV4b:** finish checked Console adapters over selected native leaves.
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
- Keep `Entry::of<H>`-style runtime relocation reification behind owner
  question #12; type-parameter invocation does not provide it.

Acceptance: a declared `<machine F>` with its required `where machine F(...)`
contract monomorphizes and calls directly; omitted contracts reject even when
current consumers happen to align.

### Frames, domains, effects, and trust

- **R5:** finish relational frame candidates and escaping mutation checks.
  Boundary write-frame spelling is owner-blocked on #15.
- **DOM1/DOM2/DOM3/DOM5:** finish operator ownership and weakening
  certificates. Delegated package authority is owner-blocked on #16.
- **STR/EFX:** finish independent service reach, `suspends`, `blocks`,
  termination, mutation, and trust publication/admission. Remove legacy mixed
  rows after migration.
- **TPR4/TPR6:** connect progress-profile grants and receipts without putting
  ranking witnesses into public identity.
- **GR6:** finish remaining qualification/trust consumers.

Acceptance: each contract axis normalizes independently; a wrapper cannot
launder reach; omission remains a strict public guarantee; private proof
improvements do not change public identity.

### Carry, multiplicity, tasks, and allocation

- **CRY:** implement the four compiler-owned positive carry permissions,
  `Carry::Portable` expansion, strict accepted-resource origins, checked-origin
  derivation, permission retention after qualification forgetting, and
  per-axis inheritance through aggregates and inferred claim transformations.
  Conserved multi-output inheritance follows P1c; admitted-root and one-to-one
  inheritance follow P1a.
- **CML4:** finish structural multiplicity migration. Implement checked
  `EdgeCleanupPlan` construction after outgoing-value materialization and
  transfer-map commitment; deterministic reverse-declaration cleanup;
  contextual cleanup-contract checking; structural partial-value cleanup;
  nominal-drop partial-move rejection; repeated-cycle resource composition;
  and conservation-witness/backend-ledger reporting. Composite resource
  frontier transformations follow P1c.
- **TR3–TR8:** finish task activation, custody, continuations, suspension-safe
  loans, and reference packages. Runtime provider publication is owner-blocked
  on #9; authority-value declarations follow P1a.
- Replace ambient allocation with `Arena`/`Allocation`; connect Arena backing
  to qualified `Extent` after P1.
- Implement owned `Vec<T>` and then `Vec<u8> in Utf8` through ordinary data and
  domain qualification.

Acceptance: linear debt cannot disappear through aggregation or bulk reclaim;
carry demands are checked against runtime behavior at admission; task and
allocation handles expose no compiler-owned continuation/control storage.

### Mathematical and float libraries

- **N6:** finish quotient/convergence packaging after owner question #4.
- **N8:** expand the construction corpus and proof-engine support needed by
  layouts, quotients, and `Real`.
- **F7:** implement float-format providers after owner question #10 determines
  the primitive-operation requirement family.

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
  erased values only after general storage ownership, size/alignment metadata,
  and cleanup contracts can support them.
- Extend compiler-run Omega/build-time evaluation after owner question #5.
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
- Portable atomic fences are owner-blocked on #13.
- Foreign retained-pointer lifetimes are owner-blocked on #14.
- External entry reification/registration is owner-blocked on #12.
- Suspension-capable direct-call spelling is owner-blocked on #6.

### Wire runtime

**OWNER-BLOCKED: `OWNER_QUESTIONS.md` #11.**

After the next wire family/presence/evolution ruling, implement remaining wire
values and codecs through ordinary data plus layout/format policy. Do not
restore `wire data` or a universal representation.

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
| #3 resource frontier and transformations | contained linear debt, cleanup, and authority transformations |
| #4 quotient convergence | N6/`Real` quotient packaging |
| #5 compiler-run Omega | richer build-time policies and generators |
| #6 suspending direct-call spelling | explicit suspension call surface |
| #9 task-runtime provider publication | task admission/dispatch |
| #10 primitive float requirement family | float-format providers |
| #11 wire family/presence/evolution | remaining wire runtime |
| #12 sealed external entry reference | callbacks and dynamic entry registration |
| #13 portable atomic fence | standalone fence surface |
| #14 retained foreign pointer | asynchronous/retained FFI borrows |
| #15 boundary write frame | R5 boundary mutation clauses |

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
- Windows GUI callback entry remains blocked on #12; do not pass a raw code
  address or add a Win32-only callback escape.

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
