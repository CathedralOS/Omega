# Design Brief: OS Memory And Hardware Foundation

Current direction as of 2026-07-18. The primitive taxonomy and security model
are settled enough to guide implementation. Exact source types, several plan
vocabularies, and backend validators remain open and are listed explicitly
below.

This brief is the common foundation for MMIO, DMA, shared-memory
IPC, descriptor tables, interrupt entry, admitted executable installation, and
early multicore boot. These are not separate language features.

## Governing invariant

An address is data, not authority. Computing or copying an `addr` grants no
right to read memory, write memory, install a mapping, execute code, or transfer
control.

Every operation that affects hardware, arbitrary memory, or external control
flow must be attributable to one of three sources:

1. checked Omega operations over storage and authority the checker can track;
2. compiler-understood low-level operations with complete instruction or plan
   contracts; or
3. an explicitly admitted provider whose unprovable commitments are represented
   by grants and receipts.

There is no raw fourth route. Wrapping an operation does not erase its service
reach, authority requirement, clobbers, trust expenditure, or installed-root
status.

## The reusable pieces

| Piece | Meaning | Primary customers |
|---|---|---|
| `Extent` capability | authority over one concrete address range with rights, provenance, address space, and lifetime | mappings, MMIO, DMA, IPC, allocators |
| `LayoutPlan` | physical geometry: offsets, alignment, overlays, bit and fragment placement, endianness | foreign records, descriptor tables, protocols |
| `AccessPlan` | permitted primitive access: read/write/atomic, width, observation, ordering contract, RMW permission, service reach | MMIO and shared storage views |
| placed view | a checked `Extent + LayoutPlan + AccessPlan` interpretation | registers, framebuffers, IPC pages |
| parsed checked assembly | target instructions whose contracts emit effects, authority, clobbers, state changes, and exits | control registers, port I/O, fences, mode changes |
| boundary entry plan | one normalized contract containing a `CallPlan` and a `StatePlan` | firmware entry, interrupts, exceptions, syscalls, callbacks |
| symbolic materialization | toolchain-resolved identities placed into structures at the last legal phase | IDT targets, image symbols, callbacks |
| executable-artifact installation | validate and place immutable admitted code under scoped authority; never convert arbitrary bytes to code | boot images, components, AP trampolines |
| external-root ledger | all installed inbound roots plus their effects, trust, stack domains, preemption relations, and version pins | interrupts, callbacks, runtime entries |
| external loan | a linear token standing in for a borrower the checker cannot observe | DMA and device ownership transfer |
| carry/runtime contracts | value demands joined with scheduler/storage behavior at admission | suspension, migration, CPU/thread affinity, address stability |

These pieces reuse existing declaration forms: `data`, `machine`, `trait`,
`domain`, `boundary`, ordinary contracts, linearity, capabilities, and plan
policies. No interrupt DSL, `volatile` qualifier, external-satisfier keyword,
instruction-wrapper keyword, or parallel admission system is introduced.

## Extents are not allocators

`Arena` is bounded allocation authority: it permits drawing storage from a
resource under capacity and lifetime rules. A borrow-backed Arena is affine; an
owned-backing wrapper derives linearity from its Extent. The returned
`Allocation<T>` borrows its Arena and carries typed establishment/ownership; it
is not itself the allocator or a fresh root authority. See
[`allocator_story.md`](allocator_story.md).

A placed view instead needs authority over an
already-existing range that was not allocated by the program, such as a UART
register block. That is an `Extent`.

The public carrier is one opaque linear declaration with no public constructor:

```omega
boundary data Extent [linear];
```

Its settled runtime direction is an inline opaque carrier citing an unpublished
ordinary layout, likely base `addr` plus `u64` length. The compiler sees that
carrier for storage and ABI but ordinary source cannot name or construct it.
Type-owned introduction operations receive compiler-scoped `pack(carrier)`;
the pack operation itself takes no authority. Root grants, consumed parents,
storage borrows, and mapping receipts are the entitlement inputs of the
enclosing introduction. Immutable owner-side projection implements published
observations such as `range()`; mutable representation projection is absent by
default.

This source carrier is live in `omega::language::core::extent`, together with
the ordinary debt-free `ExtentSlot { Empty | Live(Extent) }` bridge. Core's
stage-1 `Arena` now returns and reclaims `Extent`; it never accepts a bare
caller-fabricated address as allocation authority.

Address space, rights, provenance, and mapping era are sealed domain facts on
that carrier, not nominal carrier types or generic parameters. Physical,
virtual, I/O-port, and provider-defined spaces share the same range algebra.
An operation requiring `Physical` statically rejects an extent carrying only
`Virtual`; unproven facts gate rather than cast. Rights such as `Readable` and
`Writable` are grant-established facts: provider evidence or a conservative
derivation may establish them, but address bits and structural observation
never do.

An extent's semantic record binds at least:

- base and length;
- address-space identity (physical, virtual, I/O, or provider-defined);
- read/write/execute or more specific rights;
- minting provenance, parent grant, and authority-origin/split ancestry;
- lifetime or mapping era; and
- ownership sufficient to split, attenuate, borrow, release, or revoke it.

Only base and `u64` length are expected to require inline bits initially.
Space, rights, provenance, and era normally remain sealed facts or receipts;
lineage becomes runtime state only if static origin identity cannot survive a
required storage/component crossing.

Admitted suppliers mint root extents: boot handoff, an address-space mapper, a
parent allocator's backing store, or a device provider. Ordinary checked code
may derive children but never mint fresh authority. Bare `addr` values never do.

Splitting consumes one owned extent and returns disjoint owned children whose
ranges exactly cover it. Linearity bounds consumption; ordinary postconditions
over the published `range()` observation prove that production conserves
geometry. Attenuation may only remove rights. Merge consumes
contiguous compatible descendants of the same authority origin; numeric
adjacency alone is insufficient because adjacent ranges may have different
grants, rights, provenance, or eras. The ordinary case is rejoining what one
split separated. Combining unrelated adjacent grants, if needed, is an explicit
provider operation that establishes the combined authority.

Subrange loans are borrow-carrying values. Their polarity follows the parent
borrow: shared loans permit only shared operations; exclusive loans permit
ordinary mutation. Loans are not linear cleanup obligations. The owned extent,
DMA tokens, shootdown tokens, and similar authority/debt values remain linear.

The normalized conservation model is live in `omega-extents`. Its opaque Rust
carrier is non-clonable; an admitted one-shot root grant is the only mint;
space, provenance, era, and lineage identities are normalized; rights are an
open set of normalized identities rather than a compiler-blessed enumeration;
and split, attenuation, sibling merge, and bounded shared/exclusive loans are
validated. Failed consuming operations return every input authority. The
opaque Omega `[linear]` declaration and normalized Rust carrier are both live.
The runtime architecture is settled by
[`opaque_runtime_representation.md`](opaque_runtime_representation.md):
Extent selects inline runtime representation, while exact clause spelling,
generated introduction/projection hooks, and whether lineage needs runtime bits
remain owner-blocked. All `boundary data` is still layoutless in the compiler
today. Sealed domain facts, provider mapping, and reclamation also remain.

### Mapping and reclamation

Mapping requires authority on both sides. A requested `addr` may be a placement
hint, but it never authorizes occupation of a virtual range. Fixed placement
therefore consumes an owned extent in the virtual space. Automatic placement
draws that destination from a caller-supplied virtual-space allocator whose
own authority descends from the destination Extent.

The source relationship is independent: a mapping may consume an owned physical
extent or borrow one. A mapped virtual extent retains that relationship, so a
borrowed source cannot be reclaimed while the mapping lives. Conceptually:

```omega
machine map_owned(source: Extent, destination: Extent) -> Extent
    requires source in Extent::Physical
    requires destination in Extent::Virtual
    ensures result in Extent::Virtual
    ensures result in Extent::Mapped;

machine map_borrowed(source: &Extent, destination: Extent) -> Extent
    requires source in Extent::Physical
    requires destination in Extent::Virtual
    ensures result in Extent::Virtual
    ensures result in Extent::Mapped;
```

The exact overload spelling is engineering; the ownership distinction is law.
Unmapping consumes the mapped extent, returns the reusable destination range,
and either returns an owned source or ends its source loan. On targets requiring
cross-core invalidation, reuse remains gated by a linear shootdown/quiescence
token. Its completion operation carries the provider's ordinary suspension or
blocking ceiling, so an interrupt root cannot hide an illegal wait.

V1 has no per-access generation probe. Reclamation requires exclusive ownership
back and therefore no live in-language views. Forced asynchronous revocation of
unreclaimed loans is deferred to provider quiescence/lifecycle machinery when a
customer requires it; translation edits, shootdowns, and process teardown remain
ordinary runtime provider work.

The provider-neutral mapping lifecycle is live in `omega-extents`. An admitted
mapping grant pins source custody, source/destination spaces and required
rights, provider-established mapped facts, and open sets of translation
activation and release facts. Fixed mapping consumes the destination Extent and
independently owns, shared-borrows, or exclusive-borrows its source. Structural
validation yields only a non-clonable pending mapping; it exposes no mapped
loans. An exact provider receipt must bind the mapping and grant, establish that
translations were installed, and discharge every activation fact before
`MappedExtent` exists. Shared source custody then cannot expose mutable mapped
loans. `begin_unmap` retains every authority until another exact provider
receipt establishes that stale translations are released and all target
completion facts hold; only then are the destination and any owned source
returned. Provider translation operations, suspension/blocking ceilings, sealed
source-domain facts, and automatic destination allocation remain.

Zero-filled storage does not establish a linear extent and creates no
must-consume obligation. A zero-usable pool uses the ordinary debt-free sum:

```omega
data ExtentSlot {
    case Empty;
    case Live(extent: Extent);
}
```

This preserves the useful ZII distinction: zero bytes remain physically
representable, while establishment decides whether those bytes are accessible
as an authority-bearing value. Bulk structures should retain meaningful zero
states where honest; authority and foreign validity must never be minted by
zero-fill.

## Layout and access are separate plans

`LayoutPlan` answers where bits live. `AccessPlan` answers how a placed range
may be touched. Combining them would force wire formats to carry MMIO concepts
and hardware plans to carry codec concepts.

The layout vocabulary needs name-keyed entries and fragmented placements so one
logical source can occupy several destination ranges. Validation must check both
destination overlap and exact source tiling: every source bit appears exactly
once unless an explicit overlay rule says otherwise.

The access vocabulary stays small and compiler-owned:

- read, write, and atomic transfer classes;
- exact transfer width;
- stable, volatile/external, or atomic observation;
- generic read-modify-write permitted or forbidden;
- provider-private versus exported access; and
- the statically pinned boundary-trait reach of the accessor.

The raw primitive is not a public `volatile_read(extent, arbitrary_offset)`
escape hatch. A provider validates extent provenance, `LayoutPlan`, and
`AccessPlan`, then derives an opaque placed view. Its public primitive surface
is projection plus only the operations authorized for that field:

- `read()` performs one exact-width access and returns a snapshot;
- `write(value)` performs one whole-container write; and
- explicitly atomic fields expose the ordinary checked atomic API (`load`,
  `store`, `compare_exchange`, and authorized atomic RMW operations).

Pure projection narrows authority without performing I/O. Projected field
accessors are ordinary passable borrow-carrying values and cannot outlive the
mapped extent. Whole-view methods may coexist as ergonomic forwarding machines;
projection is the least-authority surface for helpers that need only one field.

Borrow polarity remains authoritative: a shared projection permits reads;
ordinary writes require an exclusive borrow; mutation through a shared borrow
is legal only through an explicitly atomic or protocol-safe field. Placed views
must not smuggle aliased unsynchronized mutation around the ownership checker.

For example:

```omega
let status_register = uart.status;
let status = status_register.read();
```

One container-width read produces one snapshot. Bit projections from that
snapshot are pure. Flow facts attach to the snapshot value, never to storage
that hardware or a peer may change.

Generic RMW is never derived for an ordinary MMIO field. Device behaviors such
as W1C, read-to-clear, FIFO pop, and doorbells remain package machines over
provider-private primitive access. This does not forbid atomic RMW on an
explicitly atomic shared-memory field; those are different access classes.

The normalized `AccessPlan` foundation is now implemented independently of its
future source spelling. It validates name-keyed entries against fixed
`LayoutPlan` geometry, exact whole-byte transfer width, observation/operation
compatibility, provider-private external RMW, and statically pinned reach. Its
normalizer erases authored entry order, sorts by field identity, and assigns a
deterministic plan identity over every permission, observation, width,
exposure, reach, and exact normalized layout-geometry fact. Thus source
reorderings that preserve the name-keyed policy and geometry preserve identity,
while a shifted field or any other lowering-relevant policy change does not.
Its
operation gate already enforces shared-read/exclusive-write polarity while
allowing explicitly atomic shared mutation. Successful geometry validation
mints a sealed field descriptor containing the plan-derived container offset;
successful operation authorization combines that descriptor with borrow
polarity and the selected operation. Primitive lowering accepts only this
authorized value, never an author-supplied offset. The normalized lowering seam
now consumes that value and binds the resulting event to the access-plan
identity, provider-admitted grant, field identity, exact address/width,
observation model, loan-derived borrow polarity/lifetime, operation-specific
atomic ordering, and static reach. Invalid load, store, or compare-exchange
ordering rejects before target lowering. Source-policy evaluation, source-level
borrow-carrying access values, and target-specific external/atomic emission
remain.

The normalized Extent/AccessPlan join is also live. A reusable
provider-admitted placed-view grant retains the exact canonical geometry-bound
access plan plus address space, provenance, required open-set rights, and
permitted static reaches. The compact plan fingerprint remains useful for
reports and caches but is not treated as collision-resistant authority.
Derivation compares the exact plan and checks the remaining facts and layout
size against an actual borrow-carrying Extent loan. Field operation
authorization then derives shared/exclusive polarity from that loan rather
than accepting a caller claim, and its sealed lowering value remains borrowed
from the view. Omega source projection and target-specific primitive emission
remain.

The extent's provenance gates construction of an access capability. The
accessor's normalized contract statically pins service reach. Runtime
provenance never changes a machine's effect row.

## IPC and DMA

Omega's responsibility stops at general range authority, mapping
conservation, programmable layouts/materializers, provider admission, and
checked instruction contracts. Address-translation tables, their hierarchy,
entry policies, construction lifecycle, scanners, and activation protocol are
OS implementation details. Cathedral or another OS package may build them from
these primitives; they are not language concepts or compiler-owned target
subsystems.
Shared-memory IPC and MMIO share external mutability, not an observation model.
For proved or mutually trusted peers, an atomic protocol may return a linear
lease whose borrow exposes stable payload bytes until explicit `release`.
Against a hostile writable peer, protocol cooperation proves nothing: the
receiver must copy then validate, or a provider must revoke/remap the peer's
write permission and complete required cross-core invalidation before zero-copy
validation.

DMA is an external borrow. A linear transfer token is the checker's proxy for
the invisible device:

- device-read is a shared loan: CPU mutation is excluded;
- device-write is an exclusive loan: CPU reads and writes are excluded;
- bidirectional sharing requires an explicit atomic/coherence protocol; and
- completion consumes the token, performs the required cache/fence contract,
  and returns the loan.

The token may remain live across suspension; waiting for completion is normal.
The provider receipt is the accepted claim that completion really means the
device has stopped using the range.

This conservation model is live in `omega-extents`. A reusable admitted grant
pins the borrower, direction, space, provenance, required open-set rights, and
an open set of completion facts (including target fence/cache facts where
needed). Starting a transfer accepts an actual Extent loan and
derives CPU exclusion from its polarity. It also requires a per-transfer reach
receipt proving either an admitted borrower contract or hardware isolation
confines that exact loan ID and borrower/direction to the lent range in the
same address space, provenance, mapping era, authority lineage, and attenuated
rights. That evidence is derived from the actual loan plus its admitted grant
rather than restating those facts. Missing, stale, or overbroad reach fails
before transfer. The non-clonable proxy holds that borrow until a
matching provider receipt establishes completion and every required
ordering/coherence fact. Completion evidence is derived from the exact live
proxy rather than restating its authority: it binds the confinement receipt,
direction, address space, provenance, mapping era, authority lineage,
attenuated rights, and lent range. Reusing a loan identity after any of those
facts drift therefore cannot replay an old completion. Failed starts and
completions return their borrow-carrying inputs.
Omega `[linear]` integration, permission-context events, provider execution,
and the DMA vertical slice remain.

## Checked assembly is the low-level operation surface

`asm { ... }` is parsed target assembly, not an opaque byte blob. Every accepted
instruction has a compiler-owned contract covering applicable machine regime,
required authority and facts, service reach, register/flag/memory changes,
ordering, and control exits.

Direct checked assembly and a higher-level boundary operation must contribute
the same normalized reach. For example, `asm { wrmsr }` cannot be the quiet way
around a `MachineControl` ceiling. Unknown instructions and raw byte emission
are rejected. Prebuilt foreign code enters through provider admission instead.

The catalog distinguishes user-available checked instructions from deriver-only
entry/exit operations such as `iretq` or `sysret`; user code cannot manufacture
an unmodeled control exit. The x86 `lidt` operation is likewise contracted and
provider-only: its contract requires consumer-supplied CPU/table publication
authority and records the exact descriptor read, scratch clobber, control-state
change, and service reach. The compiler owns that target instruction contract;
it does not own the consumer's IDT lifecycle type. Regime-changing instructions
state their transition directly: require regime R, establish regime R'.

The former `Binding::Instruction` duplication is retired; parsed checked
assembly is the only source-level instruction surface.

## Boundary entry plans

An ordinary boundary call often makes ABI placement and machine-state policy
look identical. Interrupts prove they are independent.

```text
CallPlan
  parameter and result placement
  stack alignment and ordinary ABI clobbers
  entry/return control shape

StatePlan
  initial machine regime
  interrupted state set
  state saved and restored by the stub
  permitted transitive machine-state use
```

A normalized boundary-entry plan carries both. The requirement cites the
calling policy through ordinary trait composition (`Calling<C>`). `C` satisfies
`CallingPolicy`; its build-time-admissible `plan` machine evaluates the
normalized signature to `Accepted(BoundaryEntryPlan)` or a structured rejection.
Accepted plans are compiler-validated and canonicalized. The canonical evaluated
plans, not the policy symbol, machine source, or construction order, enter the
published contract identity. Policy authorship is open to ordinary packages,
while the plan vocabulary and validator remain compiler-owned and closed.

Implementation evidence is firewalled from that identity. A provider artifact
contains its emitted transitive state/register footprint and validation result.
Changing register allocation or choosing a different legal implementation does
not change caller contract identity.

State ceilings constrain instruction selection and register allocation before
code is emitted. Codegen may clone a machine under a different state ceiling;
this is contextual specialization, not generic type monomorphization, although
the backend may share cloning and cache infrastructure. Ordinary call-return
boundaries derive their transitive ceiling from ABI
volatile register banks plus caller-volatile condition flags. Flag-producing
comparisons and dispatch branches are therefore representable without granting
that state to an interrupt boundary which did not save it.

The final realized artifact is validated after inlining, relaxation,
veneers/thunks, generated stubs, and admitted indirect leaves:

```text
actual_transitive_footprint subset_of StatePlan.permitted_state
actual_clobbers intersect unsaved_interrupted_state = empty
```

Checked Omega code produces checkable footprint evidence. Admitted leaves
supply accepted footprint claims under receipt. The validator, not backend
optimism, decides acceptance.

The first backend consumer now derives inbound runtime-frame storage writes
from the complete validated boundary plan. Each target encoder publishes the
registers its generated copy fragment overwrites; derivation unions those
clobbers into implementation-only `StateFootprintEvidence`, rejects a selected
input register destroyed before capture, and validates the fragment against the
plan's state ceiling. This evidence covers only inbound storage realization.
The normalized calling-plan foundation also composes any number of fragment
footprints by deterministic register/state union and validates the aggregate
against one retained boundary plan. Ordering and duplicate fragments cannot
perturb that implementation-only evidence fingerprint. Validated inbound
fragments now remain attached to the abstract-operation plan with
`entry_storage` provenance, and `08_boundary_footprints.json` publishes the
fragments plus their composed fingerprint. The special bytes-handoff entry also
retains a distinct `entry_slice_descriptor` fragment whose fixed x86-64 or
AArch64 scratch set is declared beside the encoder that emits it, so both the
raw-register spill and constructed slice view are covered. Direct result
materialization similarly retains `exit_result_registers` evidence for both
immediate writes and runtime-storage loads, including target result registers,
relocated frame-base scratch, and AArch64 large-offset scratch. Indirect results
add a structurally scoped `exit_indirect_result_copy` fragment for the copy
through the captured hidden pointer; generic body copies are deliberately not
classified as boundary evidence. Its explicit
`boundary_contract_fingerprint` binds every retained fragment to the canonical
validated plan under which it was checked. Fragment retention revalidates the
evidence and rejects cross-contract composition; this is a reference to
requirement identity, not implementation evidence entering that identity. The
fixed ordinary `CallReturn` path also retains `call_return_mechanics` evidence.
x86-64 records its fixed saved-register frame, RSP, and control restoration;
AArch64 records its fixed frame prologue and x19-x30, SP, and control
restoration. Provenance-aware
validation admits these prescribed boundary mechanics without widening the
handler body's transitive-state ceiling. x86-64 now likewise uses a fixed
64-byte frame to preserve the generated-code nonvolatile GPR union for SysV
AMD64 and Microsoft x64; inbound stack offsets include the saved frame, and
the retained mechanics evidence names every restored register. Runtime-dispatching entries add a
structurally scoped `dispatch_scaffold` fragment whose target-owned facts cover
x86-64 R12/AArch64 X28 dispatch-state writes and case-entry condition flags;
storage-backed static guards add `static_guard_comparison` evidence with their
exact GPR/vector scratch and flag effects. Storage-free and other guard-lowering
shapes remain outside that structurally limited fragment. Dedicated runtime-text
literal-buffer and descriptor-vs-literal guards add
`runtime_text_guard_comparison` evidence for their encoder-owned base, pointer,
length, loop, byte-scratch, large-offset, and flag effects; cross-target artifact
canaries exercise the literal-buffer path. Place-pair and place-vs-immediate
guards likewise retain `place_guard_comparison` evidence: x86-64 covers the
complete place walk and compare scratch, while AArch64 covers its admitted
direct-place shapes and offset-dependent address scratch. Cross-target artifact
canaries exercise the place-pair path. Recursive `CompareRuntimeValues` guards
add `runtime_value_guard_comparison` evidence from each ISA's closed evaluator
may-write ceiling. x86 reports balanced push/pop SP use only for operand trees
that contain nested binary evaluation, and that stack scratch is scoped to an
ordinary call-return activation. Cross-target artifacts exercise text-equality
value operands. The footprint plan lives in the canonical semantic boundary
summary and is retained unchanged through target selection, assignment, machine
instruction lowering, and machine-byte emission. The machine-readable artifact
is generated from that encoded-machine carrier and names
`evidence_stage: encoded_machine`; it no longer reads the earlier abstract-plan
root. The artifact's explicit
`enumeration_complete: false` status is a firewall: this retained slice is
checkable implementation evidence, not yet the final certificate.
Final-image construction now supplies the complementary placement inventory.
It classifies object function spans and every PE/Mach-O import thunk appended by
the format writer, resolves each region to its final image address, rejects
overlap or out-of-bounds records, and publishes any unclassified executable
gaps in `13_executable_regions.json`. Whole-text and per-region/gap fingerprints
bind those records to the exact relocated bytes. The artifact repeats the exact
boundary-contract and composed implementation-evidence identities from the
encoded carrier and hashes them with the final inventory identity, preventing a
placement record from being substituted across contracts or implementations.
When a boundary contract is retained, the composed encoded-machine evidence is
also attached to exactly one placed compiler entry-function region. Final
inventory emission rejects a missing or duplicate entry-symbol match, so the
handler evidence cannot float beside an unrelated compiler-function span. The
typed placed inventory is recomputed after attachment, making that association
part of its fingerprint rather than a presentation-only JSON annotation.
Direct-image emission also validates the fixed encoder-owned function-entry
prologue and return epilogue against the exact relocated entry-region bytes on
x86-64 and AArch64 before publication. The inventory names this narrow
call-return class separately from compiler-function bodies that still require
final-byte footprint decoding.
The final image's compiler-authored `.text` prefix is also compared bit-for-bit
with encoded-machine bytes under the checked relocation plan. Only declared
x86 displacement/address fields or the exact AArch64 immediate bitfields may
change; opcode and register bits must remain identical. Bad widths, overlap,
out-of-range records, and mutations outside that envelope reject before the
output leaves checked image emission. Format-owned thunk tails retain their
separate exact validators.
The compiler publishes the encoded-prefix, final-prefix, and canonical
relocation-envelope fingerprints plus their composed derivation identity. The
boundary/placement binding includes that derivation identity, so a valid final
inventory cannot be paired with evidence from a different encoded-to-final
derivation.
Checked image emission rejects any unclassified final executable gap. The
current closed emitter therefore publishes `region_enumeration_complete: true`:
compiler functions and format-owned import thunks cover every `.text` byte,
while relaxation products, veneers, and general generated stubs are absent by
construction. This is deliberately separate from
`footprint_enumeration_complete: false`; compiler-body decoding and admitted
leaf evidence remain unfinished.
The format writers also validate their own exact final import-thunk encodings
after patching and relocation. PE `jmp [rip+disp32]` carries an
instruction-pointer-only footprint; Mach-O `ADRP/LDR/BR X16` carries X16 plus
instruction-pointer effects. Opcode mutations reject before placement, and the
attached per-region evidence participates in the inventory fingerprint.
The inventory explicitly lists relaxation products, veneers, generated stubs,
and admitted leaves as missing classes, so this new post-layout seam cannot
accidentally promote the partial evidence to a complete certificate.
The final certificate must still decode and validate compiler-function bytes,
then aggregate StatePlan-driven nonordinary
save/restore and return sequences, decoded compiler-function handler regions,
relaxation products, veneers, generated stubs, and admitted indirect leaves
after final placement.

## Symbolic materialization and admitted executable installation

Runtime-known addresses are ordinary `addr` data, used only with separate
authority at mapping, installation, or access. Toolchain-known identities do
not become user-visible code addresses. Plans refer to a closed symbolic source
vocabulary such as:

```text
RelocationTarget = Data(DataSymbolId) | Entry(EntryStubId)
```

The materializer resolves a symbolic source at the last legal phase: fixed
image layout, native relocation, loader relocation, or a generated runtime
writer. Split IDT offsets require the fragment plan above; they must not be
reconstructed by user arithmetic.

Each target also declares its consumption phase. A field the loader consumes
before the first Omega instruction must fit the object format's native
relocation vocabulary. Post-handoff structures may use the generated writer.
The normalized writer program is now derived from the same actions: it
validates the concrete placement, resolves each sealed target once through the
provider, derives every fragment, writes only the unpublished destination, and
publishes only after the complete result validates. Its sealed preparation and
address-free generated target/machine carrier are live. The packed private
context ABI, exact footprint, x86 emission, and opaque once-resolved population
gate are also live. A validated one-private-pointer invocation plan now selects
the exact GPR copied into R10; concrete provider insertion/execution remains
engineering work.
Placement plans may constrain range, alignment, phase, machine regime, and
scoped artifact-installation authority. The normalized materialization
foundation now carries those five facts: policy alignment is joined with the
layout's alignment, compiler-issued identities cite regime and installation
scope, and a concrete-site validator checks the complete occupied range before
linker/loader/provider consumption. Propagation through the final artifact
pipeline remains engineering work.

### Generated hardware-table materialization

Omega supplies a general way to derive a checked writer from a normalized
layout and sealed symbolic sources. The generated machine receives one exact
mapped/pinned/writable unpublished placement plus a resolver restricted to the
admitted artifact. It writes directly into the unpublished destination.
Failure establishes no consumer value, so partial bytes cannot become
hardware-visible merely because they occupy memory. This is atomic
*publication*, not transactional restoration of the destination.

Structural layout validity is not hardware-table admissibility. The consuming
package owns the semantic validator and the established value that results.
For an IDT, Cathedral checks admitted roots, selectors, gate kinds, privilege
levels, IST assignments, reserved bits, and canonical base/limit. Another
consumer supplies different rules without extending Omega's compiler.

The first post-firmware writer also carries a software-fault-free bootstrap
certificate. That certificate is a conjunction of existing obligations:
mapped/pinned/writable destination and stack facts, WCSU provisioning,
validated offsets and fragment tiling, admitted CPU-profile support, bounded
work, and no suspension, blocking, allocation, dynamic dispatch, or unsupported
instruction path. It excludes deterministic software faults under those
facts; NMI, machine check, and physical failure remain explicit boot-envelope
assumptions rather than falsely proved guarantees.

Materialization and installation remain separate authorities and produce
separate receipts. The materializer reaches only destination writing and the
sealed resolver; it cannot publish a hardware table. The consumer's installer
cannot manufacture bytes; it accepts only the consumer-established table value
and separate publication authority. It records external roots before invoking
the relevant checked instruction, so there is no reachable-but-unreported
root.

Hardware-table materialization and executable placement reuse establishment,
content binding, and linear consumption as shared infrastructure. They remain
different consumer algebras and do not justify one compiler-owned generic
typestate ladder.

There is no general `ExecutableMemory` capability, arbitrary byte-to-code
conversion, JIT facility, or self-modifying-code path. Executable eligibility
is the sealed `Artifact::AdmittedExecutable` domain fact over a reusable
immutable `Artifact` carrier. Admission binds it to normalized content,
identity, relocation plan, footprint, and placement plan; packages cannot
self-establish it. Mutation destroys that eligibility. Runtime loading borrows
the admitted artifact and consumes linear placement authority; it never
consumes the reusable artifact itself.

The normalized lifecycle is:

```text
ArtifactCandidate
    -- canonical decode + PCC/contract admission -->
Artifact in Artifact::AdmittedExecutable                 reusable

CodePlacement                                            linear, W + NX
    + borrow admitted artifact
    -- materialize declared sections/relocations -->
FrozenPlacement                                          linear, R + NX
    -- validate exact final bytes/footprint -->
ValidatedPlacement                                       linear, R + NX
    -- installer provider + visibility completion -->
InstalledCode                                            linear, R + X
```

These are normalized semantic states, not a promise of literal generic source
types. Content/placement binding prevents transplanting a validation
certificate to different bytes; linear placement ownership prevents spending
one destination twice. The certificate may remain reusable/reportable while
`ValidatedPlacement` is consumed.

This normalized ladder is live in `omega-executable-installation`. Canonically
decoded artifacts are immutable and reusable; exact admission evidence checks
the complete immutable artifact rather than relying on compact FNV identities
as collision-resistant authority before establishing executable qualification.
Admission from a validated container also binds the proof-payload identity and
the exact proof bytes independently of content identity. The normalizer derives
the former from the latter rather than accepting a restated pair, so verifier acceptance
cannot be replayed across byte, proof, or semantic-container drift even if a
compact proof identity collides;
informational sections remain excluded from the gate. This is the substitution-
safe verifier seam, not an implementation of the PCC verifier itself. A
one-shot authority claims an Extent-backed
placement. The reusable artifact retains its exact bytes and canonical
relocations through admission. A provider-side pure materializer resolves only
sealed entry/data identities, applies checked target relocations to a private
copy, validates AArch64 instruction shapes, and derives a content- and
placement-bound final-byte identity; this inert result grants neither writes
nor execution. The write/freeze transition consumes that exact output and
its receipt retains the complete canonical output rather than restating compact
FNV identities. The gate matches artifact, admission, placement, base, plan,
exact bytes, byte length, and final identity. `FrozenPlacement`
retains the immutable final-byte snapshot, so final footprint/PCC validation
examines exactly the bytes whose write authority was frozen. This is a
provider-side inspection surface, not a source-visible byte-to-code operation.
A separate provider writes those bytes and freezes authority.
The final certificate can be constructed only from the exact frozen carrier it
claims to validate and retains the complete artifact and byte snapshot plus
placement and realized footprint. Compact artifact/final-byte IDs remain report
keys, never collision-resistant authorization. Installation consumes an
authority scoped to the complete validated placement: canonical artifact,
frozen bytes, realized footprint, validation result, placement geometry,
Extent space/rights/provenance/era/lineage, constraints, scope, and audience.
The provider's installation receipt retains the same exact evidence; compact
artifact, placement, and validation IDs remain report keys rather than
authorization surrogates.
Synchronous visibility and
`HardwareEnforced | ConventionOnly | Unsupported` W^X reporting are checked.
Failed linear transitions return their inputs. Schema byte decode, actual PCC
and final-code validators, destination write/freeze and installation-provider
execution, Omega linear integration, and live replacement remain.

`CodePlacement` now consumes the existing placement-plan vocabulary rather
than duplicating it. The one-shot authority carries normalized range,
alignment, phase, machine-regime, and installation-scope constraints plus the
provider's concrete site. It is minted from and retains the destination
Extent's exact range, address space, rights, provenance, mapping era, and
authority lineage. Claiming the Extent checks that complete authority evidence,
then runs the shared `PlacementConstraints` validator before materialization.
A caller cannot substitute either a friendlier placement hint or a
same-address Extent from another authority lineage.

The normalized retirement path is live as well. It consumes one exact
`InstalledCode`; both retirement authority and provider receipt retain the
complete installed realization, including its validated artifact/bytes,
placement authority, validation result, and W^X fact. Visibility evidence
cannot satisfy it. The provider receipt must separately establish executor
quiescence, removal of execute permission, restoration of write authority, and
every open target completion fact. Only then does the placement return to W+NX
for a later admitted artifact. The runtime quiescence/provider implementation
and replaceable requirement binding remain separate work.

This invariant covers every route to execute permission. Translation providers
require admitted-artifact provenance before deriving an executable mapping,
and checked assembly emits the same installation authority
and reach obligations rather than exposing a back door. Device firmware and
GPU/NIC programs are device-provider uploads, not host executable artifacts.

Installation performs final post-materialization validation, target-specific
W^X transition, cache maintenance, ordering, and instruction-fetch
synchronization through one contracted provider operation. Its authority is
scoped to the admitted artifact identity, `CodePlacement`, and audience.
`CodePlacement` composes existing physical/virtual `Extent` authority and
placement constraints; it is not a replaceable dispatch binding or a new
parallel authority family. Requirement binding happens later and separately. AP
bringup installs a compiler-produced low-memory trampoline and
then invokes a target boot protocol; it does not generate host code at runtime.

Audience state has three cases:

1. a dormant/local target needs local installation completion;
2. a future remote fetcher needs a visibility-completion fact before entry; and
3. a possible current executor requires component replacement and quiescence.

Visibility gates future entry; quiescence gates retirement of existing code.
They may share linear-obligation infrastructure but establish different facts
and are not one token algebra. Template patching of already-live code uses
admitted fragments at declared patch sites through the replacement path. The
v1 loader completes visibility synchronously (it may itself suspend); a
non-blocking visibility token waits for a real provider customer. Successful
installation reports `HardwareEnforced` or `ConventionOnly` W^X; an
`Unsupported` provider rejects installation.

Installation prevents code injection. It does not by itself establish
legal forward-edge indirect targets over already-admitted code. Backward-edge
return integrity in checked Omega derives from memory safety, sufficient WCSU
provisioning, and compiler-owned live or parked control state that ordinary
code cannot address. Forward-edge indirect calls instead require sealed
requirement-compatible entry references or descriptors retaining
satisfier/contract identity; the runtime descriptor contract remains in
`OWNER_QUESTIONS.md`.

Checked assembly cannot omit catalog-derived stack/control effects. Opaque
providers must supply admitted `CallPlan + StatePlan` exits or remain
hardware-isolated; missing evidence rejects. DMA can touch only explicitly lent
extents and cannot reach control storage by numeric coincidence. Independent
final-byte transfer validation and CET, PAC, or shadow-stack hardening remain
deferred PCC/TCB assurance, not timer or language-semantics blockers.

The component container is a minimal canonical Omega-native artifact, decoded
through checked schema/layout machinery: bounded length-delimited tables,
checked arithmetic, content identity, a closed relocation vocabulary, and
explicit PCC/contract/footprint sections. It has no constructors, scripts,
ambient imports, recursive metadata, or permissive semantic extensions. An
ignorable section is informational only and contributes no admission authority;
anything affecting meaning or trust is required. UEFI may require a thin
PE/COFF boot envelope, but that envelope is not Omega's component format.

The normalized post-decode validator is live in
`omega-executable-installation`. It applies configured total/section/count
bounds, checked range arithmetic, non-overlap, exact presence and identity of
code/relocation/contract/footprint/placement/proof sections, and the required
versus informational unknown-section rule. Its output is only an immutable
`Artifact` candidate; executable qualification still requires the separate
admission receipt. Decoded relocation records now cross a second closed
validator: only the current absolute-64, x86 relative-32, and AArch64
page/page-offset/branch meanings enter the canonical set; configured count,
exact destination width, code bounds, overlap, and arithmetic overflow are
checked while targets remain sealed entry/data identities. Actual byte
decoding through LayoutPlan/schema machinery remains. The decoded carrier and
resulting immutable artifact retain the exact code bytes, so later
materialization needs no unmodeled byte side channel. Validation derives the
normalizer-owned content identity over those bytes plus contract, footprint,
placement, canonical entry, and canonical relocation promises. Section/entry
presentation order, proof evidence, and informational sections do not perturb
that identity. A backend
adapter translates only the validated canonical carrier into
the existing object-relocation plan, resolves each sealed target through
compiler/provider infrastructure, and fails atomically on target-architecture
mismatch, missing symbols, or offset overflow. Signed semantic addends survive
the object carrier, direct-image application, reports, and identity
fingerprints.

The boot base case preserves the same discipline:

```text
trusted build validates the artifact and signs an admitted artifact identity
    -> secure boot authenticates and gates entry
    -> measured boot records the entered identity
    -> the boot-admitted installer loads later admitted artifacts
```

Secure boot gates; measured boot records. Measurement alone never establishes
admission.

## External roots and interrupt tables

`boundary machine` already describes an inbound callable. Interrupts add no new
machine species. Installation makes the selected provider's entry stub an
external root because hardware reaches it without an Omega caller.

The root ledger is a normalized artifact, not user-authored prose. Each entry
records package-qualified requirement/provider identities, evaluated boundary
plan, artifact and receipt identities, authority and scope actually granted,
effects, stack domain, preemption/nesting relationships, and version/liveness
pins. It also records the public ceilings, realized demands or footprints, and
validation receipts for the root's stack, structural work, and machine-state
resource columns. Friendly names are presentation only.

The ledger closes three whole-program holes:

- effects and trust reachable only from hardware callbacks remain visible;
- WCSU composes across interrupt nesting and same-stack roots; and
- hard-root structural work and final machine-state use refine their admitted
  ceilings instead of disappearing behind the absence of an Omega caller; and
- dynamic install, replacement, and removal are checked against version pins
  and quiescence.

The general normalized root foundation is live in `omega-external-roots`. A validated root
binds one compiler-issued entry identity to the complete evaluated
`BoundaryEntryPlan`, an open effect/receipt set, provider identity, stack and
nesting policies, optional acknowledgement policy, WCSU size/alignment, and
component-era pins. Installation consumes owner-scoped entry authority and
an admission that retains the complete validated root and selected provider
execution alongside its report identities, installed code, artifact, entry,
owner, and receipts. The admission also retains the opaque complete
installed-code lifecycle context. Compact FNV identities therefore cannot
replay acceptance across acknowledgement, component-pin, trust-receipt, entry,
final-byte realization, or resource drift. Installation also proves that the
selected entry belongs to that admitted artifact; no numeric entry address
enters the ledger.

The installed-root handle borrows the linear installed-code claim. Code
retirement therefore cannot recover ownership while hardware may still enter
it. Removal is the opposite-facing gate: the provider receipt derives from the
live borrowed root handle, binds that exact installed-code context, and must
establish both that the slot no longer makes the entry reachable and that old
executions are quiescent before the slot authority is returned. Failure returns
all consumed values. The live ledger separately retains the complete installed
root evidence. Interrupt-entry receipts derive from the live borrowed root
handle and bind that exact root, provider execution, and installed-code
context, so colliding report IDs cannot mint an invocation or its linear
obligations. Provider receipts for mask save/restore and acknowledgement derive
from those live opaque carriers and retain the same complete invocation
evidence; compact control, guard, acknowledgement, and invocation IDs cannot
settle debt from another root. The live ledger also owns a deterministic report
fingerprint that binds each normalized root contract to its exact installed
code, artifact, entry, owner, and admission. `omega-artifacts` writes this live
state as `external_roots.json`: the complete evaluated `CallPlan + StatePlan`,
provider/effect/trust identities, the three resource columns, nesting and
acknowledgement policy, and component pins are machine-readable, while friendly
names, numeric entry addresses, and private proof internals are absent by
construction. Stack admission now accepts only a sealed result of artifact-wide
composition, not a caller-authored composed number. Provider-local demands are
joined under one exact nesting relation: an `Interrupted` entry adds to the
active domain with alignment, a distinct dedicated class switches domains, and
sequential roots sharing a class provision their maximum. Missing endpoints,
cycles, unknown nested provider-selected stacks, overflow, and re-entry of an
already active dedicated class fail closed. Every installed root in a ledger
must bind the same exact canonical nesting relation and provider-summary set.
The sealed stack and work realizations retain those complete inputs; compact
composition fingerprints are report keys, not admission evidence.

The current Rust implementation also contains IDT-named writer, table, load,
grant, and receipt states. They are implementation debt, not part of this
language architecture. Cathedral owns its IDT schema, writer lifecycle, and
installation protocol. The compiler keeps only generic plan validation,
symbolic/fragment materialization, external-root analysis, provider admission,
and checked instruction contracts. `TASKS.md` P0 tracks removing the
customer-shaped specialization.

Machine-state admission checks the final footprint against the `StatePlan`;
canonical fixed-work provider summaries compose transitively while rejecting
missing callees, cycles, zero invocation bounds, overflow, and excess demand. The report is
deliberately not a numbered compiler phase because roots may be installed after
image build. A sealed provider-execution binding now joins the normalized
selected provider-plan identity, exact entry and boundary, effects, and the
three independent resource realizations at root admission. It is identity-bound
into the ledger/report and cannot be replayed after entry or realization drift.
IDT construction is an acceptance customer for these generic mechanisms, not
another Omega subsystem. Cathedral composes an exclusively held mapped
placement, fragmented layout/materialization plan, sealed admitted-artifact
resolver, established table value, root records, checked `lidt`, and its own
installation receipts. Omega must not define the IDT writer/table/load
typestate, private context ABI, vector policy, or PIC/LAPIC realization.

The general materializer validates and resolves every symbolic write before
directly writing the unpublished destination. It exposes no numeric entry
address or arbitrary-offset writer. Generic checked target lowering retains
the complete plan, placement, source identities, final-content validation, and
derived machine-state footprint; compact fingerprints remain report/cache
identities rather than authority.

### Installed-root resource contract

Every installed root carries three independent
policy-or-provision/realization/evidence triples:

| column | policy or installed provision | realized artifact fact | private evidence |
| --- | --- | --- | --- |
| stack | selected stack domain and provision; optional fixed policy ceiling | WCSU bytes/alignment plus composed nesting demand | frame/place liveness and WCSU derivation |
| structural work | installed execution budget; optional fixed policy ceiling | composed fixed-work demand | acyclic CFG, ranking bounds, callee summaries, and codegen certificate |
| machine state | `StatePlan` permitted state and save/restore commitment | emitted transitive footprint and clobbers | instruction selection, allocation, and footprint derivation |

The ledger and its report retain each applicable policy ceiling, installed
provision, realized fact, and validation receipt. They never retain private
ranking witnesses or codegen proof internals.
Sharing this record shape does not fuse the three algebras or their identity
rules: the evaluated `StatePlan` is published boundary identity, while stack and
work figures normally belong to candidate admission and current provisioning.
A fixed stack/work ceiling enters requirement identity only when policy
deliberately promises replacement without reprovisioning. Otherwise a changed
realized demand changes the candidate artifact/report and requires fresh
provisioning; it does not change the semantic requirement.

Structural work is not WCET. V1 proves that a hard root has no
workload-dependent unbounded path under its admitted provider summaries. Exact
cycles, deadlines, cache behavior, and MMIO latency require target/provider
timing models and remain in the quantitative resource/WCET work. The first
timer uses the trivial evidence tier: acyclic final control flow, no dynamic or
recursive path, and fixed-work acknowledgement, clock-capture, wake, and return
leaves. Provider work summaries compose transitively just as reach summaries do;
an acyclic caller cannot launder an unbounded leaf.

The IDT is consequently a first serious customer, not a special construct:

1. ordinary `data` describes the logical gate;
2. an x86 layout policy supplies bit and fragment placements;
3. a target-specific interrupt requirement pins `CallPlan + StatePlan`, stack
   class, acknowledgement protocol, and service/suspension/blocking ceilings;
4. build/provider selection chooses a satisfying handler;
5. the materializer resolves its sealed entry-stub identity into gate bits;
6. Cathedral's checked writer validates the unpublished table and establishes
   a content-bound materialized-table fact;
7. Cathedral's separate installer presents that fact and its CPU/table
   publication authority, records roots before hardware reachability, and
   executes checked `lidt`; and
8. a linear acknowledgement token forces exactly-once completion.

The source obligation contract is live in
`omega::language::core::interrupt`. `InterruptMaskControl::save_and_mask`
returns an opaque linear `InterruptMaskGuard`; only consuming `restore` may
settle it. An independently opaque linear `InterruptAcknowledgement` is
settled only by consuming `complete`. The two tokens deliberately remain
different: restoring the prior CPU interrupt mask reaches `machine_control`,
while acknowledging the interrupt source reaches `device_io`. Ordinary
opacity and linearity reject construction, forgotten settlement, and double
completion; no interrupt-specific cleanup or implicit drop rule exists.
The normalized installed-root entry path supplies provider minting and
settlement: its receipt binds the exact root/entry/code/provider execution,
invocation, initial mask state, and acknowledgement policy. Replayed
invocation or acknowledgement identities reject, nested saved-mask guards
restore only the newest exact prior state, active entries pin root retirement,
and deriver-owned exit requires the entry mask state plus the exact completed
acknowledgement. The concrete Cathedral PIC/LAPIC entry implementation remains
to execute those admitted transitions.

### OS interrupt policies are consumer-owned

Omega validates whichever root, stack, nesting, acknowledgement, and
machine-state policy a target package supplies. It does not choose exception
coverage, IST assignments, interrupt-gate masking, PIC versus LAPIC, timer
handoff, or fatal-fault policy. Cathedral's current choices live in Cathedral's
hardware-foundation and boot documents.

The reusable acceptance conditions remain here: every hardware-reachable entry
is recorded before publication; WCSU composes over the supplied nesting graph;
final machine-state use fits `StatePlan`; acknowledgement and mask restoration
obligations settle exactly once; and static construction may retain sealed entry
identity without exposing a source-level numeric code address.

## Carry contracts and runtimes

Canonical frontend place-liveness is shared infrastructure. WCSU, linear
consumption, permission/external-loan checking, and carry checking consume it
independently; they do not share an algebra.

Values/resources state independent demands:

- may or may not cross suspension;
- same CPU required or not;
- same host thread required or not; and
- stable address required or not.

The settled type-wide source is one compiler-built-in property over the full
product:

```omega
boundary data PerCpuLease [
    linear,
    carry(
        suspension: allowed,
        cpu: same,
        thread: any,
        address: movable,
    ),
];
```

The property lowers directly to normalized compiler IR. It is not ordinary
`omega::core` data, a trait, or the output of a policy machine: the vocabulary
is closed because the compiler must interpret every axis. Transparent data
derives structurally; opaque data with no declaration is maximally strict. An
opaque declaration is only a claim and remains inert until proved or accepted
under admission receipt. Constructor `ensures` may establish sealed per-mint
carry domains, monotonically adding permissions above the type-wide floor.

The old `[send]` placeholder is retired. Cross-activation exclusive transfer is
ordinary ownership plus carry/runtime compatibility; crossing shared references
also requires a sanctioned shared-access contract. Carry and sharing are not
marker traits and do not follow from copyability.

Runtime providers state independent behavior:

- safe-point or asynchronous preemption;
- migration behavior and available affinity/pinning;
- host-thread behavior; and
- stable or movable continuation storage.

Admission joins demand and behavior. The effect row stays static: a live mask
or affinity token may make a particular call locally inadmissible without
editing or masking the machine's published effects.

The normalized join is live in `omega-task-plans`: suspension is rejected
locally against possible park crossings, while provider admission selects a
safe-point or all-instruction migration envelope and checks CPU/thread
affinity, continuation stability, frame provisioning, cancellation, and inline
behavior. Missing opaque-runtime evidence is pessimistic. Compiler liveness
derivation and provider-plan integration remain.

The enforcement sites are deliberately asymmetric. A value that forbids
suspension is checked locally against possible suspension; provider
selection cannot erase that ceiling. CPU affinity, host-thread affinity, and
address stability instead join the activation's demands with the runtime's
normalized behavior at admission. Preemption granularity selects which points
need those live-value checks. Runtime behavior is born pessimistic: a checked
provider proves narrower behavior, while an opaque provider needs an admission
receipt authorizing reliance on its narrower claim. The receipt does not change
behavior; it changes what admission may trust.

Structural composition selects the most restrictive live-field demand on each
axis; the axes share traversal, not an algebra. Interrupt masking and
scheduler-switch suppression are different linear tokens: the former defers
delivery; the latter prevents an Omega activation switch but cannot prevent a
host kernel from preempting its thread.

Cathedral currently chooses safe-point scheduling and stable continuation
storage in its own hardware/runtime profile. The language representation keeps
the axes independent so Cathedral or another runtime may admit a stricter
asynchronous provider without redesigning Omega.

Local checking, runtime admission, and future temporal verification are three
consumers of the same facts. Local checking combines canonical liveness with
carry policy at each transition. Admission joins accumulated demands with the
selected runtime. A future TLA-style layer adds interleavings, protocol state,
and liveness hypotheses; it consumes normalized policy and provenance rather
than re-reading source attributes.

## Implementation status and ownership

`TASKS.md` owns the current implementation sequence. The language/compiler
lanes are the checked-assembly catalog, opaque runtime representation, generic
layout/materialization, `AccessPlan` projection, evaluated entry plans, final
machine-state validation, external loans, carry/runtime admission, and
relocatable admitted artifacts.

Cathedral owns page-table hierarchy and teardown, IDT/TSS schema and lifecycle,
exception/IST policy, PIC/LAPIC providers, timer top and bottom halves, DMA
protocols, hostile-IPC mapping policy, and task-runtime provisioning. Those
customers are acceptance tests for the generic machinery, never compiler
implementation phases.

## Gauntlet

The foundation is not complete because this brief says so. It earns confidence
when the same pieces implement, without new customer-shaped syntax:

1. UART MMIO with read-only, write-only, and W1C registers;
2. an OS-package address-translation implementation;
3. trusted and hostile shared-page IPC;
4. zero-copy DMA with completion and revocation;
5. IDT, timer interrupt, nesting, and acknowledgement;
6. SMP AP bringup through installation of an admitted low-memory trampoline and
   checked mode changes.

Required negative tests include hidden reach through direct assembly, forged
addresses without authority, stale hostile-peer validation, CPU access during
an external loan, a split relocation consumed too early, and a final-image
veneer/thunk that introduces a register class forbidden by the root's
`StatePlan` despite all earlier per-function checks passing. Extent/access tests
must also reject merging numerically adjacent ranges from different authority
origins and reject ordinary non-atomic writes through two shared projections.
IDT tests must show that an open-authored `Layout` or `Calling<C>` policy can
produce only a candidate plan: it cannot mint the sealed resolver, Cathedral's
materialized-table fact, or Cathedral's CPU/table publication authority; cannot
publish a structurally valid but semantically inadmissible table; and cannot
hide installer reach behind a wrapper or direct checked assembly.

## Open decisions

These are the remaining design questions, not permission to invent local
syntax while implementing:

- the final artifact-footprint certificate format and validation boundary for
  static and dynamically loaded admitted artifacts.

Dynamic source-visible entry references, movable continuations, asynchronous
revocation, live patching policy, general quantitative resource/WCET algebra,
recoverable faults inside hard interrupt roots, independent final-byte
control-transfer certificates, and CET/PAC/shadow-stack hardening remain
deliberately deferred until their owning assurance profile or customer is
implemented.
