# Tasks

Last pruned: 2026-07-24.

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

### P0 — Remove compiler-owned IDT lifecycle

The page-table specialization has been removed. Apply the same ownership rule
to the IDT specialization currently embedded in `omega-external-roots` and its
backend consumers.

Keep:

- the general external-root ledger and installed-root analysis;
- sealed entry identities;
- generic fragmented/symbolic materialization;
- checked `lidt` instruction encoding and contracts; and
- provider admission and general grants/receipts.

Remove or migrate into Cathedral package code:

- `PreparedIdtWriter`, `PopulatedIdtWriter`, and `MaterializedIdt`;
- `PreparedIdtLoad` and `InstalledIdt`; and
- IDT-specific destination, control, materialization, and installation
  identities/receipts.

Do not add another customer-shaped replacement. If Cathedral cannot express a
step, route the missing general primitive into P1/P2. Done when compiler crates
contain no IDT lifecycle model and Cathedral still owns its gate schema and
policy.

### P1 — Runtime representation for opaque boundary values

**OWNER-BLOCKED: `OWNER_QUESTIONS.md` #8.**

`boundary data` is correctly non-constructible, but runtime values currently
have no admitted storage/passing representation. This blocks honest source
values for `Extent`, `Ptr<T>`, task runtimes, interrupt guards, acknowledgement
tokens, and similar provider-minted authority.

After the owner ruling:

- implement the normalized representation plan and ABI/storage lowering;
- make provider minting the only introduction path;
- preserve linearity, carry policy, identity, and lookup/consumption receipts;
- distinguish erased proof-only values from runtime sealed handles; and
- migrate Cathedral's temporary plain `Extent` record.

Acceptance: an opaque linear `Extent` can cross a checked call and occupy
storage without exposing a constructor, public fields, or forgeable numeric
handle.

### P2 — Source-visible materialization and placed access

References:
`wiki/design_briefs/programmable_layouts.md`,
`wiki/design_briefs/os_memory_and_hardware_foundation.md`, and chapter 20.

#### L4/L5 — plan-laid views

- Finish source-visible validate/materialize establishment over owned storage.
- Complete non-scalar tiling, mutable views, and remaining aggregate
  representation-set checks.
- Keep validation as the only route from raw bytes to established typed facts.
- Do not expose a raw-offset writer.

Acceptance: an Omega-authored compact-bit policy validates, materializes, and
projects a typed value on x86-64 and AArch64; malformed tiling and fact-minting
from raw bytes reject.

#### L6b — `AccessPlan` and placed views

**OWNER-BLOCKED: `OWNER_QUESTIONS.md` #17 for the authored policy surface.**

- Expose the settled normalized access plan through Omega-authored policy.
- Derive sealed field access from `Extent loan + LayoutPlan + AccessPlan`.
- Enforce borrow polarity: shared reads, exclusive ordinary writes, and shared
  mutation only through explicit atomic/protocol-safe fields.
- Connect target external/atomic emission without a public
  `volatile_read(base, offset)` escape.

Acceptance: UART/MMIO and shared-page IPC use the same extent/layout foundation
with different access plans; an unplanned offset or illegal RMW cannot compile.

#### L6c — symbolic materialization

- Carry symbolic data/entry sources and placement constraints through final
  artifacts.
- Lower post-handoff writer plans generically into checked target code.
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
- **DOM1/DOM2/DOM3/DOM5:** authored facets, operator ownership, sealed
  introduction, and weakening certificates are owner-blocked on #16.
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

- **CRY:** finish sealed/per-mint carry facts and admission integration.
- **CML4:** finish structural multiplicity migration. Automatic cleanup and
  partial-value semantics are owner-blocked on #2/#3.
- **TR3–TR8:** finish task activation, custody, continuations, suspension-safe
  loans, and reference packages. Runtime provider publication is owner-blocked
  on #9; opaque runtime storage also depends on #8.
- Replace ambient allocation with `Arena`/`Allocation`; connect Arena backing
  to sealed `Extent` after P1.
- Implement owned `Vec<T>` and then `Vec<u8> in Utf8`; do not restore a
  text-specific primitive.

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
- Dynamic traits are owner-blocked on #1.
- Extend compiler-run Omega/build-time evaluation after owner question #5.
- Implement separate compilation, pinned component contracts, and hot-swap
  quiescence without new replacement syntax.
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
| #1 dynamic trait contract | runtime descriptors and indirect dispatch |
| #2 cleanup graph/partial values | automatic cleanup and multiplicity completion |
| #3 composite resource frontier | contained linear debt and cleanup |
| #4 quotient convergence | N6/`Real` quotient packaging |
| #5 compiler-run Omega | richer build-time policies and generators |
| #6 suspending direct-call spelling | explicit suspension call surface |
| #7 bootstrap helper staging | generic prebuilt-helper/template staging; remove IDT-specific framing during P0 |
| #8 opaque runtime boundary data | `Extent`, pointer, task-runtime, and linear provider values |
| #9 task-runtime provider publication | task admission/dispatch |
| #10 primitive float requirement family | float-format providers |
| #11 wire family/presence/evolution | remaining wire runtime |
| #12 sealed external entry reference | callbacks and dynamic entry registration |
| #13 portable atomic fence | standalone fence surface |
| #14 retained foreign pointer | asynchronous/retained FFI borrows |
| #15 boundary write frame | R5 boundary mutation clauses |
| #16 authored domain policy | facets, operators, introduction, units |
| #17 authored `AccessPlan` policy | placed views and MMIO projection |

## Vertical acceptance slices

- **Termination firewall:** cyclic components strictly decrease one joint rank;
  private witnesses never enter public contract identity.
- **Contract-axis split:** service reach, suspension, blocking, termination,
  mutation, trust, and resource ceilings admit independently.
- **Units:** after #16, implement two units in one dimension with explicit
  conversion, generic preservation, and operator coherence.
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
