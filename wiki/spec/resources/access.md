# Access policies, views, and resident custody

Named decision: `access-policy-vocabulary`, settled 2026-10-30.

[Content custody](content_custody.md) defines claim-local roles.
[Boundary realization](../terminal-psi/boundary_calls.md) defines demanded
applications and selected provider evidence.

## Plans and admitted supply

`LayoutPlan` defines representation and physical geometry, with no access rights.
`AccessPlan { layout, fields, reach }` contains one `LayoutPlan`, per-field
operation/exposure decisions, and required static boundary reach. An access plan
contains a layout plan; a layout plan carries no access policy. `FieldEntry` uses
`representation: FieldPlan`: the alternatives describe offsets, bit fragments,
integer representations, and wire encodings, not merely positions.

An ordinary named implementation of `AccessPolicy` supplies
`plan(schema: Schema) -> AccessPlan`. Its complete checked execution closure is
effect-free and terminating under compile-time admission: no boundary services,
suspension, blocking, or unadmitted linear carriers. It may call ordinary layout
policies and helpers; there is no separate two-argument composition trait. Such a
call returns plan data, not a validation certificate. The final validator
independently establishes layout, field coverage, operation, and reach obligations;
it trusts no intermediate calculation or asserted property of the policy result.
Runtime range provenance proves that the declared reach may access the region;
constructing a plan grants no reach and never edits a machine's reach row.

`AccessPlan::inaccessible(layout)` retains the supplied layout, starts with empty
required reach, and seeds one inaccessible decision for each distinct schema-field
key appearing in its entries. Several fragments of one field seed one decision.
Erased bindings have no physical entries and seed none. `AccessPlan::with` changes
the decision for an existing key; it does not authorize an unknown field. Final
validation independently requires exactly one decision for every runtime-relevant
schema field, no decisions for erased or foreign fields, and valid complete layout
coverage. An omitted field cannot become valid merely because both the layout and
the seeded decisions omitted it. Access decisions use schema-field identity, not the
layout entry count or entry positions.

`Access<P, T>`, `Accessor<T>`, `AccessPolicy`, `AccessPlan`, and `LayoutPlan` are
core names, not language keywords. Ordinary name resolution and import rules apply;
there is no special local-wins rule for a user declaration named `Access`.

Ordinary `&T`, `&mut T`, and `&write T` loans, together with domain-owned access
restrictions, suffice when the whole-value loan and ordinary projections express
the intended operations. Access policies express field-specific operations,
exposure, and observation behavior. Access or mutation outside ordinary Omega
loan discipline requires the appropriate external/atomic backing contract; it
does not justify manufacturing ordinary readable or mutable references. Another
cooperating Omega thread alone does not require an access policy.

There are two sources of backing authority:

- Ordinary RAM views retain the lender's existing loan and compatible Stable
  supply. They need no provider admission and cannot strengthen that loan.
- Boundary-backed views use exact admitted provider evidence: `Granted` from
  issuance, `ResourceProfile` from supply, and, for a device interpretation,
  schema correspondence with its datasheet/platform provenance. Build accepts
  the boundary requirements and selects their providers; package acceptance
  records those trust decisions. A boundary may supply an established view or
  qualified backing from which the view is established under these obligations.

Neither route dispenses with content validity, resident custody, lifetime, or
operation-specific obligations. Policy evaluation and the demand/supply join are
checked against the evidence; constructing their data cannot establish it.

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
issuance to the access-policy package, not range authority. Possession may
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

A separate admitted schema-correspondence fact relates the nominal access policy
to the actual provider/device, retaining its datasheet/platform provenance.
Revision checks may condition this fact on an observed ID only when observation,
grant, access plan, and stable device instance agree. Compatibility does not prove
that the schema describes the physical device.

## Establishment and retirement

`Access<P, T>` interprets one exact qualified owned extent or ordinary borrow;
it adds no authority. There is no source `ExtentLoan` or accepted-admission
object. Weakening a borrow to unqualified `&Extent` loses `Granted` and cannot
establish an access view. Ordinary owned RAM remains ordinary `T` and lvalue
borrows; access views retain the selected field policy and exact source authority.

The ordinary-RAM routes are `Access::view<P, T>(&T)` and
`Access::view_mut<P, T>(&mut T)`. Existing resident backing uses
`Access::view_resident_mut<P, T>(&mut Extent in Granted & Resident<P, T>)`.
These routes preserve their distinct source polarity and custody contracts;
they are not an overloaded `over` operation. The whole handle is `Access<P, T>`;
field projection returns `Accessor<FieldType>`, not an unrestricted field loan.
Even `&mut Access<P, T>` cannot enable a field operation the policy denies.
There is no unwrap to unrestricted `&mut T`. An ordinary lender resumes its
original access when the view's loan ends; attenuation does not permanently
restrict the lender's ownership.

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

The static validator's complete structural machine contract includes input and
result, guarded guarantees, effects, crashes, and applicable termination.
Specialization invokes that exact machine directly; it is not a retained foreign
callback. Its identity and derivation enter occurrence provenance. Dynamic
validation rejection preserves the validator's declared error sum as ordinary
typed data, never an opaque number, string, or generic `InvalidContent` code.

`Extent::Resident<P, T>` owns one complete live `T` over the exact placement
range, including padding and transfer footprint, and every non-runtime Type
field. Its invariant type indices are semantic identity; addresses, revisions,
mapping, and claim occurrences are dynamic evidence. It cannot be weakened
away and is mutually exclusive with `Vacant`. Ordinary extent split/merge reject
resident content: object extraction uses access-view partial-move state, not an
inferred multi-object algebra. `Vacant` means no established live value on that
range; it says nothing about zeroing, readability, allocation strategy, or
capacity, and cannot be inferred from the absence of an active view.

These protected qualifications must declare their authorized establishment
routes under [ordinary domain rules](../language/domains.md#declaration-and-membership).
`Extent` being linear supplies no implicit route restriction, and neither the
names `Vacant`/`Resident` nor `Granted`'s routes protect a separate empty domain.
A bare `domain Extent::Vacant;` would be freely introducible and would not express
the required custody contract. The core declarations and checked operations must
encode that contract rather than relying on a carrier-wide exception.

Every `Resident` instantiation shares one route set. Initialization derives it
from `Vacant` and owned `T`; `ResidentContentTransfer<P, T>` introduces it only
at an exact installed/provider-issuance occurrence with its receipt. View,
loan end, and resident-preserving retirement only forward existing custody.
`Vacant` establishment likewise requires an authorized operation establishing
the absence of live content, or transfer of existing vacancy evidence through a
checked operation. Route authorization alone proves neither absence of content
nor possession of a resident; predicates and custody obligations remain separate.

Non-runtime Type fields use an ordinary authored custody record `C` and one
explicit named implementation of `AccessCustody<C, P, T>`. Agreement
checks exact canonical field paths, types, multiplicities, and the evaluated
plan's represented/custody decisions. It grants no storage, content, provider,
or domain authority. Case-dependent custody requires an authored machine that
classifies first and transfers the selected authority; no ambient matching or
call-site-generated conformance is permitted.

`AccessOutcome<View, Returned, Reason>` declares dynamic establishment
outcomes. Borrowed rejection returns custody and ends the loan; owned rejection
returns `AccessReturn<Extent in Granted, C>`. Every moved Type input is
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

Package-aware `Access<P, S>` discovery resolves exact policy/schema identities
and checks both declarations' visibility and direct-dependency authority before
synthesis. Because the compiler shell is erased before ordinary type-selection
capture, both inputs must be public even for local use. The inert opaque field
carrier follows shell visibility; operation visibility follows exact
`AccessExposure`, and binding-private access stays inside the policy package.
Statement operations retain their exact generated target. Equal spellings cannot
launder private declarations through a source-free shell.

## Occurrences and views

Terminal Psi does not serialize a concrete address as authority or re-resolve
source fields, access policy `P`, or resident type `T`. An `AccessOccurrenceId`
binds the qualified root occurrence, normalized access plan, provider/profile
receipt, mapping/revision, range, lifetime, and boundary reach without an address.

`TerminalAccessViewPlan` retains the composite plan for its consumers. These
semantic identifiers do not prescribe a new wire tag or digest merely because
an implementation identifier is renamed; the encoding contract specifies the
serialized fields and canonical commitments independently.

A separate `ResidentClaimId` names owned dormant content.

| View | Establishment | Resident-preserving retirement |
| --- | --- | --- |
| Owned | Transfer the dormant claim into one temporary access occurrence. | Return the same dormant claim. |
| Borrowed | Establish a loan naming exact parent claim, range, polarity, and lifetime. | Release the loan; do not remint custody. |

Installation metadata describes the demanded binding; the installer must consume
the corresponding provider receipt and custody. Metadata alone is not authority.

Provider-backed Stable and Atomic resident lifecycles replay exact access plan,
profile/resource, observation, origin, lineage, geometry, provenance, era, claim,
and receipt relationships. Atomic borrowed views retain the lender's authority;
exclusive borrowing adds no Atomic permission. Rejection returns the complete
inputs or active carrier unchanged.

## Projected events

Projection is pure and returns `Accessor<T>`, not lvalues. `read`, `write`,
`take`, `swap`, and the atomic families retain their exact operation requirements;
the accessor's value type alone grants none of them. No public primitive takes
an arbitrary base/offset. A sealed operation authorization binds exact plan,
receipt, loan, field, geometry, observation, borrow polarity, lifetime, reach,
and atomic ordering. Specialization failure returns its unchanged custody.

Each event retains its access occurrence, normalized `P`/`T`, canonical field
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
write's exclusive accessor parameter serializes that value, not the device or
all other views. Shared-source External views may perform admitted complete
transfers because Omega aliasing is distinct from external mutability.

Conflict checks use the physical footprint: reads may share; destructive reads
and read-patch-write reserve the whole container. Logically disjoint bitfields
therefore cannot acquire overlapping exclusive RMW access. A destructive read
returns one whole owned snapshot before pure field projection; it derives
`DestructiveRead`, not `Readable`. Granular accessor requirements take explicit
ordinary parameters: `Readable<Accessor, Value>::read(accessor: &Accessor)`,
`DestructiveRead<Accessor, Value>::take(accessor: &mut Accessor)`, and mutable
accessor parameters followed by the value for `Writable` and `Swappable`.
`Accessor` and `Value` are ordinary declared trait parameters. Named
implementations supply these operations; accessor data does not satisfy a
trait. The exact atomic families likewise name their argument types explicitly.

External guarantees a non-elided transfer at its declared width and preserves
relative External order within one region and activation. It does not guarantee
posted-write observation or completion. See [device protocols](device_access.md).
Access primitives require stable addressable lifetimes, finite compiler-owned
non-suspending operations, and no recoverable failure under the admitted
contract. Demand-paged/truncatable mappings, disks, streams, RPC, and device-only
storage use fallible services, not this view contract. Durability remains a
protocol. A live view retains a lease or claim preventing asynchronous revocation
until retirement; a revocable mapping requires a distinct fallible provider
protocol. View-to-view recast is not an establishment route: request another
access view explicitly through the underlying qualified authority.

Each atomic primitive has an independent sealed core requirement shared by
ordinary atomics and accessors. Subject parameters are shared and ordering is
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
contract compares the complete access-plan structure, field key/width, claim,
occurrence, operation axes, and result shape. It returns the unchanged non-clonable
request on rejection. This join is not an attempt, result, source call, provider
selection/installation, or Terminal/native lowering authority.

## Ordinary backing initialization

[Deferred-establishment recasts](../layouts/recasts.md#deferred-establishment-on-exclusive-storage-casts)
provide an ordinary exclusive-storage route to initializing a stricter shape.
They do not recast `Access<P, T>` or bypass its access plan. Only a legitimate
underlying storage claim with the required geometry, authority and custody can
support that ordinary route. Opening a pending view does not mint `Resident`,
discard occupied content or infer `Vacant`; the declared content-establishment
and conservation obligations still apply. Explicit access `Initialize` remains
the route for encoding into access-policy-governed storage.

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
