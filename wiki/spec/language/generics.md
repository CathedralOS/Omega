# Generic declarations and static machine parameters

Data declarations additionally admit [`some` binders](some_packages.md): their
witness is chosen per value and omitted from the external type application.
This is not a machine generic binder and does not replace `machine F`.

Generic declarations bind types, values, machine declarations, or explicit
conformance evidence. Value binders distinguish runtime-capable `Count: u32`
from static `const Count: u32`. Bodies are checked against their declared
requirements; a concrete selection cannot silently strengthen the generic API.
Runtime indices follow [dependent-value rules](dependent_values.md), not
const-generic substitution. Lifetime binders follow [lifetimes](lifetimes.md).

Requirement spelling follows [unified requirement clauses](state_contracts.md#requirement-clauses);
the spelling does not merge static admission with runtime entry obligations or
flatten nested callable contracts.

A static callable parameter is written `const Step: machine(...)` for a
structural contract or `const Step: Trait::requirement` for a nominal one
under [typed static parameters](#typed-static-parameters).
The type is an originating contract, not a contract guessed from an
invocation's shape or the current consumer set. A `const Step: machine`
parameter on a wrapper may instead acquire its exact type by
[inference](#inferred-callable-parameter-types). Missing or conflicting
contracts reject. Neither route infers whole-trait conformance evidence.

## Generic machine requirements

Named decision: `generic-requirement-evidence-binders`, settled 2026-11-03.

A machine requirement admits the same generic binder kinds as an ordinary
machine, including a named implementation parameter such as
`const Order: Ordering<T>`. This applies to members of ordinary and boundary
traits and to static callable requirements; declaring a requirement does not
remove a binder kind that its checked implementation may use.

```omega
boundary trait Sorting {
    machine sort<T, const Order: Ordering<T>>(
        items: &mut [T]
    )
        crashes none;
}
```

Here `Ordering<T>` is an ordinary declared trait. The caller supplies the exact
named ordering implementation at each static application, just as for an
ordinary generic machine. The selected body declares the matching ordered
binders, kinds and requirements and satisfies the complete requirement contract
for every admitted application. Binder names need not match; identities and
substitution must. Fixing an arbitrary concrete implementation inside the body
cannot implement a requirement universally quantified over `Order`.

For a boundary requirement, Build selects the realizing implementation for the
target. It does not choose or fill `Order`, enumerate all possible generic
arguments, or strengthen the caller's requirements. Ordinary specialization
retains the caller's exact static selections; runtime-capable value binders
retain their ordinary value-argument meaning. Executable applications follow
ordinary demand and realization rather than a new runtime generic engine.
This introduces neither a generic foreign ABI nor implicit native adapters.

Static generic eligibility does not confer unrestricted runtime polymorphism.
[Machine values](machine_values.md) require closed callable signatures; bind
ordinary type, machine and evidence arguments before constructing them. Explicit
finite value families retain their declared tuple contracts under
[callable dispatch](../terminal-psi/dynamic_dispatch.md#finite-generic-method-families).

[Some data binders](some_packages.md) choose a type per packaged value rather
than per external type instantiation. A generic helper may construct a package
using its concrete argument. No global roster of helper instantiations or hidden
types is needed for the package's uniform borrowed representation.

## Applications and specialization

An `asm machine` uses these same binders and machine-satisfaction rules; its
body language does not require an asm-specific generic or iterator facility.
Every admitted specialization must justify its exact instruction widths,
layouts, copying rights and target obligations. Selecting an assembly callback
does not imply textual expansion, unrolling or inlining. See
[generic assembly](assembly.md#generics-slices-and-tracked-storage).

Lexically nested machines may refer to enclosing static generic selections
(types, static values, machine selections and conformance evidence) by their
exact declaration identities. Elaboration retains that context in the nested
machine's application identity; flattening must not bind by spelling or merge
distinct outer applications. This is not a runtime closure. Runtime-value
generic subjects, like ordinary runtime parameters, must be passed explicitly
when a nested machine needs their values. Lifetime context constrains explicit
references and does not supply an enclosing activation or borrowed storage.
See [nested machines](machines.md#nested-machines-and-identity).

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

## Transparent type aliases

Named decision: `transparent-generic-type-aliases`, settled 2026-10-27.

A data declaration with `=` and a type expression is a transparent alias, not
a new nominal data definition:

```omega
public data CLayout<T> = LayoutStorage<T, CAbiPolicy<T>>;
data ByteBuffer<const Capacity: u64> = Buffer<u8, Capacity>;
data SmallBytes = ByteBuffer<64>;
```

The aliases partially apply an existing constructor: they may fix, forward,
reorder or compute arguments and expose the remaining ordinary parameters.
Their binders use the same type, lifetime, value, static-machine and named
implementation kinds and contracts as other generic declarations. Buffer and
the policy implementation above are selected ordinary declarations, not names
that grant alias-specific compiler behavior. General aliases are not the
domain-conjunction aliases described under [domains](domains.md#aliases-and-identity).

Expansion substitutes exact declaration identities, validates every underlying
argument and obligation, and precedes canonical type identity and ordinary
matching. An alias and its complete expansion are the same type, with the same
representation, ABI, multiplicity, cleanup, qualifications, access and lifetime
obligations. There is no wrapper field, constructor, conversion, additional
nominal case owner or new ownership identity. Existing data construction,
case qualification and matching name the normalized underlying carrier.
The alias cannot weaken requirements, hide an incompatible representation or
turn a conversion into type equality. Nominal distinction requires an ordinary
data declaration with a body.

Closed argument expressions may execute ordinary admitted compile-time code.
A computed const value retains its declared carrier and canonical value;
a selected machine or named implementation retains its exact code/evidence
identity. Runtime-capable indices retain exact runtime subjects and cannot be
silently promoted to compile-time constants. Alias expansion supplies no host
grant or exception to [evaluation admission](evaluation.md).

Alias expansion is acyclic, including indirect and growing generic cycles;
normalization reports the alias chain rather than looping. Name resolution,
visibility, public-signature exposure and retained private implementation
identities follow their existing rules. Exporting an alias does not grant
permission to inspect or name private constituents. The compiler retains enough
application and expansion provenance for diagnostics, serialization and replay
without treating the alias spelling as a distinct semantic or ABI key.
This facility does not introduce first-class higher-kinded arguments or ambient
conformance search.

## Closed applications and canonical identity

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
requires
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

[Typed runs](../resources/typed_runs.md) own a runtime count of established
elements over supplied backing without creating a runtime-sized inline `[T; N]`
value. Their protected custody and cell transformations supply the initialized
prefix contract needed by a library vector; a count or byte slice alone does not.

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
requires
    T == u16 || T == u32 || T == u64
```

Each operand must resolve in its type role. A type/value mixture rejects rather
than converting a type to a runtime identifier. These propositions also serve
as static branch conditions: a branch with established `T == u32` may use that
equality to check its operations and values as u32. On leaving the branch, only
facts common to its surviving predecessors remain. An unspecialized body checks
every admitted alternative; one consumer cannot authorize an otherwise invalid
generic body.

Outside such a branch, an operation on a value of type `T` is well-typed when
every alternative the body admits supports it, and its obligations are owed for
every alternative: `requires T == u32 || T == u64` with `{ left + right }`
checks the addition as `u32` and as `u64`, whichever a caller uses. Checking the
body once per listed alternative, each with `T` fixed to that type, satisfies
this rule for a finite family; the list is closed, so those checks cover every
admissible use. An open `T` has no such list and uses only what its bounds
promise.

A type equation in a contract guards what follows it, as a case test does:
in `(T == u8 && result == 0u8) || (T == u16 && result == 0u16)`, each
conjunct after `T == u8` is typed with `T` fixed to `u8`. Conjuncts are read
left to right; a disjunct whose type equation is false for the instantiation
holds vacuously. Type parameters remain static even when their admissible set has
several alternatives. No runtime type objects or RTTI are introduced.

A [case-dependent default domain](data_and_literals.md#case-constraints) can
relate case membership to these type equations. Construction proves it; a case
match combines its established predicates with the selected case to recover the
equation, including when checking a result against an enclosing generic return
type. Named generic domains use the same reasoning. The generic parameters
remain fixed. This does not introduce constructor-local hidden types or change ordinary specialization
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

A `requires` type equation relates already declared binders. Its structural matching
may recover omitted type/value arguments from known type structure:

```omega
domain<const Capacity: u64> u64::AtMost<Capacity>
    requires self <= Capacity;

data TinyBytes<Length, const Capacity: u64>
requires
    Length == u64::AtMost<Capacity>
{
    storage: [u8; Capacity];
    length: Length;
}
```

`TinyBytes<u64::AtMost<256>>` binds Capacity to 256 without constructing a dummy
Length value. The array has a static extent and length remains an ordinary
runtime field. Ordinary initialization and invariant checking
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
by `requires`, `ensures`, or flow establish obligations after selection;
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

[Stable canonical index identity](../proofs/contracts.md#stable-canonical-index-identity)
uses constant evaluation, transparent aliases, and fixed simplifications for
exact core operations and arithmetic policies. User algebra laws and
distributive expansion establish [compatibility](../proofs/contracts.md#proved-index-compatibility)
without renaming indices. A caller's proof budget cannot change inferred
arguments, declaration matching, or specialization keys.

Open `Buffer<A * (B + C)>` and `Buffer<A * B + A * C>` can exchange
values through checked equality while retaining separate canonical keys.
A conformance for one exact form does not automatically match the other.
Use a common canonical expression/alias when declaration identity is required;
generic index parameters and contracts can expose the intended relationship
without embedding one spelling in every declaration. Separate symbolic keys
may yield separate specializations, while fully closed equal values still
share canonical value identity.

Root Build controls recorded proof-discovery effort. An unresolved obligation
may need a larger search allowance or an explicit theorem, but an unequal key
is not evidence of mathematical inequality. Carry checked derivations through
specialization and published artifacts without re-searching under a consumer's
smaller discovery budget. Ordinary formation, custody and trust obligations
remain in force.

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

An explicit finite disjunction of equalities in a generic `requires` clause can
define the supported specialization family. No separate roster keyword or
reflection API is required. This example illustrates a runtime-capable machine
family with one common result shape:

```omega
trait ByteScanner {
    machine scan<Width: u32>(bytes: &[u8]) -> u64
    requires
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

[Finite callable families](../terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
retain canonical contract-and-tuple rows independently of target discovery.
Static conformances retain their own complete requirement maps. A result
may remain inside a statically selected branch or use a common representation.
If different specialized results must escape together, author an ordinary finite
sum with exact case payloads, a uniform borrowed [some package](some_packages.md),
or an already specified owner/descriptor contract.
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
the [scoped selection context](reflection.md#explicit-operation-selection),
not ambient discovery or a new field-declaration binder. Policies may use
generic type rules and member-specific exceptions. Reflection does not weaken
types before generic binding, change exact `==` into assignability, or make
selection records into conformance arguments. Separately, compile-time code may
compute and return an [operation record](conformances.md#implementations-are-operation-records)
under ordinary typed reflection and access rules.

## Requirements and conformance evidence

Generic code proves declared preconditions and may use declared guarantees.
Calls through a requirement retain its complete
[substitution contract](machines.md#substitution), including reach, independent
suspension/blocking possibilities, crashes, progress, and context-visible
resources. Selected implementations must refine that contract. Reach through a
structural or nominal static callable, or a selected conformance member, uses
the published
[reach dependency](effects.md#static-callback-reach-dependencies); suspension,
blocking, crashes and termination use the other rows of
[static binder contract dependencies](effects.md#static-binder-contract-dependencies).
Generic bodies use the abstract requirement envelope. Callers substitute selected
published contracts into those dependencies, not incidental stronger behavior.
A wrapper publishes a selected callback's result guarantee only through an
authored [result relation](effects.md#callback-result-relations) claim.

A required member operation belongs to a named trait and is supplied through an
explicit implementation parameter, for example `const Order: Ranked<Element>`.
It is a static parameter of the trait's
[operation-record schema](conformances.md#implementations-are-operation-records).
There is no anonymous member requirement inferred or carried by a
`T::member(...)` clause. A structural contract for an independently
supplied callable parameter is the distinct form below.

[Named conformances](conformances.md#declaration-and-selection) owns whole-map
selection and explicit arguments to generic conformance applications. Expected
shape validates an explicitly selected map; it does not discover one or fill
its non-lifetime arguments. An already-closed evidence binder forwards its map.
Explicit trait type parameters supply all data types needed by the interface.
There is no implementing data subject, `Self` binder, or associated-type
declaration surface. Evidence parameters name implementations, not data types.

Omitted service reach on a bodyless non-boundary requirement defers to the
selected member's published row, and omitted `crashes` defers to its published
routes. An explicit clause bounds that row, with `reaches none` requiring it
to be empty and `crashes none` admitting only crash-free members. Omitted
`suspends` and `blocks` still mean never, since their markers need a known
bound. A public generic whose body may crash under those requirements states
its crash ceiling like any other public machine. A conformance evidence binder retains a
separate dependency for each called requirement member and exact application.
Calling `Order::compare` cannot substitute the reach of another row in `Order`.

<a id="static-machine-binder-categories"></a>
## Typed static parameters

Named decision: `typed-static-parameters`, settled 2026-11-10;
[the design draft](../../drafts/designs/typed_static_parameters.md) records the
migration. A generic parameter list has one binder form, `name: Type`,
optionally marked `const`. A machine, an implementation and a machine
description are ordinary typed values passed through that form. What were
separate binder species are types:

| Role | Parameter | Permitted use |
| --- | --- | --- |
| Structural callable | `const Key: machine(card: &Card) -> u64` | Static calls under that complete callable contract. |
| Nominal callable | `const Handler: HookProcedure::call` | Static calls or contextual realization under that exact requirement. |
| Declaration identity | `const Requirement: reflect::CallableInfo` | Identity relationships only; no invocation. |
| Implementation | `const Order: Ranked<T>` | Calls to its exact selections, written `Order::member(...)`. |
| Inferred callable | `const F: machine` | As inferred under [inferred callable parameter types](#inferred-callable-parameter-types). |

```omega
machine apply<const F: machine(value: u32) -> u32>(x: u32) -> u32 {
    F(x)
}

machine sort<T, const Order: Ranked<T>>(items: &mut [T]) { ... }

machine install<const Handler: HookProcedure::call>(slot: &mut HookSlot) { ... }
```

`const` says when the value is known. It does not require the selected machine
to execute during compilation, to be effect-free or to be inlined, and forming
a selection performs none of its effects. An optimizer's prediction of a
runtime callable's target never satisfies `const`. Value binders and runtime
indices keep the rules of [value binders](#value-binders-and-const-requirements):
a non-`const` index has an exact runtime value subject and is never a
specialization key. The generic list and the ordinary parameter list stay
separate; this decision unifies only what the generic list contains.

### Callable parameter types

A structural callable type is the signature and clauses of a
[callable contract](effects.md#open-callable-contracts). A callable parameter
needs a complete originating or inferred type. The compiler cannot invent one
from direct invocations, matching signatures, visible conformances or the
current consumer set, even with one whole-program instantiation. Missing or
ambiguous types reject. If only one implementation is intended, source can call
that machine directly without introducing a generic abstraction.

A requirement path in type position, `Trait<Arguments>::member`, is the type of
a callable selection bound to that exact requirement. It inherits the
requirement's complete shape, conditions, operational ceilings, and any
boundary calling/entry plan, and does not repeat the signature. A machine with
a matching signature is not a value of that type; only a selection
[bound to the requirement](conformances.md#binding-a-machine-to-a-requirement)
is, and nothing about an argument position creates that binding. A
signature-free requirement path must identify one overload independently of
its satisfiers. A path in type position resolves by the kind of its left-hand
name: a trait gives a requirement type, and a data or scalar type gives a
[nested type](data_and_literals.md#records-sums-and-nested-declarations) or a
domain-qualified type.

Both nominal and structural callable invocations contribute reach and crash
dependencies under the type's open or finite bound. The consumer authors
neither a forwarding clause nor a second row argument. The generic body checks
suspension/blocking against the type's upper bound. Applications substitute
published
[operational dependencies](effects.md#static-callback-operational-dependencies)
to determine their caller envelope. Declaration selection obeys ordinary
visibility and direct-dependency rules, including nested contracts.

An open callable contract may appear wherever a value is static: a `const`
parameter, a `const` local, a field of a static record or a compile-time
result. Runtime materialization requires the closed form and a finite envelope.
Static selection supplies no purity, termination or proof admissibility of its
own.

A callable type may take static parameters of its own, written
`machine<const Inner: ...>(...) -> ...`. Such a type is a family: it is usable
only where the value is static, and it is applied with explicit arguments
under [nested callable types](#nested-contracts-and-proof-families).

Prose elsewhere that speaks of a static machine binder or an evidence binder
means a `const` parameter of callable type or of implementation type.

### Declaration identity

`reflect::CallableInfo` is the type of a parameter that identifies a machine or
requirement without permission to call it. It accepts one exact free-machine or
signature-free trait-requirement description from
[`reflect::callable`](reflection.md#typed-declaration-selectors). Types,
conformances, runtime values and overloaded requirement families reject. No
default, law or consumer may invoke through it. This role expresses
relationships such as a private callback slot's selected requirement; it is not
missing-contract inference.

A `CallableInfo` is a canonical static argument. Independently obtained
descriptions of the same exact declaration and application denote the same
argument; descriptor allocation, display metadata and graph-local handles do
not distinguish them. A generic declaration family and a closed executable
application remain different descriptions.

### Static calls

Each invocation of a static callable argument becomes a direct selected call
after specialization. The parameter is not a runtime function value or hidden
callable argument. Stateful behavior uses ordinary context data passed through
explicit parameters with their normal ownership and access contracts. Authors
construct that data and explicitly select
the named callback declaration. No anonymous body/environment association or
free-variable capture inference is provided. Dynamic selection uses
[runtime machine values](machine_values.md), operations records and
[some packages](some_packages.md), and the same callable type may type a
runtime parameter when it is closed.

A static selection whose executable application is closed may construct or
return a machine value through ordinary callable-contract refinement. It may
also be observed with `address_of<Selected<Arguments>>()` under
[address exposure](counts_and_addresses.md#address-exposure). That query
creates an entry demand, not a callable value or an implicit callback selection.
[Private callback realization](../build/private_callbacks.md) remains contextual:
an exact selected requirement and destination authorize the private ABI entry
relocation; an exposed numeric address cannot replace that relationship.

### Retired spellings

`<machine F>` with `requires machine F(...);` is written
`<const F: machine(...)>`. `<machine F>` with
`requires machine F satisfies Trait::member;` is written
`<const F: Trait::member>`. A `machine Requirement` binder with no clause is
written `<const Requirement: reflect::CallableInfo>`. `<Order satisfies Ranked<T>>`
is written `<const Order: Ranked<T>>`. `satisfies` remains the spelling of
declarations and of the binding expression. Mathematical declarations keep
their machine-shaped hypotheses and binders under
[logical hypotheses](../proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use);
this decision covers executable code.

<a id="forwarded-callback-requirements"></a>
## Inferred callable parameter types

`const F: machine` leaves a callable parameter's type to be inferred and
published. It replaces `forwarded-static-callback-requirements` (2026-11-06),
whose behavior it keeps as ordinary parameter-type inference, with one change
noted below.

```omega
public trait ElementStep<T> {
    machine apply(item: &mut T) terminates;
}

public machine apply_one<T, const F: ElementStep<T>::apply>(item: &mut T)
terminates
{
    F(item);
}

public machine visit_one<T, const F: machine>(item: &mut T)
terminates
{
    apply_one<T, F>(item);
}
```

`visit_one` publishes `const F: ElementStep<T>::apply`. Neither machine
searches for an implementation; the consumer still supplies a selection bound
to that requirement.

- A typed use supplies a constraint: passing the parameter as a static argument
  to an exact selected callee contributes that callee's published parameter
  type, after substituting the wrapper's arguments and retaining nested
  parameters.
- Every use participates, independent of source order.
- A bare call `F(item)`, a matching machine signature or a visible conformance
  is not an origin. Recursive forwarding needs an exact originating type; a
  cycle alone cannot invent one.
- The inferred type is published with exact substitutions, origin provenance
  and ordinary visibility checks, independently of the implementations current
  callers select. Public consumers check it without reading the wrapper's
  private body. Changing a use can change that public contract and requires
  ordinary dependent revalidation. Inferred and authored copies normalize
  identically, and diagnostics expose the inferred type and the use that
  requires it.
- An authored type is the bound. A use checks against it and cannot strengthen
  it to admit an otherwise invalid body.
- Structural constraints from several uses combine: the selection must satisfy
  all of them, including nested preconditions, guarantees, progress and
  operational permissions. Incompatible shapes reject.
- One parameter carries one nominal route. Uses that require two different
  requirement types reject; the wrapper takes two parameters, or a record
  holding both views. This changes the 2026-11-06 rule, under which one binder
  satisfied the conjunction of several nominal requirements.

Omitted reach stays a deferred dependency, explicit empty reach an empty bound,
and named ceilings finite bounds through inference. In the example,
`ElementStep::apply` leaves service reach to the selected implementation and
both machines publish that same dependency; inference does not collapse it to
empty. The termination promise and the suspension/blocking restrictions are
unchanged. No inference of callback implementations, trait maps, boundary
authority or arbitrary runtime entry predicates is introduced, and a
`CallableInfo` parameter stays noncallable.

A wrapper's machine contract also retains the callee's
[conditional suspension-carry obligations](effects.md#conditional-suspension-carry).
Re-express conditions over the wrapper's exact callable parameters and substitute
the corresponding resource subjects and provenance. These rows constrain the
enclosing application; they do not strengthen the parameter's originating type.
Preserve scoped local acquisitions as such rather than exposing a caller-supplied
permission for them. Ordinary inference cannot discard an unresolved condition.

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

A callable type may bind static parameters of its own:

```omega
machine forward<
    const Schema: machine<const Inner: machine(index: Nat) -> Rat>(
        value: Stream<Inner>
    ) -> Stream<Inner>,
    const Selected: machine(index: Nat) -> Rat
>(value: Stream<Selected>) -> Stream<Selected> {
    Schema<Selected>(value)
}
```

`Inner` belongs to the type of `Schema`; `Selected` belongs to `forward`. Each
callable type carries its own clauses, so nesting needs no terminators. The
machine-shaped hypotheses of a mathematical declaration keep one terminator
for each nested bodyless callable under
[machine header delimiters](state_contracts.md#machine-header-delimiters).

Here `Stream` denotes an appropriate proof-only family. Refinement is
positional: renaming `Inner` changes no contract, but changing its nested shape,
conditions, reach, operational/crash ceilings, or termination guarantee can.
Forwarding a distinct callable parameter uses the same complete refinement judgment.
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

A public callable parameter's identity is its complete normalized type: a
recursively alpha-normalized structural contract, one exact nominal
requirement, or the noncallable `reflect::CallableInfo`. Inferred types carry
the same identity obligations as authored ones; structural constraints from
several uses are retained without signature-based merging. Closed applications
retain exact selections.
Renaming parameters is stable; changing a nested contract or its authority is not.
Private nominal requirements and missing checked contract evidence reject public
review under [package boundaries](../packages/boundaries.md).

Generic axioms spend admission on the normalized template and required machine
contract, not separately on each instance. Instances retain selected contract
identity and the template receipt. Instance-specific trust instead uses an
explicit nongeneric admitted fact. [Proof receiving policy](../proofs/contracts.md#axioms-and-receiving-policy)
owns those grants; specialization never launders admitted premises into proofs.
