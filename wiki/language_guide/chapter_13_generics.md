# Chapter 13: Generics

Generics let one declaration work with different types, values or selected
operations. Start with type parameters and fixed capacities; static callbacks
let callers choose behavior. Runtime indices and mathematical families are
covered later in the chapter.

## Generic data

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

## Transparent aliases

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

An alias expands before matching and layout selection. It adds no wrapper or
conversion. Domain aliases separately name combinations of domain facts. See [transparent type aliases](../spec/language/generics.md#transparent-type-aliases).

## Generic machines

A type parameter lets a machine preserve whatever type its caller supplies:

```omega
machine identity<T>(value: T) -> T {
    value
}

let number: u32 = identity<u32>(42);
```

Operations on T need a contract. Given an `Equatable<T>` trait with an `equals`
requirement, select its named implementation explicitly:

```omega
machine equal<T, const Equality: Equatable<T>>(left: &T, right: &T) -> bool {
    Equality::equals(left, right)
}
```

A call such as `equal<Card, CardEquality>(&left, &right)` supplies that evidence.
The body checks against the trait's contract. [Traits](chapter_14_traits.md)
explains how to declare and satisfy it.

## Generic requirements

A trait can declare the same generic interface as an ordinary machine, including
an implementation parameter supplied by the caller:

```omega
boundary trait Sorting {
    machine sort<T, const Order: Ordering<T>>(
        items: &mut [T]
    )
        crashes none;
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
trait TypedSorter<T, const Order: Ordering<T>> {
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

## Const and proof parameters

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

## Machine parameters

A static parameter can select a machine declaration. Its type is the callable
contract the selection must satisfy:

```omega
machine Deck::score<const Key: machine(card: &Card) -> u64>(card: &Card) -> u64 {
    Key(card)
}
```

Calling `Deck::score<Card::power_key>(&card)` substitutes the selected symbol
and produces a direct call in that specialization. The parameter is not a
hidden runtime function argument or inferred capture.

`Key`'s own `requires`, `ensures` and operational clauses are written inside
its type; clauses after the closing `>` and parameter list belong to the
enclosing machine:

```omega
const Key: machine(card: &Card) -> u64
    suspends
    blocks
    terminates
```

One form, `const Name: Type`, covers every static parameter that carries code
or a declaration:

| Parameter type | What the declaration provides |
| --- | --- |
| `machine(card: &Card) -> u64` | A structural callable with exactly this authored contract. |
| `HookProcedure::call` | A nominal callable: a machine bound to that one requirement. |
| `Ranked<T>` | An implementation; call its members as `Order::member(...)`. |
| `reflect::CallableInfo` | A declaration identity. It relates declarations and is never invoked. |
| `machine` | A callable whose exact type is [inferred](#forwarding-a-callback-contract) from where it is forwarded. |

The nominal form retains the originating requirement's complete signature,
conditions and operational/boundary contract without repeating it. A path with
several overloads is not an exact selection. A parameter carries one nominal
requirement; a machine that needs two takes two parameters or one
implementation.

### Forwarding a callback contract

The inner operation declares what it accepts; its wrapper need not repeat it:

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

The published contract for `visit_one` types `F` as
`ElementStep<T>::apply`. Forwarding `F` into `for_each<T, F>` works the same
way. Several structural uses combine, and the selection must satisfy all of
them; uses that need two different nominal requirements reject and name both.
A bare call such as `F(item)` does not supply a type, and an authored type is
never strengthened to repair an invalid body.

`ElementStep::apply` omits reach, so both helpers propagate the services of the
selected step. A trait author can write `reaches none` to require service-free
steps instead. Forwarding keeps that choice. See
[callback services](chapter_19_capabilities_effects_boundaries.md#forward-a-callbacks-services)
for a complete example with an application-defined service.

There is no trait lookup or implementation selection here: callers still name
their chosen machine, [bound to the nominal requirement](chapter_14_traits.md#binding-a-machine-to-a-requirement)
when the type is nominal. Public inferred types must obey visibility rules,
and changing a forwarding use can change the public contract and require
rechecking consumers. Diagnostics show the inferred type and its originating
use. See [inferred callable parameter types](../spec/language/generics.md#inferred-callable-parameter-types).

## Requirements on generic arguments

Requirements can constrain static values or the behavior of selected operations.
A required member belongs to a named trait and is supplied through an explicit
implementation parameter, not an anonymous member declaration on a type.

For example, assuming `Ranked` declares the required ordering operations:

```omega
machine sort<Element, const Order: Ranked<Element>>(values: &mut [Element]) {
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

## Static dispatch

Calls through explicitly selected evidence are static by default:

```omega
machine Runner::tick<T, const Increment: Incrementable<T>>(subject: &mut T) {
    Increment::increment(subject);
}
```

The specialization uses the selected conformance's implementation. Another
visible `increment` machine cannot replace it. Runtime machine values provide
the separate dynamic route, with their own callable evidence and custody.

## Specialization and code size

A static application can specialize a generic body for its type, value and
machine arguments. The selected operations become direct calls and may inline.
Using many combinations can produce many copies of the algorithm.

Runtime machine values offer another choice: share an algorithm with a fixed
callable interface and select its operation at runtime. You can mix the two,
specializing one parameter while passing another as a runtime value. See
[machine values](chapter_14_traits.md).

Runtime-capable value arguments do not require a specialization per observed
number. Their dependent types retain which value they refer to. A runtime
capacity still needs explicit backing or an allocator, while inline fixed
arrays need static extents. A bound of 65536 does not allocate 65536 elements
by itself. See [runtime index storage](../spec/language/generics.md#runtime-index-identity-and-storage).

## Generic invariants and reach

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

## Interface types

Use explicit trait type parameters for an interface's input and output types:

```omega
trait WireReadable<Message, Value> {
    machine from_wire(message: Message, out: &mut Value);
}
```

The conformance binds both types, so callers can see the relationship directly.
## Runtime-capable value parameters

Value binders distinguish two staging contracts:

```text
<Count: u32>        runtime-capable; a static argument is also permitted
<const Count: u32>  statically known within the compiled specialization
```

Use `const` when a declaration needs a statically known argument, for example
to determine inline storage. A runtime-capable binder also accepts known
arguments, but its body must support runtime values under the declared bounds.

```omega
machine prefix_count<Count: u32>(items: &[u8]) -> u32
requires
    embed(Count) <= embed(items.len)
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

See the [finite-specialization rules](../spec/language/generics.md#finite-specialization-boundary)
when a runtime choice selects among a bounded set of static kernels.

## Type equality and indexed-domain matching

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

See [structural inference](../spec/language/generics.md#structural-type-equations-and-inference).
[Semantic reflection](chapter_21_reflection.md) uses these same generic rules for
typed member callbacks. Structural inference itself is not declaration inspection
or permission to discover arbitrary predicates.

## Advanced static indices

Use these rules when an API carries computed dimensions or algebraic indices.

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
`const Algebra: CommutativeCombine<Index>` supplies both the operation and
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
    machine zero() -> T reaches none terminates;

    machine + add(
        left: T,
        right: T
    ) -> T reaches none terminates;

    machine right_identity(value: T)
    reaches none
    ensures core::Identity<T>(add(value, zero()), value);
}

trait WrappingByteIndices : Additive<u8 in Wrapping> {
    machine zero_is_literal()
    reaches none
    ensures core::Identity<u8 in Wrapping>(zero(), 0 as +Wrapping);
}

domain<T, const Count: u8 in Wrapping> T::Indexed<Count>;

machine remove_zero<
    const Count: u8 in Wrapping,
    const Arithmetic: WrappingByteIndices
>(
    value: u32 in Indexed<Count + 0>
) -> u32 in Indexed<Count> {
    value
}
```

A caller selects a complete implementation, for example
`remove_zero<8, ByteArithmetic>(value)`. `ByteArithmetic` supplies the inherited
operations and proves both laws; their names have no compiler authority. The
`reaches none` clauses make the operations service-free for use in these laws. The
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

This changes proof discovery, never type identity. Other sponsored resource ceilings still
apply. If an obligatory equality remains unproved, the diagnostic names the
missing evidence or exhausted budget; it does not claim mathematical inequality.
A checked theorem can establish equality without expanding the whole product.

A generic compatibility judgment survives specialization. A consumer that trusts
the library's exact Psi artifact reuses that judgment, so it needs no larger
proof-search budget to import a difficult proof. Adding a valid law leaves
existing proofs valid and canonical keys unchanged.

Exact declaration and conformance matching still uses canonical keys.
Consequently, a conformance for one symbolic spelling does not automatically
match the other, and the two forms may produce separate specializations.
Use the same expression or a shared transparent alias when an API requires
exact identity. Closed indices can still evaluate to the same value and share
a key. The rules are in
[stable identity](../spec/proofs/contracts.md#stable-canonical-index-identity),
[proved compatibility](../spec/proofs/contracts.md#proved-index-compatibility),
and [Build proof-work policy](../spec/build/configuration.md#proof-work-policy).

### Relating two index forms once

When one library selected `const Order: Ranked<Buffer<A>>` and another
requires `Ranked<Buffer<B>>`, with `A` and `B` provably equal but spelled
differently, write the adapter once for the trait. A `requires` clause on the
conformance declaration admits the application; inside its members the
equality is a fact, so arguments pass to the base with no conversion:

```omega
Reindexed<const A: u64, const B: u64, const Base: Ranked<Buffer<A>>>
satisfies Ranked<Buffer<B>>
requires A == B;
{
    machine before(left: &Buffer<B>, right: &Buffer<B>) -> bool {
        Base::before(left, right)
    }

    machine rank_value(buffer: &Buffer<B>) -> u32 {
        Base::rank_value(buffer)
    }
}

machine bridge<
    const A: u64,
    const B: u64,
    const Order: Ranked<Buffer<A>>
>(
    items: &mut [Buffer<B>]
)
requires A == B
{
    sort<Buffer<B>, Reindexed<A, B, Order>>(items);
}
```

`bridge` checks as a generic body: `A` and `B` stay symbolic there, and its own
`requires` supplies the adapter's premise. The adapter's members are new
declarations, so any laws `Ranked` states are obligations on them; they follow
from `Base`'s laws under the same equality. Nothing selects `Order` at
`Ranked<Buffer<B>>` by itself, and no proof budget changes which ordering runs.

## Advanced machine arguments

A trait can retain a machine's identity without granting permission to call it:

```omega
trait PrivateCallbackSlot<Layout, const Requirement: reflect::CallableInfo> { }
```

To invoke an argument, give it a callable type instead. Callable types can
themselves take static parameters; every nested selection must satisfy its
declared signature, effects, and guarantees.

Proof libraries can also index a family by a generator machine, for example
`CauchySeq<machine S>`. A theorem may relate sequences with different generators
without giving them the same identity. See [nested contracts and proof families](../spec/language/generics.md#nested-contracts-and-proof-families)
and [mathematical term binders](../spec/proofs/mathematical_bindings.md).
