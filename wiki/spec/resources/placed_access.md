# Placed access and resident custody

[Content custody](content_custody.md) defines claim-local roles.
[Boundary realization](../terminal-psi/boundary_calls.md) defines demanded
applications and selected provider evidence.

These are language contracts, not a claim that every source operation or target
realization is implemented. [The execution board](../../../TASKS.md) tracks the
remaining placement and lowering work.

## Plans and admitted supply

`LayoutPlan` defines physical geometry. `AccessPlan` defines requested primitive
operations and exposure. A nominal build-time `Placement` policy evaluates
`plan(schema: Schema) -> PlacementPlan`, bundling layout, access, and static
boundary reach. An `Access` policy receives the same schema and validated layout;
it does not restate transfer geometry. Runtime range provenance proves that the
declared reach may access the region; it never edits a machine's reach row.

Opaque compiler-issued field keys bind the exact canonical layout with a
domain-separated strong commitment; compact layout coordinates are report-only. Each
runtime-relevant field has exactly one access decision; erased bindings retain
semantic identity but have no physical key or decision. An inaccessible default
makes omission deny access. Private bootstrap capacity does not enter source
semantics or normalized identity; overflow rejects.

| Field access | Requested operations |
| --- | --- |
| `Inaccessible` | None. |
| `Stable(read, take, write, swap, exposure)` | Independently permitted ownership-aware operations. |
| `External(read, write, exposure)` | `read` is `None`, repeatable `Read`, or destructive `Take`; write is one complete admitted transfer. |
| `Atomic(operations, exposure)` | Only the listed atomic operation families. |

Exposure is `Exported` or `BindingPrivate`. The latter restricts naming and
issuance to the placement-policy package, not range authority. Possession may
delegate public accessor requirements to generic code. Copyability,
cross-activation sharing, and counted permits independently govern duplication,
concurrency, and bounded delegation. A legitimate range holder may establish
another interpretation, but cannot name or issue another package's private
accessor.

`ResourceProfile` is immutable admitted supply over disjoint normalized
offset/length regions. Uncovered bytes support nothing. Each region carries
independent Stable, External, and Atomic capability slots plus reach:

| Supply | Alternatives |
| --- | --- |
| Stable | None, read, write, or read/write. |
| External | None, or read behavior (none/repeatable/destructive), whole-container write permission, and width/alignment transfer rules. |
| Atomic | None, or width/alignment rules with exact operation sets. |

Capabilities may coexist. Stable RAM may provide conservative External and
explicit Atomic access; MMIO normally supplies External only. Protocols such as
W1C, posted-write completion, FIFO coherence, locks, and DMA completion are not
profile fields. Constructible profile data becomes authoritative only when the
selected-provider receipt binds its complete normalized value to the qualified
range, rights, and provenance. Consumers find that receipt through qualification,
not a caller-supplied profile. Subrange profiles intersect the parent interval
and attenuate rights, reach, and operations; they never add capabilities.

## Compatibility

Plan validation checks layout, representation, legal transfer derivation,
requested operations, widths, and reach. Each transfer at field offset `o` with
alignment `a` requires `base mod a = -o mod a`. Power-of-two constraints combine
only when residues agree modulo the smaller alignment; the larger modulus wins.
Conflicting fields reject before deployment. Admission checks the remaining
base condition against actual geometry or a provider alignment guarantee.

Compatibility requires interval containment, exact transfer width, absolute
alignment, operation/reach subsets, and compatible observation. Stable supply
may satisfy conservative External demand; External cannot satisfy Stable.
Atomic requires an explicit matching rule. Reads need total decoding, stable
validation, or admitted content-validity evidence. Writes need encoding for
every admitted value or proof the concrete value fits; no silent narrowing or
compiler-invented fitting domain is allowed.

Stable exclusive bitfields may use bounded read-patch-write. External writes
cover a whole admitted container or use an explicit provider masked operation.
Atomic set/clear may use an admitted fetch, but arbitrary assignment does not
hide a retrying compare-exchange loop. A narrow External field may read one
container snapshot and project bits; a field wider than one admitted transfer
requires an authored protocol. Shadows are valid only if software owns every
bit rewritten.

A separate admitted schema-correspondence fact relates the nominal placement
to the actual provider/device, retaining its datasheet/platform provenance.
Revision checks may condition this fact on an observed ID only when observation,
grant, placement, and stable device instance agree. Compatibility does not prove
that the schema describes the physical device.

## Establishment and retirement

`Placed<P, T>` interprets one exact qualified owned extent or ordinary borrow;
it adds no authority. There is no source `ExtentLoan` or accepted-admission
object. Weakening a borrow to unqualified `&Extent` loses `Granted` and cannot
establish placement. Ordinary owned RAM remains ordinary `T` and lvalue borrows;
placed access is needed when a loan or field policy must remain visible.

| Family | Meaning |
| --- | --- |
| View | Interpret already-established content. |
| Initialize | Encode an owned `T` into exclusive `Vacant` Stable storage. |
| Validate | Inspect Stable content using one structurally checked static validator. |

Each has distinct borrowed and owned operations: rejection ends the former's
loan or returns the latter's exact extent. Provider-specific open/adopt first
establishes its external qualification, then uses the appropriate view. There
is no generic adopt, cast-authorization registry, or initialization expanded
into device programming. Validation establishes representation/predicates,
not custody or uniqueness. Non-resident view/validate reject represented
non-copy fields; those require initialization, existing resident custody, or
an admitted transfer.

`Extent::Resident<P, T>` owns one complete live `T` over the exact placement
range, including padding and transfer footprint, and every non-runtime Type
field. Its invariant type indices are semantic identity; addresses, revisions,
mapping, and claim occurrences are dynamic evidence. It cannot be weakened
away and is mutually exclusive with `Vacant`. Ordinary extent split/merge reject
resident content: object extraction uses placed partial-move state, not an
inferred multi-object algebra. `Vacant` means no established live value on that
range; it says nothing about zeroing, readability, allocation strategy, or
capacity, and cannot be inferred from the absence of an active view.

Every `Resident` instantiation shares one route set. Initialization derives it
from `Vacant` and owned `T`; `ResidentContentTransfer<P, T>` introduces it only
at an exact installed/provider-issuance occurrence with its receipt. View,
loan end, and resident-preserving retirement only forward existing custody.

Non-runtime Type fields use an ordinary authored custody record `C` and one
explicit named `C satisfies PlacementCustody<P, T>` conformance. Agreement
checks exact canonical field paths, types, multiplicities, and the evaluated
plan's represented/custody decisions. It grants no storage, content, provider,
or domain authority. Case-dependent custody requires an authored machine that
classifies first and transfers the selected authority; no ambient matching or
call-site-generated conformance is permitted.

`PlacementOutcome<View, Returned, Reason>` declares dynamic establishment
outcomes. Borrowed rejection returns custody and ends the loan; owned rejection
returns `PlacementReturn<Extent in Granted, C>`. Every moved Type input is
embedded in the view, returned, or consumed by a named authorized operation;
every borrow is retained or released. Checked `ensures` conclusions have no
custody disposition. Missing outputs cannot prove consumption. Known static
plan/supply/rights incompatibility rejects compilation or installation instead.

Borrowed initialization constructs and destroys the resident within the
exclusive borrow, restoring the lender from `Vacant` to `Vacant`. Complete
owned retirement returns either `Granted & Vacant` after destruction/move-out,
or the same `Granted & Resident<P, T>` claim intact. Borrowed resident retirement
ends only the exact loan; provider-owned content returns to its provider.
Partially moved views must restore holes or dispose of every remaining field
before retirement. In-place migration takes the whole old value, reaches
`Vacant`, then initializes the new one; incompatible footprints need another
range.

A partial-view crash frontier retains range, resident lineage, live/moved/vacant
paths, non-runtime custody, active operation, and provider dependencies. It proves
neither `Vacant` nor `Resident`. Reclamation requires structural isolation plus
admitted reset, recovery, quarantine, or custody exit; otherwise it stays abandoned.

Package-aware `Placed<P, S>` discovery resolves exact policy/schema identities
and checks both declarations' visibility and direct-dependency authority before
synthesis. Because the compiler shell is erased before ordinary type-selection
capture, both inputs must be public even for local use. The inert opaque field
carrier follows shell visibility; operation visibility follows exact
`AccessExposure`, and binding-private access stays inside the policy package.
Statement operations retain their exact generated target. Equal spellings cannot
launder private declarations through a source-free shell.

## Occurrences and views

Terminal Psi does not serialize a concrete address as authority or re-resolve
source fields, placement `P`, or resident type `T`. A `PlacedOccurrenceId`
binds the qualified root occurrence, normalized placement, provider/profile
receipt, mapping/revision, range, lifetime, and boundary reach without an address.

A separate `ResidentClaimId` names owned dormant content.

| View | Establishment | Resident-preserving retirement |
| --- | --- | --- |
| Owned | Transfer the dormant claim into one temporary placed occurrence. | Return the same dormant claim. |
| Borrowed | Establish a loan naming exact parent claim, range, polarity, and lifetime. | Release the loan; do not remint custody. |

Installation metadata describes the demanded binding; the installer must consume
the corresponding provider receipt and custody. Metadata alone is not authority.

Provider-backed Stable and Atomic resident lifecycles replay exact placement,
profile/resource, observation, origin, lineage, geometry, provenance, era, claim,
and receipt relationships. Atomic borrowed views retain the lender's authority;
exclusive borrowing adds no Atomic permission. Rejection returns the complete
inputs or active carrier unchanged.

## Projected events

Projection is pure and returns accessors, not lvalues. No public primitive takes
an arbitrary base/offset. A sealed operation authorization binds exact plan,
receipt, loan, field, geometry, observation, borrow polarity, lifetime, reach,
and atomic ordering. Specialization failure returns its unchanged custody.

Each event retains its placed occurrence, normalized `P`/`T`, canonical field
key, normalized displacement and width, logical extent, physical effect footprint,
operation family, plan receipt, and applicable lifetime, revision, reach, and
custody identities.

The backend receives the bound base and retained displacement. It must not
reevaluate layout or resolve names.

Closed access families are Stable read/take/write/swap, External read/take/write,
atomic load/store/swap, observing/non-observing decisive/single-attempt
compare-exchange, and the individual fetch families.

| Event | Effect |
| --- | --- |
| Pre-event specialization/installation rejection | Return all inputs unchanged; emit no access event. |
| Admitted write | Commit the write. A physical fault is a no-successor crash, not a program-visible write rejection. |
| Stable take | Transfer the exact resident field, leaving a structurally partial occurrence. |
| Stable swap | Return displaced custody. |
| External take | Advance the external content version and return a provider-provenanced whole snapshot. Introduce a content root only when the snapshot contract is content-bearing. |

There is no generic External swap. An authored provider exchange is a separate
operation.

Stable read cannot duplicate non-copy custody. Stable write requires discarded
content to be discardable; swap returns it. Ordinary Stable mutation requires
plan permission plus both an exclusive active borrow and exclusive source
borrow; reborrowing a shared-source view cannot upgrade authority. External
write's exclusive accessor receiver serializes that value, not the device or
all other views. Shared-source External views may perform admitted complete
transfers because Omega aliasing is distinct from external mutability.

Conflict checks use the physical footprint: reads may share; destructive reads
and read-patch-write reserve the whole container. Logically disjoint bitfields
therefore cannot acquire overlapping exclusive RMW access. A destructive read
returns one whole owned snapshot before pure field projection; it derives
`DestructiveRead`, not `Readable`. Granular accessor requirements are
`Readable::read(&self)`, `DestructiveRead::take(&mut self)`,
`Writable::write(&mut self, value)`, `Swappable::swap(&mut self, value)`, and
the exact atomic families.

External guarantees a non-elided transfer at its declared width and preserves
relative External order within one region and activation. It does not guarantee
posted-write observation or completion. See [device protocols](device_access.md).
Placed primitives require stable addressable lifetimes, finite compiler-owned
non-suspending operations, and no recoverable failure under the admitted
contract. Demand-paged/truncatable mappings, disks, streams, RPC, and device-only
storage use fallible services, not this view contract. Durability remains a
protocol. View-to-view recast is not an establishment route: request another
placement explicitly through the underlying qualified authority.

Each atomic primitive has an independent sealed core requirement shared by
ordinary atomics and placed accessors. Receivers are shared and ordering is
explicit proof-static operation data. Only exact forwarding derives wrappers
automatically; other implementations need checked proof or admitted evidence.
Every operation fits one admitted fixed width/alignment. Load needs duplication,
store permits discarding the displaced value, and swap conserves custody.
Affine/linear swaps require Stable-initialized resident ownership, not merely
provider-opened device content. Cross-activation sharing additionally requires
resident transferability. Fetch operations prove their exact raw transition for
every provider-reachable representation. Single attempts are bounded; retrying
wrappers retain their ordinary unbounded/unknown work attribution.

## Atomic compare-exchange outcomes

Observation and attempt policy are separate axes; `Once` means single-attempt,
not non-observing.

| Operation | Result type and canonical cases | Resident requirement |
| --- | --- | --- |
| `AtomicCompareExchange<T>` | `AtomicCompareExchangeOutcome<T>`: `Mismatched(observed: T)`, `Exchanged`. | Copyable `T`. |
| `AtomicCompareExchangeOnce<T>` | `AtomicCompareExchangeOnceOutcome<T>`: `Mismatched(observed: T)`, `Exchanged`, `Uncommitted(observed: T)`. | Copyable `T`. |
| `AtomicTryExchange<T, Key>` | `AtomicTryExchangeOutcome<T>`: `Mismatched(proposed: T)`, `Exchanged(displaced: T)`. | Affine or linear `T` is allowed. |
| `AtomicTryExchangeOnce<T, Key>` | `AtomicTryExchangeOnceOutcome<T>`: `Mismatched(proposed: T)`, `Exchanged(displaced: T)`, `Uncommitted(proposed: T)`. | Affine or linear `T` is allowed. |

Observing failure exposes a copy of the resident; affine/linear residents reject.
Canonical outcome tags are `Mismatched = 0`, `Exchanged = 1`, and, where present,
`Uncommitted = 2`.
The checked observing contract binds exact field and resident type, unrestricted
multiplicity, transfer width, permission identity, and the selected closed result
shape. A try-exchange-only field gains no observing permission.

For non-observing operations, the copyable key and checked raw-transition law
determine comparison without constructing another owned `T`. The law cannot
erase displaced custody: success returns it, and ordinary multiplicity determines
whether it may be discarded. Key and encoding law remain call evidence; they are
not runtime outcome parameters.

An independently checked join between a provider-backed request and a resident
contract compares the complete placement structure, field key/width, claim,
occurrence, operation axes, and result shape. It returns the unchanged non-clonable
request on rejection. This join is not an attempt, result, source call, provider
selection/installation, or Terminal/native lowering authority.

## Generic provider demand

`ResidentContentTransfer<P, T>` is one requirement schema, not an ambient provider
slot for every monomorph. Artifacts retain concrete applications and export
symbolic applications. Final composition substitutes reachable arguments, derives
closed demand, and verifies the selected realization for every application.
Installation binds the resulting exact occurrences.

Distinct provider slots are justified only when applications require independent
selection. Provider-asserted generic/family coverage and indexed arity/string
schemas do not establish a realization. Missing checked application coverage
rejects; it grants no admission, resident custody, transfer, or installation authority.
