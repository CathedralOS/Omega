# Content conservation and custody

[Storage](storage.md) defines capacity and installed root introduction.
[Placed access](placed_access.md) defines resident views.
[Verification](../terminal-psi/verification.md) checks the corresponding evidence.
[Authority establishment](authority.md) defines qualified claim origins.
These contracts are not a claim of complete source/native integration;
[production limits](../../../omega-rust/psi/pipeline/checked-trees-to-lowered-psi/content_custody.md)
remain beside the implementation.

## Content terms

Structural content terms distinguish entry and current versions. This is not a
general historical-expression modality.

| Term or row | Meaning |
| --- | --- |
| Content projection | An exact owner-unique projection and its algebra. |
| `IntervalSet<CoordinateSpace>` | Canonical intervals in one identified coordinate space. |
| `CountedQuantity<Unit>` | A quantity in one identified unit. |
| `separate(...)` | Variadic partial separation of compatible content. |
| Containment, equality, residual difference | Canonical relations/operations within the identified algebra. |
| Claim-frontier row | Content introduced into or transferred out of checked custody. |

Report fingerprints support diagnostics and caches. Neither a producer row nor
a matching compact identifier proves a content theorem.

## Projection and algebra

A content-bearing exact qualification publishes at most one owner-unique
conformance to core `Content<A>`. The selected machine projects its qualified
subject into compiler-owned algebra `A`. Only the qualification owner may
publish this conformance; other packages cannot reinterpret its resource
denominator by selecting another projection.

The projection body is a closed fragment: subject-field reads, runtime-scalar
embedding into proof mathematics, closed proof arithmetic, and constructors of
the selected algebra. Branches, loops, allocation, effects, hidden state, and
arbitrary helper calls reject. Its canonical symbolic expression and coordinate
space/unit are semantic interface identity. There is no `content(value)`
intrinsic: contracts name the exact projection machine, such as
`Granted::content(&value)`, selecting the claim explicitly when a carrier has
several qualifications.

`IntervalSet<CoordinateSpace>` contains finitely many disjoint half-open intervals
with proof-natural bounds. Normalization sorts lower endpoints, removes empty
members, merges adjacency, and gives the empty set one form. `separate(...)`
requires one coordinate space and no overlap, then produces canonical union.
`residual(whole, kept)` requires containment and produces canonical difference,
possibly several intervals. Equality compares canonical forms. A single interval
would not be closed under residual difference. Address projections embed to
proof integers before converting proven nonnegative endpoints to naturals;
[extent geometry](extents.md) allows a one-past endpoint at the address bound.

`CountedQuantity<Unit>` contains a proof-natural magnitude in one exact unit.
It accounts for fungible units, not delivery to exactly that many destinations,
claim identity, or placement in a fragmented heap. Equal quantities do not
equate slots, addresses, handles, custody, or lineage. An identity-specific
operation needs an identity-bearing or joint correspondence algebra as well;
the checker cannot discover identity the program never modeled. Reports expose
the chosen algebra and whether unit identity lies inside its projection.

New algebra kinds require compiler support and a concrete customer. Packages
cannot define arbitrary authority-bearing composition. Fractional permissions
are absent: ordinary shared, write-only exclusive, and read/write exclusive
loans do not divide an authority into fractions. Ordinary non-content-bearing
claims still retain complete identity, custody, and cleanup accounting; content
adds decomposition rather than replacing linearity.

## Source conservation contracts

`old(place)` denotes the callable-entry revision of a parameter, `self`, or
their structural places. It is proof-only, does not copy an owned value, and is
not general historical quantification. `separate(a, b, ...)` is proof-only
partial n-ary algebra composition; it generates compatibility and separation
obligations. Both are compiler-owned call-shaped operations, erased rather than
executed or overridden by packages. For example:

```omega
ensures
    Granted::content(old(&whole))
    == separate(
        Granted::content(&result.left),
        Granted::content(&result.right),
    );
```

Terminal represents entry/current revision on structural-place terms, sharing
revision identity with scoped facts and borrow certificates. Paths retain exact
roots, domain/projection identities, fields, fixed indices, and cases without
source-arena identity. An entry-claim binding independently names its dense
claim identity, projection, algebra, and structural entry place; it does not
require an input/output equality merely to name a partition input.

For every content-bearing claim kind, the frontier proves:

```text
separate(entry content, introduced content)
    = separate(output content, content leaving checked custody)
```

This is general n-to-m conservation. Per-output containment and scalar sums
cannot establish it: children could overlap or omit gaps. Split and merge are
opposite dataflow directions of the same theorem. Introductions and custody
exits are independently authorized frontier rows, not freely authored algebra
terms. Establishment proves the exact projection fits its backing; each access
fits that projection; transformations conserve it; terminal consumers account
for every exiting remainder.

Inference covers identity-preserving moves, forwarding, and transparent
construction/extraction. The primitive changing a partition authors its theorem;
checked wrappers may compose proved operations without restating them. Every
established claim has its own identity, distinct from its current path, root
lineage, content, permissions, and carry. Moving a place moves its claim subtree;
aggregate construction nests claims and destructuring reverses that operation.
Ambiguous origin/outcome mapping rejects rather than guessing.

## Call conservation

A qualified input's content subject binds the same exact entry parameter and
structural claim, its entry revision, and the qualification owner's normalized
projection/algebra. That definition is independent of routes and producer
schemas. Replay its expression and carrier paths against the owner definition;
rewriting both a schema and its fingerprint cannot authorize less capacity.
Carrier bytes or a domain name alone establish no content. Projected exits
require checked partition/residual geometry.

Identity-preserving reshuffles are derived. A partition-composition row names
the exact call that produced it. Validation checks:

1. Exact argument substitution and its relation to the call's structural operands.
2. An alpha-equivalent authored conservation guarantee in the internal callee
   contract or bodyless boundary declaration.
3. The exact result/returned-claim correspondence where a structural result is used.

The theorem becomes available only after that call completes successfully, not
before the call or on rejection/crash paths. Returned claims obey the
[call contract](../terminal-psi/calls_and_outcomes.md#normal-results).

At a partial bodyless boundary, Psi derives kept content and the exact residual.
The provider may admit acceptance of custody for that residual, not the partition
arithmetic. External root correspondence and fresh provider issuance remain
scoped admitted hypotheses with provenance; subsequent conservation is derived.

That residual is not presumed destroyed, reclaimed, reissuable, or retained by
the provider; those are separate provider-ledger states. If the closed algebra
cannot derive it, partial-boundary admission rejects. A report's `retired` means
only that content left this checked frontier. A checked partial exit composes
the partition primitive's authored theorem with the exact terminal call on the
residual; field names or constructor shape cannot establish the partition.

A qualified linear input crossing a native boundary retains its exact argument
path, entry claim source, and completion receipt. Selection, target settlement,
object/image construction, and installation replay that same record. Qualification
does not alter the ABI carrier or authorize replacement claims.

## Independent and related content

Independent content-bearing qualifications on one value conserve independently.
If correspondence between quantities carries authority, they require one joint
algebra: conserving virtual intervals and physical pages separately would allow
children to exchange their backing assignments. A virtual-to-physical algebra
must preserve associations in a compact canonical form with decidable
containment, restriction, equality, and separation. Owned decomposition of such
correspondence rejects until that algebra is specified; it cannot enumerate
pages as a substitute for the required compact model.

Algebra values and denominators are pure proof data, not authority. Equal
denominators allow arithmetic but do not unite qualifications or unrelated root
lineages. A local budget relates to external capacity only through separate
admitted unit/backing correspondence; the checker derives known containment.
Pools spending one hardware capacity must split or lease from its common
provider-issued root. Independently installed local pools cannot each discharge
the same hardware bound.

Every content-capable root has a canonical internal account even if source
exposes no projection. Checked establishment charges an existing account for
the transfer/lease duration, never creates a new runtime root. Hardware access
additionally binds the exact qualified root and footprint to the same selected
provider's backing/correspondence evidence. Matching projection or denominator
alone cannot authorize it.

## Installed introduction schemas

Program-local root introduction is derived from an authorized declaration, not
admitted from a producer-authored aggregate. The reconstructed schema binds:

- Exact domain-authorized requirement and parameter position.
- Qualification and owner-unique content projection/algebra.
- Normalized finite capacity or constrained-family instance.
- Parent lineage where required, artifact scope, and lifecycle scope.

Projection, algebra, and capacity must equal the independent normalized definition
on the qualification owner. A result route, ordinary invocation, missing lineage,
unbounded capacity, or non-enumerable installation shape cannot impersonate
introduction.

Installation supplies selected satisfiers, exact slot occurrences and lineages,
finite cardinality, and epoch. Verification derives the aggregate for that
installed artifact instance. The aggregate is an ordered roster of occurrence,
algebra, and per-occurrence expression plus derived cardinality. Do not infer
scalar multiplication for interval or subject-dependent content.

A closed cohort binds the complete target-required slot set and commits the
eligible set atomically for one lifecycle ledger/epoch. Its reporting snapshot
retains that closure and cohort identity even when the aggregate rows are empty.
Coexistence accounting requires exactly one snapshot per authoritative live era;
it retains occurrence and epoch attribution instead of combining unlike algebras
or symbolic capacities.

A snapshot is accounting evidence, not lifecycle or minting authority. System
admission composes verified instance totals and charges coexistence at peak.
An assembly-local parent cap does not establish a cross-epoch ceiling.

## Runtime establishment

The required-slot closure is derived from exact target declarations; missing,
duplicate, extra, and cross-profile selections reject. It is descriptive
evidence, not authority. One non-clonable registry authority tied to the exact
installed-code occurrence creates the sole ledger for that installation scope;
dropping a ledger does not reissue it. Slot and owner identities share the
target declaration's derivation, not compiler-local restatements.

The ledger issues its cohort verifier once, with no public constructor.
Prebindings must come from retained required roots; sealing all eligible members
is transactional for one ledger/epoch and failure returns every lease. Mutable
prebinding counts and compact fingerprints cannot establish a root.

The closed cohort becomes a non-clonable epoch runtime, not a vector of reusable
mint grants. An installed-entry subject binds the exact root, ABI and semantic
parameter positions, qualification, carrier, invocation, and runtime place.

Establishment joins that subject to one dormant cohort member, requires exactly
the scalar paths named by the portable expression, evaluates proof-natural
arithmetic, and commits the account only while the lifecycle lease remains current.
Failed evaluation or substitution returns the subject without removing the member.

The account retains the full installed occurrence and lease. Its lineage ID is
a report key. Aggregate schemas cannot silently become one shared parent root.

## Claim-local classifications

Classify each exact claim row, not an entire operation:

| Role | Required evidence |
| --- | --- |
| Introduction | No parent; exact authorized installed/provider-issuance occurrence and receipt. |
| Identity forwarding | Consume one exact parent and produce that same claim occurrence. |
| Derived transformation | Parent claims plus a checked theorem relating their content to the result. |
| Custody exit/consumption | Exact authorized sink for an input with no checked output. |
| Loan | Non-owning edge with exact parent claim, range, polarity, and lifetime. |

One event may have several roles: for example, forward a buffer extent, introduce
provider-originated content, derive the new resident relation, and consume a
completion token. An operation-wide label cannot replace those independent rows.
