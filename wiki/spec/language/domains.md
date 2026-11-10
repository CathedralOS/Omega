# Domains and qualification

Named decision: `qualified-fixed-byte-array-carrier`, settled 2026-10-14.

A domain attaches a static theory to a carrier without adding a runtime tag,
wrapper, hidden storage, or second object model. Predicate requirements,
semantic contributions, authorized establishment and access routes, cleanup
selection, and transparent aliases are independently normalized aspects of that theory.
Multiplicity, content, carry, and provider evidence keep their own judgments.

Qualification never selects or changes storage representation, capacity, live
length, layout, or collection kind. In particular, `[u8; N] in D` is the same
exact `N`-byte fixed-array carrier as `[u8; N]`; its `.len` is `N`, and removing
`D` with a cast changes no bits or geometry. A variable-fill owner must name
an explicit variable-length carrier such as `FixedVec<u8, N> in D` or
`Vec<u8> in D`. Ordinary accessor machines may lend that owner's live prefix as
a qualified `&[u8] in D` view.

## Declaration and membership

Requirement spelling follows [unified requirement clauses](state_contracts.md#requirement-clauses).

Named decision: `domain-brace-retirement`, settled 2026-10-28. Domain
declarations end with `;`, not a body. Empty `{ }` is also retired, not an
alternative spelling. Predicates belong in `requires`, establishment routes
and access restrictions belong in the policy selected by `access`, and executable
operations are separate machines. A domain
without predicate requirements remains distinct from one explicitly requiring
`true`; neither form has an executable body.

Named decision: `predicate-subject-self`, settled 2026-10-24. In a domain's
`requires` predicates and a data declaration's default-domain `requires`, `self`
denotes the exact value being constrained:

```omega
domain Player::Alive
    requires self.health > 0;
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

### Carrier-exact membership

Named decision: `carrier-exact-domain-membership`, settled 2026-11-03.

A domain over `u16` classifies `u16` values, not values of any other carrier
that happen to represent the same number. Before substituting a candidate into
the domain predicate, form membership against the domain's exact resolved
applied carrier. This rule applies equally to proof facts, contracts,
qualifications, and executable membership tests.

For `domain u16::Small requires self <= 10;`, `7u16 in Small` has the right
carrier, while `7u8 in Small` is ill-formed even though seven fits both types.
An explicit exact conversion, `(7u8 as u16) in Small`, precedes membership in
the other carrier's domain. Anonymous literals follow ordinary expected-carrier
landing; a suffix or an already typed operand cannot be re-landed by membership.
Cross-carrier numeric equality remains a different judgment and grants no
domain membership or conversion.

Carrier correspondence uses retained type/declaration identities and the exact
generic substitutions and declared type equations. Matching domain names,
matching record fields, or erasing an operand to its mathematical value cannot
replace it. The comparison concerns the data carrier, not identical sets of
domain qualifications; ordinary predicate, semantic-role, access, provenance,
ownership, and borrow obligations remain independently enforced.

A carrier-generic domain family has a separately applied carrier at each use.
Sharing its definition does not transfer membership between those carriers.
Possible restrictions to all integer, signed integer, or unsigned integer
carriers are discussed in the unaccepted
[arbitrary-width integer proposal](../../proposals/0008_arbitrary_width_integers.md#carrier-families-and-domains).
That proposal neither permits concrete cross-carrier membership nor admits
arbitrary integer widths into the language.

Scalar interval relationships use ordinary propositions, not a bracketed type
suffix (`scalar-range-contract-spelling`). Put common-field relationships in the
data declaration's `requires`, including guarded case-payload relationships,
machine-parameter obligations in `requires`, and result obligations in
`ensures`. Locals receive facts from their initializer, guards, calls, and
checked qualification. A call's checked postconditions apply to its exact result
even when the receiving local is declared with the bare carrier.

Domain qualification remains a distinct facility. A parameter `value: T in D`
accepts only an argument whose static type carries the exact named domain `D`,
and gives the body the fact `value in D`. A caller holding only a proof of
membership adds the domain with a [domain cast](#domain-casts), as in
`value as +D`. Qualification is appropriate when that nominal domain identity,
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
domain; the body then ends in a cast such as `size as +AlignmentSize`. Otherwise
return plain `u64` with `ensures result >= 1 && result <= 8`; the receiving
local needs no qualification to use those established facts.

Data `requires` couplings, including case-conditional predicates, remain ordinary
[default-domain obligations](dependent_values.md#default-domains-and-zero-initialization).
Range expressions, range membership, slices, fixed arrays, and bracketed
multiplicity properties are separate facilities and are unchanged.

```omega
domain i32::Positive
    requires self > 0;

public domain Reservation::Issued
    access ReservationAccess;

ReservationAccess satisfies AccessPolicy {
    machine plan(schema: reflect::TypeSchema) -> AccessPlan {
        let mut plan: AccessPlan = AccessPlan::new(&schema);
        AccessPlan::establish(&mut plan, [reflect::callable(Issues::issue)]);
        plan
    }
}

domain i32::Km;
```

Every `requires` proposition must hold. A Boolean-returning validator is not a
logical declaration merely because it has call syntax. Eligible total, pure
machine calls may occur as denotational value terms inside propositions under
[proof-term rules](../proofs/contracts.md); checking qualification does not
execute a validator. Executable validation is an ordinary machine whose exact
guarantee may establish a proposition after its successful call.

Each callable descriptor in the policy's establishment list names an exact trait
requirement or concrete machine as an alternative authorized origin, not an invocation.
Predicates alone establish predicate-only membership.
A routed domain additionally needs exact authorized provenance, even when all
its predicates are proved. A predicate-free domain with establishment routes
still requires that provenance. A declaration with neither predicates nor
establishment routes adds no predicate or issuer obligation: a valid bare `i32`
may be qualified with `as +Km` without an owner grant. Predicate-free does
not mean uninhabited.

The same policy adds independent reader/writer obligations under
[domain-owned read/write authority](#domain-owned-read-and-write-authority);
absence of an issuer restriction does not waive those obligations.

This rule also applies to affine and linear carriers
(`linear-carrier-vacuous-qualification`). Multiplicity governs use of the carrier,
not authority to introduce a qualification. A predicate-free, route-free tag on
a linear value may be introduced by an `as +D` cast; the same value is transferred, not
copied, and its ownership obligations remain. Carrier validity and inherited
domain obligations still apply. Omitting an establishment restriction imposes no issuer
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

Assignment, move, and permitted copy transport existing qualification between
places of the same declared type; rebuilding equal carrier fields does not. Mutation invalidates subject-bound facts unless
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

Named decision: `domain-access-unification`, settled 2026-11-01. This retains the
enforcement of `domain-read-write-authority` under one policy surface.

Domains independently specify predicates and select an ordinary access policy:

```omega
public domain Cell::Controlled
    access CellAccess;

CellAccess satisfies AccessPolicy {
    machine plan(schema: reflect::TypeSchema) -> AccessPlan {
        let mut plan: AccessPlan = AccessPlan::new(&schema);
        AccessPlan::establish(&mut plan, [reflect::callable(Cell::create)]);
        AccessPlan::allow_readers(
            &mut plan, TypeSchema::root(&schema), [reflect::callable(Cell::read)]
        );
        AccessPlan::allow_writers(
            &mut plan, TypeSchema::root(&schema), [reflect::callable(Cell::set)]
        );
        plan
    }
}

public data Counter {
    cell: Cell in Controlled;
}
```

The former `established by`, `read by` and `written by` clauses are retired, not
parallel ways of configuring the policy. `access` belongs to the domain, not a
field-private modifier. Fields may use qualified types; a whole-value domain's
policy may also name individual fields with `reflect::field(Type::field)`.

`AccessPlan::new(&schema)` imposes no additional caller restrictions. Explicit
establishment, reader and writer lists are independent ceilings: omission leaves
that axis unrestricted by this policy, an empty list permits nobody, and a
nonempty list contains alternative exact callable identities. Authorizing writers
does not authorize readers or issuers. Whole-subject, field, inherited and repeated
restrictions intersect without order-dependent overrides. A field rule alone
does not require a whole-object rule. Backing/loan authority still limits every
operation; default-permissive caller authorization manufactures none of it.

An ordinary named `AccessPolicy` computes the plan at compile time under the
[access-plan contract](../resources/access.md#plans-and-admitted-supply). It consumes
the semantic schema, not a layout. The plan also retains any observation demands,
exposure and required reach. Layouts stay independently selected and validated;
establishment checks their correspondence with actual backing and custody.
The separate `Access<P, T>` source wrapper is retired in favor of domain-qualified
values/views. External/Atomic projections still use restricted operation accessors
where an ordinary field loan would promise invalid stable/exclusive backing.

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
Copyability does not grant read authority or remove qualification.
Removing a domain remains subject to [domain casts](#domain-casts),
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
restrictions. A cast
that removes a non-owning qualification forgets its displayed meaning, but does
not end the loan or authorize broader access to still-governed backing. A bare
returned view therefore cannot launder the restriction beyond the issuing call.

After the complete qualified loan cohort and its descendants end, ordinary
[loan restoration](../terminal-psi/loans.md#reborrow-lineage-and-access) restores
the lender's prior access, subject to its remaining custody and invariant
obligations. This is retirement of the temporary view, not removal of a domain.
Returning from a function alone is insufficient if a derived view survives it.

Policies already belonging to the owner, a declared field or another surviving
governed occurrence remain intact. Ending one temporary view does not clear an
independently governed value's policy, mint membership or discharge a resource
claim. Intact-value transport and observed-result contracts retain their separate
obligations.

### Composition and removal

Every applicable atom must authorize the requested access. Alternatives within
one list are disjunctive; independent domains and inherited policies compose
conjunctively. Alias expansion cannot drop a constituent's policy, and a child
domain cannot widen a parent's access. Normalization retains the separate exact
read and write route sets, subject, and substituted indices in semantic interface
identity. Changing a route set is an interface change, not a private-body detail.

Removing a displayed qualification never increases access. A bare or recast
view must retain the governed storage's access restrictions; if the target/use
requires broader authority, it rejects. This applies equally to by-value
transport, intermediate byte views, atomic-to-atomic recasts, and structural
projection. A `-D` entry cannot discharge the policy merely because a domain is
non-owning or has no predicates. Removing a predicate or unit domain remains
available when it does not evade an access or custody obligation. See
[recast authority](../layouts/recasts.md#access-authority-and-concurrent-observation).

These policies add no runtime permission bits, storage wrapper, alias/newtype
declaration form, or field-private syntax. They attach checked access policy to
ordinary domain-qualified values. Mutex exclusion, ordering and payload custody
still require their own checked protocol; caller authorization alone proves none.

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
The selected machine takes the carrier by value as its one parameter
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
expansion and domain removal cannot remove cleanup debt or evade a selection
conflict. Borrowing does not authorize disposal
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
A type carrying the child carries every ancestor, and a
[domain cast](#domain-only-casts) names each of them.
Pure total helper machines may supply denotational Boolean terms in predicates,
but do not replace general mathematical predicates with executable deciders.

`x in A | B` states union membership; `x in A & B` states both facts. An ordinary
value match is ordered, so overlapping domain patterns are permitted and the
first matching arm wins. Each selected arm imports the domain facts, not a
changed declared type for the matched value, and each
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
can mint it. Runtime testing refines proof knowledge. It changes neither the
tested value's declared type nor operator selection; a cast adds the domain.

<a id="exact-coercion-and-erasure"></a>
## Domain casts

Named decision: `named-domain-casts`, settled 2026-11-09. Compiler, library and
source migration are tracked by `NAMED-DOMAIN-CASTS`. The
[design record](../../drafts/designs/named_domain_casts.md) lists the questions
left open.

A declared type gains or loses a domain only through a cast that names that
domain. No proof, expected type, literal, call boundary or join attaches or
drops one. The rule is uniform: a predicate-only domain is named in the same
way as a unit, an arithmetic policy or a routed domain.

```omega
domain i32::Km;
domain i32::Positive
    requires self > 0;

machine previous(distance: i32 in Km & Positive) -> i32 in Km {
    let plain: i32 = distance as -Km -Positive;
    (plain - 1) as +Km
}
```

`distance` enters carrying both domains. The first cast removes both and names
each. The subtraction then takes plain `i32` operands, and `plain > 0` is still
known, so the Exact subtraction cannot overflow. The second cast adds `Km` to
the result.

Facts and attached domains are separate. A `requires x in Positive`, a guard or
a call guarantee proves a fact about `x`. The fact supports later proofs and
leaves the declared type of `x` unchanged. Only a cast changes the set of
domains a type carries, and operator and overload selection read only declared
types and written casts.

`as` never calls user code, rounds, saturates, traps, allocates, reaches a
service, suspends or fails at runtime. All predicates and representability
obligations are discharged before lowering. Scale changes between units and
other policy-bearing conversions are ordinary named machines or explicitly
selected operators, never conversions the cast discovers.

### Domain-only casts

A cast with no carrier, `value as =A -B +C`, keeps the carrier and states what
happens to every domain:

| Entry | The operand's type | The result's type |
| --- | --- | --- |
| `=D` | carries `D` | carries `D` |
| `-D` | carries `D` | does not carry `D` |
| `+D` | does not carry `D` | carries `D` |

The entries are one simultaneous accounting, not a sequence, and their order
has no meaning. Every domain attached to the operand's type appears exactly
once, as `=` or `-`. That includes each inherited parent, each constituent of
an alias and each compiler-owned domain. Every `+` entry names a domain the
operand lacks. The result carries exactly the `=` and `+` domains.

A missing entry, a repeated domain, `=D` or `-D` for a domain the operand lacks,
and `+D` for a domain it already carries each reject, and the diagnostic names
the domain. A reader therefore sees the complete before and after of every
cast, and adding a domain to a declaration surfaces every cast that must now
decide what to do with it.

```omega
let both: i32 in Km & Positive = 5 as +Km +Positive;

let km: i32 in Km = both as =Km -Positive;
let plain: i32 = both as -Km -Positive;

// Rejected: `Positive` is attached and not accounted for.
let lost: i32 in Km = both as =Km;

// Rejected: `Km` is already attached.
let again: i32 in Km & Positive = both as +Km =Positive;
```

"Carries" means attached to the static type, not merely provable. A plain `x`
with a proved `x in Positive` lacks the domain and may write `x as +Positive`;
it may not write `x as =Positive`.

`+D` obeys the establishment rules of `D`:

| Domain | Obligation of `+D` |
| --- | --- |
| Has predicates | Prove them for this value. |
| Has neither predicates nor establishment routes | None beyond carrier validity; ordinary ownership rules still apply. |
| Has establishment routes | Retain exact existing or authorized evidence; a cast cannot mint it. |

`-D` discards no fact: after `both as =Km -Positive`, the value is still known
to exceed zero. Removal also never increases authority. Owned claims must be
consumed or transferred, not cast away. A cleanup selection survives. Access
policies survive whenever they are needed to prevent authority amplification,
and a view that cannot retain them rejects. Carry demands survive until the
underlying claim is discharged. [Content custody](../resources/content_custody.md)
governs resource decomposition; multiplicity alone does not imply divisibility
or create a content projection.

No cast infers a relation between domains. `distance as -Km +Degrees` is
accepted because both are predicate-free and route-free: it visibly drops one
meaning and introduces another, and converts nothing. Unit conversion remains
a library operation. Changing arithmetic policy likewise changes future
operations, not the interpretation of earlier work or the stored payload.

A child domain travels with its ancestors, and a cast names each one. The
result must be ancestor-complete:

```omega
domain i32::Positive::Small
    requires self < 100;

let small: i32 in Positive::Small = 7 as +Positive +Positive::Small;
let positive: i32 in Positive = small as =Positive -Positive::Small;

// Rejected: the result would carry the child without its parent.
let orphan = small as -Positive =Positive::Small;

// Rejected: `Positive` is not attached and not added.
let short = 7 as +Positive::Small;
```

Naming an alias names its expansion: for `Socket::Usable` below, `-Usable`
removes `Connected` and `Authenticated`, and the same cast may not also name
either constituent.

A data declaration's default domain is never removed. It belongs to the
carrier: no cast names it, and no type spells the value without it, so every
machine that receives a value of a data type receives it in that domain. A
property that a helper must be able to receive false belongs in a named
domain, which a [mutable view](#mutable-views) can remove and owe back.

### Carrier casts

`value as T` changes the carrier through a unique exact compiler-derived
transformation: exact widening, or narrowing with representability proved. The
denoted value does not change.

The operand of a carrier cast must carry no domain. The cast may add domains to
the result, so `plain as i64 +Km` abbreviates `(plain as i64) as +Km`, and it
accepts no `=` or `-` entry. Casts chain left to right, so moving a qualified
value to another carrier reads as three visible steps:

```omega
domain<T> T::Metres;

let near: i32 in Metres = 5 as +Metres;
let far: i64 in Metres = near as -Metres as i64 +Metres;

// Rejected: each of these changes the carrier of a qualified value.
let a = near as i64;
let b = near as i64 =Metres;
let c = near as i64 -Metres +Metres;
```

Membership is [carrier-exact](#carrier-exact-membership), so a domain in the
new carrier is a different application whose obligations are established
there. Writing the removal and the addition makes both visible. A domain with
establishment routes cannot be re-added without its route, which effectively
pins the carrier of such a value.

For an anonymous expression, `as` to an integer destination explicitly lands
the complete operand under the [numeric landing rules](numeric_values.md#fractional-intermediate-diagnostics).
It acknowledges fractional intermediates while requiring an integral, in-range
final value and all target obligations. For example, `((5 / 2) * 2) as u32`
is `5`, while `(5 / 2) as u32` rejects. This does not change the rules for
already-typed carrier coercions or floating landing.

The whole-type cast targets `value as T in D` and `value as T::D` are retired,
not alternative spellings. They restated the complete result and showed neither
what was added nor what was dropped.

### Literals and expected types

An expected type selects the carrier in which an anonymous literal lands. It
never inserts a domain cast. A literal therefore receives a domain the way any
other value does:

```omega
let distance: i32 in Km = 5 as +Km;
```

An annotated `let` is no exception, and the same rule governs arguments, field
initializers, returns and arm results: the supplied expression's static type
must carry exactly the domains the destination declares. Arms and joins that
produce one result agree on that exact set; a join neither intersects the
arms' domains nor weakens to their common part.

[Result-domain overloads](#result-domain-overloads) are unchanged. There the
expected type selects among declared machines, each of which returns its own
declared type. It inserts no cast.

### Operands

Builtin operators and comparisons, including the suffixed
[policy operators](numeric_values.md#policy-operators), accept unqualified
operands only. The rule is the same in executable code, `requires` and
`ensures`. A qualified operand either has a declared operator for its exact
type under [semantic roles and operators](#semantic-roles-and-operators) or is
cast first:

```omega
machine farther(left: i32 in Km, right: i32 in Km) -> bool {
    (left as -Km) > (right as -Km)
}
```

In a domain's own `requires`, `self` is the value in the underlying carrier
without the domain being defined, so `self > 0` in `Positive` is a builtin
comparison on `i32`. Indexing follows the same rule through its operator
declarations: `[]` applies when one declaration's parameter types equal the
collection and index types, so a qualified collection or index has such a
declaration or is cast. Field projection and pattern matching are not
operators and keep their own rules. A named call admits an argument only at
its parameter's exact declared domain set.

Explicit [zero defaults](dependent_values.md#default-domains-and-zero-initialization)
are not literals. An omitted field is established at its declared type under
the existing rules.

### Mutable views

A cast applies to a reference by accounting for the domains displayed on its
referent, with the same syntax: `&value as =Km -Positive`. A borrow groups
before `as`, so no parentheses are needed. The cast preserves the referent,
lifetime and access mode, and it changes neither the stored owner's declared
type nor the carrier's default domain. Adding a domain to a view follows
[temporary qualified views](#temporary-qualified-views). Through a shared
reference any removal a by-value cast permits is accepted and nothing is owed,
because nothing can be written.

Named decision: `mutable-view-reestablishment`, settled 2026-11-09. Through a
mutable reference, `-D` is accepted exactly when the lender could write
`as +D` on that place itself when the loan ends. The end of the loan is that
`+D`: it is not written, and it is checked by the same
[establishment rules](#domain-only-casts) for whatever the place then holds.

```omega
machine reset(n: &mut i32) ensures n == 1 { n = 1; }
machine scribble(n: &mut i32) { n = 0; }

let mut trip: i32 in Km & Positive = 5 as +Km +Positive;

reset(&mut trip as -Km -Positive);
// `Km` returns with nothing to prove. `Positive` is proved by `n == 1`.

scribble(&mut trip as -Km -Positive);
measure(trip);
// Rejected: `Positive` is not re-established for `trip` after `scribble`.
```

The rule has no cases of its own; each row follows from what `+D` requires:

| `D` | Re-establishment when the loan ends |
| --- | --- |
| Has neither predicates nor establishment routes | Nothing to prove. |
| Has predicates | Prove them for the current contents. |
| Has establishment routes | Accepted only where `+D` is accepted, which is inside its authorized routes. Elsewhere the removal rejects. |

A removal that a by-value cast would reject stays rejected: owned claims,
cleanup selections and access policies are not removed through a reference
either. A domain kept with `=D` is part of the callee's parameter type, so the
callee maintains it on every write and the lender owes nothing for it.

Each removed domain is a debt on the lender's place under
[invariant windows](dependent_values.md#invariant-windows) and
[contract-based repair](dependent_values.md#contract-based-repair). The place
may be invalid for that domain after the loan. Until the debt closes it cannot
be used under the domain, passed where the domain is required or handed back
to its own owner. A callee guarantee or the lender's own later writes may
supply the proof. An open debt at the lender's completion or handback rejects,
and the diagnostic names the domain, the place and the cast that removed it.
A callee that removes further domains and lends again owes only what it
removed.

The owner's declared type is unchanged for the whole loan, and exclusivity
means nothing else observes the place while a debt may be open. Earlier facts
about the contents do not return; only fresh proof about the current contents
does. An exact carrier conversion applies to values and never through a
reference; [representation recasts](../layouts/recasts.md) are a separate
facility with their own footprint and access rules.

### Grouping

After `as` comes an optional carrier type and then the longest run of entries,
each a `=`, `+` or `-` followed by a domain path. At least one of the two is
present. Parsing never consults name lookup: a `+` or `-` after a cast always
begins an entry and never an addition or subtraction, whether the cast names a
carrier or not. Arithmetic on a cast result therefore takes parentheses, as in
`(distance as -Km) + adjustment` and `(byte as u32) + 1`. Other operators keep
their existing grouping around `as`.

A cast produces a result expression. Ordinary copy, move and borrow rules
decide whether the source remains usable afterward.

## Semantic roles and operators

Role identity is closed and compiler-owned. Packages supply theories within
these roles, not new roles by declaration:

| Role | Meaning |
| --- | --- |
| Predicate knowledge | Facts such as positivity, ranges, or UTF-8 validity. |
| Denotation/dimension | Unit or other semantic interpretation. |
| Arithmetic policy | Exact, wrapping, saturating, or trapping behavior. |

A type may carry contributions in different roles, such as `Km & Wrapping`.
Competing contributions to one role reject: `Wrapping & Trapping` has
conflicting arithmetic policies.

An operator applies only when the instantiated parameter types of one
declaration equal the operands' complete static qualified types, after alias
expansion, normalization and generic substitution. Nothing composes
automatically. `Km & Wrapping` has no `+` because `Km` declares one and
`Wrapping` declares one; it has a `+` only when a declaration names that exact
set, and that declaration states the combined meaning in its own body:

```omega
machine + Km::add_wrapping(
    left: i32 in Km & Wrapping, right: i32 in Km & Wrapping
) -> i32 in Km & Wrapping {
    ((left as -Km -Wrapping) +% (right as -Km -Wrapping)) as +Km +Wrapping
}
```

No matching declaration is a missing operator and several are an ambiguity;
neither selects a nearest candidate, drops a domain to find one or runs
unrelated implementations in sequence. Operand types may differ, so a domain
may declare an operator against a plain operand:

```omega
machine > Positive::greater(left: i32 in Positive, right: i32) -> bool
ensures result == ((left as -Positive) > right)
{
    (left as -Positive) > right
}
```

`Wrapping`, `Saturating` and `Trapping` are the `omega::core` contributions to
the arithmetic-policy role. Their unsuffixed integer operators are defined by
the [policy operators](numeric_values.md#policy-domains): the domain selects
which suffixed operation `+` means for its operands. A policy operator written
directly reads no qualification and contributes to no role.

Resolution reads declared types and written casts, never flow facts: a proved
`x in D` or a `requires x in D` on a plain `x` selects nothing.
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
or trap. Removing the policy domain with a cast leaves a bare numeric carrier,
whose unsuffixed operators are Exact, with all subsequent safety obligations
intact. Numeric operation behavior is
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

The expected type only selects among declared machines. The selected machine
returns its own declared type, and that type must equal the destination's
exactly: because predicate-only domains contribute no key, a destination that
differs from the selected result by one still needs a written
[domain cast](#literals-and-expected-types).

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
affected callers, implementations, and consumers. A
[domain cast](#domain-only-casts) that names the alias accounts for every
expanded atom, each under its own predicate, route or custody rules.

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
Ordinary operator conformances require only the carrier operations they use.
Each applies to the exact domain set its parameters declare under
[semantic roles and operators](#semantic-roles-and-operators); an operand that
also carries a policy or predicate domain is cast to that set first or has its
own declaration. Their guarantees establish surviving predicate facts.

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
Open indices preserve their selected operation and structural expression under
the fixed core profile. Compatibility with an expected index is a verification
condition. Selected checked algebra laws and ordinary cited guarantees can
discharge it without renaming either domain application.

Operations use ordinary expression resolution, including explicit conformance
selection where required. The domain selects no hidden algebra and adds no
`using` clause. A canonical primitive operation needs no extra user conformance
to receive its fixed simplifications. For user operations, consumers declare the
laws needed for compatibility and callers supply the checked conformance.
Commutativity is not required to form an indexed family.

Preserve qualified carriers such as `u8 in Wrapping` and `u8 in Saturating`
and each exact operation identity. A predicate-only qualification grants no
arithmetic law. [Literal connections](../proofs/contracts.md#literal-linked-index-identities)
for user algebras prove compatibility; they do not identify applications in
signatures and specialization keys. Core-profile neutral-element rules can
still identify `Indexed<N + 0>` and `Indexed<N>` where licensed.

[Distributivity](../proofs/contracts.md#proved-index-compatibility) also proves
compatibility without canonical expansion. Its default 64-term search allowance
is configurable through root Build proof-work policy, never a domain parameter.
An unresolved comparison reports missing evidence or the exhausted allowance,
not mathematical inequality. Carry successful evidence through specialization
and artifact consumption. Exact conformance matching continues to use keys,
so authors needing identical applications use a common expression or alias.

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
For [declared local and field qualifications](dependent_values.md#lent-storage-retains-its-declared-domains),
this includes loan handback after a call, even when the callee receives a view
from which a cast removed the encoding qualification. An additional entry-only fact over
otherwise unqualified storage remains distinct from a declared storage obligation.
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
