# Generic declarations and static machine parameters

Generic declarations bind types, canonical static values, machine declarations,
or explicit conformance evidence. Bodies are checked against their declared
requirements; a concrete selection cannot silently strengthen the generic API.
Runtime witnesses follow [dependent-value rules](dependent_values.md), not
const-generic substitution. Lifetime binders follow [lifetimes](lifetimes.md).

## Applications and specialization

Type parameters use ordinary angle-bracket applications. `const N: u64` binds
a static value of the declared carrier, usable in layouts, ranges, and contracts.
Every closed application validates the complete argument tuple, including kinds,
ranges, and constraints of unused parameters. Conflicting, excessive, or
incomplete selections reject.

Ordinary type/result inference may obtain arguments from runtime arguments,
expected destinations, and selected static-machine contracts when the exact
application is determined. Inference does not waive a binder's declared carrier
or obligations. Named constants and forwarded binders must match that carrier
before canonical identity erases their source names. Authored local annotations
remain independent store obligations; inferred call-result types follow the
selected application's substituted result.

Closed values use [canonical static identities](evaluation.md#canonical-static-identities).
Equivalent literals and named canonical values have the same application
identity. Anonymous numeric expressions land under the expected carrier's
[exact rules](numeric_values.md), not their decimal spelling. Target-semantic
observations may be arguments and retain their exact
[target dependencies](evaluation.md#target-dependent-applications).

Substitution is declaration-identity based throughout bodies, contracts, field
types, and nested applications. A local shadowing declaration does not rename
another binder. Const values retain their declared width and arithmetic policy.
Closed data applications retain their generic base and exact tuple; substituted
fields determine layout, not rendered names or provenance encodings.

Generic fields obey ordinary ownership, borrowing, and cleanup, including the
active payload's substituted linear obligations. Concrete runtime applications
need valid concrete layout; proof-only applications do not acquire runtime
layout merely by being closed. [Binding properties](ownership.md#property-declarations)
constrain generic multiplicity independently of const range requirements.

Static dispatch and monomorphization are the baseline realization. Code sharing
may optimize physical emission without changing checked application identities,
contracts, ownership, or observable behavior.

## Requirements and conformance evidence

Generic code proves declared preconditions and may use declared guarantees.
Calls through a requirement retain its complete
[substitution contract](machines.md#substitution), including reach, independent
suspension/blocking possibilities, crashes, progress, and context-visible
resources. Selected implementations must refine that contract. Ordinary call
acknowledgements follow the abstract envelope, not incidental stronger behavior.

A required member operation belongs to a named trait and is supplied through an
explicit conformance binder, for example `Order: Element satisfies Ranked`.
There is no anonymous member requirement inferred or carried by a
`where machine T::member(...)` clause. A structural contract for an independently
bound machine parameter is the distinct form below.

[Named conformances](conformances.md#declaration-and-selection) owns whole-map
selection and explicit arguments to generic conformance applications. Expected
shape validates an explicitly selected map; it does not discover one or fill
its non-lifetime arguments. An already-closed evidence binder forwards its map.
Explicit trait type parameters supply dependent interface types; no associated-
type declaration surface is specified here.

## Static machine binder categories

| Category | Declaration contract | Permitted use |
| --- | --- | --- |
| Structural callable | `where machine Key(card: &Card) -> u64` | Static calls under that complete callable contract. |
| Nominal callable | `where machine Handler satisfies HookProcedure::call` | Static calls or contextual realization under that exact requirement. |
| Declaration identity | A trait's `machine Requirement` binder without a callable clause | Identity relationships only; no invocation. |

A callable binder always has an authored contract. The compiler cannot infer
it from body calls, matching signatures, visible conformances, or the current
consumer set, even if there is one whole-program instantiation. Missing or
ambiguous contracts reject. If only one implementation is intended, source can
call that machine directly without introducing a generic abstraction.

The nominal form inherits the exact requirement's complete shape, conditions,
operational ceilings, and any boundary calling/entry plan; it does not repeat
the signature. Declaration selection obeys ordinary visibility and direct-
dependency rules, including nested contracts. A signature-free requirement
path must identify one overload independently of its satisfiers.

A declaration-identity binder accepts one exact free-machine or signature-free
trait-requirement declaration. Types, conformances, runtime values, and overloaded
requirement families reject. No default, law, or consumer may invoke that binder
without a callable contract. This category expresses relationships such as a
private callback slot's selected requirement; it is not missing-contract inference.

Each executable use of a static-machine argument becomes a direct selected call
after specialization. There is no runtime function value, hidden callable
argument, or capture inference. Stateful behavior uses ordinary instance data
and its receiver access contract; dynamically selected callables use the
ordinary dynamic-trait mechanism instead.

A static parameter cannot be stored as a field type, converted to an address,
or returned as a runtime callback reference. [Private callback realization](../build/private_callbacks.md)
is contextual: an exact selected requirement and destination authorize a private
entry relocation, not a general reified machine value.

## Nested contracts and proof families

A structural machine contract may bind machine parameters of its own:

```omega
machine forward<machine Schema, machine Selected>(value: Stream<Selected>) -> Stream<Selected>
where machine Schema<machine Inner>(value: Stream<Inner>) -> Stream<Inner>
where machine Inner(index: Nat) -> Rat;
where machine Selected(index: Nat) -> Rat;
{
    Schema<Selected>(value)
}
```

Here `Stream` denotes an appropriate proof-only family. Refinement is binder-
positional: renaming `Inner` changes no contract, but changing its nested shape,
conditions, reach, operational/crash ceilings, or termination guarantee can.
Forwarding a distinct machine binder uses the same complete refinement judgment.
Specialization substitutes the outer selections and continues through nested
applications until executable calls are direct; it creates no runtime dictionary.

A static-machine application in a contract instantiates a logical schema.
It does not by itself demand executable monomorphization of a selected generic
machine. Recursive proof-only data may use machine binders as family indices;
finite-layout data rejects those binders, and a binder is not a stored field type.
The selected argument must satisfy the full callable contract.

A proof carrier's telescope is its complete ordered static-parameter list.
Relations choose whether left/right representatives have independent packs or
one shared pack; carrier parameters have no global relational role.
[Quotients](../proofs/quotients.md) owns family matching, laws, and lifting.
Machine-symbol binders do not replace the still-required general mathematical
function/predicate binders or settle their [foundations](../proofs/contracts.md#undetermined-foundations).

## Publication and assumptions

Public machine-binder identity retains its category: a recursively alpha-
normalized structural contract, exact nominal trait/requirement, or explicitly
noncallable declaration identity. Closed applications retain exact selections.
Renaming binders is stable; changing a nested contract or its authority is not.
Private nominal requirements and missing checked contract evidence reject public
review under [package boundaries](../packages/boundaries.md).

Generic axioms spend admission on the normalized template and required machine
contract, not separately on each instance. Instances retain selected contract
identity and the template receipt. Instance-specific trust instead uses an
explicit nongeneric admitted fact. [Proof receiving policy](../proofs/contracts.md#axioms-and-receiving-policy)
owns those grants; specialization never launders admitted premises into proofs.
