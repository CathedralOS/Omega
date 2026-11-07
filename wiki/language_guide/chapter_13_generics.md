# Chapter 13: Generics

Generics let one declaration work over different types, values, or named
implementations. A value binder can be runtime-capable or explicitly `const`.
The body is checked against its declared requirements; uses select applications
satisfying those requirements. Static applications can monomorphize; dynamic
value arguments can share ordinary code. Neither changes ownership or proof rules.

[Generic declarations](../spec/language/generics.md) owns the source contracts.
Examples here assume their named types, traits, and conformances are in scope;
they illustrate intended semantics rather than every current implementation path.

## Generic Data

```omega
data Optional<T> {
    #0 case None;
    #1 case Some(value: T);
}

data Pair<A, B> {
    first: A;
    second: B;
}
```

`Optional<T>` is ordinary cased data, not a special language feature. Its empty
case can provide the zero representation without constructing `T`. That property
comes from the home representation and its checked zero contract, not the name
`Optional`. Packages can declare other domain-specific generic sums.

Concrete runtime instances need valid concrete layouts. Their fields obey
ordinary ownership, borrowing, and cleanup; a generic wrapper does not erase
an active payload's substituted linear obligation. Proof-only instances do not
acquire runtime layout merely by being closed.

## Transparent Aliases

An alias supplies arguments to an existing type expression without declaring
another nominal carrier:

```omega
public data CLayout<T> = LayoutStorage<T, CAbiPolicy<T>>;
data ByteBuffer<const Capacity: u64> = Buffer<u8, Capacity>;
data SmallBytes = ByteBuffer<64>;
```

This is partial application: fix some arguments and expose the remaining ones.
An alias may forward or reorder arguments and use admitted compile-time
expressions for static values. Selected machines and implementations retain
their exact code identities. The underlying constructor's constraints still
apply; an alias cannot attenuate them or hide an incompatible representation.

Aliases add no wrapper, storage, conversion or independent type identity.
Their generic arguments expand before ordinary matching, layout and ABI
selection. Domain aliases are a separate existing facility, and this general
type-alias surface is ratified implementation work, not a claim that the current
parser already accepts it. See [transparent type aliases](../spec/language/generics.md#transparent-type-aliases).

## Generic Machines

A generic machine can require one explicitly passed conformance:

```omega
machine equal<T, Equality satisfies Equatable<T>>(left: &T, right: &T) -> bool {
    Equality::equals(left, right)
}
```

The body uses the selected map's requirement. Its implementation is checked
against that contract, not inferred from whichever equality implementation a
consumer happens to pass. An ordinary call supplies the type and named evidence,
such as `equal<Card, CardEquality>(&left, &right)`.

Explicit conformance selection is different from guessing a machine from its
name or finding a visible implementation. [Chapter 14](chapter_14_traits.md)
explains the complete map and its laws.

## Generic Requirements

A trait can declare the same generic interface as an ordinary machine, including
an implementation parameter supplied by the caller:

```omega
boundary trait Sorting {
    machine sort<T, Order satisfies Ordering<T>>(
        items: &mut [T]
    );
}
```

For `Sorting::sort<u32, Ascending>(items)`, the caller selects u32 and the named
Ascending map. Build independently selects the platform implementation of
Sorting. The compiler specializes that member with the caller's selections;
Build does not supply or enumerate the generic arguments. Every selected member
must honor the requirement for all admitted arguments.

That static interface is not an unrestricted generic virtual method. For a
runtime-selected implementation, parameters can instead belong to the trait:

```omega
trait TypedSorter<T, Order satisfies Ordering<T>> {
    machine sort(items: &mut [T]);
}
```

A machine field with a closed signature over `u32` may hold an explicitly
selected `TypedSorter<u32, Ascending>` implementation member. Its target can vary
at runtime; its callable contract remains fixed. An open requirement-local type or evidence binder cannot
become a dynamic slot merely because current callers happen to use one tuple;
the attempt rejects during source checking. Explicit finite value families keep
their existing rules. See [generic requirements](../spec/language/generics.md#generic-machine-requirements)
and [dynamic eligibility](../spec/terminal-psi/dynamic_dispatch.md#source-eligibility-and-operational-envelopes).

## Runtime-Capable Value Parameters

Value binders distinguish two staging contracts:

```text
<Count: u32>        runtime-capable; a static argument is also permitted
<const Count: u32>  statically known within the compiled specialization
```

`const` is a requirement, not an optimization hint. Without it, known arguments
may still specialize. With it, a declaration can require fixed layout or static
instruction operands without supplying a runtime fallback. Runtime-capable binders are
implemented for machine signatures: `machine f<Count: u32>(...)` compiles to one
body carrying `Count` as an ordinary argument, with its `requires` bound proved
at the call site. They are not implemented for data declarations, so the
`Index<Limit: u32>` example below still specifies the intended contract rather
than currently executable source, and layout-determining positions such as array
extents continue to reject. The slice-length bound shown in the next example is
likewise not what the landed canaries exercise — those bound a scalar
parameter.

```omega
machine prefix_count<Count: u32>(items: &[u8]) -> u32
requires
    embed(Count) <= embed(items.len);
{
    Count
}
```

`prefix_count<16>(items)` supplies a static value. If `count` is a runtime `u32`,
`prefix_count<count>(items)` is also eligible once the caller proves the bound.
The latter can compile to one body with an ordinary Count argument, not one
machine per observed count. A plain parameter would suffice for this example;
generic indices also connect dependent types and results:

```omega
data Index<Limit: u32>
requires value < Limit,
{
    value: u32;
}
```

An index binds the exact supplied value, not its variable name. Reassigning
`count` later does not retag an earlier indexed object. To use an object indexed
by one runtime value where another is required, establish their applicable
relationship, often equality. Dependent borrows, mutation, and linear custody
retain their ordinary rules. Retain a witness as ordinary data when runtime
operations need it; proof-only uses may erase.

A static-only use inside a runtime-capable body needs a valid bridge or rejects;
the compiler does not quietly rewrite the public binder to `const`. For a small
supported set, ordinary dispatch can select a literal specialization and retain
the equality to the runtime choice. The remaining values need an authored
fallback, a proved supported-set precondition, or an explicit failure outcome.
The finite size of `u32` is not permission to enumerate all its inhabitants.

Squalr's runtime-selected SIMD widths motivate this distinction: widths 16, 32,
and 64 select different kernels without requiring arbitrary dynamic layouts.
An explicit OR constraint can describe the family once:

```omega
trait ByteScanner<Scanner> {
    machine scan<Width: u32>(scanner: &Scanner, bytes: &[u8]) -> u64
    requires
        Width == 16 || Width == 32 || Width == 64;
}
```

The intended compiler route emits the required static bodies and dispatches at
the requirement boundary for a runtime width. The caller proves roster membership;
an arbitrary input needs an authored guard or failure outcome. There is no
automatic fallback or enumeration of the entire u32 range. Each body still owes
its target, bounds, and effect obligations. A const-only inner kernel receives
the selected constant; const itself does not become a runtime parameter.

A callable with a finite family contract supplies one row per declared width, with
one exact index shared by the scanner, comparator, and prepared state. The row
can call a specialized loop rather than redispatching on every vector. Escaping
results need a common representation or an explicit finite sum/eligible owner;
no implicit boxing or variable-layout return is introduced. The
[specialization contract](../spec/language/generics.md#finite-specialization-boundary)
and [dynamic-family contract](../spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
define the rules.

The implementation canary
`tests/omega/pass/traits/dyn_finite_family_dispatch` exercises separate generated
rows for closed widths 16 and 32. Any remaining receiver or data-subject
conformance syntax is migration debt under the named-implementation contract.
Its `dyn` surface also needs the [machine-value migration](../drafts/designs/machine_values_some.md).
The implemented limit is
narrower than "no generic virtual calls": the binder tuple must be one complete
roster tuple of closed static values; runtime width selection is not yet covered.

## Type Equality And Indexed-Domain Matching

Exact type comparisons use ordinary Boolean connectives:

```omega
requires
    T == u16 || T == u32 || T == u64
```

This restricts T to those exact normalized types, not anything with a compatible
layout or conformance. A static branch establishing `T == u32` can check its
operations using that equality. It does not need runtime reflection or a special
value-comparison implementation. Generic bodies must cover all admitted cases.

A type equation can also recover a declared generic binder from known structure:

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

`TinyBytes<u64::AtMost<256>>` infers Capacity as 256. The array extent is static;
only the live length varies at runtime. This uses no dummy Length value or
compiler bound-query intrinsic. A general vector still needs its own element
initialization/disposition rules. Placement must fit the complete stack/storage
plan; choosing capacity explicitly does not guarantee available backing.

Matching reads the exact domain declaration and its explicit index, not the
predicate's maximum or a tighter bound discovered from runtime flow facts.
Closed const arguments use canonical value identity, so `AtMost<128 + 128>`
and `AtMost<256>` select the same index after valid static evaluation. This does
not identify distinct domains merely because their predicates describe equal
sets. Empty domains remain legal; they cannot supply an unspecified capacity.
Repeated binders must agree, and explicit arguments cannot be overwritten.
Aliases use their defined expansion, and named domains retain their identities.

For example:

```omega
machine upper_bound<const N: u64>(value: u64::AtMost<N>) -> u64 {
    N
}
```

An actual declared `u64::AtMost<256>` selects N = 256 when omitted. Explicit
N = 512 does not introduce implicit variance between the distinct applications.
An explicit qualification to `u64::AtMost<512>` must establish that domain's
predicate before supplying it instead. TinyBytes likewise requires exact type
equality, not merely a proof that its length fits some larger capacity. Selection
and proof of call legality are separate. A zero-argument `upper_bound<N>()`
merely returns an already bound N; an unconstrained `upper_bound()` cannot infer it.

These are settled source rules with incomplete implementation. See
[structural inference](../spec/language/generics.md#structural-type-equations-and-inference).
[Semantic reflection](chapter_21_reflection.md) uses these same generic rules for
typed member callbacks. Structural inference itself is not declaration inspection
or permission to discover arbitrary predicates.

## Const And Proof Parameters

Static values can parameterize stored shape or contracts:

```omega
data FixedBuffer<T, const N: u64> {
    items: [T; N];
}
```

`N` is a static value of its declared carrier. Each application proves its kinds,
ranges, and constraints even if the body never uses it. A runtime length remains
a runtime witness, not an argument to this `const` binder. Use a runtime-capable
value binder when the declaration supports dynamic execution instead.

Anonymous arithmetic lands exactly under the expected carrier.
`FixedBuffer<u8, 0.1 * 70>` has length seven; `FixedBuffer<u8, 7.5>` rejects
because its exact value is not integral. Named constants and forwarded binders
must also match their declared carriers. A decimal point alone does not select
floating-point arithmetic.

Target-semantic observations may supply const arguments under the same rules.
Before target closure they remain symbolic; afterward their exact dependencies
remain in application compatibility. This does not introduce a target-native
count type or an unspecified `UInt<Bits>` carrier family. See
[target-dependent applications](../spec/language/evaluation.md#target-dependent-applications).

### Structured values and indexed domains

A canonical static index can be an integer, Boolean, fixed array, record, or case.
Its value enters identity, not the source initializer's field order or computation
trace. Eligibility needs decidable equality and one canonical encoding; it is
stricter than merely evaluating or materializing a constant.

For example, a rational index must satisfy its canonical representation contract.
An ordinary quotient constant may retain an opaque noncanonical representative,
but that alone cannot give it canonical index identity. References, dynamic
identities, and arbitrary opaque data are not static atoms. The complete rules
belong to [canonical static identities](../spec/language/evaluation.md#canonical-static-identities).

An erased domain family can use such an index:

```omega
domain<T, const U: Unit> T::Quantity<U>;
```

The domain imposes no carrier-wide arithmetic requirement. A units operation
selects an ordinary conformance supplying just the operations it needs. Closed
units and destination-typed conversions need no compiler knowledge of metres,
seconds, or scaling policy.

An open result index can be an expression over input indices. Fixed canonical
simplifications identify some expressions; checked equality proves additional
value compatibility. Selected algebra laws and ordinary theorem guarantees can
supply that proof without new citation syntax or ambient lemma search.
Stronger proof search may accept more compatible uses but cannot rename an
interface's canonical index. [Indexed domains](../spec/language/domains.md#indexed-families) and
[licensed normalization](../spec/proofs/contracts.md#licensed-normalization)
own the details.

For example, a conversion between these open indices needs an equality:

```text
Indexed<combine(A, B)>  ->  Indexed<combine(B, A)>
```

Require a trait containing `combine` and its commutativity law, then have the
caller supply a checked conformance. An ordinary binder such as
`Algebra satisfies CommutativeCombine<Index>` supplies both the operation and
the proof; `CommutativeCombine` is a library trait, not a compiler keyword.
Reassociation needs an associativity law as well. Merely carrying an index
requires neither law, and naming an operation `+` proves neither one.

The checker consumes that ordinary trait evidence to prove compatibility,
rather than changing canonical keys. There is no domain-specific selection
syntax, implicit implementation search, or need to repeat a selection beside
every expression. Citing an exact equality remains an alternative.

### Literal identities

A generic library can accept `Indexed<Count + 0>` as `Indexed<Count>` without a
proof call at each use. Its selected implementation must prove both that adding
its abstract zero preserves the operand and that this zero is the index literal
`0`. An ordinary child trait adds the connection without making every abstract
algebra use integer literals:

```omega
trait Additive<T> {
    machine zero() -> T terminates;

    machine + add(
        left: T,
        right: T
    ) -> T terminates;

    machine right_identity(value: T)
    ensures core::Identity<T>(add(value, zero()), value);
}

trait WrappingByteIndices : Additive<u8 in Wrapping> {
    machine zero_is_literal()
    ensures core::Identity<u8 in Wrapping>(zero(), 0);
}

domain<T, const Count: u8 in Wrapping> T::Indexed<Count>;

machine remove_zero<
    const Count: u8 in Wrapping,
    Arithmetic satisfies WrappingByteIndices
>(
    value: u32 in Indexed<Count + 0>
) -> u32 in Indexed<Count> {
    value
}
```

A caller selects a complete implementation, for example
`remove_zero<8, ByteArithmetic>(value)`. `ByteArithmetic` supplies the inherited
operations and proves both laws; their names have no compiler authority. The
same pattern connects multiplicative identity to `1`. Each rewrite needs only
its own laws, not a full ring or semiring. `Count + 1` is not an identity.

Compatibility proving consumes the complete selected conformance, including
inherited laws. It does not evaluate abstract machines inside type identity.
The two open applications above retain separate canonical keys even though
the value transfer is proved automatically. Core operations have a separate
fixed identity profile, so ordinary core `N + 0` can have the same key as `N`
without a user algebra binder.

Each exact operation and arithmetic policy keeps its own meaning. Signed
Saturating addition is not associative; Wrapping and Exact also retain their
different meanings. No simplification removes an Exact overflow obligation
on an authored intermediate. See
[literal connections](../spec/proofs/contracts.md#literal-linked-index-identities).

### Distributivity and bounded work

A selected algebra can prove that a value fits either of these indexed types:

```text
Buffer<A * (B + C)>
Buffer<A * B + A * C>
```

The open expressions have different canonical keys. The compiler can
automatically prove compatibility through distributivity and transport the
value with no authored cast or runtime conversion. Reordering and regrouping
need their own laws, and all operations must be well-formed.

Automatic expansion defaults to at most 64 resulting terms per product
expansion. That counts terms, not normalization steps or the magnitude of a
buffer size. The root can raise the recorded proof-search allowance through an
ordinary Build setting:

```omega
builder.proof.index_expansion_terms = 128;
```

This ratified setting is part of the pending compiler migration. It changes
proof discovery, never type identity. Other sponsored resource ceilings still
apply. If an obligatory equality remains unproved, the diagnostic names the
missing evidence or exhausted budget; it does not claim mathematical inequality.
A checked theorem can establish equality without expanding the whole product.

A generic compatibility proof travels through specialization and in published
artifacts. Consumers check it without repeating the producer's search under a
smaller allowance. A reverse-distributivity law does not invalidate a forward
proof: the old rule disabling both directions is retired.

Exact declaration and conformance matching still uses canonical keys.
Consequently, a conformance for one symbolic spelling does not automatically
match the other, and the two forms may produce separate specializations.
Use the same expression or a shared transparent alias when an API requires
exact identity. Closed indices can still evaluate to the same value and share
a key. The rules are in
[stable identity](../spec/proofs/contracts.md#stable-canonical-index-identity),
[proved compatibility](../spec/proofs/contracts.md#proved-index-compatibility),
and [Build proof-work policy](../spec/build/configuration.md#proof-work-policy).

## Machine Parameters

A static parameter can select a machine declaration:

```omega
machine Deck::score<machine Key>(card: &Card) -> u64
requires machine Key(card: &Card) -> u64;
{
    Key(card)
}
```

Calling `Deck::score<Card::power_key>(&card)` substitutes the selected symbol
and produces a direct call in that specialization. The binder is not a hidden
runtime function argument or inferred capture.

The semicolon ends `Key`'s callable contract before the enclosing machine body.
Its own `requires` or `ensures`, when present, belong before that terminator;
clauses following it belong to the enclosing machine.

Operational clauses stay inside that same callable until its declaration ends:

```omega
requires machine Key(card: &Card) -> u64
    suspends
    blocks
    terminates;
```

There are no semicolons between these clauses. The one after the last clause
ends `Key`; it is not a termination-clause delimiter. Higher-order requirements
retain one declaration terminator for each nested callable.

There are three distinct binder categories:

| Binder | What the declaration provides |
| --- | --- |
| Structural callable | An authored `requires machine Key(...)` contract. |
| Nominal callable | One exact `requires machine Handler satisfies HookProcedure::call` requirement. |
| Declaration identity | A trait's noncallable machine binder, used only to relate declaration identities. |

An originating callback contract is explicit. A wrapper may omit its repeated
clause when it forwards an explicitly kinded machine binder into an exact
callee's required slot. The compiler publishes the inherited requirement for
public and private wrappers alike. Direct calls such as `Step(item)` do not
invent a requirement, and the current set of consumers cannot select one.

The nominal form retains the originating requirement's complete signature,
conditions and operational/boundary contract without repeating it. A path with
several overloads is not an exact selection.

### Forwarding A Callback Contract

The inner operation declares what it accepts; its wrapper need not repeat it:

```omega
public trait ElementStep<T> {
    machine apply(item: &mut T) terminates;
}

public machine apply_one<T, F>(item: &mut T)
requires machine F satisfies ElementStep<T>::apply;
terminates
{
    F(item);
}

public machine visit_one<T, machine F>(item: &mut T)
terminates
{
    apply_one<T, F>(item);
}
```

The published contract for `visit_one` includes
`requires machine F satisfies ElementStep<T>::apply;`. Forwarding `F` into
`for_each<T, F>` works the same way. Multiple uses contribute all their
requirements; the selected callback must satisfy every one. An explicit
originating contract is not silently strengthened to repair an invalid body.

There is no trait lookup or implementation selection here: callers still name
their chosen machine with its required nominal satisfaction edges. Public
inferred requirements must obey visibility rules,
and changing a forwarding use can change the public contract and require
rechecking consumers. Diagnostics show the inferred clause and its originating
use. See [forwarded requirements](../spec/language/generics.md#forwarded-callback-requirements).

### Identity-Only Binders

An identity-only trait binder has no callable signature:

```omega
trait PrivateCallbackSlot<Layout, machine Requirement> { }
```

This illustrates the existing identity relationship, not a replacement core
declaration. `Layout` is an ordinary type argument; the machine argument must
identify one exact free machine or requirement, not a type, conformance,
runtime value, or overloaded family. Neither a default
nor a consumer may invoke it as if a missing callable contract were inferred.

A structural callable contract can itself bind machine parameters. Matching is
binder-positional, including all nested contracts and operational guarantees;
renaming a nested parameter is harmless, weakening its promised behavior is not.
Specialization continues through nested selections until executable calls are
direct. See [nested contracts](../spec/language/generics.md#nested-contracts-and-proof-families)
for a complete example.

Generic bodies prove a selected parameter's `requires`, use its `ensures`, and
respect its reach, suspension, blocking, crash, and progress contract. Concrete
selections must refine that public bound. The generic body uses the callback's
upper-bound suspension/blocking permissions; applications use the published
[operational dependencies](../spec/language/effects.md#static-callback-operational-dependencies)
after substituting selected contracts. Ordinary type/result arguments may
be inferred when the selected signature and value arguments determine an exact
application; that does not select conformance evidence or invent an originating
callable contract. Forwarded requirements retain already declared contracts.

Stateful callbacks take ordinary explicit context parameters; dynamically
selected operations use [machine values](../spec/language/machine_values.md).
[Some packages](../spec/language/some_packages.md) relate heterogeneous borrowed
state and its operations without exposing the state type. Invocation scratch
belongs in local bindings, not persistent context merely to make it accessible.
A static machine binder remains a declaration argument. Its closed executable
selection may construct a runtime machine value under a checked callable type.
`address_of<Selected<u32>>()` may separately expose the address
of a fully selected executable application, but the resulting number is not
call authority; see [address exposure](chapter_20_memory_layout_abi.md#exposing-addresses).
Registered foreign callbacks still use contextual realization with an exact
requirement and private destination; see
[Chapter 19](chapter_19_capabilities_effects_boundaries.md#foreign-callbacks-through-platform-adapters).

### Proof-family index telescopes

A proof-only carrier may use a machine as a static family index:

```text
Rat                       no static indices
CauchySeq<machine S>       generator S, under its full callable contract
```

The telescope is the complete ordered parameter list, not stored metadata.
One relation may compare independent generators `A` and `B`; another may require
both subjects to share `S`. Those roles belong to the relation, not a global
annotation on the carrier parameter. Finite-layout data does not thereby acquire
machine-valued fields.

An application in a proof contract instantiates a schema without itself
demanding executable monomorphization. This supports universal law statements
without reifying a runtime function. It still does not replace arbitrary
mathematical function/predicate [term binders](../spec/proofs/mathematical_bindings.md)
under the [selected foundation](../spec/proofs/foundation.md).
Logical hypotheses may consume checked theorem evidence; executable callbacks
still require declaration supply under their complete operational contracts.
[Quotients](../spec/proofs/quotients.md) owns relation-family matching and laws.

## Where Clauses

Requirements can constrain static values or the behavior of selected operations.
A required member belongs to a named trait and is supplied through an explicit
conformance binder, not an anonymous `requires machine T::member(...)` declaration.

For example, assuming `Ranked` declares the required ordering operations:

```omega
machine sort<Element, Order satisfies Ranked<Element>>(values: &mut [Element]) {
    ... // implementation and proof omitted
}

sort<Card, PowerOrder>(&mut cards);
```

When the selected conformance owns parameters, its application nests inside the
evidence argument, as in `SequenceEncoding<u8, PlayerMessage>`. Every type,
const, and static-machine argument owned by that conformance is explicit. The
expected trait application validates the resulting closed map; they do not fill
missing arguments or `_` holes. An already-closed evidence binder forwards bare.

Only lifetime arguments follow ordinary application-site elision, and only
when borrow constraints determine one unique mapping. Resolved regions remain
in semantic identity even though they create no runtime generic arguments.
See [named conformance selection](../spec/language/conformances.md#declaration-and-selection).

Static `requires` obligations are checked at instantiation. Data `requires` facts over
runtime fields instead define a default domain maintained at consumption points.
Runtime value-binder requirements are proved for the exact subjects at each
application; a runtime guard may establish them. These are different binding
times, not permission to reinterpret runtime data as arbitrary type declarations.

## Static Dispatch

Calls through explicitly selected evidence are static by default:

```omega
machine Runner::tick<T, Increment satisfies Incrementable<T>>(subject: &mut T) {
    Increment::increment(subject);
}
```

The specialization uses the selected conformance's implementation. Another
visible `increment` machine cannot replace it. Runtime machine values provide
the separate dynamic route, with their own callable evidence and custody.

## Monomorphization

```text
declaration + static arguments + exact runtime-index bindings -> checked application
```

Substitution preserves declaration identities through bodies, contracts, fields,
and nested applications. A shadowing local cannot rewrite another binder.
Substituted field types determine eligible layout, not the display spelling of
a generic origin. Runtime values do not enter a cache as if their contents were
already known constants. Inferred call-result types retain the selected static
arguments and dynamic subjects; authored local annotations still impose their
own store obligations.

Logical extent is not allocation. A runtime-capacity owner may use explicitly
supplied backing or an allocator with an ordinary failure outcome. It need not
be growable. Inline fixed storage still needs static extent and a fitting stack
plan; neither `0..=65536` nor a full `u32` range orders the compiler to reserve
the maximum. Runtime-dependent indices authorize no hidden boxing, dynamic
stack allocation, or runtime code generation. See
[runtime index storage](../spec/language/generics.md#runtime-index-identity-and-storage).

Physical code sharing is possible as an optimization only when it preserves
these semantics. It does not merge distinct application identities, change
ownership, or select a different contract.

## Generic Invariants And Reach

Generic code carries ordinary obligations. Copying an element requires a
copy-eligible parameter, independently of proving a nonempty buffer:

```omega
machine Buffer::first<T [copy], const N: u64>(buffer: &FixedBuffer<T, N>) -> T
requires
    N > 0
{
    buffer.items[0]
}
```

A concrete length of eight proves `N > 0`; a generic caller with unknown `N`
must carry the corresponding fact. The `[copy]` bound separately permits the
read to return a value without moving it out of borrowed storage.

Reach and operational ceilings likewise remain obligations of the abstract
requirement. Calls use `suspend`, `block`, or both according to that envelope.
A transparent refinement can require narrower behavior when the algorithm needs
it. Capacity and cleanup remain explicit resource and ownership obligations,
not extra service-reach members.

## Associated Types

Use explicit trait type parameters for interface types:

```omega
trait WireReadable<Message, Value> {
    machine from_wire(message: Message, out: &mut Value);
}
```

The current contract specifies no associated-type declaration surface. An
ergonomic extension would need its own justification; ordinary explicit
parameters already have defined application and conformance rules.
