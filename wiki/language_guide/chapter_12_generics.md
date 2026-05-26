# Chapter 12: Generics

Generics let one data or machine declaration work over many concrete types or
compile-time values.

The baseline model should stay close to Rust:

- Type parameters are written with angle brackets.
- Const/proof parameters can appear where compile-time values matter.
- Constraints live in `where` clauses.
- Generic code is statically checked once, then instantiated for concrete uses.
- Static dispatch and monomorphization are the default.

## Generic Data

Generic data declarations parameterize stored shape.

```omega
data Option<T> {
    has_value: bool;
    value: T;
}

data Pair<A, B> {
    first: A;
    second: B;
}
```

Working rules:

- `T`, `A`, and `B` are type parameters.
- Each concrete instantiation has a concrete layout after type checking.
- Generic fields follow the same ownership, move, borrow, and cleanup rules as
  non-generic fields.
- If `T` has cleanup, then `Option<T>` or `Pair<A, B>` may have structural
  cleanup obligations.

## Generic Machines

Machines may be generic over types.

```omega
machine Inventory::find<T>(
    items: &[T],
    target: &T,
    out: &mut Option<usize>
)
where
    T: Equatable
{
    transition items.len > 0 {
        true -> find_at(items, target, 0, out)
        false -> not_found(out)
    }

    state find_at(
        items: &[T],
        target: &T,
        index: usize,
        out: &mut Option<usize>
    ) {
        let found: bool = items[index].equals(target);
        let next_index: usize = index + 1;
        let has_next: bool = next_index < items.len;

        transition (found, has_next) {
            (true, _) -> found_at(index, out)
            (false, true) -> find_at(items, target, next_index, out)
            (false, false) -> not_found(out)
        }
    }

    state found_at(index: usize, out: &mut Option<usize>) {
        out = Some(index);
    }

    state not_found(out: &mut Option<usize>) {
        out = None;
    }
}
```

The syntax is provisional, but the intended shape is not exotic: generic
machines use type parameters and constraints like Rust does.

## Const And Proof Parameters

Some generic facts are values known at compile time or proof time.

```omega
data FixedBuffer<T, const N: usize> {
    items: [T; N];
}

machine Math::clamp_i32<const MIN: i32, const MAX: i32>(
    value: i32,
    out: &mut i32
) {
    match (value < MIN, value > MAX) {
        (true, _) -> {
            out = MIN;
        }
        (false, true) -> {
            out = MAX;
        }
        (false, false) -> {
            out = value;
        }
    }
}
```

Working rules:

- `const` parameters are compile-time values, proof-visible values, or both.
- Const parameters may appear in array lengths, value constraints, and proof
  obligations.
- The compiler must prove const constraints at each instantiation.

## Where Clauses

`where` clauses describe requirements on generic parameters.

```omega
machine Metrics::sample<T>(
    source: &T,
    out: &mut CounterSnapshot
)
where
    T: CounterLike
{
    source.snapshot(out);
}
```

Common requirements:

- Trait requirements: `T: CounterLike`.
- One-off machine requirements: `machine T::poll(&mut self) -> PollResult`.
- Value/proof requirements: `N > 0`.
- Effect requirements: a generic operation may be callable only when its
  effects fit the caller's context.

Traits are covered in the next chapter. Generics only need to provide a place
for constraints to live.

## Static Dispatch

Generic dispatch should be static by default.

```omega
machine Runner::tick<T>(
    subject: &mut T
)
where
    machine T::increment(&mut self)
{
    subject.increment();
}
```

For a concrete call with `Counter`, the compiler resolves `Counter::increment`
at compile time. This keeps generic code fast, proof-visible, and compatible
with monomorphization.

Dynamic dispatch is a separate feature for runtime-selected interfaces,
plugins, hot-swap boundaries, and language-neutral extension points.

## Monomorphization

The default implementation strategy should be monomorphization:

```text
generic machine + concrete type arguments -> concrete machine instance
```

This gives the compiler concrete layouts, concrete drop obligations, concrete
effects, and concrete machine targets during later pipeline stages.

The language may later support shared generic code generation where profitable,
but that should be an optimization. It should not change generic semantics.

## Generic Invariants And Effects

Generic code emits generic obligations.

```omega
machine Buffer::first<T, const N: usize>(
    buffer: &FixedBuffer<T, N>,
    out: &mut T
)
where
    N > 0
{
    out = buffer.items[0];
}
```

The obligation `N > 0` is proven when the machine is instantiated. If a caller
has `FixedBuffer<Item, 8>`, the obligation is easy. If a caller has an unknown
`N`, that caller must carry a proof fact for `N > 0`.

Generic effects work the same way: if a generic operation may call a platform
entry, block, allocate, or drop a resource, those effects become obligations at
the call site.

## Associated Types

The first design should avoid associated types unless they become necessary.

Prefer explicit type parameters:

```omega
trait WireReadable<Message, Value> {
    machine Value::from_wire(message: Message, out: &mut Value);
}
```

This is noisier than an associated type slot, but it is clearer while the trait
system is still young. It also keeps the generic surface close to ordinary data
and machine signatures.

Associated types can be added later if explicit parameters become too clumsy.
