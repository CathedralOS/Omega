# Domains and qualification

A domain attaches a static theory to a carrier without adding a runtime tag,
wrapper, hidden storage, or second object model. Predicate requirements,
semantic contributions, authorized establishment routes, and transparent aliases
are independently normalized aspects of that theory. Multiplicity, content,
carry, and provider evidence keep their own judgments.

## Declaration and membership

```omega
domain i32::Positive
    requires self > 0;

pub domain Reservation::Issued
    established by Issues::issue;

domain i32::Km;
```

Every `requires` proposition must hold. A Boolean-returning validator is not a
logical declaration merely because it has call syntax. Eligible total, pure
machine calls may occur as denotational value terms inside propositions under
[proof-term rules](../proofs/contracts.md); checking qualification does not
execute a validator. Executable validation is an ordinary machine whose exact
guarantee may establish a proposition after its successful call.

Each comma-separated `established by` entry names an exact trait requirement or
concrete machine as an alternative authorized origin, not an invocation.
Predicates alone establish predicate-only membership.
A routed domain additionally needs exact authorized provenance, even when all
its predicates are proved. A predicate-free domain with establishment routes
still requires that provenance. A declaration with neither predicates nor routes
adds no membership obligation: a valid bare `i32` may be explicitly qualified as
`Km` without an owner grant. Predicate-free does not mean uninhabited.

[Authority establishment](../resources/authority.md#establishment-routes) owns
exact route resolution, result and installed-parameter subjects, checked
versus admitted routes, and receipts. The domain owner has no ambient minting
privilege. A result annotation or `ensures` creates an obligation, not evidence.
Trait visibility governs who may conform; machine visibility governs who may
invoke an existing route. Requirement routes deliberately permit valid conformers
under their visibility and contract; boundary routes also require admission.
An exact-machine route authorizes only the named declaration's invocation.

A public domain may authorize private requirements or machines accessible to its
author. Their exact identities remain verifier-visible issuer metadata, not
consumer selection authority. Public wrappers may return qualified values while
issuance stays private. This [limited visibility exception](../resources/authority.md#private-issuer-routes)
does not expose private carriers or relax public predicate/signature visibility.

Assignment, move, and permitted copy transport existing qualification; rebuilding
equal carrier fields does not. Mutation invalidates subject-bound facts unless
preserved or re-established. Declared default-domain obligations are enforced
at [consumption points](dependent_values.md#invariant-windows), not replaced by
an assumption that old flow facts survived a write.

### Visibility and foreign declarations

A carrier-qualified domain owns its visibility independently of its carrier.
It is private unless declared `pub`; publishing it does not publish a private
carrier. Public signatures must be authorized to name both declarations under
[package boundaries](../packages/boundaries.md).

A package may declare a domain over a foreign carrier without a carrier-owner
veto or orphan restriction. Its declarations cannot change the carrier's validity,
reveal private representation, or alter another domain's establishment routes.

Foreign domains follow [ordinary import scope and exposure](modules.md#import-scope-and-exposure):
importing the declaring module exposes its directly declared public domains in
the importing source file; importing one domain exposes only that declaration.
Sibling files, descendant modules, and transitive imports add no exposure.
Importing a carrier or ordinary machine does not expose unrelated domains merely
because their source was loaded. Direct qualified selection uses the
[declaring-owner-first address](modules.md#foreign-attached-declaration-paths).

The exact domain owner and carrier owner remain independent in interfaces and
reports. Repeated exposure of one exact declaration is not a collision; distinct
visible declarations competing for the same carrier-qualified case, domain, or
machine name reject without inherent-declaration or import-order priority.
Broad imports may be affected by later module additions; narrow imports avoid
unrelated exposure. Diagnostics identify both owners and the exposing imports.

Importing enables selection, not membership, minting, or implicit operator
changes. A value carrying a foreign qualification retains its exact identity and
evidence without activating the domain for authored lookup. Carrying it does not
grant additional dependency reach or expose other extensions.

## Refinement and executable membership

Domains classify values within the carrier's ordinary validity. A domain cannot
license observing a value outside its default domain. Establishing membership
requires all carrier, predicate, and provenance obligations; the target
qualification cannot be assumed to prove itself.

### Uninhabited domains

Well-formed domain declarations need not establish nonemptiness. Contradictory
predicates and uninhabited generic specializations are permitted, even when the
compiler can trivially prove that no value satisfies them:

```omega
domain i32::Impossible
    requires self > 0 && self < 0;
```

This defines a classification with no members, not a logical paradox or an
inconsistent axiom. Existing predicate formation, name resolution, cycle, and
semantic-role compatibility checks still apply. A declaration neither asserts
that a member exists nor makes its predicates ambient assumptions.

Mentioning an uninhabited domain or declaring a machine parameter qualified by
it is legal. The body reasons under its hypothetical entry contract; each caller
must establish that contract for its actual arguments. Conclusions proved under
an impossible premise remain conditional and cannot manufacture an inhabitant
or discharge that premise at a reachable call. Predicate-free authority domains
retain their independent establishment-route rules.

Nonemptiness is required only where an operation's contract actually needs it,
using ordinary witnesses or premises. An index classification for a zero-length
collection may have no members without invalidating the generic definition.
Construction, qualification, argument supply, and default-domain establishment
must still prove the obligations they require. Zero initialization and invariant
windows provide no exemption.

No general satisfiability decision or witness search is required. Proved
uninhabitedness may support a warning or explain a failed use, but is not itself
a declaration error. An unproved membership obligation rejects that attempted
establishment; it need not diagnose whether the domain is uninhabited or the
proof automation is insufficient. Failure of one candidate or of proof search
does not prove that every candidate fails. Stronger proof automation must not
make a well-formed declaration illegal merely by discovering a contradiction.

### Refinement and tests

`A::B::C` is a single-parent refinement of `A::B`: its predicate requirements
include the parent's requirements. Multiple-parent predicate reuse is explicit,
as `requires self in X & Y`. Names do not create a separate classifier declaration.
Establishing the child also requires the parent's routed provenance when present;
nesting a name cannot turn a routed parent into predicate-only membership.
Pure total helper machines may supply denotational Boolean terms in predicates,
but do not replace general mathematical predicates with executable deciders.

`x in A | B` states union membership; `x in A & B` states both facts. An ordinary
value match is ordered, so overlapping domain patterns are permitted and the
first matching arm wins. Each selected arm imports the domain facts, and each
transition proves its destination's requirements. A match without a wildcard
over a known domain union must be exhaustive. Unordered domain-union reasoning
requires mutually exclusive alternatives; a child and its parent are not such
alternatives.

An executable domain test requires pure, finite, runtime-checkable predicates.
Subdomain tests check the parent before the child's added predicates. Quantifiers,
opaque proof calls, and nonexecutable facts do not become runtime tests. A test
that cannot establish membership rejects; no hidden runtime domain tag is added.
For routed membership, predicate tests still require the retained authorized
provenance. Neither a successful Boolean test nor inherited predicate knowledge
can mint it. Runtime testing refines proof knowledge, not operator selection.

## Exact coercion and erasure

`as` does not silently change denotation. A qualified target preserves the
denoted value or referent; an explicitly bare target visibly erases non-owning
semantic meaning. Representation may change only through a unique exact
compiler-derived carrier transformation. All predicates and representability
obligations must be discharged before lowering.

There is no hidden rounding, saturation, trapping, allocation, service reach,
suspension, fallible conversion, or arbitrary user code in `as`. Exact widening
and proved representable narrowing are carrier coercions. Domain-specific scale
changes and other policy-bearing conversions are ordinary named machines or
explicitly selected operators, never ambient conversion discovery.

| Use | Obligation |
| --- | --- |
| Predicate qualification | Prove the target predicates. |
| Predicate-free, route-free qualification | None beyond carrier compatibility. |
| Routed qualification | Retain exact existing or authorized evidence; `as` cannot mint it. |
| Exact carrier conversion | Prove representability and unchanged denotation. |
| Explicit semantic erasure | Name a target omitting the non-owning meaning. |

Unit conversion remains a library operation. `5 as i32::Km` can introduce the
predicate-free semantic tag, but `distance as i32::Degrees` cannot infer a relation
from kilometres to degrees. `distance as i32 as i32::Degrees` visibly erases
and then introduces meaning. Changing arithmetic policy similarly changes future
operations, not the interpretation of earlier work or the stored payload.

Weakening is per normalized atom:

- Predicate-only atoms may disappear implicitly.
- Semantic atoms and non-owning routed provenance require explicit `as` erasure.
- An atom containing both predicates and a route follows the routed rule.
- Owned claims cannot be cast away; they must be consumed or transferred.

Thus `i32::Km & Positive` may flow to `i32::Km`, but not implicitly to bare
`i32`. Carry demands survive qualification forgetting until the underlying
claim is discharged. [Content custody](../resources/content_custody.md) governs
resource decomposition; multiplicity alone does not imply divisibility or
create a content projection.

## Semantic roles and operators

Role identity is closed and compiler-owned. Packages supply theories within
these roles, not new roles by declaration:

| Role | Meaning |
| --- | --- |
| Predicate knowledge | Facts such as positivity, ranges, or UTF-8 validity. |
| Denotation/dimension | Unit or other semantic interpretation. |
| Arithmetic policy | Exact, wrapping, saturating, or trapping behavior. |

Compatible contributions in different roles may compose only when their
contracts determine one checked operator meaning. Competing contributions to
one role reject: `Km & Wrapping` has different roles; `Wrapping & Trapping`
has conflicting arithmetic policies. Composition is not sequential execution
of unrelated operator implementations.

Resolution reads static binding qualifications, never incidental flow facts.
The selected operation is unambiguous and retained in the checked artifact;
an unrelated import cannot inject a competing meaning. Predicates do not
synthesize operators. An operator must publish and prove its relational
contract: a degree result in `[0, 360)` alone would incorrectly admit an
implementation always returning zero. Its relation to the operands must also
be established. A local proof that two policies coincide on a bounded operation
does not make them globally equal.

Standing ranges must remain valid at their consumption points under the chosen
operations; optional flow refinements may be invalidated and re-proved. Policy
qualification itself does no runtime work, although later arithmetic may check
or trap. Explicit erasure to a bare numeric carrier selects Exact arithmetic,
with all subsequent safety obligations intact. Numeric operation behavior is
defined by [numeric values](numeric_values.md) and total proof formation by
[proof contracts](../proofs/contracts.md#mathematical-values-and-quantification).

### Result-domain overloads

Named machine and requirement calls may overload by result domain. Their
dispatch projection contains normalized semantic-role contributions, routed
provenance, and empty explicit tags. Predicate-only domains contribute no key.
The expected result projection must equal one declared projection; without an
expected result, lookup selects the empty projection. Only afterward check
predicates and ordinary compatibility.

Equal projections on the same path and parameter signature are declaration-site
duplicates, even if written results differ by predicate refinement. Fixed
operator spellings retain operand-directed selection, not this result rule.

## Aliases and identity

```omega
pub domain Socket::Usable =
    Socket::Connected & Socket::Authenticated;
```

A transparent alias is a nonempty conjunction over compatible subjects.
Expansion precedes sorting, deduplication, and identity formation. The alias
and expanded atoms have one normalized identity; edits affect every published
contract using that expansion. Diagnostics name missing atoms. Compiler-owned
atoms, including the positive carry permissions, may participate in aliases.
Alias expansion is acyclic. Every constituent of a public alias must be legal
to publish for its subject. Adding or removing conjuncts changes the normalized
requirements and guarantees; compatibility checks attribute the consequences to
affected callers, implementations, and consumers. Weakening the expanded atoms
still follows their individual predicate, semantic, or custody rules.

The deterministic normalizer owns domain identity, semantic interface identity,
and specialization keys: exact carrier, normalized atoms/roles, routes,
canonical indices, and alias expansion. Stronger entailment may accept more
programs but cannot change identity or operator meaning. Physical ABI remains
the carrier's; semantic interface identity retains the domain theory.

## Indexed families

```omega
domain<T, const U: Unit> T::Quantity<U>;
```

The family explicitly binds its carrier and canonical proof-static index. It
imposes no carrier-wide arithmetic requirement and contributes no layout.
Ordinary operator conformances require only the carrier operations they use;
they do not enumerate combinations of unit, policy, and predicate refinements.
Their guarantees establish surviving predicate facts.

Families may bind a generic carrier or a fixed carrier with invariant indices:
`domain<P, T> Extent::Resident<P, T>;` keeps `Extent` as its exact runtime carrier.
Type indices use normalized type identity and substitute structurally at generic
calls; different applications have no implicit variance relationship. Const
indices use canonical value identity, not authored record-field order.

One declaration owns the entire family's route set. Instantiation substitutes
indices into those routes; it cannot append routes or create a per-application
registry. Lifetime and static-machine domain indices are not specified by this
type/const-index surface.

Closed index expressions evaluate under [canonical static identity](evaluation.md#canonical-static-identities).
Computed open indices retain their selected operation and normalized expression;
compatibility with an expected index is a verification condition, not arbitrary
evaluation in type equality. [Licensed normalization](../proofs/contracts.md#licensed-normalization)
requires the exact selected checked algebra. Ordinary cited guarantees may
discharge remaining compatibility; the compiler invents neither public generic
preconditions nor ambient lemma search.

Units, coordinate frames, currencies, tensor shapes, scales, and encodings are
library customers, not compiler cases. Metadata erasure removes no scaling,
range-check, or rounding work from the chosen library operation. Runtime
witnesses and view geometry remain distinct from these static applications;
see [dependent values](dependent_values.md).

## Byte containers and encoding domains

Text separates a byte container, an encoding-validity domain, and ordinary
codec/operation contracts. Views, fixed arrays, and owned byte collections keep
their own storage and lifetime semantics. No builtin `String` or `Bytes` type,
encoding intrinsic, or implicit encoding qualification is required. Quoted
literals supply raw bytes; an explicit qualification must establish the selected
library predicate even when the known bytes permit compile-time proof.

Encoding recognition and preservation are checked library behavior. Validation
establishes the exact predicate once; subsequent operations preserve it through
their contracts instead of implicitly rescanning. Reading a byte does not break
the encoding, while a slice preserving UTF-8 must prove codepoint-boundary
endpoints. Mutation must preserve or re-establish the facts on affected places.
An abstract codepoint-text quotient is distinct from any chosen byte encoding;
its observers obey [quotient contracts](../proofs/quotients.md).

Fixed capacities do not define different meanings for one normalized byte-domain
name. Declarations over different bounded carriers may share that name when
their normalized facts agree; differing facts reject rather than selecting a
declaration by order. Carrier capacities and representation remain distinct.
Bounded-carrier construction proves the live byte length fits its capacity;
it neither truncates nor defers capacity failure. This does not change a raw
fixed array's exact-length construction rule.

A validator may return a sum whose cases carry differently qualified views.
Matching the successful case imports its established facts; matching does not
repeat validation. Wire decoding ordinarily supplies structure and raw bytes,
not encoding facts. A schema may explicitly request encoding validation and
include its failure in the decode outcome; it is not the default. Selected
[codec contracts](../layouts/codecs.md) own that validation and any separately
authorized provenance. Encoding validity never proves external authority.
