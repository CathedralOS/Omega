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
    case #0 None;
    case #1 Some(value: T);
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

## Generic Machines

A generic machine can require one explicitly passed conformance:

```omega
machine equal<T, Equality: T satisfies Equatable>(left: &T, right: &T) -> bool {
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

## Runtime-Capable Value Parameters

Value binders distinguish two staging contracts:

```text
<Count: u32>        runtime-capable; a static argument is also permitted
<const Count: u32>  statically known within the compiled specialization
```

`const` is a requirement, not an optimization hint. Without it, known arguments
may still specialize. With it, a declaration can require fixed layout or static
instruction operands without supplying a runtime fallback. Runtime-capable
binder support remains implementation work; these examples specify the intended
contract rather than currently executable source.

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
generic indices also connect dependent types and results, such as
`data Index<Limit: u32> { value: u32 [0..Limit]; }`.

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
trait ByteScanner {
    machine scan<Width: u32>(&self, bytes: &[u8]) -> u64
    where
        Width == 16 || Width == 32 || Width == 64;
}
```

The intended compiler route emits the required static bodies and dispatches at
the method boundary for a runtime width. The caller proves roster membership;
an arbitrary input needs an authored guard or failure outcome. There is no
automatic fallback or enumeration of the entire u32 range. Each body still owes
its target, bounds, and effect obligations. A const-only inner kernel receives
the selected constant; const itself does not become a runtime parameter.

A named dynamic conformance supplies one concrete row per declared width, with
one exact index shared by the scanner, comparator, and prepared state. The row
can call a specialized loop rather than redispatching on every vector. Escaping
results need a common representation or an explicit finite sum/eligible owner;
no implicit boxing or variable-layout return is introduced. The
[specialization contract](../spec/language/generics.md#finite-specialization-boundary)
and [dynamic-family contract](../spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
define the rules. This is intended support, not implemented generic virtual calls.

## Type Equality And Range Matching

Exact type comparisons use ordinary Boolean connectives:

```omega
where
    T == u16 || T == u32 || T == u64
```

This restricts T to those exact normalized types, not anything with a compatible
layout or conformance. A static branch establishing `T == u32` can check its
operations using that equality. It does not need runtime reflection or a special
value-comparison implementation. Generic bodies must cover all admitted cases.

A type equation can also recover a declared generic binder from known structure:

```omega
data TinyBytes<Length, const Capacity: u64>
where
    Length == u64[0..=Capacity]
{
    storage: [u8; Capacity];
    length: Length;
}
```

`TinyBytes<u64[0..=256]>` infers Capacity as 256. The array extent is static;
only the live length varies at runtime. This uses no dummy Length value or
compiler bound-query intrinsic. A general vector still needs its own element
initialization/disposition rules. Placement must fit the complete stack/storage
plan; choosing capacity explicitly does not guarantee available backing.

The matching rules normalize `u64[0..257]` and `u64[0..=256]` to the same
integer interval. They extract declared endpoints, not a tight bound discovered
from runtime flow facts or arbitrary domain predicates. Empty domains remain
legal, but missing or ambiguous endpoint information cannot select an arbitrary
capacity. Repeated binders must agree; explicit arguments cannot be overwritten.
Aliases use their defined expansion, and named domains retain their identities.

Given `upper_bound<const N: u64>(value: u64[0..=N])`, an actual declared
`u64[0..=256]` selects N = 256 when omitted. Explicit N = 512 can still pass
ordinary value compatibility. A type equation such as TinyBytes' is stronger:
it requires exact equality, not a larger containing interval. Selection and
proof of call legality are separate. A zero-argument `upper_bound<N>()` merely
returns an already bound N; an unconstrained `upper_bound()` cannot infer it.

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

An open result index can be an expression over input indices. Compatibility then
requires closed evaluation, licensed canonical normalization, or an exact
established local fact. A theorem citation contributes its ordinary guarantee;
there is no new citation syntax or ambient lemma search. Stronger proof search
may accept more compatible uses but cannot rename an interface's normalized
index. [Indexed domains](../spec/language/domains.md#indexed-families) and
[licensed normalization](../spec/proofs/contracts.md#licensed-normalization)
own the details.

## Machine Parameters

A static parameter can select a machine declaration:

```omega
machine Deck::score<machine Key>(card: &Card) -> u64
where machine Key(card: &Card) -> u64
{
    Key(card)
}
```

Calling `Deck::score<Card::power_key>(&card)` substitutes the selected symbol
and produces a direct call in that specialization. The binder is not a hidden
runtime function argument or inferred capture.

There are three distinct binder categories:

| Binder | What the declaration provides |
| --- | --- |
| Structural callable | An authored `where machine Key(...)` contract. |
| Nominal callable | One exact `where machine Handler satisfies HookProcedure::call` requirement. |
| Declaration identity | A trait's noncallable machine binder, used only to relate declaration identities. |

Every callable binder needs its contract at declaration. Uses in the body and
the current set of consumers cannot infer it, even with one whole-program
instantiation. The nominal form inherits the requirement's complete signature,
conditions, and operational/boundary contract without repeating it. A path with
several overloads is not an exact selection.

An identity-only trait binder has no callable signature:

```omega
trait PrivateCallbackSlot<machine Requirement> { }
```

This illustrates the existing identity relationship, not a replacement core
declaration. Its argument must identify one exact free machine or requirement,
not a type, conformance, runtime value, or overloaded family. Neither a default
nor a consumer may invoke it as if a missing callable contract were inferred.

A structural callable contract can itself bind machine parameters. Matching is
binder-positional, including all nested contracts and operational guarantees;
renaming a nested parameter is harmless, weakening its promised behavior is not.
Specialization continues through nested selections until executable calls are
direct. See [nested contracts](../spec/language/generics.md#nested-contracts-and-proof-families)
for a complete example.

Generic bodies prove a selected parameter's `requires`, use its `ensures`, and
respect its reach, suspension, blocking, crash, and progress contract. Concrete
selections must refine that public bound. Ordinary type/result arguments may
be inferred when the selected signature and value arguments determine an exact
application; that is not inference of conformance evidence or callable contracts.

Stateful callbacks use ordinary instance fields and receiver access; dynamically
selected interfaces use `dyn` traits. A static machine symbol cannot be stored,
converted to an address, or returned as a runtime callback value. Registered
foreign callbacks use a separate contextual realization gate with an exact
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
mathematical function/predicate binders, whose general source forms remain
[undetermined](../spec/proofs/contracts.md#undetermined-foundations).
[Quotients](../spec/proofs/quotients.md) owns relation-family matching and laws.

## Where Clauses

Requirements can constrain static values or the behavior of selected operations.
A required member belongs to a named trait and is supplied through an explicit
conformance binder, not an anonymous `where machine T::member(...)` declaration.

For example, assuming `Ranked` declares the required ordering operations:

```omega
machine sort<Element, Order: Element satisfies Ranked>(values: &mut [Element]) {
    ... // implementation and proof omitted
}

sort<Card, PowerOrder>(&mut cards);
```

When the selected conformance owns parameters, its application nests inside the
evidence argument, as in `SequenceEncoding<u8, PlayerMessage>`. Every type,
const, and static-machine argument owned by that conformance is explicit. The
expected subject and trait validate the resulting closed map; they do not fill
missing arguments or `_` holes. An already-closed evidence binder forwards bare.

Only lifetime arguments follow ordinary application-site elision, and only
when borrow constraints determine one unique mapping. Resolved regions remain
in semantic identity even though they create no runtime generic arguments.
See [named conformance selection](../spec/language/conformances.md#declaration-and-selection).

Static `where` obligations are checked at instantiation. Data `where` facts over
runtime fields instead define a default domain maintained at consumption points.
Runtime value-binder requirements are proved for the exact subjects at each
application; a runtime guard may establish them. These are different binding
times, not permission to reinterpret runtime data as arbitrary type declarations.

## Static Dispatch

Calls through explicitly selected evidence are static by default:

```omega
machine Runner::tick<T, Increment: T satisfies Incrementable>(subject: &mut T) {
    Increment::increment(subject);
}
```

The specialization uses the selected conformance's implementation. Another
visible `increment` machine cannot replace it. Dynamic dispatch is the separate
mechanism for runtime-selected interfaces, with its own evidence and custody.

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
where
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
