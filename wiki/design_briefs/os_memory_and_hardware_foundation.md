# Design Brief: OS Memory And Hardware Foundation

This brief specifies the primitive taxonomy, security model, and placed-access
source contract. `TASKS.md` owns implementation state and ordering.

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
pub data Extent [linear] {
    base: addr;
    length: u64;
}

pub domain Extent::Granted
    requires no_wrap(self.base, self.length)
    established by ExtentRootProvider::grant;

pub boundary trait ExtentRootProvider {
    machine grant(root: Extent) -> Extent
    ensures
        result in Extent::Granted;
}
```

Program image and initial-storage roots use a second core-owned route on the
same `Extent::Granted` domain. The provider-selected physical entry stack is a
separate execution resource and is never established as source-visible storage.
The live
`ProgramStorageEntry::enter(image: Extent in Granted, initial_storage: Extent in
Granted)` semantic arrival requirement names the exact qualified positions
inside the target entry bridge. A separate target-fixed physical requirement
names the platform ABI and result. Installation joins that physical invocation,
its calling-plan fingerprint, exact bootstrap providers and provenance, the
semantic requirement, generated captures, and selected continuation, then
introduces both roots only after validating both `Granted::no_wrap`
obligations. Rejection returns every moved bootstrap input without importing
either complete fact. Image-section ranges remain borrowed views beneath the
installed image root; owned initial-storage allocations produce a conserved
partition with every nonempty remainder and can recompose the exact parent
lineage. Target-owned root domains or name-based role recognition are not
alternatives.

The selected target profile owns a required environment-to-program slot whose
schema fixes the physical requirement and target-authored bootstrap adapter,
selects this semantic requirement, and declares the smaller source entry shape
exposed to the program. `build.omg` binds the target-qualified slot to one exact
source machine; it does not discover `main` or bind the physical entry. A hosted
source normally sees neither raw root, while a freestanding schema may forward
both already-qualified values. The compiler-generated physical ABI shell and
authored adapter retain both identities and contribute their composed crash,
reach, write, work, stack/state, provisioning, introduction, result-map, and
provenance contract before installation compares portable demands with target
supply.

The fields carry runtime geometry. `Extent::Granted` states that the geometry
descends from a live admitted or checked authority claim. Constructing the same
fields creates an unqualified linear value. Operations that consume range
authority require `Granted`, so a fabricated or dequalified Extent has no legal
resource consumer. `no_wrap` embeds the base and length into proof `Int`, uses
their derived nonnegative carrier ranges, and proves their unbounded sum does
not exceed the target address-space bound; it does not perform wrapping `addr`
addition. Content projection then converts those nonnegative coordinates into
the `IntervalSet<Nat>` algebra explicitly.

The settled transparent form is
`embed(base) + embed(length) <= addr::Bound`, where `addr::Bound` is a typed
observation from the sealed selected-target capsule. The formula is a
proposition, never an executable Boolean validator. Proving it cannot establish
the routed `Granted` authority; an exact `established by` occurrence and its
lineage remain independently necessary.

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
`Arena` returns and reclaims qualified Extents. Cathedral's UEFI target package
supplies the selected checked bootstrap providers and physical entry adapter.
The first returning application profile leaves Boot Services live and reclaims
adapter-owned allocations. Its composed shell, adapter, continuation, and
provider WCSU must fit the symbolic selected-target entry-stack guarantee unless
it switches to a separately allocated checked stack. Normal Unit return maps to
success; recoverable adapter failures map to exact statuses; non-returning
failures synthesize no status.

The distinct OS-handoff profile's bounded map/exit state machine threads boot-
services capability, allocation custody, final-map snapshot, and a decreasing
attempt measure explicitly.
Successful `ExitBootServices` consumes boot-scoped services while preserving
allocation occurrence identity through transfer to program custody; stale-key
rejection returns the live capability and allocations for another measured
attempt. The final snapshot and exit receipt may separately introduce only the
physical-memory regions admitted by target memory policy. Physical-space,
rights, and algebra-denominated backing are never inferred from bare firmware
geometry.

The active handoff stack is accounted independently from the source-visible
residual. Before the final exit attempt the adapter switches to a target-owned
stack whose cross-exit lifetime is proved, or proves the incoming stack has the
same property. If stack and initial storage share a parent allocation, a
conserved partition remains in the execution frontier while only a disjoint
contiguous residual is installed as source-visible `initial_storage`; a
`Granted` extent never contains an inaccessible live-stack hole.

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
borrow: shared loans permit only shared operations; write-only exclusive loans
permit non-observing replacement; read/write exclusive loans permit ordinary
mutation. Loans are not linear cleanup obligations. The owned extent, DMA
tokens, shootdown tokens, and similar authority/debt values remain linear.

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

There is no per-access generation probe. Reclamation requires exclusive ownership
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
materialized as an owned `T`, it uses `&T`, `&write T`, `&mut T`, and lvalue
projection.
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
decision per runtime-relevant reflected schema field. An `[erased]` binding
remains in semantic/type identity but has no physical field key or access
decision. Starting from the inaccessible plan makes zero and omission deny
access. A fixed-capacity bootstrap representation may back this model until
computed generic sizes are implemented, but its capacity and tail do not enter
source semantics or normalized identity; overflow rejects.

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
    case Stable(
        read: bool,
        take: bool,
        write: bool,
        swap: bool,
        exposure: Exposure
    );
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

Stable access distinguishes non-consuming read, ownership-transferring take,
discarding write, and ownership-conserving swap. An exclusive active borrow and
exclusive source borrow additionally permit ordinary mutation and compound
updates where resident custody allows them. External never derives generic
read-modify-write. Atomic exposes only the listed operation families and their
ordinary orderings.

Generic atomic code uses one sealed `omega::core` requirement per primitive
operation. Load, store, swap, observing and non-observing decisive
compare-exchange, observing and non-observing single-attempt compare-exchange,
and each fetch operation remain independent; ordinary core atomics and placed
accessors conform to the same family. All receivers are shared, ordering is
explicit proof-static operation data, and exact forwarding is the only
automatic wrapper derivation. Any other implementation needs checked proof or
admitted provider evidence.

Every operation requires a fixed representation fitting one admitted atomic
width and alignment. Load requires duplication, store requires the displaced
resident to be discardable, and swap conserves ownership. An affine or linear
swap is available only when Stable initialization made the placement owner of
its resident; provider-opened device content never does. A local cell may retain
an activation-bound value, but sharing that cell across activations separately
requires resident transferability. `AtomicCompareExchange<T>` and its weak
single-attempt `AtomicCompareExchangeOnce<T>` sibling expose the observed
resident on failure and therefore require it to be copyable.
`AtomicTryExchange<T, Key>` and `AtomicTryExchangeOnce<T, Key>` instead return
the proposed value on mismatch or an uncommitted attempt and may transfer
affine or linear custody. Their copyable key and selected transition law prove
the exact comparison encoding without constructing a second owned `T`;
success returns the displaced resident unless that law proves it discardable.
Strength and failure observation are independent axes. Each fetch operation
additionally proves its exact authorized raw transition over every
provider-reachable representation; External read/write capability never
synthesizes fetch or exchange.

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

### Placement compatibility and establishment

Source uses ordinary borrows of qualified places. There is no source-visible
`ExtentLoan` or accepted-admission object: the borrow checker already records
the exact projected range, lifetime, ownership, and polarity, while compiler
and artifact records retain the normalized provider/profile receipt. Weakening
the input to `&Extent` deliberately loses `Granted` and therefore cannot place
content. Static subranges use ordinary place projection; independently owned
runtime subranges use conserved `Extent` split and merge.

`Placed<P, T>` is the compiler-known identity-preserving view of that exact
range through `P`'s geometry and access plan. It adds no authority. Existing
domains establish provider or external qualifications, ordinary Type inputs
transfer non-runtime custody, and the proof lane supplies proposition terms.
Raw bytes, a plan, and profile data manufacture none of those.

Dormant owned content is represented by the source-visible core domain
`Extent::Resident<P, T>`. It qualifies the exact placement range, including
padding and whole-transfer footprint, as owning one complete live `T` through
`P`. The normalized type indices are invariant semantic identity; address,
mapping, revision, and claim occurrence remain dynamic evidence rather than
type arguments. `Resident` is owned, cannot be weakened away, and is mutually
exclusive with `Vacant` over the same range. It also carries every non-runtime
Type field belonging to the resident value, including zero-layout and erased
custody.

Ordinary extent split and merge reject while `Resident<P, T>` is present: a
split cuts the one object and a merge would describe several objects as one.
Field-level extraction uses `Placed` partial-move state instead. A future
multi-object resident map requires an explicit closed algebra and is not
inferred from adjacency.

Three core operations remain distinct: `view` interprets existing content,
`initialize` encodes a newly constructed `T` into exclusive `Vacant` Stable
storage, and `validate` inspects Stable content with one structurally checked
static validator. A provider-specific adopt/open machine first establishes its
external qualification and then uses `view`; there is no generic adopt route or
cast-authorization registry. Generic initialization never expands into a
device-programming sequence.

`view` and `validate` over a non-resident range reject a `T` with represented
non-copy fields: decoding bytes cannot establish custody. Such a value comes
from initialization, an exact existing `Resident<P, T>` claim, or an admitted
provider transfer. Non-runtime Type fields are ordinary supplied inputs
regardless of whether they have bytes. Validation may prove representation and
predicates but never ownership or uniqueness.

The `Resident<P, T>` declaration has one route set shared by every
instantiation. Initialization is derived establishment from `Vacant` and an
owned `T`. `ResidentContentTransfer<P, T>` may introduce resident custody only
at an exact installed/provider-issuance occurrence carrying its receipt.
Viewing, ending a loan, and resident-preserving retirement merely borrow or
forward an existing claim; they never establish a new one.

The concrete foundation representation enforces the provider-issued owned
portion of that lifecycle before the source-visible domain is wired through.
Provider transfer seals one nonzero resident-claim identity into a dormant
Stable carrier. Activating the carrier requires a fresh nonzero placed-
occurrence identity; every derived field and primitive-access request retains
both. Resident-preserving retirement ends that occurrence and returns the same
claim and provider receipts to the dormant carrier. A later view uses a new
occurrence without reminting custody. Borrowed resident views retain the
lender's exact claim and receipts, a fresh placed occurrence, and one
whole-range shared or exclusive `ExtentLoan`; ending the view releases only
that loan. Ordinary non-resident borrowed views carry neither identity, and the
dormant carrier exposes neither field access nor a route to a bare `Extent`.

Known plan, supply, rights, and geometry incompatibility rejects compilation or
installation. Genuinely dynamic geometry, validation, or establishment-time
revision checks return ordinary cases. The instantiated core operation derives
its Type-only rejection payload from its canonical formal input row. Every
moved Type input has an explicit per-outcome disposition: embedded in the live
view, returned now, or consumed by one named authorized operation. Every borrow
is retained by the view or released. Prop terms have no custody disposition.
Missing output is never accepted as proof of consumption.

Unconditional non-runtime Type fields become ordinary by-value inputs keyed by
canonical declaration path; whether they are represented, zero-layout, or
explicitly erased does not change their origin or multiplicity. Case-dependent
Type custody requires an authored establishment machine that classifies first
and transfers the selected authority. Outcome evidence stays in the separate
proof-output lane after `;` and is never packaged with Type payloads.

Retirement is reconstructed from the successful disposition row. Borrowed
initialization constructs and destroys `T` wholly inside the exclusive borrow,
returning the lender from `Vacant` to `Vacant`. An owned complete Stable view
may destroy or move out `T` and return `Extent in Granted & Vacant`, or leave it
intact and return `Extent in Granted & Resident<P, T>`. The latter returns the
same resident-claim occurrence rather than establishing another. Ending a
borrowed resident view ends its exact loan and exposes the lender's unchanged
claim; provider-owned content returns provider custody.

Resident-content identity persists across retire/re-view cycles while each
active view has a fresh occurrence identity. Borrowed views are ordinary loans
of the exact parent claim, range, polarity, and lifetime; moving an owned range
transfers the claim into the view and resident-preserving retirement transfers
it back. A partially moved view cannot retire until its hole is restored or all
remaining content is moved out or destroyed to establish `Vacant`. In-place
migration therefore takes the whole old `T`, reaches `Vacant`, and initializes
the new `T`; an incompatible footprint requires a second range.

A crash frontier for a partial view records the exact range, resident lineage,
live/moved/vacant field paths, non-runtime custody, in-progress operation, and
provider dependencies. It establishes neither `Vacant` nor `Resident`.
Reclamation requires structural isolation plus an admitted reset, recovery,
quarantine, or custody-exit route; otherwise the resource remains abandoned.

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
public primitive accepts an arbitrary base or offset. The target-neutral
foundation specializes that carrier linearly for Stable read/take/write/swap,
External read/take/write, or one exact Atomic family and ordering; failure
returns the unchanged sealed request and its custody authority.

Stable read is available only when it cannot duplicate non-copy custody.
Stable take moves the exact represented resident field occurrence out and
leaves the placed value structurally partial; it does not claim that old bits
were cleared. Stable write requires the displaced resident to be discardable,
while Stable swap returns it. External take instead returns one
provider-provenanced whole snapshot and advances the external content version;
it introduces a conserved content root only when that snapshot is itself
content-bearing. There is no derived generic External swap, because device
state was not program-owned custody, though a provider may author an explicit
exchange protocol.

Specialization or installation rejection occurs before the physical event,
returns unchanged inputs, and emits no Terminal access row. Once an admitted
write event begins it commits; a physical access fault is a tracked crash edge,
not a recoverable `Rejected` outcome.

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
- `Swappable<T>::swap(&mut self, value)`; and
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

The current first carrier closes only the provider-coverage shape. It retains
the complete private mapping evidence plus exact subrange and the complete
admitted schema/device correspondence behind opaque contexts; compact mapping,
placement, and device IDs cannot substitute for either structure. All five
operation families remain distinct, and structural closure rejects missing,
extra, duplicate, or drifted provider assertions without consuming retry
custody. The carrier proves no provider admission and mints no device event,
publication/acquisition fact, completion, Stable view, or lowering authority;
source emission and provider/event realization remain.

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

### Implementation boundary

The source vocabulary lives in `omega::language::core::layout`; `Plan` remains
the current source name for `LayoutPlan`. `psi-access-plans` owns normalized
layout/access/profile joins, exact-loan admission, opaque field identities, and
sealed primitive requests. The compiler derives concrete `Placed<P, T>`
accessors fail-closed: inaccessible or unauthorized operations have no method,
destructive reads remain distinct from repeatable reads, and atomic accessors
retain their exact operation family. Observing decisive and single-attempt
compare-exchange additionally remain distinct in the shared ordering carrier
and permission check. The single-attempt form is not source-admitted or lowered
yet because its three-arm result carrier is absent. The outcome cases are
settled, but the public nominal result-type identities and case paths remain an
owner language-design question; checked interpretation and legacy native
lowering reject it at their entry boundaries rather than collapsing it into
decisive compare-exchange.

This section records the implementation boundary, not its history. The P2
`AccessPlan`/`Placed` and symbolic-materialization entries in
[`TASKS.md`](../../TASKS.md) own the remaining source establishment,
retirement, transfer, effect-footprint, and target-lowering work.

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

A compiler-selected implicit freestanding program entry is the admitted boot
root, so its normalized ceiling additionally permits instruction-pointer,
balanced-stack, and control-state use by checked catalog instructions. An
explicit source-selected boundary `StatePlan` remains authoritative and is
never widened by that compatibility entry rule.

The final realized artifact is validated after inlining, relaxation,
veneers/thunks, generated stubs, and admitted indirect leaves:

```text
actual_transitive_footprint subset_of StatePlan.permitted_state
actual_clobbers intersect unsaved_interrupted_state = empty
```

Checked Omega code produces checkable footprint evidence. Admitted leaves
supply accepted footprint claims under receipt. The validator, not backend
optimism, decides acceptance.

### Current footprint pipeline

Normalized footprint fragments remain bound to the exact
`BoundaryEntryPlan` through target selection, layout, and emission. Final-image
construction inventories compiler and format-owned text, binds regions to
relocated bytes, rejects overlap or gaps, and replays each supported class
against its closed target recipe and relocation envelope.

One typed certificate binds final placement, text derivation, executable-region
inventory, boundary contract, and composed implementation evidence. The current
closed region vocabulary is complete: compiler/catalog text and import thunks
are replayed, and unsupported generated or admitted executable classes are
explicitly absent by construction. Missing or mismatched evidence rejects, and
no second whole-image decoder supplies an alternate answer.

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

The executable-installation foundation implements this ladder with exact,
non-replayed carriers. Admission, materialization, freezing, validation,
installation, and retirement bind the complete artifact bytes, relocation and
proof payload, placement authority/lineage, final-byte snapshot, footprint,
audience, and provider receipts; compact fingerprints are report keys only.
Failed linear transitions return their inputs.

Materialization resolves only sealed entry/data identities into a private copy.
Installation separately validates W^X, cache/order visibility, and audience.
Retirement requires exact realization identity plus quiescence, execute removal,
restored write authority, and completion facts before returning placement.
The selected provider plan and sealed provider execution now prepare an exact
installed-entry post-handoff writer context and join it to the matching AOT
fragment. The bound invocation consumes an activated mapping and an exact
provider receipt establishing nonempty write rights, pinning, and
non-publication, writes through the opaque installed-entry context, and returns
the mapping as written but still unpublished; failures return the complete
linear input. Consumer semantic validation/publication, physical AOT
invocation, schema decoding, PCC/final-code validation, source linear
integration, and live replacement remain.

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
current loader completes visibility synchronously (it may itself suspend); a
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

The post-decode validator enforces configured bounds, checked ranges,
non-overlap, required semantic sections, informational-section isolation, and a
closed relocation vocabulary. It returns only an immutable candidate; separate
admission establishes executable qualification. The exact bytes and normalized
semantic promises feed content identity, while presentation order and
informational sections do not. Backend translation accepts only this validated
carrier and fails atomically on target or relocation mismatch.

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

The settled external-entry unit is not that scalar provider-local demand. Each
installed root supplies a normalized `EntryStackRealization`: a complete finite
set of admissible arrival contexts, each containing a finite sequence of
`Enter`, exactly one `Body`, and `Exit` epochs. An epoch records the active
stack domain, per-domain occupancy and alignment, and its nesting allowance.
Stack transitions delimit epochs. Hardware-atomic switching creates no
observable intermediate epoch; software switching does. Conditional arrival,
including privilege-dependent switching through one installed gate, produces
different sequences under different validated contexts.

For each physical or provisioned domain, composition takes the maximum over
contexts and epochs of base occupancy plus body WCSU on the body execution
domain plus phase-permitted nested demand. A nested `Interrupted` entry resolves
to the active domain of its parent epoch. It therefore follows a handler onto a
dedicated stack rather than referring forever to the stack interrupted by the
outermost root. Simultaneously live occupancy sums with alignment; sequential
roots and mutually exclusive contexts take their maximum. A finite declared
nesting depth bounds repeated occurrences. `Nestable(maximum_depth)` counts
concurrently live occurrences on one root lineage, including the current
occurrence, and zero rejects. Provider-defined depth or stack
selection must resolve to finite admitted evidence before a bounded root may be
installed, and a phase without narrower evidence conservatively inherits the
root's full nesting policy.

Architectural arrival is derived by applying one sealed target rule to the
exact validated installation facts, including entry mechanism, interrupted
regime or privilege, and switch mode. The provider cannot choose a numeric row.
Compiler-generated adapter epochs derive from the installed stub bytes. Only
opaque adapter behavior uses provider-authored byte/alignment evidence, and
that evidence remains an admitted receipt bound to the complete target, entry,
context, epoch, and domain identity. Unknown contexts, unresolved
`ProviderSelected` domains, incomplete evidence, and bare numeric assertions
reject. No architecture-specific interrupt-frame vocabulary enters Omega
source.

Emitter-derived terminal stack closures now follow the fixed-fuel trust shape:
a decoded canonical installation record is revalidated against its exact image,
then the demand binds exact installed bytes and entry before artifact-wide
nesting composition. Provider execution rechecks that binding, and the root
report distinguishes recomputable terminal evidence from an opaque provider's
admitted numeric summary. The live scalar `ProviderStackSummary` and composer
remain an implementation precursor; they do not yet represent context-indexed
epochs or the per-domain arrival/adapter join. Zero-byte internal closures
remain insufficient for root admission until that settled realization is
implemented and composed.

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
| stack | selected stack domain and provision; optional fixed policy ceiling | context-indexed per-domain entry epochs, WCSU, and composed nesting demand | target arrival rule and installation facts; emitted-stub/frame/place liveness; admitted opaque-adapter evidence |
| logical fuel | installed schedule-keyed fuel provision; optional fixed policy ceiling | composed same-schedule fuel demand | current provider summaries; eventually IR control flow, ranking bounds, callee summaries, and fixed-work proof |
| machine state | `StatePlan` permitted state and save/restore commitment | emitted transitive footprint and clobbers | instruction selection, allocation, and footprint derivation |

The ledger and its report retain each applicable policy ceiling, installed
provision, realized fact, and validation receipt. They never retain private
ranking witnesses or codegen proof internals.
Checked callback selection does not own these installed triples. Its narrower
realized-machine envelope retains one exact per-entry derivation anchor naming
all three required axes, while numeric stack demand, scheduled logical work,
and emitted machine-state footprint remain absent until their respective
Terminal, target, and backend derivations exist. The callback-placement spine
now retains and independently rejoins one exact checked receipt over that
entry anchor, then carries it through backend placement identity. This closes
identity custody only: each axis must still bind its own downstream derivation,
and neither the anchor, its receipt, nor `BoundaryEntryPlan` grants resource or
installation authority.
Sharing this record shape does not fuse the three algebras or their identity
rules: the evaluated `StatePlan` is published boundary identity, while stack and
fuel figures normally belong to candidate admission and current provisioning.
A fixed stack/fuel ceiling enters requirement identity only when policy
deliberately promises replacement without reprovisioning. Otherwise a changed
realized demand changes the candidate artifact/report and requires fresh
provisioning; it does not change the semantic requirement.

Fixed IR work is not WCET. The current contract proves that a hard root has no
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
`omega::language::core::interrupt`. `InterruptMaskGuard` is public opaque
linear boundary data whose provider-owned representation retains the exact
root, invocation, control, guard, prior-state, and masked-state identities used
at settlement; routed `Active` records valid issuance and is required by
consuming `restore`. Independent public opaque linear
`InterruptAcknowledgement` retains provider-owned root, execution, invocation,
policy, and acknowledgement identity; routed `Pending` is required by
consuming `complete`. Package code cannot inspect or reconstruct either
representation. Restoring the prior CPU interrupt mask reaches
`MachineControl`. Interrupt acknowledgement is provider-neutral: the installed
completion operation resolves one bounded abstract reach row beneath
`MachineControl + PortIo`. A legacy PIC realization publishes `PortIo`; a
LAPIC/x2APIC realization publishes `MachineControl`; an admitted realization
may publish both or the empty row when its actual mechanism warrants that
contract. The bound grants no authority and adds no Boolean choice algebra.
Linearity rejects forgotten settlement and double completion.
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
its plan, ABI, and bounded installation reach row. Installation makes its exact
qualified parameter an introduction, while an ordinary checked call treats the
same parameter as a precondition. No entry marker or authored parameter
selector is added. The
compiler now resolves the selected entry claim to the exact propagated checked
parameter fact and rejects occurrence evidence whose plan, requirement,
semantic position, domain, or carry policy drifts. The admitted occurrence also
retains the exact ABI placement selected for that semantic position, and an
out-of-range position rejects before installation. Concrete entry lowering
must still consume that admitted match before executing the checked adapter;
wiring the remaining mask-transition evidence into source `Active` facts and
the Cathedral PIC/LAPIC implementation also remain.

Entry and completion retain distinct operation rows. Equal rows neither
identify a provider nor prove that one completion may settle another provider's
debt. The installed-root receipt instead binds the exact entry, completion
operation, provider execution, acknowledgement policy, and token lineage.
The installation owner now seals that join as an interrupt-completion route and
replays the exact installed reach resolution when the provider receipt settles
the acknowledgement. Publishing the provider-selected completion row in source
remains implementation-blocked on **TOP-LEVEL-BOUNDARY-REQUIREMENTS** in
`TASKS.md`: the explicit requirement, satisfier edge, typed selection, and
token-lineage replay must land together.
Their bounded symbolic rows may propagate only inside that installation
closure. Its preselection manifest reports every row and bound; installation
substitutes each selected provider row, and final admission rejects an
unresolved row. Ordinary callable package and component contracts cannot
export these rows: they bind the provider first or publish a fixed conservative
ceiling.

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

After installation closes every bounded row, the reach stays static: a live
mask or affinity token may make a particular call locally inadmissible without
editing or masking the machine's published reach. A value that forbids
suspension is checked locally at explicit semantic suspension points; provider
selection cannot widen the bound or later erase the resolved ceiling.
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

## Ownership and acceptance

Omega owns the reusable extent, layout, access, admission, materialization,
entry-plan, artifact, and runtime-contract machinery described here. Cathedral
owns page tables, interrupt structures and policy, timer/device providers, DMA
and IPC protocols, runtime provisioning, and device schemas. Cathedral
workloads test the generic machinery; they never become compiler phases.

[`TASKS.md`](../../TASKS.md) is the sole implementation queue, acceptance
gauntlet, blocked index, and deliberately deferred list for this foundation.
