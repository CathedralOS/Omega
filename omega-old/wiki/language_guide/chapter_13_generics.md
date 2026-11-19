# Chapter 13: Generics

A generic declaration takes parameters in `<>`: types, values, or code. Types,
`const` values and code are fixed at compile time, and each combination
compiles to its own direct code.

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

`Optional` is an ordinary sum, not a built-in. Its zero value is `None`.

An alias fixes some arguments of an existing type without creating a new one:

```omega
data ByteBuffer<const Capacity: u64> = Buffer<u8, Capacity>;
data SmallBytes = ByteBuffer<64>;
```

## Generic machines

```omega
machine identity<T>(value: T) -> T {
    value
}

let number: u32 = identity<u32>(42);
```

A body can only do with a `T` what its header says is possible. Copying
needs `[copy]`:

```omega
machine Buffer::first<T [copy], const N: u64>(buffer: &FixedBuffer<T, N>) -> T
requires N > 0
{
    buffer.items[0]
}
```

Anything else comes from a parameter that supplies the operation.

## Passing an implementation

To compare, order or otherwise operate on a `T`, take an implementation of a
[trait](chapter_14_traits.md) as a parameter:

```omega
machine sort<Element, const Order: Ranked<Element>>(values: &mut [Element]) { ... }

sort<Card, PowerOrder>(&mut cards);
```

Inside `sort`, `Order::before(a, b)` calls whatever implementation the caller
named. The caller always names it. Omega never searches for "the"
implementation of a trait for a type, so two orderings for `Card` can coexist
and nothing changes meaning when a new one is imported.

## Passing a machine

A parameter can be a single machine. Its type is the signature it must have:

```omega
machine Deck::score<const Key: machine(card: &Card) -> u64>(card: &Card) -> u64 {
    Key(card)
}

Deck::score<Card::power_key>(&card);
```

This is not a function pointer. `Deck::score<Card::power_key>` compiles to a
direct call of `Card::power_key`.

Every parameter that carries code has the same form, `const Name: Type`:

| Type | The argument is |
| --- | --- |
| `machine(card: &Card) -> u64` | Any machine with that signature and contract. |
| `HookProcedure::call` | A machine bound to that trait requirement ([Chapter 14](chapter_14_traits.md#binding-a-machine-to-a-requirement)). |
| `Ranked<T>` | An implementation of the trait. |
| `machine` | A machine whose type is taken from where it is passed on. |

### Forwarding a callback contract

A wrapper that only passes its callback along can leave the type to be
inferred:

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

`visit_one` gets `F: ElementStep<T>::apply` from its call to `apply_one`.

## Compile-time values

```omega
data FixedBuffer<T, const N: u64> {
    items: [T; N];
}

data Matrix<const R: u64, const C: u64> {
    cells: [f64; R * C];
}
```

A `const` argument is known at compile time and can size an array.
`FixedBuffer<u8, 16>` and `FixedBuffer<u8, 32>` are different types.

## Runtime-capable value parameters

Without `const`, a value parameter may be supplied at run time. It ties types
together without making the compiler produce one copy per value:

```omega
data Index<Limit: u32>
requires value < Limit
{
    value: u32;
}

machine prefix_count<Count: u32>(items: &[u8]) -> u32
requires embed(Count) <= embed(items.len)
{
    Count
}
```

`prefix_count<16>(items)` and `prefix_count<count>(items)` both work. Use
`const` when the value must size storage, and a plain parameter when nothing
else in the signature refers to it.

A runtime value can pick between a few compile-time variants, such as SIMD
widths:

```omega
trait ByteScanner<Scanner> {
    machine scan<Width: u32>(scanner: &Scanner, bytes: &[u8]) -> u64
    requires Width == 16 || Width == 32 || Width == 64;
}
```

## Restricting a type parameter

```omega
requires T == u16 || T == u32 || T == u64
```

`T` must be exactly one of those. A branch that establishes `T == u32` may use
`u32` operations on its `T` values. Outside such a branch an operation on `T`
is allowed when every listed type supports it, and is checked for each.

## Static or runtime?

Type, `const` and code parameters are static: the choice is made at compile
time, calls are direct, and many combinations mean many copies of the code. When one copy
should serve choices made at run time, store a machine in a field instead
([Chapter 14](chapter_14_traits.md#dispatch)). The two mix freely.

## Further

The spec covers parts of generics that most code never needs:

- [Computed type indices](../spec/proofs/contracts.md#proved-index-compatibility),
  where `Buffer<A * (B + C)>` and `Buffer<A * B + A * C>` are shown compatible
  from algebra laws you supply.
- [Recovering a parameter from a type's structure](../spec/language/generics.md#structural-type-equations-and-inference).
- [Callable types that take their own parameters](../spec/language/generics.md#nested-contracts-and-proof-families).

The complete rules are in [generics](../spec/language/generics.md).
