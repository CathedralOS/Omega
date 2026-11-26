# Mathematical bindings and logical hypotheses

## Scope

This is the selected source model for the [mathematical foundation](foundation.md).
Ordinary machines establish contracts;
mathematical definitions name terms; ordinary data and squash carry witnesses
and existence evidence. No `forall`, `exists`, `claim`, `proposition`, or
`implies` syntax is added by this settlement. Dedicated formula declarations and
proposition-returning machines remain optional, unaccepted naming proposals.

Psi owns elaboration to the common calculus, with independently checked terms,
contracts and assumption closure.

## Importing the mathematical foundation

The toolchain file `{omega}/language/core/foundation.omg` exposes `Level`, `Type`,
`Strict`, `Squash`, and `Identity` as public declarations. Examples
on this page use the following file-local import.

```omega
use {omega}/language/core/foundation.omg as proof;
```

Ordinary [file lookup](../language/modules.md#names-and-imports) resolves the
alias and member. The proof kernel recognizes the exact declaration exposed
by that toolchain file and applies its existing formation and elimination
rules. Choosing a different alias preserves that identity. A local declaration
or another package's declaration with the same name has its ordinary meaning.
The five names are available to authors as identifiers. Only the explicit
import exposes these core members to another file.

The sealed core mathematical declaration `|proof|::Identity::<T>(left, right)`
exposes proof-relevant identity, with its type/universe and exact subjects
checked by the existing `Id` formation rules. It does not return an executable
Boolean, assume its own proof or introduce arbitrary proposition-returning
machines. Value `==` retains ordinary operator selection in facts; see
[data identity in facts](contracts.md#data-identity-in-facts).

## Universes and mathematical function types

Compiler-known core names expose the selected universe hierarchy without new
reserved sort keywords:

```text
u: |proof|::Level                    universe-level binder
A: |proof|::Type<u>                  mathematical type
P: A -> |proof|::Strict<v>           predicate
F: A -> |proof|::Type<v>             dependent family of types
f: (value: A) -> F(value)            dependent mathematical function
```

`|proof|::Level` denotes the checked universe-binder kind, not a runtime integer
carrier or a type containing all universe levels as ordinary values. It is
admitted in generic telescopes; level expressions use the selected calculus's
level grammar and constraints. A level binder may use a builtin type spelling
such as `u`. A reference to it resolves in the level role under the ordinary
[builtin naming rules](../language/modules.md#builtin-type-names).
`|proof|::Type` and `|proof|::Strict` resolve to fixed
compiler-known declarations, not ordinary structs, import-sensitive conventions
or per-binding modifiers. Display aliases cannot change those identities.
Sort formation, equality and elimination follow the foundation; `[erased]`,
multiplicity and runtime representation remain separate judgments.

Arrow types are mathematical dependent Π-types. `A -> B` abbreviates a Π whose
result does not depend on the argument. Arrows associate to the right; a named
domain binder scopes over its codomain, never over earlier parameters. A type
family `A -> |proof|::Type<v>` does not itself require a dependent arrow; applying
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
let greater_than(limit: i32, value: i32): |proof|::Strict<0> =
    value > limit;

let compose<u: |proof|::Level, v: |proof|::Level, w: |proof|::Level,
            A: |proof|::Type<u>, B: |proof|::Type<v>, C: |proof|::Type<w>>(
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
greater_than        : i32 -> i32 -> |proof|::Strict<0>
greater_than(10)    : i32 -> |proof|::Strict<0>
greater_than(10,20) : |proof|::Strict<0>
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

A theorem declares each proof its caller must supply as an explicit generic
parameter, such as `Step: NonzeroIsPositive`. A
[named alias](../language/machine_values.md#named-structural-callable-types)
defines the hypothesis's premises and conclusions over its own formal
parameters. The theorem header determines the order of the proof arguments.

```omega
data NonzeroIsPositive = machine(x: u8)
    requires x != 0
    ensures x > 0;

machine use_step<Step: NonzeroIsPositive>(n: u8)
requires n != 0
ensures n > 0
{
    Step(n);
}
```

A citation such as `use_step::<nonzero_is_positive>(7)` supplies a proof that
works for every `x` admitted by the alias. The theorem supplies the premise
for the particular application `Step(n)`. Reject an inline `requires machine`
hypothesis at the declaration, including one that repeats a header parameter.
Reject a citation that omits a required proof argument.

A proof argument may cite a named theorem, instantiate another theorem, or
project checked evidence from a bundle. Check the evidence against the full
aliased contract after substituting its arguments. If the contract itself
takes a proof parameter, that parameter belongs to the contract's own header.
The outer caller supplies one proof of the whole contract.

Checked theorem citations may supply logical evidence under the expected
requirement without adding a dummy Type result to theorem machines. This is
proof application, not treating the ordinary runtime result of a resultless
call as a function. Evidence projected from a bundle follows its declared type
and subject bindings; no search among visible declarations supplies it implicitly.

This expected-evidence rule also applies to ordinary typed field initializers,
including [trait schemas](../language/conformances.md#implementations-are-operation-records).
An initializer explicitly citing a checked machine in a mathematical-evidence
position supplies evidence for the declared type, not a machine-selection
field. For a dependent function type, introduce every input under its exact
binder, establish the cited contract's premises, and check its guarantee against
the expected codomain. A theorem restricted to positive inputs cannot supply
unconditional evidence for all integers. Sibling operation selections are
substituted exactly before checking; matching signatures are insufficient.
The same elaboration must work outside traits, with no trait-proof-field
conversion or automatic search. It neither runs the machine nor assumes the
unfinished record's own constraint.

Identity follows the evidence's actual sort under the
[foundation](foundation.md#logical-proofs-and-mathematical-witnesses).
Strict evidence does not retain a cited machine's selection identity; relevant
witnesses do not collapse merely because the same proposition is established.
The citation retains its derivation and assumption dependencies in either case.
An expected callable type instead selects a machine under ordinary callable
construction and refinement; erasure or resultlessness does not change that type.

The checked application records logical evidence versus executable declaration
supply explicitly. Logical evidence must establish the contract's implication
over fixed mathematical subjects and all its admitted parameters. Premises are
introduced under those exact binders, conclusions proved under them, and the
premises discharged at abstraction; application supplies their proof at the exact
substitution. Nested hypotheses use the same rule and ordinary lexical scope.

This premise supply is also required for named proof-machine calls and
result-bearing mathematical machine applications, under the common
[call-precondition rule](../language/machines.md#call-preconditions). Forming a
result without citing its guarantees is not a bypass. Recursive citations must
establish both the substituted premises and the independent descent obligation.

This does not generalize executable static callback selection. A logical
hypothesis cannot promise an observed Type result, runtime mutation, ownership
transfer, authority acquisition/consumption, scheduling or other executable action.
Operational clauses cannot be justified by a logical Π-term. No-result, empty
reach, termination and absent failure ceilings alone do not classify a contract
as logical: a resultless reset through `@mutable` still promises a state change.
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
data Witness<u: |proof|::Level, v: |proof|::Level,
             A: |proof|::Type<u>, P: A -> |proof|::Strict<v>>
requires
    P(value)
{
    value: A;
}
```

Construction establishes the complete default domain; zero storage is not
evidence of `P(value)`. The mathematical interpretation retains the witness and
checked predicate evidence (using the core's strict/relevant bridge where needed).
No authored proof field is needed for this strict constraint. That fact is not
permission to drop derivation or assumption dependencies.

`|proof|::Squash<Witness<u,v,A,P>>` expresses witness-hidden existence. Ordinary
squash elimination may reason into an eligible strict conclusion; it does not
extract an accessible witness. General dependent pairs remain in scope now:
when evidence is relevant, a bundle carries that evidence explicitly, rather
than identifying it with predicate gating. Universe levels of bundles and
squashes follow the checked formation rules, never a fixed level-zero shortcut.

The following theorem transfers existence from predicate `P` to predicate `Q`.
The alias takes the predicates explicitly so its contract applies to exactly
the predicates named in the theorem header.

```omega
data Implication<
    u: |proof|::Level, v: |proof|::Level, w: |proof|::Level,
    A: |proof|::Type<u>, P: A -> |proof|::Strict<v>, Q: A -> |proof|::Strict<w>
> = machine(value: A)
    requires P(value)
    ensures Q(value);

machine existence_follows<
    u: |proof|::Level, v: |proof|::Level, w: |proof|::Level,
    A: |proof|::Type<u>, P: A -> |proof|::Strict<v>, Q: A -> |proof|::Strict<w>,
    Step: Implication<u,v,w,A,P,Q>
>(
    present: |proof|::Squash<Witness<u,v,A,P>>
)
ensures |proof|::Squash::<Witness<u,w,A,Q>>
{
    // Open present, apply Step to the witness, then squash the resulting Q witness.
}
```

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
boundary let choose<u: |proof|::Level, A: |proof|::Type<u>>(
    inhabited: |proof|::Squash<A>
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
