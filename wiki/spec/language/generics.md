# Generic declarations and static machine parameters

Generic declarations bind types, values, machine declarations, or explicit
conformance evidence. Value binders distinguish runtime-capable `Count: u32`
from static `const Count: u32`. Bodies are checked against their declared
requirements; a concrete selection cannot silently strengthen the generic API.
Runtime indices follow [dependent-value rules](dependent_values.md), not
const-generic substitution. Lifetime binders follow [lifetimes](lifetimes.md).
These are source contracts, not a claim of complete implementation; current
normalization limits belong beside [source resolution](../../../omega-rust/psi/pipeline/README.md#value-generic-staging).

## Applications and specialization

Type parameters use ordinary angle-bracket applications. `const N: u64` binds
a static value of the declared carrier, usable in layouts, ranges, and contracts.
`N: u64` binds a value which may be known statically or supplied at runtime.
Every application validates the complete argument tuple, including kinds,
ranges, and constraints of unused parameters. Conflicting, excessive, or
incomplete selections reject.

Ordinary type/result inference may obtain arguments from runtime arguments,
expected destinations, and selected static-machine contracts when the exact
application is determined. Inference does not waive a binder's declared carrier
or obligations. Named constants and forwarded binders must match that carrier
before canonical identity erases their source names. Authored local annotations
remain independent store obligations; inferred call-result types follow the
selected application's substituted result.

Closed static values use [canonical static identities](evaluation.md#canonical-static-identities).
Equivalent literals and named canonical values have the same application
identity. Anonymous numeric expressions land under the expected carrier's
[exact rules](numeric_values.md), not their decimal spelling. Target-semantic
observations may be arguments and retain their exact
[target dependencies](evaluation.md#target-dependent-applications).

Substitution is declaration-identity based throughout bodies, contracts, field
types, and nested applications. A local shadowing declaration does not rename
another binder. Const values retain their declared width and arithmetic policy.
Static data applications retain their generic base and exact tuple; substituted
fields determine layout, not rendered names or provenance encodings. Runtime
indices retain the same declaration identity plus exact value-subject bindings;
their future observed values are not static specialization/cache keys.

Generic fields obey ordinary ownership, borrowing, and cleanup, including the
active payload's substituted linear obligations. Concrete runtime applications
need valid concrete layout; proof-only applications do not acquire runtime
layout merely by being closed. [Binding properties](ownership.md#property-declarations)
constrain generic multiplicity independently of const range requirements.

Static dispatch and monomorphization remain the baseline for static applications.
Runtime-capable bodies may share code operating on ordinary value arguments.
Code sharing or specialization must preserve checked application identities,
contracts, ownership, and observable behavior.

## Value binders and const requirements

| Binder | Contract |
| --- | --- |
| `Count: u32` | Runtime-capable value index; a static argument is also permitted. |
| `const Count: u32` | Value must be statically known within the compiled specialization. |

`const` is a staging requirement, not an optimization hint. Omitting it does not
forbid specialization when a value is known. Adding it allows the body to rely
on static layout or static-only operations without providing a runtime fallback.
It does not describe source-variable mutability or a permission to use a mutable
variable's later contents as an earlier type index.

The following illustrates a runtime-capable application and its checked bound;
it is not an already supported parser/compiler example:

```omega
machine prefix_count<Count: u32>(items: &[u8]) -> u32
requires
    embed(Count) <= embed(items.len);
{
    Count
}
```

Both `prefix_count<16>(items)` and `prefix_count<count>(items)` are eligible when
the caller establishes the requirement. The dynamic case may lower to one body
taking Count as an ordinary argument. It does not compile a new machine for
each input value. This simple example could use an ordinary value parameter;
value-indexed applications additionally connect parameter/result types, domains,
and stored objects through the same bound subject.

For example, `data Index<Limit: u32> { value: u32 [0..Limit]; }` relates a
field to a possibly runtime limit. Constructing an instance owes the range;
the declaration does not establish an inhabitant when Limit is zero. The index
can remain proof-only when its runtime value is not needed by any operation.

Bodies must have a valid realization for their admitted arguments. A static-only
use of a dynamic binder rejects unless an explicit, checked dispatch route or
other supported realization supplies the required static value. The compiler
does not silently strengthen a public binder to `const`, add a fallback body,
or convert a runtime type choice into an unrelated statically selected type.

## Runtime index identity and storage

An application binds the exact argument value at that program point under
ordinary evaluation and lifetime rules. Later assignment to the source variable
does not retag existing indexed values. Compatibility between applications with
different runtime subjects requires the relevant checked relationship, such as
equality established by a guard; matching variable spellings is not evidence.
Mutation and dependent loans retain their ordinary invalidation and witness
preservation rules. Indexing a linear resource cannot duplicate or erase custody.

If an index is needed by executable operations, retain it as ordinary data,
an argument, or an already present descriptor field. Erase proof-only uses where
valid. No runtime proof object, JIT, hidden allocation, or global type factory
follows from value indexing. An index escaping with a result must remain bound
to that result through an explicit parameter/result or carried-data contract;
this rule does not invent existential packaging syntax or a dynamic ABI.

An extent index specifies logical shape, not storage provision. Inline fixed
arrays keep static extents, such as `[u8; Capacity]` under `const Capacity: u32`.
Runtime-capacity owners need explicit existing backing or allocation and the
appropriate initialization, size-arithmetic, lifetime, and failure contract.
A runtime capacity may remain fixed for the owner's lifetime without implying
growth or automatic reallocation.

A range such as `0..=65536` or a full `u32` carrier is not an instruction to
reserve its maximum on the stack. Fixed placement must fit the actual proved
stack plan and supply; dynamic stack layout is not granted by a value binder.
Insufficient static placement rejects; explicit runtime storage acquisition
handles its own failure. The compiler cannot silently box a value to make an
otherwise unsupported application work.

## Finite specialization boundary

A runtime-selected member of a small supported set can enter a static
specialization through explicit dispatch. For vector widths 16, 32, and 64,
each branch selects a literal width and proves its correspondence to the
runtime choice. This does not make an arbitrary runtime expression a `const`
argument. A general fallback must be authored, or the caller must establish
membership in the supported set or handle an unsupported result.

Finite integer representation alone is not a request to enumerate every value.
Neither a range proof nor generic syntax authorizes uncontrolled specialization
across widths, datatypes, operations, and other configuration axes. SIMD target
availability, legal lane shapes, and immediate constraints remain separate
obligations. Derived indices such as lanes from byte width and element size need
their exact arithmetic and divisibility relationships, not independent numbers
that happen to agree at existing call sites.

Squalr's runtime-selected 16/32/64-byte scanners, repeated datatype comparison
families, and const-only rotation bridges motivate reducing manual dispatch.
Preserving a selected width together with its prepared comparator is a useful
customer beyond storage sizing. The binder decision does not itself define
automatic dispatch generation, finite generic methods on dynamic interfaces,
or packaging of differently represented specialized results. Those routes need
explicit coverage, ownership, and representation design before implementation;
existing explicit branches and static calls remain valid. A dynamically loaded
implementation does not become statically known merely because an index is finite.

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
