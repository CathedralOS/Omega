# Design Brief: Authority Values And Boundary Evidence

Status: semantic direction updated 2026-08-20. The core `Extent` declaration,
owner-authored root requirement, state-local constrained-parameter evidence
boundary, canonical compiler-owned `IntervalSet` content algebra, Cathedral's
first admitted `Granted` root, and ordinary interrupt
obligation carriers are live; further provider, carry, resource-frontier, and
artifact work remains staged in `TASKS.md`.

The target-neutral content ledger represents both permitted fresh root origins:
selected provider issuance with its exact invocation and supply/custody record,
and a program-local introduction at one statically enumerable installed root
position. The domain owns the authorized requirement in both cases. The route
contract fixes the program-local capacity per occurrence, while installation
fixes the occurrence count, scope, and lifecycle epoch. There is no separate
provision declaration or ambient minting route.

## Purpose

Omega represents runtime authority with ordinary data plus compiler-tracked
qualification and provenance. The data carries the runtime information an
operation needs. Domain membership states what authority, validation,
interpretation, or historical fact has been established for that data.

For example, a concrete range authority has ordinary runtime geometry:

```omega
pub data Extent [linear] {
    base: addr;
    length: u64;
}
```

The fields describe a range. Membership in `Extent::Granted` states that the
range descends from a live admitted or checked authority claim. Reconstructing
the same fields creates another value but does not reproduce that membership.
Publishing this structural carrier is therefore deliberate: callers may name
and construct geometry, including geometry offered to the public root-provider
requirement, while only the selected admitted provider can establish
`Granted`. The record literal is not an authority mint.

Multiplicity belongs to the data type. `Extent` is linear because every owned
range must eventually be discharged. Its outstanding provenance cannot be
cast away; it must be consumed or transferred. Operations that release, split,
map, or otherwise consume authority require the relevant domain membership.

## Qualification evidence

A domain declares predicate obligations in `requires` and exact authorized
establishment requirements in `established by`. `Extent::Granted` requires
geometry
that fits the target address space and names its boundary root directly:

```omega
pub domain Extent::Granted
    requires no_wrap(self.base, self.length)
    established by ExtentRootProvider::grant;
```

The clause does not call `grant`; it authorizes that exact requirement to
originate the domain at its exact qualified subjects. Here the subject is the
result. A selected provider satisfies the requirement, and admission records
its evidence. A third party cannot create a look-alike trait or machine to
establish `Granted`.

Route entries are signature-free requirement references, so each path must
resolve to one exact overload. Ambiguity rejects without consulting visible or
selected satisfiers. The same rule governs nominal static-machine callback
binders and every other signature-free requirement reference. Adding an
overload to an existing requirement name is consequently a breaking change for
distant establishment clauses and binders as well as local callers;
compatibility reporting surfaces that at the requirement declaration.
`as Name` remains an exact-edge satisfier-set label or a complete-conformance
selector according to its grammar position; it is never an overload selector.

Predicate-only membership is established by proof, including proof guaranteed
by a checked validator or accepted under an admitted receipt. Routed
membership is established by an authorized checked conformance, propagation,
checked transformation, or admitted boundary conformance. When one domain has
both forms, its predicates are proved at the exact established subject.

Core's first live authority root uses this exact shape:

```omega
pub boundary trait ExtentRootProvider {
    machine grant(root: Extent) -> Extent::Granted
    ensures
        result in Extent::Granted;
}
```

A checked adapter may realize the requirement, but its ordinary direct-call
surface does not mint `Granted`. Semantic checking consumes the boundary
requirement and admitted receipt first; only afterward does execution dispatch
rewrite the selected trait slot to that adapter.

The compiler therefore does not treat an unlisted `boundary machine` guarantee
as domain evidence merely because the machine is accepted. An admitted
membership guarantee must be inherited from a boundary requirement, must spell
the bare `result` or an exact non-`self` parameter as its subject, and that
subject's carrier must match the domain target. A parameter route originates
only at an installed external-root occurrence; the same signature at an
ordinary call remains a precondition. Checked proof facts retain the
authorizing boundary trait, exact requirement signature, subject position, and
installation evidence; the qualification-evidence artifact publishes them
with the selected provider-plan evidence.

This keeps crossing semantics on machines and requirements. Internal checked
operations may still transfer the same fact, subject to resource-frontier
validation.

## Evidence and guarantees

A qualified result type or `ensures` clause is an obligation on a checked
implementation. It becomes available to callers only when the implementation
does one of the following:

- proves the domain's predicate requirements from visible propositions;
- receives the fact from a guard, parameter, or checked callee guarantee;
- validates runtime input through an ordinary checked machine;
- transfers an existing resource claim through a checked transformation; or
- crosses an admitted boundary satisfying a requirement named by the domain
  whose receipt supplies the fact.

These evidence sources feed one membership judgment while retaining their own
validation rules. Arithmetic proof, resource transfer, and accepted provider
evidence are not interchangeable.

Domain predicates use ordinary declaration visibility. Public predicates let
consumers discharge their propositions directly. Predicates not visible
outside the package are established externally through checked validators and
guarantees. A routed qualification depends on its exact authorized
conformance, existing evidence, checked resource transformations, or admitted
boundary receipts.

## Root authority

Root authority bottoms out at an admitted crossing. A platform memory provider
may return `Extent::Granted` together with address-space and rights
facts through the memory requirement it satisfies. Omega cannot prove that
firmware, a hypervisor, or a host OS transferred those ranges; provider
admission supplies that evidence and records its scope.

The root value carries runtime geometry when the platform discovers that
geometry at runtime. Domain membership itself adds no runtime tag.

A content-bearing root crosses three distinct checks. An ordinary fingerprinted
postcondition states its per-invocation **geometry** over parameters and result
paths. The selected provider separately attests **fresh issuance** from backing
under its custody: every newly owned exclusive claim is separated from every
other live exclusive issuance, while intentional aliases carry an explicit
shared-view authority instead. Finally, providers that issue claims over the
same stable backing identity must derive their custody from a common root; a
set of individually honest provider ledgers does not establish cross-provider
separation.

No source-visible backing-receipt binder exists. Parameters, structural places
at their callable-entry revision, and result paths are the contract subjects.
The static requirement fingerprints the normalized algebra expression; each
invocation substitutes its actual values. For multiple content-bearing results,
one n-ary separated relation bounds all newly established result claims together. Content
transferred from input claims remains ordinary conservation and is not counted
again as new boundary supply.

Geometry and external supply retain different trust. A checked adapter may
derive that a returned length is at most an input count or that a returned
interval has the promised size. A result-selected base can establish that
geometry but cannot prove the range is new. External ownership, fresh issuance,
and the provider's correspondence to physical reality remain admitted facts
with exact provider provenance. If a checked runtime validation can reject, its
boundary signature must publish an `Outcome` path or an explicit trap effect;
the compiler never invents a hidden rejection edge. A provider merely violating
an admitted assertion is a trust violation, not a failed runtime check.

Provider plans and invocation evidence retain the normalized geometry theorem,
backing identity, issuer, live-issuance premise, custody lineage, alias class,
and trust provenance. Ordinary record construction can reproduce geometry but
cannot establish any of those authority facts.

A provider-neutral grant may be useful when platform admission and a portable
library need a separate handoff. Such a handoff is a resource transformation,
not a prerequisite for the root model: the admitted provider may return the
qualified authority directly.

## Checked resource transformations

Internal operations conserve authority through a generic claim frontier.
A normalized transformation records:

- which input claims are consumed;
- which output claims inherit each origin;
- how claims decompose across result fields or cases;
- which claims are discharged; and
- which claims remain borrowed or retained.

Every establishment creates a fresh claim identity. Its current canonical path,
root lineage, projected content, permissions, and carry policy are independent
metadata. Moving a path moves its claim subtree; aggregate construction nests
claims under field/case/index paths; destructuring performs the inverse.
One-to-one and otherwise unambiguous outcome mappings infer. Ambiguous mappings
reject rather than guessing.

Terminal Psi names an entry claim independently of any output equality. One
machine-local semantic binding records its dense claim identity, projection,
algebra, and entry structural place. Partition theorems and later content
axioms reference that binding. An identity-reshuffle row is emitted only for a
separate one-to-one input/output equality; it is never required merely to name
a partition input.

Current terminal production retains dense entry-claim bindings, exact
one-to-one identity reshuffles, and direct authored partition substitutions.
It independently replays every substitution and rejects ambiguous claim paths,
theorem-shape or algebra drift, and staged derivations not represented by the
current vocabulary. Content projection and conservation FNV values are report/
cache coordinates only: semantic authority remains the exact owner definition,
algebra, structural places, substitution, producer call, and verifier replay.
Compact-equal structural substitution therefore rejects. Sealed introduction
and custody-exit frontier rows remain.

Repository architecture validation scans exported Rust `u64` fingerprint
fields across Psi and Omega. Explicit report/cache/compatibility vocabulary is
accepted directly; every remaining legacy field is held to a shrinking
path-and-count ceiling, so new unclassified fields and duplicate occurrences
fail before they can become an accidental authority convention.

Checked machine-contract carriers apply that rule end to end. Their compact
FNV values are named contract report fingerprints beside the canonical
domain-separated `MachineContractCommitment`. Replay rejects an empty strong
commitment, and a boundary row cannot authorize itself with a locally stored
digest when the canonical checked contract plan or crash capsule is missing.

### Content-bearing claims

A content-bearing exact qualification publishes one owner-unique conformance
to the core `Content<A>` projection requirement. Having that conformance marks
the qualification as content-bearing, `A` selects one compiler-owned partial
composition algebra, and the conforming machine projects the qualified subject
into it. Only the qualification owner may publish the conformance, and one
qualification has at most one projection identity. A second package cannot
reinterpret the same authority through conformance selection.

Schematically, the `Granted` projection has this shape:

```omega
machine Granted::content(e: &Extent) -> IntervalSet<PhysicalMemory>
    satisfies Content<IntervalSet<PhysicalMemory>>::project
{
    IntervalSet::singleton(
        embed(e.base) as Nat,
        (embed(e.base) + embed(e.length)) as Nat
    )
}
```

The projection body belongs to the closed **content-projection fragment**. It
may read fields of the subject, embed runtime scalars into proof-level
mathematics, perform proof-defined closed arithmetic, and apply constructors
of the selected algebra. Branches, loops, allocation, effects, hidden state,
and arbitrary helper calls reject at the conformance. The compiler must reduce
the body to one canonical symbolic expression; that normalized expression and
its coordinate-space or quantity identity are fingerprinted as semantic
interface identity.

The initial closed algebras are:

- `IntervalSet<CoordinateSpace>`, a canonical finite set of disjoint half-open
  intervals with proof-level `Nat` bounds and one normalized coordinate-space
  identity; and
- `CountedQuantity` with a proof-level `Nat` magnitude and one normalized unit
  identity.

`IntervalSet` rather than a single interval is the algebra carrier because
separated composition and residual difference are not closed over single
intervals. Its unique canonical form sorts by lower endpoint, removes empty
members, merges adjacency, and stores one representation of the empty set.
`separate(...)` rejects overlap and otherwise produces canonical union.
`residual(whole, kept)` requires `kept` contained in `whole` and produces the
canonical set difference, which may contain several intervals. Equality is
structural equality of those canonical forms.

Terminal Psi carries canonical content equations over declared structural
roots, stable domain and projection identities, entry/current paths, fields,
fixed indices, sum cases, and flat separation. Canonical encoding and
independent verification retain no source-arena identity.

An address interval-set member uses embedded proof `Int` arithmetic rather than
wrapping runtime `addr` arithmetic, then converts the proven-nonnegative bounds
into the algebra's proof `Nat` coordinates. The source-carrier range facts
discharge those conversions. Its half-open end may equal the address-space bound
even when that one-past value is not representable as `addr`.
`no_wrap(base, length)` therefore means that the embedded sum does not exceed
the target address-space bound, and every route establishing `Granted` proves
that predicate. Separated interval-set composition requires the same coordinate
space and nonoverlap. Exact equality with a parent rejects omitted gaps; equal
numbers in different spaces never compose.

The proposition is transparent source mathematics:

```omega
pub proposition no_wrap(base: addr, length: u64) =
    embed(base) + embed(length) <= addr::Bound;
```

`addr::Bound` is the selected target's exclusive one-past address bound as
proof `Int`. Proving this formula establishes geometry only. `Granted` also has
`established by` routes, so the same proof cannot create range authority or
provider lineage. Core's current `pub boundary machine no_wrap(...) -> bool`
declaration is transitional bootstrap spelling until the typed target-capsule
projection lands; it has no executable meaning.

`CountedQuantity` has its first concrete customer in bounded bump/arena
residual capacity. Allocation consumes normalized payload size, alignment
padding, and allocator metadata, while split and return conserve the quantity.
It models a divisible pool of units, not an obligation to deliver one value to
exactly `n` destinations. Count alone does not prove placement in a fragmented
heap, which remains fallible or requires an exact free-extent or reservation
theorem.

`CountedQuantity` proves fungible magnitude only. It never equates claim
identities. Whole-claim identity, root lineage, custody, and any separately
modeled slot/address/handle identity remain independent. If an operation
creates or selects identity-specific authority for the same conserved units,
a quantity theorem alone cannot discharge it; the operation needs an
identity-bearing or joint correspondence algebra. The compiler can reject a
modeled mismatch, but cannot discover identity that the program never modeled.
Artifacts therefore report the selected algebra and whether unit identity is
covered or remains outside the content projection.

Ordinary claims publish no content projection. Whole-claim identity, custody,
transfer, and cleanup remain in the frontier, so file handles and other
nondecomposable linear claims are already fully accounted for. Content is
supplementary fine-grained accounting, not another spelling of linearity.

The compiler owns algebra normalization, containment, equality, and partial
separated composition. New algebra kinds require a compiler release and a
concrete customer; arbitrary owner-defined composition is not authority
evidence. Fractional permissions are deliberately absent. Ordinary `&`,
`&write`, and `&mut` express shared-read, exclusive-write-only, and
exclusive-read/write loans without creating fractions in an authority algebra.
`&write` attenuates what may be observed through one exclusive loan; it does
not divide or mint the underlying permission. Root lineage remains an
additional authority-family check rather than part of numeric geometry.

Backing geometry and transformation correspondence use ordinary machine
postconditions over the exact owner-unique projection machine. There is no
`content(value)` intrinsic: a contract names `Granted::content(&value)` or the
corresponding exact machine for another qualification. This makes projection
selection explicit when a carrier has several independent content-bearing
claims.

Two conservation-specific proof-only operations complete this source surface:

- `old(place)` denotes the callable-entry revision of a parameter, `self`, or
  one of their structural places; it is not executable, does not copy an owned
  value, and initially is not a modality over arbitrary propositions; and
- `separate(a, b, ...)` applies the selected closed algebra's partial n-ary
  composition and generates compatibility and separation obligations.

Proof operations compose around revisioned places. A content split may
therefore state:

```omega
ensures
    Granted::content(old(&whole))
    == separate(
        Granted::content(&result.left),
        Granted::content(&result.right),
    );
```

Terminal Psi represents `old` through the existing revision on the structural-
place term (`CallableEntry | Current`), not as variants of every proposition
node. The revision identity is shared with scoped facts and borrow certificates;
`old` does not create a parallel history model.
`separate` is a compiler-owned proof intrinsic over the closed algebra
vocabulary. Both use ordinary call-shaped source spelling, erase completely,
and cannot be implemented or overridden by packages.

An establishment postcondition may similarly bound
`Granted::content(&result)` by an algebra expression over parameters and result
paths. Such postconditions relate already-established evidence; they cannot
make an ordinary record authoritative. Clear identity-preserving outcome maps
infer from the frontier. An ambiguous mapping or partition change requires an
explicit postcondition or rejects.

One projection is load-bearing in four places:

1. establishment proves the exact projection of a value is within checked or
   admitted backing;
2. every authority-bearing access proves its touched footprint is within
   that exact projection;
3. transformations conserve the separated composition of all consumed and
   produced content; and
4. a custody-exit frontier row accounts for every remainder through an exact
   authorized terminal route.

Only an authorized establishment route introduces new content, and only an
ordinary terminal claim consumer authorized by its contract transfers content
out of the checked custody frontier.

Each root occurrence's origin is a property of its authority source, not of a
nominal data declaration, content denominator, constructor name, or concrete
requirement implementation. Every fresh internal account records one exact
authorized route, subject position, capacity, lineage, qualification, source,
installation scope, and lifecycle epoch:

- one statically enumerable installed parameter position may introduce a
  program-local root, such as an artifact-instance parser budget or protocol
  session pool; and
- a selected admitted issuance establishes a provider-backed root, such as
  physical memory or device slots.

The domain declaration remains the sole source-level authorization. It names an
exact trait requirement, not every future satisfier. A later package may supply
a checked implementation when visibility permits, but implementing or calling
that requirement never creates a root. At an ordinary call the qualified
parameter is a precondition and must carry an existing lineage. Introduction
occurs only at the exact installed root position, where the generated bridge and
installation evidence identify the occurrence. A checked runtime establishment
event may otherwise qualify, transfer, lease, split, recombine, or expose an
existing account; a result route with no parent lineage rejects as an attempted
mint.

The verifier classifies authority flow per exact claim occurrence and algebra
row, never once for an entire operation. The closed classifications are:

- introduction: no parent claim, with an exact authorized installed or
  provider-issuance occurrence and receipt;
- identity forwarding: one exact parent and the same claim occurrence in the
  result;
- derived transformation: one or more parents plus the checked theorem
  relating their exact content to the result;
- custody exit or consumption: an input with no checked result names its exact
  authorized sink; and
- loan: a non-owning edge names its exact parent claim, projected range,
  polarity, and lifetime.

The classification is semantic rather than an artifact-boundary heuristic.
Moving an adapter across an artifact boundary cannot turn forwarding into
minting or the reverse. One operation may contain several rows: a device
read-into-buffer can forward the buffer extent, introduce provider-originated
content, derive its resident relation, and consume a completion token. Each row
must independently close; an operation-wide label cannot stand in for them.

The route contract publishes one exact finite per-occurrence content expression,
or an owner-constrained family whose selected instance reduces to one. The
portable verifier reconstructs that introduction schema from the requirement,
qualification, and content projection. Terminal Psi retains the owner-unique
projection independently on the domain declaration; route schemas and content
claims must replay that exact identity, algebra, and normalized expression.
Changing a producer expression together with its locally recomputed fingerprint
cannot redefine the owner's content denominator. Installation verification joins it to
the exact finite slot cardinality and derives the aggregate for one installed
artifact instance and lifecycle epoch. A producer-authored manifest total has
no authority. Cathedral composes those verified aggregates across concurrently
live artifacts and replacement eras; coexistence is charged at peak, not at
steady state.

The composition input is an opaque aggregate snapshot projected from the
sealed cohort or its runtime. It retains the exact installed-slot closure,
lifecycle-qualified cohort identity, occurrence roster, algebra, and symbolic
per-occurrence expression while carrying no authority. A live report accepts
exactly one snapshot for every era in the authoritative lifecycle roster and
rejects stale, missing, duplicate, or substituted contributors. Omega does not
collapse the rows to one scalar; Cathedral applies deployment policy to the
preserved exact demands.

These scopes are deliberate. Four installed workers may consume four times a
per-worker capacity. A component that promises one shared cap instead receives
children split from one aggregate parent root, so conservation rejects another
child without additional supply. That parent itself is bounded per installed
assembly instance and epoch. A fresh root in a later epoch is a new budget, not
recovered lifetime capacity; a cross-epoch or machine-lifetime ceiling requires
persistent authority carried across the replacement boundary.

`PhysicalMemory` and other algebra parameters remain pure proof-level
vocabulary. Arbitrary proof code may construct an
`IntervalSet<PhysicalMemory>` value; doing so grants no authority. Two claims
using the same denominator compose arithmetically, but conserved authority
composes only through compatible root lineage, exact qualification, and backing
evidence. Program-local and provider-backed roots may therefore share a
denominator without becoming interchangeable.

Relating a local root to external reality requires a separate admitted
correspondence. That correspondence states the unit mapping and external
capacity, while the compiler checks known containment arithmetic. Multiple
pools that spend one hardware capacity must derive by separated split or lease
from the same provider-issued root; independently generated local pools cannot
each discharge the hardware bound.

Every content-capable root has one internal canonical algebra account even
when source exposes no `Content<A>` projection for it. Checked establishment
may qualify or project content only by charging an existing account for the
duration of a transfer or lease. It never creates a fresh runtime root. A fresh
program-local root exists only at a verifier-recognized installed introduction
position with exact finite capacity and cardinality; an external root exists
only at selected admitted issuance. Checked sub-allocators transform existing
content, while externally rooted conduits require admitted backing identity,
fresh issuance, and custody evidence.

The installation gate is itself exact and one-shot. The canonical installed
root ledger seals the target-required slot closure against full installed-root
evidence, then burns issuance of one cohort verifier. That verifier atomically
closes every eligible prebinding under one lifecycle ledger and epoch and
returns every lease on failure. Only the resulting non-clonable cohort may feed
the later runtime establishment transition; a closure, prebinding, mutable
count, compact identity, or individually acquired lease cannot do so.

The real source-to-verified-Terminal installation canary now exercises that
transaction across an epoch change. Two epoch-10 members become stale when
epoch 20 publishes; sealing returns the exact source-derived prebindings, root
borrows, and leases without consuming their schemas. After releasing those
holds, fresh epoch-20 leases seal and complete the handoff. Exactly two lineage
accounts are established, and both artifact audit origins retain the original
schema identity with epoch 20. The test does not mint replacement schemas or
recover lifetime capacity from the retired epoch.

The complementary one-root canary now proves the cardinality-one boundary
without borrowing facts from the two-root bridge. One source producer schema
becomes one installed prebinding, cohort member, aggregate, coexistence row,
account, lineage, and audit origin. Terminal-artifact, lifecycle-ledger, and
materialization-plan substitution each return their exact custody for retry;
none can introduce a second schema or lineage.

The finite installed-instance canary then reuses one real source-derived
schema and verified Terminal catalog for two distinct installations of the
same artifact. Their snapshots compose only as two lifecycle-qualified,
unreduced rows: each retains cardinality one and the common symbolic capacity,
while its occurrence roster names the exact installed-code instance. Omitting
one live instance or repeating the other rejects. This is distinct from a
cardinality-two cohort, which requires two authentic target-owned slot
occurrences rather than test-authored slot authority.

The selected provider schema retains the canonical carrier of each routed
entry claim separately from both the complete qualified parameter type and the
domain identity. Installation joins that carrier directly to the verified
Terminal producer catalog; it never parses a display type or substitutes a
short source spelling such as `Extent` for `named(name(Extent))`.

The Rust product implementation consumes that cohort into one non-clonable epoch runtime. The
runtime retains every still-dormant occurrence instead of distributing loose
mint tokens. A generated installed-entry bridge supplies a single-use subject
binding naming the exact installed root, physical and semantic parameter
positions, qualification, carrier, invocation, and runtime place, together
with the checked proof-natural observations required by the reconstructed
capacity expression. Establishment replays the complete observation key set,
evaluates each occurrence independently, rechecks the current lifecycle lease,
and removes the matching occurrence only at the commit point. Rejection returns
the subject binding and leaves the occurrence dormant. Success produces one
non-clonable account retaining the full occurrence, exact evaluated content,
and lifecycle hold; its copyable lineage identity is reporting data only.
Interval and subject-dependent expressions are never replaced by aggregate
scalar multiplication.

Any operation that realizes content against an external substrate must name an
exact qualified root and carry backing or correspondence evidence connecting
that root to the same selected provider. The verifier checks that the touched
footprint lies within the qualified content and that its lineage matches the
provider evidence. A bare content projection or matching denominator can never
authorize hardware access. This rule belongs to terminal external-operation
validation, not to author convention.

Entry-provisioned image and initial-storage extents use the same inbound route
rule as other admitted parameters. Core owns the stable
`ProgramStorageEntry::enter` semantic arrival requirement, whose two exact
`Extent in Granted` positions name the image and initial storage roots, and
`Extent::Granted` lists it as an alternative route. A target entry schema
composes it with a separate target-fixed physical requirement and bootstrap
adapter, then declares which already-typed values its build-bound source
continuation sees. Hosted continuations normally see neither root; freestanding
continuations may receive both. No target-native handle extends the semantic
parameter list. Those exact arrival positions remain the portable keys by
which the compiler derives image sections, receiver storage, and initial
stack/storage subextents after installation. The installation bridge joins the
physical invocation and calling-plan fingerprint, target schema, bootstrap
provider realizations and input provenance, semantic requirement, generated
captures for positions 0 and 1, and selected continuation. Both runtime
geometries must satisfy
`Granted`'s `no_wrap` predicate before semantic installation commits; a
rejected handoff returns every moved bootstrap input without importing either
complete qualified fact. A receiver-bound entry additionally validates its
checked layout's alignment and capacity before root custody moves, then
reserves a conserved owned partition beneath initial storage. The reservation
record does not establish a value: the bootstrap adapter must still zero those
bytes into the checked ZII receiver and lend that occurrence once. A completed
installation
produces a non-authoritative audit record with
the exact binding, geometry, authority metadata, lineage, and whole root-origin
evidence, plus the exact receiver placement when present. The installing
handoff releases the roots only after successfully emitting its canonical JSON
record; a write failure seals the installed roots for retry.
Ordinary compilation only emits the pending contract and removes stale
completion records. Thus an artifact cannot claim installation merely because
the compiler selected a target entry. Image sections derive as borrowed
subrange views under the one installed image root. An independently owned
allocation from initial storage
instead produces an explicit conserved partition containing the selected range
and every nonempty prefix/suffix remainder; invalid extraction returns the
original pool and an unmodified partition recomposes the exact parent lineage.
An active entry stack or provisioned receiver retained by the bridge is one
such owned partition. Source receives only a disjoint residual, never one
qualified extent containing inaccessible live infrastructure.

The target profile declares the external-root slot and schema, fixes the
physical requirement, and owns the bootstrap adapter; `build.omg` binds the
target-qualified slot to one exact semantic source entry. A generated ABI shell
implements physical arrival and invokes that adapter. The adapter may interpret
a validated native service-table layout only through its selected providers.
The UEFI lifecycle join replays the complete target entry slot and every exact
native field and aggregate-layout row; its compact FNV is a report/cache
coordinate and cannot admit a layout. A physical handle or pointer is never
itself an `Extent`. Slot selection and raw geometry authorize no claim. The
domain owner's route authorizes what the semantic installation edge may
introduce, while the physical invocation, provider evidence, selected slot,
bridge, and installation receipt identify the concrete occurrence.
Pre-installation rejection calls no source entry and
introduces no complete semantic root. The bridge's derived contract and
provenance compose into the artifact before target supply is admitted.

The admitted root is a scoped hypothesis import, not a proof that external
reality equals the compiler model. A selected provider states the exact
correspondence between runtime geometry and one backing/root identity. The
compiler then proves every downstream transformation conditionally on that
premise and retains it in the provenance of every dependent fact. Only a sealed
owner-authorized route at one selected provider invocation may introduce the
hypothesis; ordinary source cannot admit a derivable obligation or fabricate
equivalent evidence. PCC rechecks all derived consequences and discloses the
external premise for profile acceptance. Checked partitions derived from
already owned storage need no admitted seam; external OS, firmware, and device
roots do.

An underapproximating projection is safe but restricts access. An
overapproximating projection rejects at checked establishment when the supplied
backing does not cover it. An admitted provider can still misstate external
backing or duplicate an issuance, and the evidence records those accepted
premises separately from derived geometry.

### Custody, reclamation, and succession

Provider-local nonduplication does not compose across providers. Exclusive
claims compare through one stable backing identity and a common custody root.
The root may delegate separated custody to provider children or authorize an
explicit shared alias. Providers without a common custody lineage cannot
establish mutually exclusive claims merely because their numeric intervals
appear disjoint.

Reclamation closes the temporal half of issuance. Automatic return is legal
only when cleanup is terminating, infallible, non-suspending, and nonblocking.
If provider release may block or fail, the claim remains linear and an explicit
terminal operation returns it. If no safe release exists, explicit abandonment
may discharge the local obligation while leaving the external capacity
permanently unavailable. Silent affine abandonment is reserved for resources
declared disposable; safety profiles may reject abandonment. Destroying a
placed `T` and recovering its `Extent` is distinct from returning that extent
to its external provider.

Custody is a tree at one instant and an append-only transition graph over time.
A platform handoff performs a classified succession rather than replacing one
root wholesale:

- preserved delegations transfer to a successor, which must honor them;
- reclaimable classes require no overlapping live claims before they become
  successor capacity;
- retained classes remain under the predecessor or another continuing root;
  and
- excluded classes remain unavailable until a later authorized transition.

The provider admits the platform classification and its correspondence to
external reality. Given that classification and exact locally tracked ranges,
the no-live-claim precondition for reclamation is derived and rejects when it
fails. Succession consumes the transferred issuance authority, prevents later
issuance by the old custodian for those classes, and moves the relevant live
ledger and unused capacity. Existing claim values retain their stable backing
identity. Provenance is not rewritten: it appends the classification,
predecessor, successor, retained custodian, and transition evidence.

### N-ary conservation

Conservation is irreducibly n-ary. Per-output containment and scalar measures
are insufficient: two children may each lie inside a parent and have lengths
that sum to the parent while overlapping completely. Qualification and
independent per-output postconditions therefore do not license authority
duplication.

For every content-bearing claim kind, checked transformation proves:

```text
separate(entry content, introduced content)
    =
separate(output content, content leaving checked custody)
```

Separated composition is partial: overlapping or otherwise incompatible
content has no composition. Exact equality rejects gaps unless an authorized
custody exit accounts for them. Split and merge are the same theorem in
opposite dataflow directions. Binary operations are a common library shape,
not the semantic limit; the frontier theorem is general n-to-m conservation.

Introduction and custody exit are claim-frontier rows, not freely authored
algebra terms. A structural introduction requires all of: no consumed content
source, an exact domain-authorized establishment requirement, and a
content-bearing subject. A provider-backed introduction additionally requires
the matching selected provider invocation and issuance receipt. A program-local
introduction instead requires the exact statically enumerable installed
parameter occurrence and its verifier-reconstructed capacity schema. The
subject may be a result of an outbound provider call or a parameter of an
installed external-root entry. A checked machine cannot mint content by
constructing equal geometry, implementing the requirement, invoking it
ordinarily, or writing a postcondition. Whole-claim custody exit follows a
visible exact terminal call. Checked partial transformations compose the
authored theorem of the partitioning primitive with the visible terminal call
on the residual claim.

That composition is operation-fenced. Terminal Psi retains the exact producer
call identity, the authored source guarantee, and the complete place
substitution. The verifier requires the named internal callee contract or
bodyless boundary declaration to publish the same guarantee modulo stable
place-root renaming, then checks every substituted root against that call's
actual structural arguments. The composed theorem enters reconstruction only
after successful completion of that exact call. It is not available before the
call and does not cross a rejecting or crashing outcome. A fingerprint is an
identity check, never a source of conservation authority.

A bodyless partial boundary cannot assert its own partition. The compiler
derives `kept` from result projections, proves `kept` is contained in entry
content, and computes the canonical residual. The admitted fact states only
that the provider accepted custody of that exact residual. It does not claim
that the provider destroyed it, returned it to a parent, made it reissuable, or
retained it internally. Those are separate provider-ledger states. If the
closed algebra cannot derive the residual, admission is unavailable and the
boundary rejects.

“Retired” in frontier and allocator reports means only “left this checked
custody frontier.” It never implies destruction, reclamation, or reusable
capacity.

Inference stops at claim-identity-preserving reshuffles: direct moves,
one-to-one forwarding, and transparent construction/extraction preserving the
existing claim identities. The primitive operation that establishes or changes
a partition must author its theorem. Checked wrappers may compose already
proved split, merge, introduction, and custody-exit facts without repeating
them. Field names, constructor shape, per-output containment, and scalar totals
never establish a partition.

Root lineage remains distinct from geometry. Equal or adjacent content from
unrelated admitted roots cannot merge merely because the algebraic ranges fit.
Fragments created by different transformations may merge when they retain
compatible common lineage and their content composes exactly; literal
siblinghood is not required.

### Independent and related content

When one value carries several independent content-bearing claim kinds, every
algebra is conserved independently. When correspondence between quantities
carries authority meaning, independent projection is insufficient and the
correspondence must be represented by one joint content algebra.

For example, independently conserving a virtual interval and a set of physical
pages would permit children to exchange which page backs which interval. A
future virtual-to-physical mapping algebra must instead conserve symbolic
`virtual range -> physical backing` associations. It must use a compact
canonical closed form, such as normalized mapping runs, rather than enumerating
every page, and must keep containment, restriction, equality, and separated
composition decidable. Until such an algebra is specified, owned decomposition
of correspondence-bearing virtual/physical claims rejects.

### Orthogonal permissions

Content answers *which resource*. Permissions answer *which operations*.
Weakening read-write to read-only preserves content and irreversibly discards
the write permission; merge must not join permissions and silently recreate it.
If authority is scarce, exclusive, recoverable, or must return later, it is a
separate claim or loan rather than a freely discardable permission.

Domain facets retain their established predicate/semantic meaning. Multiplicity
governs copy/discard obligations, content governs decomposition, permissions
govern allowed operations, carry governs mobility, and root lineage governs
which authority family descendants may recombine.

### Borrowed geometry is not owned decomposition

Layout fields, placed views, subrange loans, and ordinary allocator-private
free-list geometry remain views under one owned root and need no content split.
Owned decomposition is required when a subresource genuinely leaves the
parent's ownership domain, including an allocator returning an independently
owned allocation claim.

Runtime-indexed owned extraction remains a monotone acceptance restriction
until the frontier and prover can name the unique moved element. Static field,
case, and array-index paths participate in the ordinary path-indexed frontier.

## Extent profile

`Extent` is one linear range carrier. Address space, permissions, mapping state,
and authority provenance are domain facts over that carrier.

Root providers may establish combinations such as:

```text
Extent in Granted & Physical & Readable
Extent in Granted & Virtual
Extent in Granted & Io
```

Checked operations preserve or attenuate those facts according to their
contracts. A content-bearing `Granted` claim projects to its address-space
interval. Split consumes its parent and proves the parent content equals the
separated composition of its descendants. Merge proves the same equation in
reverse over compatible common lineage. Loans carry the parent borrow and its
polarity. Mapping consumes destination authority and either consumes or borrows
its source; correspondence-bearing owned decomposition remains unavailable
until a symbolic joint mapping algebra exists.

Every operation that finally discharges an owned range requires
`Extent::Granted`. Forgetting that fact leaves the linear value live but removes
its legal consumer, so the program cannot satisfy linear checking. Constructing
an unqualified Extent has the same outcome.

`ExtentSlot { Empty | Live(Extent) }` remains the debt-free boundary form for
optional storage. Zero-filled Extent storage is unestablished; a live qualified
extent originates through an admitted or checked establishment route.

## Boundary declarations

`boundary` marks the aspect of a declaration that participates in an admitted
crossing:

| Declaration | Crossing concern |
|---|---|
| boundary machine | control, calling, reach, and guarantees |
| boundary trait | service requirement and provider realization |
| boundary data | representation supplied at a crossing |

Direction comes from supply and use: a checked body, requirement, selected
provider, accepted declaration, parameter, or result. The keyword does not
encode inbound versus outbound traffic.

For routed parameters, the installed external-root occurrence is what changes
an ordinary precondition into an introduced fact. The requirement and domain
use their existing syntax; no `[entry]` or `[accepted]` declaration is needed.
An obligation is considered derivable only from evidence and operations already
present in the program. The compiler does not insert an MMIO read or other
validation merely because one could confirm an admitted arrival premise; that
would change work, reach, and device effects and therefore define a different
program.

Evidence crosses in the contracts of boundary machines and requirements. A
receipt records the exact qualified subject and the owner-authorized
requirement that licensed the accepted assertion.

Proof-only abstract values such as `Real` continue to use boundary data without
a runtime representation. Runtime authority values whose owner publishes a
fixed structure use ordinary data declarations whose layouts derive from their
fields. A provider-owned opaque value may instead cross by value through one
exact, target-closed representation application selected during build
composition. Reference-only uses do not demand such an application.

Representation, minting, and authority are independent axes. A representation
application says how the bits move; an owner-authorized operation says who may
mint a valid occurrence; a domain such as `Active` or `Pending` says what that
occurrence permits and how it must be discharged. Selecting a carrier therefore
cannot make a linear declaration copyable, add a terminal disposition, or
establish any domain fact.

The source relationship uses the ordinary named-conformance model, conceptually
`Carrier satisfies OpaqueRepresentation<Opaque>`, and remains inert until the
build selects that exact conformance. Compiler-owned families such as `Ptr<T>`
derive their application from pinned target semantics instead. In both cases
the compiler derives the closed value-shape graph and physical move/finalization
plan from the concrete carrier; source never authors byte size, alignment, ABI
class, or a numeric representation identifier.

Representation supply and accepted facts remain separate evidence lanes. A
runtime by-value opaque demand produces an exact representation-TCB row binding
the opaque declaration, authorized representation source, target-semantics
identity, closed shape or sealed ABI leaf, physical move/finalization plan, and
evidence origin. `Unbound` is a valid complete state only when no runtime
by-value crossing demands the value. A foreign carrier recommends code/ABI
audit and may require admission; a compiler-derived target carrier remains
checked evidence. Opacity alone does not admit a proposition or authority.
Accepted guarantees, qualification establishment, dangerous mechanisms,
executable supply, and compatibility changes retain their own independent
admission policy. An absent service-reach row never suppresses representation
evidence.

## Identity and reporting

Public data shape contributes its ordinary package/type identity. Domain
identity records its body, semantic contributions, and establishment
relationships. Private proof steps and resource-checker witnesses remain
implementation evidence.

Artifacts record each fact origin as checked, transferred, validated, or
accepted. Accepted origins include the domain, subject type, boundary machine,
selected provider, and receipt. Authority-flow reports continue to record
which packages accept, derive, retain, return, release, or acquire qualified
values. Static provider authority-flow rows keep the readable requirement owner
and the canonical overload identity separately, plus the predicate-body and
carry-policy facts bound by the provider-plan receipt. Content-bearing reports
additionally retain the normalized projection, receipt backing, root lineage,
outcome mapping, and n-ary conservation witness.

A compact hash is never artifact, image, installation, replay, or admission
authority. A boundary making one of those decisions retains the canonical bytes
or exact structural carrier, or uses a domain-separated collision-resistant
digest over them. Format-specific planning stages may keep a compact FNV value
only as an explicitly non-authoritative compatibility fingerprint while
independently replaying the exact owned carrier; final-image and installation
authority cannot be reconstructed from that compact value.

Native-fuel sponsor-route admission applies this rule after installation as
well as during image replay. The route retains exact transfer-code custody over
the admitted plan, terminal Psi identity, opaque installed occurrence, runtime
evidence, and sponsor coordinate. Its compact transfer-code and route FNVs are
report coordinates only; executable runtime binding compares the exact custody
and rejects compact-equal substitution. Target policy independently carries a
domain-separated SHA-256 commitment to the complete canonical transfer plan,
including target/context shape, activation slots, machine-state sets, sponsor
stack, and entry identities. Plan admission requires that commitment even when
the historical compact plan report identity matches.

Target-owned physical-entry package provenance follows the same rule. Build
evaluation first validates exact toolchain origin and canonical source
membership, then commits the package identity, package-relative source path,
and source bytes with a domain-separated SHA-256 digest carried in the physical
contract plan. Its historical FNV source value is a report coordinate only; a
report-equal source substitution cannot reproduce the strong commitment.

Provider execution follows this rule across source-free native lowering. The
selected plan, execution, normalized root, and boundary-contract `u64` values
in target operations, machine code, installation encoding, and retained native
artifacts are report coordinates only. The lowering entrance still borrows the
ledger-owned, non-constructible admitted execution evidence. Retained native
custody separately carries the domain-separated selected-provider-closure
digest and exact requirement strings and plan requirement catalogs; native
replay compares those exact strings before comparing the compact report rows.
Consequently a report-equal execution cannot be substituted for another exact
requirement, and decoding an installation record cannot recreate admission.
The evidence interface names each compact projection as a report identity or
report fingerprint; fixed-fuel, stack-demand, progress-profile, and native-fuel
composition expose no shorter authority-looking aliases.

Provider service schemas preserve the boundary-plan authority split before
that lowering. Each `ServiceMethod` retains a compact calling-plan report
fingerprint beside the typed domain-separated calling-plan commitment. The
provider-plan digest and selected-provider-closure digest commit the strong
value, and source-boundary, program-entry, and native-entry consumers replay it
against the exact evaluated plan. Equal compact coordinates therefore cannot
replace a selected service requirement's calling convention.

Component-progress and executable-TCB projections preserve the same provider
authority. Their plan and selected-closure `u64` values are report identities;
each progress demand, installed provider occurrence, TCB entry, allowance,
closure-evidence row, and opaque executable admission carries the matching
`ProviderPlanDigest`. Sealing resolves report plus digest against the retained
exact selected closure. A compact-equal foreign plan therefore cannot issue a
progress establishment or authorize an executable entry. Selected-provider
application and coverage coordinates likewise use report vocabulary beside
the exact plans, application rows, and strong closure digest.

The external-root producer preserves the same distinction before that
projection. Its normalized root, provider-execution, opaque-exit, stack, fuel,
boundary-contract, and selected-closure FNV values are named report
identities/fingerprints. `ValidatedExternalRoot`, `ProviderExecution`, and the
installed-root ledger retain the complete candidate, validated boundary,
resource columns, machine-state receipt, and exact exit assurance. Installed
root reporting also retains the selected-provider-closure SHA-256 digest. All
admission and writer preparation replay the exact carriers; holding every
compact summary equal while changing root policy therefore still rejects.

The generic native-image inventory follows the same split. Region and gap byte
coordinates, final-text coordinates, and the aggregate inventory coordinate are
named report fingerprints and remain beside domain-separated commitments to the
exact bytes, normalized rows, gaps, and footprint evidence. Compiler publication
retains the strong inventory commitment through its certificate, publication
receipt, and flat/bundle equality replay; preserving the compact report value
while substituting the strong commitment rejects.

Native-image output and footprint-certificate summaries apply that split to
callback placement, boundary contracts, fixed/body mechanics, composed
footprints, final-region joins, validation, and compiler-text derivation. Their
compact values are report fingerprints only. Exact callback rows are replayed
before emission, exact relocated text and relocation envelopes retain separate
digests, placed-region inventory commits exact region and state-footprint rows,
and entry-region plus footprint mutation custody retains strong evidence
digests. A collision-resistant digest over a report summary preserves that
summary's custody but does not promote an imported compact coordinate into the
underlying authority.

Trust artifacts make the same distinction visible to tooling users. Provider-
plan rows retain the normalized plan digest, and the report header retains the
complete selected-provider-closure digest; their compact values are labeled
report fingerprints. Generic accepted-instance rows label template and
specialization values as report coordinates while retaining exact type and
const argument identities, strong selected machine-contract and closed-
conformance-application commitments, and the specialized instance-contract
commitment. Equal compact display coordinates therefore never imply equal
trust authority. The closed-conformance commitment length-frames the complete
canonical state-signature bytes directly; an FNV summary of that signature is
never an input to strong authority.

Target locators and image-relocation metadata use the same explicit naming.
Their compact values are compatibility or report fingerprints, while exact
locator bytes, interpreter paths, target profiles, relocation envelopes, and
relocated text digests remain the replay subjects. Layout-plan, generated
writer, typed-boundary calling-plan, and executable-fragment accessors likewise
label compact projections as reports; exact structural plans and installed
contexts continue to decide compatibility and admission. ELF dynamic-import,
symbol-version, and procedure-linkage rows label their copied locator FNV as a
compatibility report identity while continuing to replay the exact target,
object, symbol, version, and relocation sites.

Owner admission follows the same rule before reporting. A provider grant
retains the complete selected plan and its `ProviderPlanDigest`; a generic
accepted grant retains the canonical `MachineTemplateCommitment`; and an
ordinary accepted machine retains its checked `MachineContractCommitment`.
Package review v89 carries each authored provider grant on that exact selected
plan as the selector kind plus `ProviderPlanDigest`, with the granting
build-machine occurrence retained separately as exact `build.omg` source
custody. Neither the selector string nor the compact plan report fingerprint
can stand in for the selected plan.
The persisted trust-admission digest domain-separates those subject kinds and
also binds the human policy commitment. The narrow standalone `omega.lock`
receipt section stores the full digest, while legacy compact-only rows fail
closed and require explicit re-acceptance.

Private and compilation-local carriers follow the same split. An access field
key retains a domain-separated commitment to the exact canonical layout that
issued it before mutation, lookup, authorization, or projection. Checked entry
resource envelopes and callback receipts retain the selected machine-contract
commitment before crossing into provider and backend planning. Installed-entry
stack facts and opaque arrival-context evidence retain the exact boundary-plan
commitment before external-root settlement. Their adjacent layout, resource-
axis, roster, stack, and target-rule FNV values are reports over retained exact
subjects, never independent authority.

The same rule applies to serialized and rendered compiler descriptions.
Terminal program-local-root producer schemas retain their historical FNV only
as a compatibility report beside exact schema fields and owner-projection
replay. Wire protocol reports label schema, codec, encode, and plan FNVs as
report identities; compatibility compares retained field and case structure
even when compact schema reports match.

The residual identity-named inventory applies the same classification. Psi
schema FNVs, indexed-provider application/coverage/closure summaries, task-plan
entry/layout/calling summaries, UEFI layout summaries, and foreign-locator
summaries use explicit report, compatibility, or discriminator vocabulary.
Checked operator uses retain the selected `ProviderPlanDigest` beside their
compact plan report and dispatch joins both. Callback binder/requirement and
native-parameter catalogs reconstruct exact retained requirements and reject
compact collisions before policy evaluation. The remaining raw `u64`
identities are exact authored schema numbers, compiler-generated graph
coordinates, or runtime-issued lifecycle tokens rather than hashes; their
namespace issuers and exact replay rules define uniqueness.

For hardware-entered provider slots, the selected service schema records a
linear routed parameter qualification as a structured `accepts` row. The row
uses the carrier-aware semantic-domain identity, begins with the strict
compiler carry policy, participates in provider-plan identity, and survives the
external-root selection bridge. This is the static admission contract bound by
the selected-plan receipt. A concrete source membership fact still requires the
matching installed-root invocation receipt; reconstructing the parameter's
ordinary fields or merely naming the selected plan establishes nothing.

The provider-neutral external-root ledger now retains those rows beside the
exact selected requirement and includes them in normalized root identity. On a
concrete interrupt entry, it binds the acknowledgement subject to the selected
provider plan and invocation receipt and carries the accepted domain and strict
carry policy into invocation-specific qualification evidence. An Omega-owned
static sidecar resolves the selected `accepts` row to the exact propagated Psi
parameter fact and admits the occurrence only when plan, requirement, semantic
parameter position, domain, and carry policy all match. The admitted occurrence
retains that semantic position beside the exact placement from the validated
boundary plan, and an out-of-range position rejects before installation.
Generated entry-prologue derivation now retains the exact semantic parameter
position, normalized ABI placement, destination, and generated write range.
The Omega sidecar can produce a borrowed body-handoff carrier only by joining
that row and propagated checked parameter fact to the matching live occurrence;
index or placement drift rejects, and no detachable receipt is created.
Concrete provider-entry dispatch performs that join internally and invokes the
checked-body executor only with the resulting borrowed carrier. The carrier
names the exact implementation machine and state, propagated fact, semantic
parameter, ABI placement, and generated write range. Failed occurrence or
placement admission never enters the body; the selected schema alone remains
insufficient.

External roots and outbound providers share typed slot binding, completeness,
admission, provenance, and lock-identity machinery. Direction distinguishes
them: the environment invokes a root, while the program invokes a provider.
Lifecycle, cardinality, sparseness, and runtime installation remain orthogonal
slot properties.

Mask transitions use the ordinary routed-result path. Core's `Active` domain
names the exact exclusive-receiver `InterruptMaskControl::save_and_mask`
boundary requirement. Selected schemas retain its linear result claim as a
structured `returns` row, provider identity binds the row, and the installed
root separately retains that mask-provider contract. The exact transition
receipt then qualifies the concrete guard subject for that invocation. A raw
guard or explicit `as ... in Active` cannot reproduce the route.

Inbound acknowledgement establishment uses the same `established by` route
spelling as a routed result. Core owns one stable acknowledgement-entry
requirement, and `Pending` names that requirement in its `established by`
clause. Target interrupt roots
inherit the exact requirement; `Calling<C>` and target policy may refine its
plan and ABI without replacing its semantic identity. Installation supplies
the direction: an installed external-root occurrence introduces every exact
matching qualified non-`self` parameter, while an ordinary direct call still
requires caller evidence.

There is no source-authored parameter selector, `[entry]`/`[accepted]` marker,
or separate receipt value. The compiler derives semantic source positions and
retains them in normalized `accepts` rows and occurrence evidence. Inheritance,
schema normalization, and specialization preserve the semantic parameter list;
changing it creates a different requirement identity. ABI lowering alone maps
those positions to physical operands. Existing installed-root and claim-
frontier evidence rejects uninstalled entries, look-alikes, missing bindings,
and replay without adding a second trust category.

## Carry of resource claims

Accepted resource claims originate with a strict four-axis carry policy. Their
result contracts may establish the positive compiler-owned permissions
`Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, and
`Carry::MovableAddress`. `Carry::Portable` is the transparent conjunction of
all four.

The carry entry belongs to the undischarged permission provenance rather than
the current predicate-fact set. Authority casts cannot discard that entry;
only consumption or transfer changes it. Freshly constructed unqualified data
has no claim entry and follows its structural/type-wide carry policy.

Checked-internal claims derive from the provenance and storage they inherit.
Claim transfer and conserved decomposition preserve permissions; combined
origins select the most restrictive demand per axis. The checker infers the
unique mapping for ordinary root, transfer, split, loan, and aggregate shapes
and rejects a transformation whose provenance assignment is ambiguous.

## Implementation dependencies

The implementation requires:

1. Migrate the domain surface to predicate `requires` plus exact requirement
   routes, remove ambient package-owner establishment, and retain route-backed
   claims in selected boundary-entry `accepts` rows and qualification
   artifacts. Add one stable core acknowledgement-entry requirement, retain
   compiler-derived semantic parameter positions, and connect concrete
   installed-root occurrence evidence throughout source qualification and the
   remaining authority-flow consumers. Ordinary calls to the same requirement
   must continue to require caller evidence.
2. The permission checker must preserve path-indexed claim frontiers and
   validate inferred resource-transformation outcome mappings together with
   their inherited carry permissions.
3. Qualified claim metadata must retain the owner-unique `Content<A>`
   conformance, its canonical content-projection expression, and its
   interval-set or counted-quantity identity; admitted receipts must carry
   backing in the same algebra. Terminal vocabulary 29 retains and independently
   validates this owner definition; remaining consumers must preserve it rather
   than reconstructing authority from a route-local schema.
4. The prover and resource checker must connect subject arithmetic and access
   footprints to compiler-owned containment and separated composition without
   teaching either system names such as `Extent`, `base`, `split`, or `merge`.

The current runtime authority declarations remain staged on their existing
boundary-data forms until those facilities land. Migration changes the source
declarations and compiler metadata while preserving the authority contracts.

## Acceptance

- reconstructing an authority carrier does not reproduce its domain facts;
- an accepted provider cannot originate membership without satisfying an
  owner-authorized requirement that names the exact qualified subject;
- every accepted fact origin appears with its provider receipt;
- a checked qualified result is rejected unless proof, existing evidence, or a
  validated resource transformation establishes it;
- split conserves one parent claim into exact separated child content without
  overlap, gaps, duplicated origin, or unrelated-root merge;
- admitted backing and projected content use the same normalized algebra;
- every authority-bearing access stays within projected content;
- every independent content-bearing qualification is conserved, while related
  quantities require one joint symbolic projection;
- permission attenuation cannot be reversed by merge, and recoverable
  authority uses a claim or loan;
- accepted resource origins default to strict carry, explicit positive
  permissions relax individual axes, and inherited claims preserve them;
- a fabricated or dequalified linear Extent has no legal consuming path;
- qualification facts and runtime authority carriers add no implicit runtime
  tag;
- claim-free boundary representation remains reported without being treated as
  an accepted fact; and
- proof-only and reference-only boundary data do not demand a by-value
  representation;
- every runtime by-value opaque crossing resolves one exact representation
  application before calling-policy evaluation, and every producer and consumer
  agrees on it; and
- representation never substitutes for minting authority, qualification, or a
  linear value's authored discharge.
