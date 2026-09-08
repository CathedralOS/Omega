# Authority values and qualification evidence

Runtime authority uses ordinary data plus checked qualification and provenance.
Fields carry runtime information; domain membership states an established
authority, validation, interpretation, or historical fact. Reconstructing equal
fields does not reconstruct membership. Qualification adds no runtime tag.

Multiplicity belongs to the carrier. An owned linear resource must be consumed
or transferred; casting away its visible qualification cannot discharge the
claim. A fabricated or dequalified `Extent` has no legal resource consumer.
Representation, minting, qualification, content, permissions, carry, and root
lineage are independent: a layout says how bits move, not who may establish or
discard authority. See [opaque representations](../build/opaque_representations.md).

A conserved resource fact requires a non-copyable carrier. A must-discharge
obligation requires a linear carrier or an independent linear token; a reusable
historical fact may instead qualify a reusable carrier. Non-copyability does not
itself imply divisibility. [Domains](../language/domains.md) owns qualification,
weakening, aliases, and semantic-role composition.

## Establishment routes

`boundary machine` describes crossing control, calling, reach, and guarantees;
`boundary trait` declares service requirements/provider relationships;
`boundary data` leaves representation supplied at a crossing. Direction comes
from supply and use, not the keyword. None is a substitute for claim evidence.

A domain's `requires` clauses state predicate obligations; `established by`
names exact trait requirements or exact machine declarations authorized to
establish provenance. Each route is an alternative origin, not a call. The
signature-free path must resolve to one exact declaration without consulting an
expected result or selecting among satisfiers. Ambiguity across overloads or
declaration kinds rejects. Adding an overload can break establishment clauses;
compatibility reporting attributes that change to the selected declaration.
`as Name` labels a satisfier edge or selects a complete conformance where its
grammar allows it, never an overload.

### Requirement and exact-machine routes

The route retains a closed target kind and exact declaration identity:

| Target | Authorized origin |
| --- | --- |
| Trait requirement | An exact invocation through that requirement and a valid selected conformance. |
| Concrete machine declaration | That exact machine's invocation, not other conformers of a requirement it happens to satisfy. |

A public ordinary requirement deliberately permits valid downstream checked
conformers. A public boundary requirement additionally requires provider
selection and admission. An exact-machine route may name a free or attached
machine; its visibility controls who can select it. Naming a machine is neither
trait sealing nor an implicit grant to other machines in its package.

Compiler-owned classifications can further restrict eligible route kinds, such
as admitted boundary routes for opaque progress profiles. General machine-route
support does not relax those classification-specific contracts.

At an authorized checked result boundary, the compiler introduces the declared
provenance only for the exact result subject, after independently establishing
carrier validity, predicates, and ordinary result/custody obligations. The body
cannot assume the qualification being introduced to prove those obligations or
its own call preconditions. Route authorization is provenance evidence, not an
axiom proving arbitrary predicates. A result annotation alone remains insufficient.
Existing evidence may instead be forwarded; a public wrapper returning an issued
value does not become a new issuer.

An exact-machine route does not bypass a boundary declaration's ordinary supply,
provider, or admission obligations. Merely implementing a requirement does not
authorize an unrelated direct call. A separately authorized exact checked machine
can establish its own declared result provenance, but cannot borrow the admitted
authority of a requirement it satisfies.

### Private issuer routes

A public domain may name a private trait requirement or private concrete machine
that its author is authorized to select. This is an issuer-authorization reference,
not publication of that route as consumer-nameable API. A private trait restricts
outside conformance; a private machine restricts outside selection. Public wrappers
may return established values without exposing either route for direct use.

The interface and proof evidence retain the exact route kind, owner, declaration,
subject/signature correspondence, and required semantic dependencies. A verifier
must still check that an establishment occurrence matches the authorized route.
Private does not mean secret, nor permit deleting evidence or hiding admissions.
Consumers gain no permission to name, call, conform to, or select private types
through that metadata. Public carriers, predicates, signatures, and ordinary
declaration selections keep their existing visibility requirements.

The catalog is owned by the domain declaration. Downstream code cannot append
routes or substitute a same-spelled issuer. A change to the authorized route set
changes the domain's semantic interface; replacing a checked body does not select
a different route, but still requires validation of its exact contract. No new
sealed-trait construct or runtime issuer registry is required.

### Membership and boundary evidence

Predicate-only membership follows from checked predicates, including exact
guarantees of checked validation or appropriately admitted evidence. A routed
domain additionally requires an authorized establishment, existing fact,
propagation, checked transformation, or admitted boundary occurrence. Proof of
its predicates alone cannot create provenance. If a domain has both predicates
and routes, its predicates must hold at the exact established subject.

Domain predicates obey declaration visibility. Public predicates may be proved
by consumers; inaccessible predicates are exposed through checked validators
and guarantees. Neither package ownership nor a qualified result annotation
provides ambient establishment authority. A checked implementation must justify
its guarantee from proof, existing evidence, validation, transfer, or an exact
authorized establishment occurrence as described above.

For an admitted membership guarantee, the boundary requirement supplies the
contract and the domain authorizes that requirement. An unrelated accepted
boundary machine's assertion is insufficient. The subject is bare `result` or
an exact non-`self` parameter with the domain's carrier. A routed result binds
the selected occurrence; a parameter introduction requires an installed
external-root invocation. At an ordinary call that parameter remains a
precondition. The receipt retains boundary trait, exact requirement/signature,
subject position, selected provider, and installation/invocation evidence.

A checked adapter realizing a boundary requirement does not acquire the same
admitted authority on ordinary direct calls. Semantic checking consumes the
requirement and receipt before execution dispatch selects the adapter. A
look-alike requirement, ordinary invocation, or source-authored entry marker
cannot substitute for that crossing.

The compiler does not insert validation reads merely because they could check
an arrival premise: that changes work, reach, and device effects. A checked
runtime validator that can reject must publish an ordinary outcome or explicit
crash contract. Violating an admitted provider assertion is a trust violation,
not a compiler-invented recoverable result.

## External roots and issuance

Fresh content has only two origins: selected admitted provider issuance with
exact supply/custody evidence, or a statically enumerable installed parameter
position with verifier-reconstructed finite capacity. The domain authorizes
the requirement in both cases. Ordinary checked transformations use existing
accounts; constructors, result annotations, implementing a requirement, and
calling it ordinarily cannot mint new capacity.

Provider-backed introduction separates three obligations:

| Obligation | Evidence |
| --- | --- |
| Per-invocation geometry | Ordinary normalized postcondition over parameters, callable-entry places, and result paths. |
| Fresh issuance | Admitted separation from other live exclusive issuance over backing in the issuer's custody; shared aliases are explicit. |
| Cross-provider custody | Stable backing identity and common custody root for providers sharing that backing. |

The provider plan and invocation retain the geometry theorem, backing, issuer,
live-issuance premise, lineage, alias class, and trust provenance. Multiple newly
established results are bounded together by one n-ary separated relation;
transferred input content is not counted again as new supply. No source-visible
backing-receipt binder is needed. A result-selected base may prove size, not
freshness. A provider-neutral grant handoff is an optional resource
transformation, not another prerequisite or minting route.

External correspondence is a scoped hypothesis, not a proof about firmware,
hardware, or an OS. Only an owner-authorized selected invocation may import it;
derived obligations cannot be admitted by disguising them as root claims.
Verification rechecks downstream consequences and retains the premise for
receiving-policy acceptance. Checked partitions of already-owned storage need
no new admitted seam. [Content custody](content_custody.md) owns conservation;
[extents](extents.md) owns range-specific backing and succession rules.

## Evidence identity and reports

Facts retain checked, transferred, validated, or accepted origins. Accepted
origins name domain, subject, requirement, selected provider, and receipt.
Checked route introductions also retain the exact requirement or machine target
kind and identity, established result subject, and invocation evidence. Private
route references remain verifiable without granting downstream name selection.
Authority-flow reports attribute acceptance, derivation, retention, return,
release, and acquisition to packages. Content reports additionally retain
projection, backing, lineage, outcome mapping, and conservation evidence.
Private checker witnesses are not public domain/type identity.

Compact hashes are report/cache/compatibility coordinates, never artifact,
installation, replay, or admission authority. Acceptance retains canonical
bytes/exact structural subjects or domain-separated collision-resistant
commitments to them. Strongly hashing a summary containing compact coordinates
does not authenticate the underlying subjects. Exact authored schema numbers,
compiler graph coordinates, and runtime lifecycle tokens instead derive their
uniqueness from their issuing namespace and replay rules; they are not hashes.

Static selected `accepts`/`returns` rows describe qualification and carry, not a
runtime fact. Establishment still requires the exact invocation, semantic
subject, ABI placement, and authorized receipt. Representation-only demand
remains reported even without service reach or accepted qualification; opacity
alone admits no proposition. [Provider selection](../build/provider_selection.md)
and [interrupt obligations](../build/interrupt_obligations.md) define their
installation joins.
