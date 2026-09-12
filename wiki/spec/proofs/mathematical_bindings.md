# Mathematical bindings and logical hypotheses

## Scope

This is the selected source model for the [mathematical foundation](foundation.md),
not a claim of complete compiler support. Ordinary machines establish contracts;
mathematical definitions name terms; ordinary data and squash carry witnesses
and existence evidence. No `forall`, `exists`, `claim`, `proposition`, or
`implies` syntax is added by this settlement. Dedicated formula declarations and
proposition-returning machines remain optional, unaccepted naming proposals.

Psi owns elaboration to the common calculus, with independently checked terms,
contracts and assumption closure. `PROOF-CONTRACT-MIGRATION` in
[TASKS.md](../../../TASKS.md) owns the connected source/evidence implementation.

## Universes and mathematical function types

Compiler-known core names expose the selected universe hierarchy without new
reserved sort keywords:

```text
u: core::Level                    universe-level binder
A: core::Type<u>                  mathematical type
P: A -> core::Strict<v>           predicate
F: A -> core::Type<v>             dependent family of types
f: (value: A) -> F(value)          dependent mathematical function
```

`core::Level` denotes the checked universe-binder kind, not a runtime integer
carrier or a type containing all universe levels as ordinary values. It is
admitted in generic telescopes; level expressions use the selected calculus's
level grammar and constraints. `core::Type` and `core::Strict` resolve to fixed
compiler-known declarations, not ordinary structs, import-sensitive conventions
or per-binding modifiers. Display aliases cannot change those identities.
Sort formation, equality and elimination follow the foundation; `[erased]`,
multiplicity and runtime representation remain separate judgments.

Arrow types are mathematical dependent Π-types. `A -> B` abbreviates a Π whose
result does not depend on the argument. Arrows associate to the right; a named
domain binder scopes over its codomain, never over earlier parameters. A type
family `A -> core::Type<v>` does not itself require a dependent arrow; applying
that family in a function's result type does.

Universe inference may omit routine written levels at definitions and calls.
Generalization must deterministically publish the full level telescope and
constraints from the declared interface, independently of private body edits.
The body checks under that interface; it cannot silently strengthen it. Require
annotations where inference/generalization is underdetermined. Preserve explicit
levels and constraints in checked interfaces even when documentation elides them.
No inferred universal sort, impredicativity or implicit resizing is permitted.

## Named mathematical definitions

Extend top-level `let` with an explicitly typed parameter telescope and a term
body. The declaration may be package/module-scoped; dependencies are parameters
or explicitly resolved declarations, not implicit captures of machine locals.

```omega
let greater_than(limit: i32, value: i32): core::Strict<0> =
    value > limit;

let compose<u: core::Level, v: core::Level, w: core::Level,
            A: core::Type<u>, B: core::Type<v>, C: core::Type<w>>(
    f: B -> C, g: A -> B, value: A
): C = f(g(value));
```

This is one form for mathematical functions, predicates and type-valued results,
not a predicate-specific declaration category. It introduces a transparent checked
term definition. Naming a proposition does not prove it; naming a mathematical
term does not request its evaluation. [Constants](../language/constants.md)
still request evaluated values. Local ordinary `let` retains its binding rules;
local parameterized definitions and anonymous mathematical expression syntax are
not introduced. Empty ordinary parameter lists may name nullary definitions.

Definitions elaborate by abstraction over their ordered parameters to nested
dependent functions. Generic arguments are resolved first under their own binder
rules. Ordinary mathematical arguments then apply left to right by prefix:

```text
let f(x: A, y: B(x)): C(x,y) = ...

f       : (x: A) -> (y: B(x)) -> C(x,y)
f(a)    : (y: B(a)) -> C(a,y)
f(a,b)  : C(a,b)
```

Fewer ordinary arguments produce the remaining function term; they do not infer
missing ordinary arguments from context. Excess arguments require the preceding
result itself to be a function, otherwise they reject. Substitution is
capture-avoiding and binder-identity based, including dependent types and level
applications. Retained arguments, exact subjects and assumption dependencies
remain represented. Alpha-renaming is immaterial; accidentally capturing a
shadowing local is not. Ordinary visibility and qualified-name resolution apply.

The following descriptions are mathematical types, not successive runtime states:

```text
greater_than        : i32 -> i32 -> core::Strict<0>
greater_than(10)    : i32 -> core::Strict<0>
greater_than(10,20) : core::Strict<0>
```

The last expression is a proposition, not its proof or an executable Boolean.
A partial application is a complete term. There is no obligation to finish it,
no pending execution and no cleanup debt introduced by partial application itself.
It allocates no implicit runtime closure. Existing ownership obligations are not
waived; mathematical abstraction cannot duplicate runtime authority.

Mathematical conversion unfolds checked definitions and uses the selected beta
and typed-eta rules. Recursive definitions must elaborate through justified
recursion in the selected core; arbitrary self-reference is not a definition.
A kernel lambda is an elaboration result, not an anonymous source machine.

## Logical hypotheses and machine use

A theorem's parameters bind arbitrary eligible mathematical subjects. Its
premises/conclusions express implication, and nested machine-shaped requirements
express universal logical hypotheses. A representative requirement is:

```text
where machine Step(value: A)
    requires P(value)
    ensures Q(value);
```

At a logical application, the supplied argument may be a checked proof term for
that complete theorem contract, not only a selected named declaration. The term
may have been obtained by instantiating another theorem or projecting an evidence
bundle. The existing requirement name and contract bind this evidence; no new
quantifier expression or anonymous body is required. Named proof machines are
one producer of it, not the universe of admissible proofs.

Checked theorem citations may supply logical evidence under the expected
requirement without adding a dummy Type result to theorem machines. This is
proof application, not treating the ordinary runtime result of a resultless
call as a function. Evidence projected from a bundle follows its declared type
and subject bindings; no search among visible declarations supplies it implicitly.

The checked application records logical evidence versus executable declaration
supply explicitly. Logical evidence must establish the contract's implication
over fixed mathematical subjects and all its admitted parameters. Premises are
introduced under those exact binders, conclusions proved under them, and the
premises discharged at abstraction; application supplies their proof at the exact
substitution. Nested hypotheses use the same rule and ordinary lexical scope.

This does not generalize executable static callback selection. A logical
hypothesis cannot promise an observed Type result, runtime mutation, ownership
transfer, authority acquisition/consumption, scheduling or other executable action.
Operational clauses cannot be justified by a logical Π-term. No-result, empty
reach, termination and absent failure ceilings alone do not classify a contract
as logical: a resultless reset through `&mut` still promises a state change.
References to live pre/post-state relationships must retain their operational
meaning, not be reinterpreted as independent mathematical parameters.

If an application demands execution, it requires an implementation satisfying
the complete operational contract. Missing ordinary machine arguments still
reject. A machine declaration is not implicitly a mathematical function value;
an eligible denotation needs its checked correspondence. Merely omitting
arguments cannot construct one. Mathematical evidence must never produce a
callback entry, effect acknowledgment or resource receipt.

## Witnesses, existence and proof construction

For a strict predicate, an ordinary constrained record supplies the source shape:

```omega
data Witness<u: core::Level, v: core::Level,
             A: core::Type<u>, P: A -> core::Strict<v>>
where
    P(value)
{
    value: A;
}
```

Construction establishes the complete default domain; zero storage is not
evidence of `P(value)`. The mathematical interpretation retains the witness and
checked predicate evidence (using the core's strict/relevant bridge where needed).
No authored proof field is needed for this strict constraint. That fact is not
permission to drop derivation or assumption dependencies, nor evidence that the
general interpretation is already implemented.

`core::Squash<Witness<u,v,A,P>>` expresses witness-hidden existence. Ordinary
squash elimination may reason into an eligible strict conclusion; it does not
extract an accessible witness. General dependent pairs remain in scope now:
when evidence is relevant, a bundle carries that evidence explicitly, rather
than identifying it with predicate gating. Universe levels of bundles and
squashes follow the checked formation rules, never a fixed level-zero shortcut.

The flagship theorem has mathematical parameters `P` and `Q`, at independent
levels, logical hypothesis `Step` above, and input evidence of squashed `P`
witness existence. Its source contract is:

```omega
machine existence_follows<
    u: core::Level, v: core::Level, w: core::Level,
    A: core::Type<u>, machine Step
>(
    P: A -> core::Strict<v>,
    Q: A -> core::Strict<w>,
    present: core::Squash<Witness<u,v,A,P>>
)
where machine Step(value: A)
    requires P(value)
    ensures Q(value);
ensures core::Squash<Witness<u,w,A,Q>>;
{
    // Checked squash-elimination proof described below; not an empty proof.
}
```

The terminating semicolon on the nested `where machine` contract separates its
clauses from the enclosing theorem's clauses. The `Step` argument may cite named
theorem evidence or a term already checked against that logical requirement;
the latter must not be forced through executable declaration lookup.

The conclusion is squashed `Q` witness existence. The proof
eliminates the input squash into that strict conclusion, obtains the temporary
witness and its premise, applies `Step`, constructs the `Q` witness and squashes
it. The witness does not escape; choice is unnecessary. Explicit constructors,
eliminators and named proof-machine calls elaborate to checked terms. Automation
may supply those terms but cannot replace them with a success assertion.

## Assumptions and executable demand

`boundary let` is the same mathematical declaration signature without a body,
explicitly classified as a mathematical assumption:

```omega
boundary let choose<u: core::Level, A: core::Type<u>>(
    inhabited: core::Squash<A>
): A;
```

It has an exact declaration/trust identity, no implementation-search slot and no
implied algorithm. Its term and transitive dependencies require the receiver's
admission for the relevant claim role. Existing admission-bearing theorem
contracts remain valid; this is not a replacement for all boundary machines.
Checked mathematical definitions and eligible machine denotations retain their
separate routes. All use the same conversion-independent assumption closure.

A chosen member of a nonempty subset of `u32` is a valid mathematical subject.
Choice can yield a relevant mathematical witness, not automatically executable
bits. Source checking, [evaluation](../language/evaluation.md#invocation-admission)
and lowering reject unjustified demanded data/control, including a choice-driven
branch with constant arms. Erased proof references demand no executable value
while retaining trust. A checked computation or realization may supply bits;
do not diagnose an axiom as an ordinary missing provider.

An admitted total Boolean computation may define the proposition that its result
is true. Apply the complete invocation-sensitive evaluation/denotation rules,
not an abbreviated purity checklist. A proposition is not implicitly an
executable condition. Mathematical term binding is not itself a runtime demand;
the demanding use, its type and its realization determine that judgment.

## Delivery controls

Require source-to-Terminal-to-independent-checker demonstrations, with invalid
controls, of: the flagship theorem supplied with derived rather than named
universal evidence; `greater_than(limit)` under an enclosing symbolic parameter;
dependent result substitution and nested shadowing; independent universe levels
and stable inferred interfaces; relevant witness bundles and strict squash
elimination; and admitted/refused choice with direct and branch materialization.
Include an operational reset/callback rejecting proof-only supply, missing machine
arguments rejecting, partial mathematical applications accepted where a function
is expected and rejected where a proposition/Boolean is expected. General
mathematical coverage, metatheory and executable correspondence are separate
obligations; these examples do not establish them all.
