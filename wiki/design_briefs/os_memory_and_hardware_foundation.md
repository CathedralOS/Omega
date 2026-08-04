# Design Brief: OS Memory And Hardware Foundation

Current direction as of 2026-08-01. The primitive taxonomy and security model
guide implementation. The placed-access source model is specified here;
`TASKS.md` distinguishes it from the smaller normalized Rust foundation already
implemented.

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
| qualified `Extent`, its borrows, and owned splits | transparent geometry plus established authority over one concrete range, with rights, provenance, address space, lifetime, and effective resource profile | mappings, MMIO, DMA, IPC, allocators |
| `LayoutPlan` | physical geometry: offsets, alignment, overlays, bit and fragment placement, endianness | foreign records, descriptor tables, protocols |
| `AccessPlan` | consumer demand: inaccessible, stable, external, or individually atomic field operations plus exposure | MMIO and shared storage views |
| `ResourceProfile` | admitted provider supply over offset-keyed regions: observation, rights, widths, alignment, atomics, and reach | RAM, MMIO, shared pages, mapped accelerators |
| `Placed<P, T>` | borrowed or owned storage interpreted through one checked nominal placement policy bundling layout, access, and reach | registers, framebuffers, IPC pages |
| parsed checked assembly | target instructions whose contracts emit effects, authority, clobbers, state changes, and exits | control registers, port I/O, fences, mode changes |
| boundary entry plan | one normalized contract containing a `CallPlan` and a `StatePlan` | firmware entry, interrupts, exceptions, syscalls, callbacks |
| symbolic materialization | toolchain-resolved identities placed into structures at the last legal phase | IDT targets, image symbols, callbacks |
| executable-artifact installation | validate and place immutable admitted code under scoped authority; never convert arbitrary bytes to code | boot images, components, AP trampolines |
| external-root ledger | all installed inbound roots plus their reach, trust, stack domains, preemption relations, and version pins | interrupts, callbacks, runtime entries |
| external loan | a linear token standing in for a borrower the checker cannot observe | DMA and device ownership transfer |
| carry/runtime contracts | value demands joined with scheduler/storage behavior at admission | suspension, migration, CPU/thread affinity, address stability |

These pieces compose `data`, `machine`, `trait`, `domain`, `boundary`, ordinary
contracts, linearity, capabilities, and plan policies.

## Extent and allocation strategies

`Extent` is the core storage authority. Bump, slab, pool, buddy, and general
heap strategies are ordinary packages that consume or borrow appropriately
qualified extents and conserve every owned subextent they issue. No Arena
capability or allocator strategy is part of the language model. See
[`allocator_story.md`](allocator_story.md).

A placed view instead needs authority over an
already-existing range that was not allocated by the program, such as a UART
register block. That is an `Extent`.

The public carrier is ordinary linear data:

```omega
data Extent [linear] {
    base: addr;
    length: u64;
}

pub domain Extent::Granted
    requires no_wrap(self.base, self.length)
{
    ExtentRootProvider::grant;
}

pub boundary trait ExtentRootProvider {
    machine grant(root: Extent) -> Extent
    ensures
        result in Extent::Granted;
}
```

Program image and initial stack/storage roots use a second core-owned route on
the same `Extent::Granted` domain. The live
`ProgramStorageEntry::enter(image: Extent in Granted, initial_storage: Extent in
Granted)` requirement names the exact qualified positions; target entry traits
inherit it and refine only their plan/ABI. Installation binds those semantic
positions to the selected calling-plan fingerprint and generated ABI captures,
then introduces both roots only after validating both `Granted::no_wrap`
obligations. Rejection returns both admitted grants without importing either
complete fact. Image/static ranges remain borrowed views beneath the installed
image root; owned initial-storage allocations produce a conserved partition
with every nonempty remainder and can recompose the exact parent lineage.
Target-owned root domains or name-based role recognition are not alternatives.

The fields carry runtime geometry. `Extent::Granted` states that the geometry
descends from a live admitted or checked authority claim. Constructing the same
fields creates an unqualified linear value. Operations that consume range
authority require `Granted`, so a fabricated or dequalified Extent has no legal
resource consumer. `no_wrap` proves in proof-level natural arithmetic that the
embedded base plus length does not exceed the target address-space bound; it
does not perform wrapping `addr` addition.

An admitted platform provider originates a root only by satisfying the
owner-authored `ExtentRootProvider::grant` requirement. The caller supplies the
ordinary geometry; the selected provider and its admission receipt establish
`Granted` on the returned carrier. A direct call to the checked adapter is not
that crossing and does not establish the fact. An ordinary fingerprinted
postcondition bounds per-invocation geometry in the same compiler-owned
canonical interval-set algebra as `Granted`'s normalized content projection. No
proof-only receipt binder exists. The provider evidence separately admits
stable backing identity, ownership, and fresh nonduplicating issuance; a
result-selected base can prove returned size but cannot prove that the interval
is new.

The live source declaration is in `omega::language::core::extent` together
with the debt-free `ExtentSlot { Empty | Live(Extent) }` bridge. Core's stage-1
`Arena` returns and reclaims qualified Extents. Cathedral's UEFI boot package
now supplies the selected checked `ExtentRootProvider` adapter, admits that
provider plan in `build.omg`, obtains one qualified root after
`ExitBootServices`, and threads it through every post-grant graph state into
owned idle. Physical-space, rights, and algebra-denominated backing remain
later qualification/frontier work rather than facts inferred from the firmware
geometry.

Address space, permissions, provenance, and mapping era are domain facts on the
carrier. Physical, virtual, I/O-port, and provider-defined spaces share the
same range algebra. An operation requiring `Physical` accepts evidence for that
space; an extent carrying `Virtual` does not meet the requirement. Rights such
as `Readable` and `Writable` originate from admitted provider evidence or
checked conservative derivation.

An extent's semantic record binds at least:

- runtime base and `u64` length;
- address-space facts (physical, virtual, I/O, or provider-defined);
- read/write/execute or more specific rights facts;
- minting provenance, parent grant, and authority-origin/split ancestry;
- lifetime or mapping era; and
- ownership sufficient to split, attenuate, borrow, release, or revoke it.

Base and length occupy runtime bits. Space, rights, provenance, and era remain
compiler facts or receipts while their provenance survives the required
storage/component crossings. A provider-owned table may back a visible handle
when operations need dynamic lookup or revocation; its key remains an ordinary
field.

Admitted suppliers originate root Extents through boundary-machine receipts:
boot handoff, an address-space mapper, a parent allocator's backing store, or a
device provider. `Granted` is routed, so an ordinary `as` proof cannot create
it. The provider requirement names the exact qualified result that admission
may establish, and the receipt records that origin. Checked code derives
children through resource transformations whose outcome mappings conserve the
parent claim's content and root lineage.

Provider-local issuance is insufficient for cross-provider exclusivity. Every
exclusive extent names stable backing. Providers issuing over the same backing
identity descend from a common custody root; that root delegates separated
ranges to child providers or explicitly authorizes shared aliases. Providers
with no common custody lineage do not gain a derived nonoverlap fact from
individually clean ledgers.

Boot handoff uses classified custody succession. At one instant custody is a
tree; over time provenance appends succession edges. Preserved delegations move
to the successor and remain honored. Reclaimable classes become successor
capacity only after a derived check proves that no live claim overlaps them.
Retained classes remain under firmware or another continuing root, while
excluded classes stay unavailable. The platform provider admits the external
classification; the compiler checks locally tracked live ranges against it.
Existing claims keep stable backing identity, and provenance records rather
than rewrites their custody history.

Splitting consumes one owned qualified extent and returns disjoint owned
children whose ranges exactly cover it. `Extent::Granted` projects its subject
through its owner-unique `Content` conformance to a normalized address-space
interval with proof-level `Nat` bounds. The half-open end may equal the
address-space bound even when that one-past value is not representable as an
`addr`. The checker proves the parent content equals the partial separated
composition of all child content; per-child containment or a scalar length sum
is insufficient. Merge proves the same equation in reverse over compatible
common root lineage. Literal siblinghood is not required, while numeric
adjacency alone is insufficient because adjacent ranges may have different
grants, permissions, provenance, or eras. Combining unrelated adjacent grants,
if needed, is an explicit provider operation that establishes new combined
authority.

Permission attenuation is orthogonal to interval content. Weakening read-write
to read-only preserves the range and permanently discards write; merge cannot
join permissions to recreate it. Authority that must return later is a separate
claim or loan.

Subrange loans are borrow-carrying values. Their polarity follows the parent
borrow: shared loans permit only shared operations; exclusive loans permit
ordinary mutation. Loans are not linear cleanup obligations. The owned extent,
DMA tokens, shootdown tokens, and similar authority/debt values remain linear.

Destroying placed content returns its extent authority but does not itself
release that extent to an OS or firmware provider. Provider reclamation may be
implicit only when it is terminating, infallible, non-suspending, and
nonblocking. Otherwise the extent stays linear and exposes explicit terminal
`release` and, where policy permits permanent loss, `abandon` operations.
Safety profiles may reject abandonment; capacity loss must not hide behind an
ordinary silent drop.

The normalized conservation model is live in `psi-extents`. Its Rust carrier
is non-clonable; an admitted one-shot root receipt establishes the first claim;
space, provenance, era, and lineage identities are normalized; rights are an
open set of normalized identities; and split, attenuation, sibling merge, and
bounded shared/exclusive loans are validated. Failed consuming operations
return every input authority. The source checker generalizes this temporary
sibling-only model to compatible common lineage, algebra-denominated backing,
and n-ary separated content conservation.

Content is also the source of truth for access: every authority-bearing
operation proves its touched interval lies within the claim's projection. A
checked overapproximation rejects when establishment backing is too small; an
underapproximation remains safe but restricts use. A provider can still lie
about external reality, and that accepted assertion remains visible in its
receipt.

Layout fields, placed views, subrange loans, and allocator-private free-list
geometry do not split owned authority. Owned split/merge is needed when a
subrange actually leaves the parent's ownership domain, including an allocator
returning an independently owned allocation claim.

Virtual and physical quantities cannot be decomposed as independent conserved
projections when their correspondence matters. That requires a future compact,
canonical symbolic mapping algebra whose containment, restriction, equality,
and separated composition remain decidable. Until it exists, owned
virtual-to-physical decomposition rejects; the initial interval and counted-
quantity algebras do not represent that correspondence.

The source migration depends on routed establishment, admitted
boundary-machine receipts, and generic resource-frontier outcome mappings. See
[`authority_values_and_boundary_evidence.md`](authority_values_and_boundary_evidence.md).

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
    requires source in Extent::Granted
    requires source in Extent::Physical
    requires destination in Extent::Granted
    requires destination in Extent::Virtual
    ensures result in Extent::Granted
    ensures result in Extent::Virtual
    ensures result in Extent::Mapped;

machine map_borrowed(source: &Extent, destination: Extent) -> Extent
    requires source in Extent::Granted
    requires source in Extent::Physical
    requires destination in Extent::Granted
    requires destination in Extent::Virtual
    ensures result in Extent::Granted
    ensures result in Extent::Virtual
    ensures result in Extent::Mapped;
```

The two contracts preserve the source ownership distinction. Unmapping
consumes the mapped extent, returns the reusable destination range,
and either returns an owned source or ends its source loan. On targets requiring
cross-core invalidation, reuse remains gated by a linear shootdown/quiescence
token. Its completion operation carries the provider's ordinary suspension or
blocking ceiling, so an interrupt root cannot hide an illegal wait.

V1 has no per-access generation probe. Reclamation requires exclusive ownership
back and therefore no live in-language views. Forced asynchronous revocation of
unreclaimed loans is deferred to provider quiescence/lifecycle machinery when a
customer requires it; translation edits, shootdowns, and process teardown remain
ordinary runtime provider work.

The provider-neutral mapping lifecycle is live in `psi-extents`. An admitted
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

`LayoutPlan` answers where bits live. `AccessPlan` answers which primitive
operations a consumer requests. `ResourceProfile` answers what an admitted
backing supplies. `PlacementPlan` bundles the first two with the boundary reach
required by the interpretation. Keeping these axes independent lets one schema
describe owned values, MMIO, DMA-visible bytes, or a shared page without
teaching its fields one permanent access environment.

Ordinary owned RAM remains ordinary. Once raw storage has been validated or
materialized as an owned `T`, it uses `&T`, `&mut T`, and lvalue projection.
Placed stable access is reserved for cases where a loan or field-level policy
must remain visible. It is not a ceremony imposed on every struct.

### The source policy surface

A placement policy is a nominal, build-time-evaluated recipe:

```omega
trait Placement {
    machine plan(schema: Schema) -> plan: PlacementPlan;
}

data PlacementPlan {
    layout: LayoutPlan;
    access: AccessPlan;
    reach: BoundaryReach;
}

trait Access {
    machine plan(
        schema: Schema,
        layout: LayoutPlan
    ) -> plan: AccessPlan;
}
```

The layout policy decides physical geometry. The access policy receives the
same reflected schema and the already validated layout so it can reject or
hide fields that do not admit a primitive access. The placement policy chooses
both and states the static service reach. Runtime provenance proves that this
reach may touch the supplied region; it never manufactures or changes a
machine's reach row. Transfer containers and effective widths are derived from
the validated layout; the access policy does not restate geometry.

`Schema` carries opaque compiler-issued field keys. Access policies address
fields through those keys, not authored indexes:

```omega
machine UartAccess::plan(
    schema: Schema,
    layout: LayoutPlan
) -> plan: AccessPlan
    satisfies Access::plan
{
    let plan = AccessPlan::inaccessible(schema);
    let plan = plan.with(
        schema.field("status"),
        FieldAccess::External(ExternalRead::Read, false, Exposure::Exported)
    );
    plan.with(
        schema.field("transmit"),
        FieldAccess::External(ExternalRead::None, true, Exposure::Exported)
    )
}
```

The field key is compile-time identity only. The evaluated plan has exactly one
decision per schema field. Starting from the inaccessible plan makes zero and
omission deny access. A fixed-capacity bootstrap representation may back this
model until computed generic sizes are implemented, but its capacity and tail
do not enter source semantics or normalized identity; overflow rejects.

The target field vocabulary is a sum, so invalid cross-products are not
representable:

```omega
data Exposure {
    case Exported;
    case BindingPrivate;
}

data ExternalRead {
    case None;
    case Read;
    case Take;
}

data FieldAccess {
    case Inaccessible;
    case Stable(read: bool, write: bool, exposure: Exposure);
    case External(
        read: ExternalRead,
        write: bool,
        exposure: Exposure
    );
    case Atomic(
        operations: AtomicOperations,
        exposure: Exposure
    );
}
```

`BindingPrivate` belongs to the nominal placement policy's package. It is an
issuance and naming restriction, not a substitute for range authority. A third
party may describe its own interpretation of a range it legitimately holds,
but only the binding package may directly name or issue its opaque private
accessor. Possession deliberately delegates the accessor's public operation
requirements to generic code. Copyability, cross-activation shareability, and
counted permits separately control durable, concurrent, and bounded delegation.

Stable read/write plus an exclusive active borrow and exclusive source borrow
derives ordinary mutation and compound updates. External never derives generic
read-modify-write. Atomic exposes only the listed operation families and their
ordinary orderings.

Generic atomic code uses one sealed `omega::core` requirement per primitive
operation. Load, store, swap, decisive and single-attempt compare-exchange, and
each fetch operation remain independent; ordinary core atomics and placed
accessors conform to the same family. All receivers are shared, ordering is
explicit proof-static operation data, and exact forwarding is the only
automatic wrapper derivation. Any other implementation needs checked proof or
admitted provider evidence.

Every operation requires a fixed representation fitting one admitted atomic
width and alignment. Load requires duplication, store requires the displaced
resident to be discardable, and swap conserves ownership. An affine swap is
available only when Stable initialization made the placement owner of its
resident; adoption of device-owned contents never does. A local cell may retain
an activation-bound value, but sharing that cell across activations separately
requires resident transferability. Scalar compare-exchange initially remains
copyable and compares raw representation with `encode(expected)`. Each fetch
operation additionally proves its exact authorized raw transition over every
provider-reachable representation; External read/write capability never
synthesizes fetch.

### Resource profiles are admitted supply

The provider describes the backing with an immutable, offset-keyed capability
set:

```omega
data ResourceProfile {
    regions: ResourceRegions;
}

data ResourceRegion {
    range: RelativeRange;
    stable: StableCapability;
    external: ExternalCapability;
    atomic: AtomicCapability;
    reach: BoundaryReach;
}

data RelativeRange {
    offset: u64;
    length: u64;
}
```

Regions are disjoint and normalized. Uncovered bytes support nothing. The
three capability slots are supply that may coexist, not one selected mode:
ordinary RAM may supply stable, conservative external, and explicitly atomic
access; MMIO normally supplies external only; a concurrently shared page may
supply external and selected atomics without stable access.

```omega
data StableCapability {
    case None;
    case Read;
    case Write;
    case ReadWrite;
}

data ExternalReadBehavior {
    case None;
    case Repeatable;
    case Destructive;
}

data TransferRule {
    width_bits: u64;
    alignment_bytes: u64;
}

data ExternalCapability {
    case None;
    case Access(
        read: ExternalReadBehavior,
        write: bool,
        transfers: TransferRules
    );
}

data AtomicTransferRule {
    width_bits: u64;
    alignment_bytes: u64;
    operations: AtomicOperations;
}

data AtomicCapability {
    case None;
    case Access(transfers: AtomicTransferRules);
}
```

Alignment is explicit per transfer width. `write` means a whole-container
write at one listed width, never permission to synthesize an RMW. A profile can
describe read-only containers, destructive reads, legal atomic widths, and
different behavior across a heterogeneous BAR. Device-specific semantics such
as W1C, FIFO protocols, posted-write completion, locks, DMA completion, and
coherent snapshots stay out of this record.

A profile record is freely constructible data, but a consumer cannot make one
authoritative. The same selected-provider act that establishes the qualified
extent binds the exact normalized profile to its range, rights, provenance,
and receipt. Admission finds that receipt through the qualification rather than
accepting a caller-supplied profile. The provider may lie about hardware; the
receipt makes that trust expenditure visible but cannot prove the physical
world. A mapping era may remain diagnostic identity, but safety comes from the
live mapping claim or from a fallible revocation protocol, not from comparing an
erased era value after asynchronous revocation.

Subrange loans restrict this profile mechanically:

```text
child profile =
    parent profile
    intersect child interval
    attenuate rights, reach, and operations
```

A child never gains a capability the parent lacked. Distinct layout fields or
checked constant ranges can establish disjoint simultaneous subrange loans.
Dynamic disjointness waits for the prover support named in `TASKS.md`.

### Placement admission and establishment

Source uses ordinary borrows of qualified places:

```omega
let shared = Placement::admit<P, T>(&extent[0..64])?;
let exclusive = Placement::admit<P, T>(&mut extent[64..128])?;
```

There is no source-visible `ExtentLoan`. The borrow checker already records the
exact projected range, lifetime, and polarity. The current Rust foundation
carrier with that name is transitional internal bookkeeping, not a language
type or a second authority root. Weakening the input to `&Extent` deliberately
loses `Granted` and therefore cannot admit a placement. Static subranges use
ordinary place projection; an independently owned runtime subrange uses
conserved `Extent` split and merge.

Admission evaluates `P`, finds the provider-bound profile through `Granted`,
and purely checks demand against supply. The opaque accepted intermediate may
be inferred or immediately consumed; ordinary code cannot construct one.
Borrowed rejection ends the borrow and reports the incompatibility. Owned
rejection returns the moved extent with that diagnostic.

The accepted intermediate proves permission, not live typed content. An
explicit establishment route follows:

- `Stable` may adopt provider-validated existing contents, initialize an
  exclusive vacant place, or validate stable existing contents;
- `External` may only adopt in v1, and every readable field must be
  total-decoding and covered by one admitted transfer; and
- generic initialization never expands into a device-programming sequence.

Borrowed initialization constructs and destroys `T` wholly inside the
exclusive borrow, returning the lender from `Vacant` to `Vacant`; mandatory
edge cleanup and the absence of a leak/forget escape are load-bearing. An owned
initialization consumes a split vacant extent, and its explicit terminal
operation destroys `T` and returns `Extent in Granted & Vacant`. Borrowed
adopt/validate retire by abandoning the view. Owned adoption of existing
content additionally requires provider evidence that custody of that content,
not merely its storage, transferred.

`Vacant` is erased place state: no live established value occupies the exact
range. It says nothing about zeroing, readability, prior use, allocation
strategy, or bump capacity. The name denotes the established-place judgment;
ordinary source inherits it from allocation/raw storage or retirement and does
not infer it merely from the absence of a live view.

Plan validation checks relative layout, requested operations, widths, and
reach. It also derives base congruence from each transfer:

```text
base mod alignment = -field_offset mod alignment
```

Power-of-two constraints combine at build time. Residues must agree modulo the
smaller alignment; the combined modulus is the larger. An impossible policy
therefore fails before deployment. Admission checks the remaining condition
against the actual base or a provider base-alignment guarantee. Diagnostics
lead with the fields, offsets, and transfer rules that conflict rather than raw
congruence notation.

The compatibility join requires interval containment, exact transfer width,
absolute alignment, operation and reach subsets, and compatible observation.
Stable supply may satisfy a more conservative External demand. External supply
cannot satisfy Stable demand. Atomic access requires an explicit matching
atomic rule.

Plan validation also checks representation and transfer derivability per field
and per requested operation. Reads require total decoding, stable validation,
or separate admitted content-validity evidence as appropriate. Writes require
an encoding for every admitted field value or proof that the concrete value
fits; the compiler never invents the fitting domain or silently narrows. A
valid encoding still needs a legal transfer. Stable exclusive bitfields may use
one bounded read-patch-write sequence. External writes cover a whole admitted
container or use a provider-supplied masked operation. Atomic set/clear may use
one admitted fetch operation, but arbitrary assignment never hides a retrying
compare-exchange loop.

Admission proves that a placement is supportable, not that its offsets and
meanings describe the actual device. A separate admitted schema-correspondence
fact ties the nominal placement to provider/device identity and records its
datasheet or platform source. A revision-sensitive fact may be conditional on
a runtime ID observation, provided that observation and the selected full
placement are tied to the same stable device instance and grant. The condition
improves provenance while the physical claim remains admitted-class.

### Placed projection and lowering

`Placed<P, T>` is a compiler-derived placed view retaining either a source
borrow or an owned split extent. Its fields are accessors, not lvalues:

```omega
let status = uart.status.read();
uart.transmit.write(byte);
```

Projection is pure. The operation consumes a sealed lowering authorization
binding plan identity, receipt, exact loan, field, address, width, observation,
borrow polarity, lifetime, reach, and atomic ordering when applicable. No
public primitive accepts an arbitrary base or offset.

Each operation retains a logical field extent and a physical effect footprint.
Conflict checking uses the latter: non-consuming reads share; destructive reads
and stable read-modify-write reserve the whole affected transfer container;
atomics follow the exact admitted operation and width. Two logically disjoint
bitfields in one word therefore cannot hold simultaneous exclusive RMW access.
A destructive transfer yields one owned whole-container snapshot and only then
permits pure field projection.

The compiler derives granular requirements:

- `Readable<T>::read(&self)`;
- `DestructiveRead<T>::take(&mut self)`;
- `Writable<T>::write(&mut self, value)`;
- atomic load, store, swap, compare-exchange, and fetch families.

The exclusive receiver of `Writable` serializes that accessor value; it does
not by itself claim that an External device or every other view is excluded.
Stable write derivation separately requires an exclusive source borrow, while
External write derivation requires one permitted complete transfer.

A destructive container derives `DestructiveRead`, never ordinary `Readable`.
Exposure remains independent: a FIFO pop may be exported, while a
read-to-clear status primitive may remain binding-private behind one authored
snapshot machine. One container read yields owned bits; pure projections or an
ordinary checked value recast may decode those bits afterward.

An external field narrower than its admitted transfer container may be read by
one whole-container snapshot and pure bit projection. A field wider than one
admitted transfer has no generic accessor; a coherent multi-transfer read is an
authored device protocol. A narrow field cannot be written generically, because
preserving unrelated bits would require an external RMW. Registers requiring
shadow state, W1C, read-back, or multi-field coherence use authored device
machines. Shadows are valid only where software owns every bit they rewrite.

External guarantees one non-elided transfer at its declared width and preserves
relative program order among External accesses to the same region in one
activation. It does not promise that a device has observed a posted write.
Fences, read-back-to-flush, and device completion are separate checked
operations because a CPU barrier alone cannot make every fabric or device
complete a transfer.

These device operations are sealed semantic provider requirements, not new
clauses on every boundary signature and not additions to `reaches`. A hosted
boundary may conform directly to a complete DMA-submission requirement; a
checked driver may compose publication, cache-maintenance, MMIO-notification,
and completion primitives. The selected provider plan records each conformance
and its exact range, mapping, observer/device instance, and ordering scope.
Every emitted requirement must receive derived or policy-permitted admitted
evidence; an open requirement rejects.

Publication evidence names the published place and current write state. An
intersecting write frame invalidates it before a later doorbell can consume it.
The erased evidence does not itself constrain emitted code: publication adds a
scoped ordering event to terminal Psi, and target lowering must preserve that
event through the required cache maintenance, barrier, OS operation, or
instruction-free coherent realization. Acquisition consumes completion
evidence tied to the same request and stable device instance. It restores a
Stable CPU view only when completion also returns custody; otherwise subsequent
device writes keep the placement External.

Stable ordinary mutation requires plan permission, an exclusive current
borrow, and an exclusive source borrow. Reborrowing a shared-source view as
`&mut` does not upgrade it. External and atomic operations instead follow their
declared transfer permissions: a shared external view may issue an admitted
whole-container write because borrow polarity governs Omega aliasing, not
whether the device exists or may mutate. A single compare-exchange attempt is
bounded; a retrying wrapper carries its ordinary unknown or
no-finite-guarantee work attribution and is never synthesized behind `.write`.

View-to-view recast is rejected in the first implementation. It could expose a
field that the source binding made inaccessible even without changing the
observation class. Ordinary recast remains available for detached values and
ordinary storage. A caller holding the underlying qualified borrow may request and
admit a different placement explicitly.

### Ownership phases and applicability

The resource profile describes backing potential; the active loan describes
the current phase. Starting DMA may exchange a stable CPU loan for an
external-only device-owned loan. Consuming completion restores stable CPU
authority. The same backing can therefore be placed differently at different
times without a mutable phase flag in the profile.

`Placed<P, T>` is restricted to stable addressable places whose lifetime and
aliasing a loan can govern and whose primitive accesses are finite,
compiler-owned, non-suspending operations without recoverable failure under the
admitted provider contract. MMIO, resident shared mappings, persistent memory,
and CPU-mapped accelerator storage can qualify. Durability and device
completion remain protocols. Demand-paged or truncatable mappings, disks,
streams, RPC endpoints, and device-only storage use fallible services or
handles.

This design has three mechanical walls and two separately attributed physical
assertions. Projection is pure, operations are explicit, and primitive access
kinds are compiler-owned. The provider is admitted to tell the truth about the
backing, while the binding is admitted to map its schema to the identified
device. Their compatibility is derived without laundering either premise.

### Implementation migration

The ordinary source records live with the layout vocabulary in
`omega::language::core::layout`; the existing `Plan` record remains the current
source spelling of `LayoutPlan`. `AccessPlan::inaccessible(schema)` produces
the required compiler-keyed denial seed, and functional keyed replacement
cannot silently accept a foreign key. The compiler now evaluates
build-time-admissible `Access::plan` machines against a reified validated
layout and evaluates `Placement::plan` machines into one validated
layout/access/reach result. It derives transfer widths from layout geometry
rather than accepting a second authored copy.

The `psi-access-plans` foundation validates normalized field geometry, exact
widths, operation and observation compatibility, borrow polarity, atomic
orderings, exact loan facts, and sealed lowering requests. It normalizes to one
inaccessible-defaulted slot per schema field, binds opaque field keys to the
exact layout, and keeps destructive external reads distinct from ordinary
reads. A normalized
`PlacementPlan` now owns the complete layout/access pairing and its one
normalized boundary reach; admission checks that reach once and lowering
requests retain it. The Rust bootstrap still consumes an internal exact-loan
carrier and returns it on rejection; source lowering must replace that carrier
with ordinary qualified place borrows and opaque inferred admission evidence.
Current view-borrow and retained source-borrow polarity are checked
independently.
Exposure uses the settled `Exported | BindingPrivate` vocabulary; stable
compound mutation is derived, external compound mutation is unavailable, and
atomic permissions distinguish the exact operation families. Admitted
`ResourceProfile` supply is now normalized into disjoint offset-keyed regions
and bound by provider receipt to an exact range, address space, provenance,
mapping era, required rights, and reach. Exact subrange loans intersect and
rebase that supply without filling uncovered bytes. The placement/profile join
checks interval containment, observation, operation and reach subsets, and
exact widths; it derives one power-of-two base congruence before admission,
while admission checks the concrete loan base and returns a rejected loan
intact. Accepted placements and sealed primitive requests retain the provider
receipt and effective per-field supply. The foundation placed-view carrier now
separates pure `project`/`project_mut` from the memory event: projection yields
a borrow-carrying field accessor, and only named read, destructive-take, write,
stable-compound, and individual atomic-family methods can seal a primitive
request. Inaccessible fields yield no accessor; current view-borrow and source
loan polarity remain independent checks at the operation boundary. `TASKS.md`
owns the remaining source and lowering work. The compiler now performs the
first fail-closed source derivation for each concrete `Placed<P, T>` spelling:
it evaluates the canonical placement policy before ordinary resolution,
revalidates the normalized identity in the authoritative typed program, and
synthesizes unique opaque stable/external field accessors implementing only
the admitted `Readable`, `DestructiveRead`, and `Writable` requirements.
Inaccessible and unauthorized operations have no projection/method. Atomic
fields now derive unique opaque `bool`/`u32`/`u64` accessors whose exact
operation subset remains attached to the authoritative typed placement plan.
Direct load, store, fetch, swap, and compare-exchange syntax is checked against
that subset; atomic mutation follows the atomic rule through a shared view
borrow, and a bare accessor cannot become an ordinary scalar. Only the nominal
placement package directly names or issues a binding-private accessor;
possession delegates its public operation requirements to generic code. Public
generic helpers use the settled sealed per-operation atomic requirement family.
Qualified-borrow admission, establishment/retirement routes, per-operation
encoding/decoding and transfer checks, effect-footprint conflicts, conditional
schema provenance, and target-specific external/atomic emission remain
implementation work.

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

This conservation model is live in `psi-extents`. A reusable admitted grant
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
Final compiler-body validation now replays both runtime-text guard forms from
their retained normalized recipes. The certificate checks the exact final bytes
outside relocation fields, binds the literal-buffer `.data` symbol and any
descriptor source-storage symbol, and requires the re-derived target register/
flags union to equal the earlier `runtime_text_guard_comparison` StatePlan
fragment.
Recursive runtime-value guard replay uses the same canonical `ValueOperand`
arena retained once beside encoded machine code; instruction rows carry only
their operand roots and normalized comparison parameters. Final validation
regenerates the evaluator bytes, reconstructs every nested storage/index/text
relocation site, and checks the target may-write ceiling plus operand-sensitive
stack/control state against `runtime_value_guard_comparison`.
Direct exit-result register materialization is also replayed from normalized
register/value or register/storage recipes. Relocated storage loads must name
their retained region, and the resulting register union must equal the earlier
`exit_result_registers` StatePlan fragment.
Entry materialization is likewise replayed from normalized register, incoming-
stack, indirect-pointer, and `args` slice-descriptor recipes. The validator
requires the exact runtime-frame relocation site—including the later address
site used after loading a stack-held indirect pointer—and requires the derived
clobber unions to equal the earlier `entry_storage` and
`entry_slice_descriptor` StatePlan fragments.
Indirect-result exit copies now retain an explicit boundary role on the
canonical `CopyPlaces` operation. Final validation refuses to infer that role
from an ordinary pointee-copy shape, regenerates the target place-copy program,
checks every source and hidden-pointer relocation, and requires the resulting
scratch union to equal the earlier `exit_indirect_result_copy` fragment.
The first ordinary compiler-body write subset covers direct storage-pair
`CopyPlaces` operations, direct-storage copies targeting a frame-held pointee,
frame-held pointee sources landing in direct storage, and frame-held
pointee-to-pointee copies. Single runtime-indexed sources with frame-held
descriptor/index slots landing in direct frame or machine storage are included
too, along with their direct-frame-source to frame-indexed-target mirror.
The frame-indexed-source to frame-held-pointee form is included as well.
Runtime-indexed reads from inline frame arrays into direct frame storage are
also covered. Selection retains their separate `compiler_body_place_copy`
evidence only for the `Ordinary` role; final validation regenerates the same
place-copy bytes, checks the storage, pointer-slot, and index relocations, and
requires the target scratch union to match. Other ordinary copy/write shapes
and calls remain outside the partial proof. The pointee-pair selector resolves
both reference operands before flat storage, so the source remains a
dereference rather than being mistaken for pointer-slot contents.
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
derivation. The single emitted artifact is now self-described as
`omega.final-footprint-certificate` format v22, with a domain-separated
certificate fingerprint over its final placement binding, compiler-text
derivation, and region inventory. It remains explicitly incomplete evidence,
not an admission certificate, until the missing footprint classes close. The
envelope now exists as a typed `omega-image` object with a closed class
vocabulary and normalized coverage rows; its identity is replayed before the
compiler serializes the artifact.
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
Compiler-function and checked-instruction replay consume only the exact
compiler-authored `.text` prefix. Appended import-thunk tails therefore cannot
be mistaken for unenumerated compiler instructions and remain covered solely
by these separate format-owned validators.
The inventory explicitly lists relaxation products, veneers, generated stubs,
and admitted leaves as missing classes, so this new post-layout seam cannot
accidentally promote the partial evidence to a complete certificate.
The final certificate must still decode and validate compiler-function bytes,
then aggregate StatePlan-driven nonordinary
save/restore and return sequences, decoded compiler-function handler regions,
relaxation products, veneers, generated stubs, and admitted indirect leaves
after final placement.

That certificate is the canonical boundary. It is self-describing and
versioned, binds every normalized row to exact final bytes and placement, and
is replayed against the closed target instruction specifications by the
admission checker. The checker also proves that the region inventory covers
every executable byte. Admitted leaves remain explicit accepted rows with
separate provenance. There is no alternative whole-image decoder whose
derived answer bypasses or competes with certificate validation.

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
satisfier/contract identity. Local dynamic descriptor and object-safety
semantics are settled in
[chapter 14](../language_guide/chapter_14_traits.md); component boundaries use
bindings rather than exporting local descriptors.

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
reach, stack domain, preemption/nesting relationships, and version/liveness
pins. It also records the public ceilings, realized demands or footprints, and
validation receipts for the root's stack, structural work, and machine-state
resource columns. Friendly names are presentation only.

The ledger closes three whole-program holes:

- reach and trust reachable only from hardware callbacks remain visible;
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
The sealed stack and fuel realizations retain those complete inputs; compact
composition fingerprints are report keys, not admission evidence.

The current Rust implementation also contains IDT-named writer, table, load,
grant, and receipt states. They are implementation debt, not part of this
language architecture. Cathedral owns its IDT schema, writer lifecycle, and
installation protocol. The compiler keeps only generic plan validation,
symbolic/fragment materialization, external-root analysis, provider admission,
and checked instruction contracts. `TASKS.md` P0 tracks removing the
customer-shaped specialization.

Machine-state admission checks the final footprint against the `StatePlan`.
Schedule-keyed fixed-fuel provider summaries compose transitively while
rejecting mixed schedules, missing callees, cycles, zero invocation bounds,
overflow, and excess demand. The logical-fuel provision must name the same
schedule, and the installed-root report publishes both identities. These
provider summaries are a precursor, not yet a terminal-Psi derivation. The
report is deliberately not a numbered compiler phase because roots may be
installed after image build. A sealed provider-execution binding now joins the
normalized selected provider-plan identity, exact entry and boundary, reach,
and the three independent resource realizations at root admission. It is
identity-bound into the ledger/report and cannot be replayed after entry or
realization drift.
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
| logical fuel | installed schedule-keyed fuel provision; optional fixed policy ceiling | composed same-schedule fuel demand | current provider summaries; eventually IR control flow, ranking bounds, callee summaries, and fixed-work proof |
| machine state | `StatePlan` permitted state and save/restore commitment | emitted transitive footprint and clobbers | instruction selection, allocation, and footprint derivation |

The ledger and its report retain each applicable policy ceiling, installed
provision, realized fact, and validation receipt. They never retain private
ranking witnesses or codegen proof internals.
Sharing this record shape does not fuse the three algebras or their identity
rules: the evaluated `StatePlan` is published boundary identity, while stack and
fuel figures normally belong to candidate admission and current provisioning.
A fixed stack/fuel ceiling enters requirement identity only when policy
deliberately promises replacement without reprovisioning. Otherwise a changed
realized demand changes the candidate artifact/report and requires fresh
provisioning; it does not change the semantic requirement.

Fixed IR work is not WCET. V1 proves that a hard root has no
workload-dependent unbounded path under its admitted provider summaries and
fits its logical fuel provision. Exact cycles, deadlines, cache behavior, and
MMIO latency require a target analysis that re-searches native paths under its
own cost model. The first timer uses the trivial evidence tier: acyclic IR
control flow, no dynamic or recursive path, and fixed-work acknowledgement,
clock-capture, wake, and return leaves. Provider work summaries compose
transitively just as reach summaries do; an acyclic caller cannot launder an
unbounded leaf. Trusted lowering/install provenance is required before a
fixed-IR certificate removes native runtime metering.

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
`omega::language::core::interrupt`. `InterruptMaskGuard` is ordinary linear
data carrying the exact root, invocation, control, guard, prior-state, and
masked-state identities used at settlement; routed `Active` records valid
issuance and is required by consuming `restore`. An independent ordinary
linear `InterruptAcknowledgement` carries the exact root, provider execution,
invocation, policy, and acknowledgement identities; routed `Pending` is
required by consuming `complete`. Reconstructing either field set does not
reconstruct the fact. Restoring the prior CPU interrupt mask reaches
`machine_control`, while acknowledging the interrupt source reaches
`device_io`. Linearity rejects forgotten settlement and double completion.
The normalized installed-root entry path supplies provider minting and
settlement: its receipt binds the exact root/entry/code/provider execution,
invocation, initial mask state, and acknowledgement policy. Replayed
invocation or acknowledgement identities reject, nested saved-mask guards
restore only the newest exact prior state, active entries pin root retirement,
and deriver-owned exit requires the entry mask state plus the exact completed
acknowledgement. The compiler-selected root schema now retains a linear
routed entry qualification such as `InterruptAcknowledgement::Pending` as a
structured `accepts` row with born-strict carry. The provider-plan receipt
identity binds that row, the external-root selection bridge preserves it, and
the qualification artifact reports it. This closes the static admitted-entry
contract. The `Pending` domain names one stable core-owned acknowledgement-entry
requirement; target roots inherit that exact semantic requirement and refine
only its plan and ABI. Installation makes its exact qualified parameter an
introduction, while an ordinary checked call treats the same parameter as a
precondition. No entry marker or authored parameter selector is added. The
compiler now resolves the selected entry claim to the exact propagated checked
parameter fact and rejects occurrence evidence whose plan, requirement,
semantic position, domain, or carry policy drifts. The admitted occurrence also
retains the exact ABI placement selected for that semantic position, and an
out-of-range position rejects before installation. Concrete entry lowering
must still consume that admitted match before executing the checked adapter;
wiring the remaining mask-transition evidence into source `Active` facts and
the Cathedral PIC/LAPIC implementation also remain.

A deferred acknowledgement leases the installed interrupt root and controller
configuration until completion. Reconfiguration, shutdown, CPU removal,
relevant power transitions, and root retirement therefore drain outstanding
acknowledgements first. Carry policy decides whether the linear value may move
to a bottom half. There is no breakable pin or asynchronous revocation path.

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
data PerCpuLease [
    linear,
    carry(
        suspension: allowed,
        cpu: same,
        thread: any,
        address: movable,
    ),
] {
    cpu_key: u64;
}
```

The property lowers directly to normalized compiler IR. It is not ordinary
`omega::core` data, a trait, or the output of a policy machine: the vocabulary
is closed because the compiler must interpret every axis. Ordinary data derives
structurally from its fields and explicit type-wide plan. Accepted resource
claims originate strict; their result contracts may establish
`Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, or
`Carry::MovableAddress`. `Carry::Portable` expands to all four. Checked
resource transformations inherit permissions through their provenance
mapping, and combined origins select the most restrictive demand per axis.

Cross-activation exclusive transfer combines ordinary ownership with
carry/runtime compatibility. Crossing shared references additionally requires
a sanctioned shared-access contract. Copyability remains an independent
duplication property.

Runtime admission is demand-driven rather than a published behavior lattice.
Each lowered activation receives one fixed nonmoving stack from
whole-call-graph WCSU. A portable activation asks the scheduler for no affinity
fact. An activation that may retain CPU- or thread-restricted values requires
the selected provider to establish the corresponding preservation claim,
commonly by consuming or borrowing an affinity/pinning capability.

The reach row stays static: a live mask or affinity token may make a particular
call locally inadmissible without editing or masking the machine's published
reach. A value that forbids suspension is checked locally at explicit
semantic suspension points; provider selection cannot erase that ceiling.
Address stability of stack-resident values follows from the fixed nonmoving
`StackLease`.

Architectural preemption may pause and restore opaque state at any instruction
without becoming a semantic suspension point. A host capable of migrating the
activation outside declared semantic points must establish activation-wide
CPU/thread preservation whenever the machine may retain a restricted value, or
reject that activation. Checked providers derive this guarantee; opaque
providers need an admission receipt. The receipt changes what admission may
trust, not actual behavior.

Structural composition selects the most restrictive live-field demand on each
axis; the axes share traversal, not an algebra. Interrupt masking and
scheduler-switch suppression are different linear tokens: the former defers
delivery; the latter prevents an Omega activation switch but cannot prevent a
host kernel from preempting its thread.

Cathedral may use arbitrary architectural preemption for scheduling and still
reserve semantic cancellation, migration, and replacement for explicit
safe points. Its activation stacks remain fixed and stable by construction;
there is no provider-selectable continuation-storage mode.

Local checking, runtime admission, and future composition proofs are three
consumers of the same facts. Local checking combines canonical liveness with
carry policy at each transition. Admission joins accumulated demands with the
selected runtime. The deferred compiler-issued composition model adds
interleavings, protocol state, and selected liveness evidence; ordinary proof
machines consume normalized policy and provenance rather than re-reading source
attributes.

## Implementation status and ownership

`TASKS.md` owns the current implementation sequence. The language/compiler
lanes are the checked-assembly catalog, authority values and boundary evidence,
generic layout/materialization, `PlacementPlan`/`AccessPlan` evaluation,
`ResourceProfile` admission, `Placed<P, T>` projection and primitive lowering,
evaluated entry plans, final machine-state validation, external loans,
carry/runtime admission, and relocatable admitted artifacts.

Cathedral owns page-table hierarchy and teardown, IDT/TSS schema and lifecycle,
exception/IST policy, PIC/LAPIC providers, timer top and bottom halves, DMA
protocols, hostile-IPC mapping policy, task-runtime provisioning, concrete
device schemas and placement policies, device `ResourceProfile` declarations,
and protocol machines such as W1C/read-back/completion. Those customers are
acceptance tests for the generic machinery, never compiler implementation
phases.

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
origins; Stable placement over MMIO or External placement beyond admitted
rights; impossible or runtime-misaligned transfer geometry; narrow external
writes requiring RMW; multi-transfer External field reads; destructive access
through `Readable`; independently projected fields from one destructive
container; overlapping stable bitfield RMW footprints; mixed-width overlapping
atomics; source-borrow polarity upgrades; overlapping live views; placed-view
recast escalation; forged provider/profile/admission evidence; unencodable
field writes; schema/revision evidence from a different device instance; and
ordinary Stable mutation through two shared projections.
IDT tests must show that an open-authored `Layout` or `Calling<C>` policy can
produce only a candidate plan: it cannot mint the sealed resolver, Cathedral's
materialized-table fact, or Cathedral's CPU/table publication authority; cannot
publish a structurally valid but semantically inadmissible table; and cannot
hide installer reach behind a wrapper or direct checked assembly.

## Deliberately deferred work

Dynamic source-visible entry references, movable continuations, asynchronous
revocation, live patching policy, general quantitative resource/WCET algebra,
recoverable faults inside hard interrupt roots, independent final-byte
control-transfer certificates, and CET/PAC/shadow-stack hardening remain
deliberately deferred until their owning assurance profile or customer is
implemented.
