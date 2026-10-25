# Domains and qualification

Named decision: `qualified-fixed-byte-array-carrier`, settled 2026-10-14.

A domain attaches a static theory to a carrier without adding a runtime tag,
wrapper, hidden storage, or second object model. Predicate requirements,
semantic contributions, authorized establishment and access routes, cleanup
selection, and transparent aliases are independently normalized aspects of that theory.
Multiplicity, content, carry, and provider evidence keep their own judgments.

Qualification never selects or changes storage representation, capacity, live
length, layout, or collection kind. In particular, `[u8; N] in D` is the same
exact `N`-byte fixed-array carrier as `[u8; N]`; its `.len` is `N`, and erasing a
predicate-only `D` changes no bits or geometry. A variable-fill owner must name
an explicit variable-length carrier such as `FixedVec<u8, N> in D` or
`Vec<u8> in D`. Ordinary accessor machines may lend that owner's live prefix as
a qualified `&[u8] in D` view.

## Declaration and membership

Named decision: `predicate-subject-self`, settled 2026-10-24. In a domain's
`requires` predicates and a data declaration's default-domain `where`, `self`
denotes the exact value being constrained:

```omega
domain Player::Alive
    requires self.health > 0 { }
```

`Player::` determines the domain's carrier. In data predicates the carrier is
the containing data type. Generic arguments are substituted in either context.
Membership `candidate in AtMost<8>` substitutes that exact candidate for `self`;
users supply no subject argument. Each declaration has one subject. Ordinary
generic parameters and other lexical contract names remain available.

`self` is a logical subject, not a field, generic index, runtime parameter,
implicit borrow, move, receiver or source of authority. Its scope is these
predicates; it cannot be rebound and does not enter a separately declared
machine or its contracts. Predicate calls retain their ordinary formation and
evaluation obligations. A predicate may not assume its own goal to justify
forming a call, projection or other partial term.

This replaces explicit domain subject binders: `domain Player::Alive(player)`
is rejected, as is a subject parameter on `data`. Subject-free declarations
such as `domain i32::Km;` are unchanged. Machine receiver syntax and the
`Self` conformance type remain absent. Existing unqualified data-field names
in data predicates remain field references; `self.field` names them explicitly.

Scalar interval relationships use ordinary propositions, not a bracketed type
suffix (`scalar-range-contract-spelling`). Put common-field relationships in the
data declaration's `where`, including guarded case-payload relationships,
machine-parameter obligations in `requires`, and result obligations in
`ensures`. Locals receive facts from their initializer, guards, calls, and
checked qualification. A call's checked postconditions apply to its exact result
even when the receiving local is declared with the bare carrier.

Domain qualification remains a distinct facility. `value: T in D` requires the
exact named domain `D` and is equivalent to an implicit `requires value in D`
for call admission; it is appropriate when that nominal domain identity,
semantic contribution, or establishment route is intentional. Do not generate
a domain merely to replace an anonymous interval. Equal predicates do not merge
domain identities.

To publish a reusable named classification, declare a predicate domain and
return or accept its qualified carrier:

```omega
domain u64::AlignmentSize
    requires self >= 1 && self <= 8;
```

A result may be declared `u64 in AlignmentSize` when callers need that exact
domain. Otherwise return plain `u64` with
`ensures result >= 1 && result <= 8`; the receiving local needs no qualification
to use those established facts.

Data `where` couplings, including case-conditional predicates, remain ordinary
[default-domain obligations](dependent_values.md#default-domains-and-zero-initialization).
Range expressions, range membership, slices, fixed arrays, and bracketed
multiplicity properties are separate facilities and are unchanged.

```omega
domain i32::Positive
    requires self > 0;

public domain Reservation::Issued
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
still requires that provenance. A declaration with neither predicates nor
establishment routes adds no predicate or issuer obligation: a valid bare `i32`
may be explicitly qualified as `Km` without an owner grant. Predicate-free does
not mean uninhabited.

Access clauses add independent permission obligations under
[domain-owned read/write authority](#domain-owned-read-and-write-authority);
absence of an issuer restriction does not waive those obligations.

This rule also applies to affine and linear carriers
(`linear-carrier-vacuous-qualification`). Multiplicity governs use of the carrier,
not authority to introduce a qualification. A predicate-free, route-free tag on
a linear value may be introduced by `as`; the same value is transferred, not
copied, and its ownership obligations remain. Carrier validity and inherited
domain obligations still apply. Omitting `established by` imposes no issuer
restriction; it does not mean that nobody can establish membership.

Establishment restrictions belong to the selected domain's normalized theory,
including inherited routes and alias constituents. Neither the carrier's
multiplicity nor predicates or routes on unrelated domains can add a restriction.
Adding another domain over a carrier cannot change an existing domain's minting
rules. No carrier-wide custody marker or sibling-domain scan substitutes for
declared establishment routes.

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
It is private unless declared `public`; publishing it does not publish a private
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
reports, including [anonymous roots](../build/declarations.md#default-builds-and-anonymous-owner-identity).
Graph preparation supplies their identities before name resolution; an absent
package field cannot identify both a root and an injected library. Repeated
exposure of one exact declaration is not a collision; distinct
visible declarations competing for the same carrier-qualified case, domain, or
machine name reject without inherent-declaration or import-order priority.
Broad imports may be affected by later module additions; narrow imports avoid
unrelated exposure. Diagnostics identify both owners and the exposing imports.

Importing enables selection, not membership, minting, or implicit operator
changes. A value carrying a foreign qualification retains its exact identity and
evidence without activating the domain for authored lookup. Carrying it does not
grant additional dependency reach or expose other extensions.

## Domain-owned read and write authority

Named decision: `domain-read-write-authority`.

Domains may independently restrict who obtains access to their qualified subject:

```omega
public domain Cell::Controlled
    established by Cell::create
    read by Cell::read
    written by Cell::set;

public data Counter {
    cell: Cell in Controlled;
}
```

`read by` and `written by` belong to the domain declaration, not the field or
data declaration. Fields use ordinary qualification. Each clause has a nonempty,
comma-separated list of alternative exact machine declarations or trait
requirements. Omitting a clause adds no restriction on that access axis; it does
not mean deny-all. `written by` does not imply `read by`, or conversely.
The clause words are recognized in domain-clause position; they do not reserve
ordinary machine names such as `Cell::read`. `established by` remains independent:
an issuer need not be a reader or writer, and an authorized reader or writer
cannot thereby mint membership.

Access routes use the same exact-declaration resolution, requirement-versus-machine
distinction, private-route visibility exception, and selected-conformance rules
as [establishment routes](../resources/authority.md#establishment-routes).
An exact machine is authorized during its invocation; a requirement authorizes
the invocation through that requirement and its selected implementation, not an
unrelated direct call to a satisfying machine. No whole-file, package, namespace,
carrier-owner, or same-trait privilege follows. These clauses change neither
declaration visibility nor package dependency authority.

### Governed subjects and operations

The policy applies to the exact qualified value/storage occurrence and its
structural subplaces, not every value of the carrier. A qualified field protects
that field; a qualified whole record protects its contained fields. Following a
stored reference additionally obeys the referent's own authority and loan rules;
the policy does not grant access to arbitrary referenced storage. Introducing a
foreign domain cannot impose a policy on unrelated values or existing domains.

Reading stored contents, including discriminants, comparisons, codecs, and
runtime reflection, requires read authority. Changing contents, including
projected writes, whole-value replacement of a governed place, and atomic stores,
requires write authority. Read-modify-write operations require both. Operations
that return or inspect displaced content must also satisfy their observation and
custody contracts. The operation's semantics determine its access, not whether
its parameter spells `&` or `&mut`.

An authorized route removes only its domain's access restriction for the supplied
subject and applicable axis. It does not create backing, widen a borrow, bypass
exclusivity, prove a predicate, waive cleanup, or confer concurrent access. In
particular, write authorization does not make an ordinary shared reference
writable. Atomic shared writes require both their ordinary atomic authority and
any applicable domain write authorization. Type/field metadata and already
established proof facts are not new runtime reads; they cannot manufacture an
executable view. This is an access rule, not an information-flow secrecy policy.

### Establishment, transport, and helper calls

Constructing a fresh valid carrier precedes establishing its qualification.
Issuance checks the existing carrier, predicate, provenance, and custody rules
and installs the access policy; it does not retroactively require a constructor
to read or write an already-protected value. Existing governed storage cannot
use reconstruction, zero initialization, or an invariant window to bypass its
policy. Introducing a policy requires compatible live loans: no surviving alias
may retain an access path that violates the policy for the governed lifetime.

Moving an intact governed value transports its policy and custody. A permitted
whole-value copy preserves the policy on the copy; it does not expose its fields
as unrestricted values. Compiler-generated byte movement is not source-level
observation or permission to inspect the bytes. Borrowed projection, extraction,
terminal transfer, and generic substitution retain the applicable subject/policy
relationships. Public field names or ownership of the enclosing record do not
waive an installed field policy.

Reading a copyable primitive through a readable reference uses ordinary
[copy rules](ownership.md); it needs no separate reference-to-value cast.
Copyability does not grant read authority or implicitly erase qualification.
Qualification forgetting remains subject to [exact coercion and erasure](#exact-coercion-and-erasure),
and authorized publication remains subject to the result contract below.

Callers may forward a restricted value or reference to an authorized operation
without personally acquiring its read/write grant. Argument admission still
proves carrier validity, qualification, lifetime, and the required ordinary loan.
The authorized invocation can lend access on those exact subjects to ordinary
helpers through checked arguments and reborrows. A helper does not need to be
listed separately; it receives only the delegated access, not ambient privilege
over other values. Bare parameter spelling, wrappers, callbacks, and indirect
calls cannot replace that argument-admission proof.

Delegated access is bounded by the authorizing invocation and its loan lifetime.
Helpers cannot retain the grant after that scope ends; suspension retains its
ordinary call/loan obligations rather than extending them. Returning a restricted
reference transports the restriction, not a fresh unrestricted grant. An
authorized reader may intentionally publish an observed result under its ordinary
result contract; that is not a promise of confidentiality. Access permission
alone does not prove a mutex's exclusion, ordering, or payload-custody protocol.

### Temporary qualified views

A policy introduced solely on a temporary borrowed view governs that loan and
its surviving descendants; it does not permanently install a new policy on an
otherwise unqualified lender. Ordinary qualification, provenance, carrier and
loan-admission obligations still apply. If the domain names establishment routes,
its introduction needs the exact authorized or existing evidence; neither a
reference annotation nor a cast mints that evidence.

While the qualified loan or any descendant remains live, the applicable access
policy must be respected. Copies of the view, aliases, reborrows, returned
references and same-carrier casts retain the loan relationship and its
restrictions. Explicit
erasure of a non-owning qualification may forget its displayed meaning, but does
not end the loan or authorize broader access to still-governed backing. A bare
returned view therefore cannot launder the restriction beyond the issuing call.

After the complete qualified loan cohort and its descendants end, ordinary
[loan restoration](../terminal-psi/loans.md#reborrow-lineage-and-access) restores
the lender's prior access, subject to its remaining custody and invariant
obligations. This is retirement of the temporary view, not qualification erasure.
Returning from a function alone is insufficient if a derived view survives it.

Policies already belonging to the owner, a declared field or another surviving
governed occurrence remain intact. Ending one temporary view does not clear an
independently governed value's policy, mint membership or discharge a resource
claim. Intact-value transport and observed-result contracts retain their separate
obligations.

### Composition and erasure

Every applicable atom must authorize the requested access. Alternatives within
one clause are disjunctive; independent domains and inherited policies compose
conjunctively. Alias expansion cannot drop a constituent's policy, and a child
domain cannot widen a parent's access. Normalization retains the separate exact
read and write route sets, subject, and substituted indices in semantic interface
identity. Changing a route set is an interface change, not a private-body detail.

Forgetting a displayed qualification never increases access. A bare or recast
view must retain the governed storage's access restrictions; if the target/use
requires broader authority, it rejects. This applies equally to by-value
transport, intermediate byte views, atomic-to-atomic recasts, and structural
projection. Explicit `as` cannot discharge the policy merely because a domain is
non-owning or has no predicates. Ordinary predicate/unit erasure remains available
when it does not evade an access or custody obligation. See
[recast authority](../layouts/recasts.md#access-authority-and-concurrent-observation).

These clauses add no runtime permission bits, storage wrapper, alias/newtype
declaration form, or field-private syntax. They attach checked access policy to
ordinary domain-qualified values. Compiler implementation and a complete mutex
construction are not established by this specification settlement.

## Domain-selected automatic cleanup

Named decision: `domain-selected-dropper`.

`dropped by` selects one exact independent static cleanup machine for an owned
qualified occurrence. It names the operation the compiler invokes at cleanup,
not a list of callers permitted to discard the value. It takes one machine
application, with generic arguments supplied from the declaration's binders:

```omega
domain Guard::Held dropped by release_guard;
```

The carrier's owning package authorizes the selection. A data declaration may
put the same clause on its default domain; this is a domain clause, not a
data-owned machine declaration:

```omega
data Guard<T> dropped by release_guard<T> {
    value: T;
}
```

These separate examples show selection, not a complete resource implementation.
The selected machine has an explicit mutable reference parameter for the carrier
and must meet the [cleanup contract](ownership.md#consumers-and-cleanup).
Name resolution is exact: no overload guessing, trait/satisfier discovery, or
lookup by the name `drop`. This clause introduces no `Drop` trait or boundary
requirement.

For one owned subject, normalize its default domain, inherited domains, aliases
and combined qualifications. The result permits zero or one distinct selected
machine application. If the parent selects none, a child may add one. Repeating
the same resolved machine with the same substituted arguments retains one
selection. A different machine or different application conflicts. A child
cannot override an inherited selection; conjunction does not order or chain
droppers. Reject conflicting concrete qualifications; generic selection must
remain consistent under substitution. Fields are separate owned subjects with
their own cleanup plans, not competing selections on their enclosing record.

One selected operation must satisfy all applicable domain, custody and loan
obligations, including inherited ones. Zero selected operations permits ordinary
structural cleanup only where disposal is otherwise legal; it does not discharge
an unresolved linear claim. Selection does not itself prove the hook's
preconditions or give it read/write or issuance authority. The owning package's
selection is the explicit authorization of that cleanup plan; a linear terminal
disposition still must account for every obligation under the ownership rules.

Moves and intact transport preserve the selection and its obligations. Alias
expansion, predicate weakening and explicit qualification erasure cannot remove
cleanup debt or evade a selection conflict. Borrowing does not authorize disposal
of the referent. Cleanup selection belongs to the owned qualified subject, not
to an ambient collection of predicates the compiler happens to prove. Normalized
machine identity and substituted arguments are part of the domain's semantic
interface. No hidden runtime domain tag is introduced.

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
as `requires value in X & Y`. Names do not create a separate classifier declaration.
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
| Predicate-free, route-free qualification | None beyond carrier compatibility; ordinary validity and ownership rules still apply. |
| Routed qualification | Retain exact existing or authorized evidence; `as` cannot mint it. |
| Exact carrier conversion | Prove representability and unchanged denotation. |
| Explicit semantic erasure | Name a target omitting the non-owning meaning. |

Unit conversion remains a library operation. `5 as i32::Km` can introduce the
predicate-free semantic tag, but `distance as i32::Degrees` cannot infer a relation
from kilometres to degrees. `distance as i32 as i32::Degrees` visibly erases
and then introduces meaning. Changing arithmetic policy similarly changes future
operations, not the interpretation of earlier work or the stored payload.

Weakening is per normalized atom:

- Predicate-only atoms with no access or cleanup policy may disappear implicitly.
- Semantic atoms and non-owning routed provenance require explicit `as` erasure.
- An atom containing both predicates and a route follows the routed rule.
- Owned claims cannot be cast away; they must be consumed or transferred.
- Access policies survive qualification forgetting whenever needed to prevent
  authority amplification; a view that cannot retain them rejects. Neither
  non-owning provenance nor explicit erasure waives `read by` / `written by`.

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
provenance, access-policy atoms, and empty explicit tags. Predicate-only domains
without access policies contribute no key.
The expected result projection must equal one declared projection; without an
expected result, lookup selects the empty projection. Only afterward check
predicates and ordinary compatibility.

Equal projections on the same path and parameter signature are declaration-site
duplicates, even if written results differ by predicate refinement. Fixed
operator spellings retain operand-directed selection, not this result rule.

## Aliases and identity

```omega
public domain Socket::Usable =
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
and specialization keys: exact carrier, normalized atoms/roles, establishment
and read/write routes, canonical indices, and alias expansion. Stronger entailment
may accept more programs but cannot change identity or operator meaning. Physical ABI remains
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
`domain<Frame> Point::Coordinates<Frame>;` keeps `Point` as its exact runtime carrier.
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

Operations in an index use ordinary expression resolution and explicit
conformance selection in the enclosing declaration. Consumers declare the trait
laws needed for a normalization, and callers supply the checked conformance;
the domain does not select a hidden algebra or redefine operators inside its
arguments. A domain declaration adds no `using` clause or new selection scope.
Commutativity is not a condition for forming an indexed family, only evidence
for a rewrite that reorders operands. Exact operation and law identities survive
generic substitution and interface publication.

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
name. Declarations over different carriers may share that name when their
normalized facts agree; differing facts reject rather than selecting a
declaration by order. Carrier capacities and representation remain distinct
from qualification. Construction of an explicit variable-fill carrier such as
`FixedVec<u8, N> in D` proves its live byte length fits its capacity; it neither
truncates nor defers capacity failure. A raw `[u8; N] in D` remains an exact
length-`N` fixed array and never acquires live-length storage from `D`.

A validator may return a sum whose cases carry differently qualified views.
Matching the successful case imports its established facts; matching does not
repeat validation. Wire decoding ordinarily supplies structure and raw bytes,
not encoding facts. A schema may explicitly request encoding validation and
include its failure in the decode outcome; it is not the default. Selected
[codec contracts](../layouts/codecs.md) own that validation and any separately
authorized provenance. Encoding validity never proves external authority.
