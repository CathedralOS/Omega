# Generic declarations and static machine parameters

Generic declarations bind types, values, machine declarations, or explicit
conformance evidence. Value binders distinguish runtime-capable `Count: u32`
from static `const Count: u32`. Bodies are checked against their declared
requirements; a concrete selection cannot silently strengthen the generic API.
Runtime indices follow [dependent-value rules](dependent_values.md), not
const-generic substitution. Lifetime binders follow [lifetimes](lifetimes.md).
These are source contracts, not a claim of complete implementation; current
normalization limits belong beside [source resolution](../../../omega-rust/psi/pipeline/README.md#value-generic-staging).

An explicit `where machine Step(...)` or `where machine Step satisfies
Trait::requirement` clause can determine the kind of Step introduced without
a kind marker in the generic list. Retaining `machine Step` there is also legal.
The clause is a declaration, not inference from body usage or the current
consumer set. Missing or conflicting kinds/contracts reject. This rule applies
to selected named bodies and does not infer whole-trait conformance evidence.

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

For example, this default-domain coupling relates a field to a possibly runtime
limit:

```omega
data Index<Limit: u32>
where
    value < Limit,
{
    value: u32;
}
```

Constructing an instance owes the coupling; the declaration does not establish
an inhabitant when Limit is zero. The index
can remain proof-only when its runtime value is not needed by any operation.

Bodies must have a valid realization for their admitted arguments. A static-only
use of a dynamic binder rejects unless an explicit, checked dispatch route or
other supported realization supplies the required static value. The compiler
does not silently strengthen a public binder to `const`, add a fallback body,
or convert a runtime type choice into an unrelated statically selected type.

## Runtime index identity and storage

Type equations and domain-application matching follow the rules below; knowing
the type of a runtime argument does not make that argument's value static.

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

## Static type equality

`==` between type expressions states exact normalized type equality, not value
equality, assignability, compatible layout, or conformance. Generic constraints
may combine these propositions with ordinary Boolean connectives:

```omega
where
    T == u16 || T == u32 || T == u64
```

Each operand must resolve in its type role. A type/value mixture rejects rather
than converting a type to a runtime identifier. These propositions also serve
as static branch conditions: a branch with established `T == u32` may use that
equality to check its operations and values as u32. On leaving the branch, only
facts common to its surviving predecessors remain. An unspecialized body checks
every admitted alternative; one consumer cannot authorize an otherwise invalid
generic body. Type parameters remain static even when their admissible set has
several alternatives. No runtime type objects or RTTI are introduced.

A [case `where` clause](data_and_literals.md#case-constraints) can state the same
type equations for that constructor. Construction proves them; a case match
recovers them as local facts, including when checking a result against an
enclosing generic return type. The generic parameters remain fixed. This does
not introduce constructor-local hidden types or change ordinary specialization
and layout requirements.

Comparisons of closed static types resolve during compilation. Types containing
runtime indices retain the corresponding exact subject/equality obligations;
the static comparison facility cannot turn an unknown runtime endpoint into a
constant or silently generate a runtime type test.

One normalizer owns source equality, generic matching, and canonical type
identity. It retains exact nominal owner, carrier, generic arguments, reference
access/lifetimes, and domain roles/provenance. Equal layout or equal bytes do not
identify different nominal types. Transparent aliases use their specified
expansion; independently named domains are not merged because a proof suggests
that their predicates have the same inhabitants. Type equality does not perform
ambient conformance selection or permit changing the meaning of `==` on values.

## Structural type equations and inference

A `where` type equation relates already declared binders. Its structural matching
may recover omitted type/value arguments from known type structure:

```omega
domain<const Capacity: u64> u64::AtMost<Capacity>
    requires self <= Capacity;

data TinyBytes<Length, const Capacity: u64>
where
    Length == u64::AtMost<Capacity>
{
    storage: [u8; Capacity];
    length: Length;
}
```

`TinyBytes<u64::AtMost<256>>` binds Capacity to 256 without constructing a dummy
Length value. The array has a static extent and length remains an ordinary
runtime field. This illustrates the settled inference contract, not current
parser or compiler support. Ordinary initialization and invariant checking
remain required. A general element-owning vector also needs its initialized
prefix and element-disposition protocol; the equation does not supply those.

Every recovered parameter has an explicit binder and declared kind. Match known
structures by exact constructor and parameter position, including domain
applications, fixed arrays, and declared generic applications. Repeated occurrences
must agree under defined normalization. An explicitly supplied argument is fixed;
it cannot be overwritten by inference. Missing, cyclic, conflicting, or
underdetermined bindings reject with a request for an explicit argument. An
occurs check prevents a parameter being defined through itself.

Bracketed scalar range shells are not type structure and cannot infer generic
arguments. Use explicit type/value/const binders and equations, or an intentionally
named indexed domain when its nominal identity is part of the API. Bounds proved
by `where`, `requires`, `ensures`, or flow establish obligations after selection;
they never choose specialization or layout identity.

For ordinary data and machine applications, omitted arguments can be recovered
only when the combined structural equations and existing argument/result
inference determine one substitution. Unanchored disjunctions do not choose a
type or integer by declaration order. All applicable occurrences must be
consistent, not just the first one visited. Named conformance selection retains
its [explicit telescope rule](conformances.md#declaration-and-selection); this
does not introduce ambient conformance search or omit its required arguments.

Inference selects arguments before checking ordinary compatibility. For example:

```omega
machine upper_bound<const N: u64>(value: u64::AtMost<N>) -> u64 {
    N
}
```

Calling this with a value declared `u64::AtMost<256>` and omitting N selects 256
from its explicit domain argument, not a tighter bound on that particular value.
The applications `AtMost<256>` and `AtMost<512>` remain distinct; containment of their
predicates supplies no implicit variance. An author can explicitly qualify a
value for `AtMost<512>` after establishing its predicate, then pass that value to
`upper_bound<512>`. The TinyBytes equation requires exact qualified type equality.

Temporary branch facts, a literal's observed value, or a satisfier's stronger
private contract cannot redefine declared type structure for this inference.
They can prove obligations after selection. If an actual lacks the required
domain-application structure, supply the argument explicitly and establish the
required qualification rather than deriving a type from a value's proved bounds.
Index expressions may be bound as a whole; matching `AtMost<N>` can bind the
argument `Limit + 1`, but solving `N * 2 == 256` is not structural inference.
A `const` index needs static inputs; a runtime index can bind only where
the value binder and its exact subject/lifetime rules permit dynamic values.

A zero-argument machine `upper_bound<const N: u64>() -> u64 { N }` merely
returns an already bound N. `upper_bound<N>()` needs no reflection, and
`upper_bound()` cannot discover an unconstrained N from nothing. No compiler
primitive with the name upper_bound is required.

## Canonical domain-index matching

Closed domain arguments normalize under their declared carrier and exact selected
arithmetic, using the ordinary static identity rules. For example,
`u64::AtMost<128 + 128>` and `u64::AtMost<256>` name the same application.
Argument formation and evaluation retain their own obligations; normalization
cannot repair an ill-formed expression. Bounds inferred from contracts are proof
facts, not an additional scalar type constructor or a source of generic arguments.

Use transparent alias expansion and defined canonical normalization, not general
predicate equivalence or heuristic theorem search. Symbolic indices retain
their exact bindings and permitted normalized expressions. A repeated inferred
parameter must match consistently; stronger proof automation must not change an
application's inferred constants, storage layout, or public identity. Separately
proved relationships may establish compatibility without renaming either index.

Explicitly uninhabited domains remain legal. Matching that lacks a unique index
rejects; emptiness cannot choose an arbitrary capacity. An opaque domain or an
unbounded classification does not implicitly expose a finite bound. Domain
predicates are not searched for a maximum, and equal inhabitant sets do not
collapse distinct domain identities. This is explicit application matching, not
a general greatest-member operation or satisfiability solver. Ordinary interval
reasoning may still discharge proof obligations without participating in type
identity or inference.

## Explicitly derived bounded storage

An author may deliberately use an explicit static domain argument as backing
capacity, as in TinyBytes above. That is distinct from the compiler implicitly allocating
the maximum of every runtime binder's range. A static capacity can also be
computed from explicitly supplied constants through an eligible ordinary machine
under [semantic evaluation](evaluation.md). Matching an argument already
present in type structure needs no such computation or predicate reflection.

The chosen capacity is an element count, not its representation's number of
inhabitants: a length in `0..=256` needs up to 256 elements. The byte requirement
also includes element size, alignment, and metadata. Placement and total stack
supply must fit independently before optional optimization. Runtime live length
does not change fixed backing extent; growth and spill allocation need the
container's explicit contract. Logical qualification supplies no backing or
initialization authority.

## Finite specialization boundary

An explicit finite disjunction of equalities in a generic `where` clause can
define the supported specialization family. No separate roster keyword or
reflection API is required. This example illustrates a runtime-capable method
family with one common result shape:

```omega
trait ByteScanner {
    machine scan<Width: u32>(&self, bytes: &[u8]) -> u64
    where
        Width == 16 || Width == 32 || Width == 64;
}
```

Each closed alternative binds the relevant parameter to a constant of its exact
carrier. Normalize explicit alternatives to a deterministic duplicate-free set.
For multiple parameters, each alternative supplies a complete tuple and retains
its correlations; do not silently specialize a cross-product of unrelated
runtime settings. Opaque predicates, arbitrary inequality ranges, and mere
finiteness of an integer carrier do not request enumeration. Caller membership
is a proof obligation; invalid runtime inputs need authored guards and outcomes.

A runtime-capable family may lower to generated dispatch among those closed
bodies when static-only operations require specialization. Static arguments
select their exact body directly. A `const` binder remains strict: runtime values
enter const-only kernels through a runtime-capable family or explicit branch,
not a silent relaxation of the const API. A general body outside the listed
alternatives must be authored under a contract admitting those inputs; the
compiler invents neither a fallback nor a failure outcome.

Every specialization must satisfy the public contract and be valid for its
selected target. SIMD availability, legal lane counts, instruction immediates,
and exact mask conversion remain checked independently. Derived indices such as
lanes from byte width and element size need arithmetic/divisibility evidence.
Generate only demanded static applications or the complete family needed by a
dynamic selection. Compilation resource exhaustion reports incomplete production,
not permission to drop cases, change behavior, or emit unbounded JIT code.

Dispatch owns one exact selected index throughout an operation, including calls
into its specialized loop. Width selection need not recur per SIMD instruction;
per-region scan methods make that boundary explicit. Repeated public invocations
may still dispatch, and unknown plugin implementations remain dynamic. A finite
width set does not prove a particular machine instruction or performance result.

[Finite dynamic families](../terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
retain canonical requirement-and-tuple rows in one named conformance. A result
may remain inside a statically selected branch or use a common representation.
If different specialized results must escape together, author an ordinary finite
sum with exact case payloads or an already specified owner/descriptor contract.
Index and result custody stay joined. No implicit boxing, arbitrary existential
type, or unspecified variable-layout result ABI is introduced.

## Reflection boundary

Static type equality and structural parameter binding do not require runtime
type objects. [Semantic reflection](reflection.md) describes an explicitly
selected authorized type and specializes ordinary typed member callbacks using
these same family and invocation-lifetime contracts. Its owned semantic schema
is distinct from the target-resolved [layout schema](../layouts/plans.md).
Reflected identity uses canonical type equality, not names or matching layouts.
Runtime metadata does not become a generic type argument; dependent applications
retain exact subject relationships rather than guessed static type keys.

An explicit per-member selection policy receives the actual qualified field
type as a static parameter. It selects an exact operation or conformance through
the [scoped selection receiver](reflection.md#explicit-operation-selection),
not ambient discovery or a new field-declaration binder. Policies may use
generic type rules and member-specific exceptions. Reflection does not weaken
types before generic binding, change exact `==` into assignability, or make
selection records into general first-class conformance arguments.

## Requirements and conformance evidence

Generic code proves declared preconditions and may use declared guarantees.
Calls through a requirement retain its complete
[substitution contract](machines.md#substitution), including reach, independent
suspension/blocking possibilities, crashes, progress, and context-visible
resources. Selected implementations must refine that contract. Reach through a
nominal static callable uses the published
[reach dependency](effects.md#static-callback-reach-dependencies), specialized
from the selected public contract within the requirement's bound. Ordinary call
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

The table describes declaration-supply roles. Mathematical proof applications
add the separately checked [logical-evidence supply](../proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use)
for machine-shaped theorem hypotheses; it is not executable callback selection.

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
the signature. Calling it contributes a reach dependency automatically; the
consumer authors neither a forwarding clause nor a second reach bound.
Suspension/blocking remain fixed by the requirement, not the selection's narrower
behavior. Declaration selection obeys ordinary visibility and direct-
dependency rules, including nested contracts. A signature-free requirement
path must identify one overload independently of its satisfiers.

A declaration-identity binder accepts one exact free-machine or signature-free
trait-requirement declaration. Types, conformances, runtime values, and overloaded
requirement families reject. No default, law, or consumer may invoke that binder
without a callable contract. This category expresses relationships such as a
private callback slot's selected requirement; it is not missing-contract inference.

Each executable use of a static-machine argument becomes a direct selected call
after specialization. The binder is not a runtime function value or hidden
callable argument. Stateful behavior uses ordinary instance data and its receiver
access contract. Authors construct ordinary context data and explicitly select
the named callback declaration. No anonymous body/environment association or
free-variable capture inference is provided. Dynamic selection still uses
ordinary sums/wrappers or eligible dynamic conformances.

A static parameter cannot be stored as a field type, converted to an address,
or returned as a runtime callback reference. [Private callback realization](../build/private_callbacks.md)
is contextual: an exact selected requirement and destination authorize a private
entry relocation, not a general reified machine value.

## Invocation-lifetime families

A static callable requirement can bind invocation lifetimes independently of
the environment's captured lifetimes. The selected body must refine the complete
requirement for every admissible choice of those invocation binders. Matching
introduces fresh arbitrary lifetimes, substitutes both contracts by declaration
identity, and verifies parameter/result correspondence, loans, and all other
refinement axes without stronger premises. Rename-bound lifetimes match by their
binding structure; matching a single existing environment lifetime is insufficient.

Each invocation instantiates the requirement afresh. Sequential calls may end
their context/item reborrows between invocations without ending the environment's
longer-lived captured loans. A callback cannot retain an item loan in a context
whose lifetime/type contract does not permit it. Ordinary rejection does not
require general authored outlives syntax. Returned views retain their declared
input relationships; fresh instantiation cannot change which storage they denote.

This higher-ranked matching rule is limited to static callable requirement
families and their exact selected bodies, including corresponding generic
requirement members. It is not a blanket extension to whole-conformance lifetime
applications, lifetime constants, variance, subtyping, or generic dynamic tables.
Those forms retain their separate restrictions. Ordinal substitution identifies
binders; it does not by itself prove universal refinement.

Type-generic callable families similarly check every admitted type alternative.
Consumers prove each demanded application satisfies the declared family rather
than inferring an interface from their current instantiations. Reflection adds
member projection obligations, not a different generic matching judgment.

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
Executable machine-symbol binders do not replace general mathematical
function/predicate [term binders](../proofs/mathematical_bindings.md).
Those elaborate to the [selected foundation](../proofs/foundation.md), whose
mathematical universes and conversion are not defined by machine-symbol lookup.

`u: core::Level` adds the checked universe-level binder category to generic
telescopes; it is not a runtime integer parameter. Mathematical function values
can appear as ordinary parameters and appropriate generic value indices under
their declared staging rules. Logical applications of machine-shaped hypotheses
may receive checked term evidence under the
[logical-use contract](../proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use).
This does not widen executable callback selection or make ordinary machine
calls partial. Checked interfaces retain logical-evidence versus executable
declaration supply, full level telescopes and constraints.

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
