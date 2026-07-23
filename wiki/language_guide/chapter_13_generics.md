# Chapter 13: Generics

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
    out: &mut Option<u64>
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
        index: u64,
        out: &mut Option<u64>
    ) {
        let found: bool = items[index].equals(target);
        let next_index: u64 = index + 1;
        let has_next: bool = next_index < items.len;

        transition (found, has_next) {
            (true, _) -> found_at(index, out)
            (false, true) -> find_at(items, target, next_index, out)
            (false, false) -> not_found(out)
        }
    }

    state found_at(index: u64, out: &mut Option<u64>) {
        out = Some(index);
    }

    state not_found(out: &mut Option<u64>) {
        out = None;
    }
}
```

The syntax is provisional, but the intended shape is not exotic: generic
machines use type parameters and constraints like Rust does.

## Const And Proof Parameters

Some generic facts are values known at compile time or proof time.

```omega
data FixedBuffer<T, const N: u64> {
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

## Machine Parameters

Settled 2026-07-20. A generic parameter may name a machine symbol:

```omega
machine Deck::best<machine Key>(&self) -> u64
where machine Key(card: &Card) -> u64
{
    let score: u64 = Key(&self.cards[0]);
    score
}
// spelled at the call site: deck.best<Card::power_key>()
```

`Key` is not a hidden runtime argument. Monomorphization substitutes the
selected symbol at every use; the example above emits a direct
`Card::power_key(&self.cards[0])` call in that specialization.

Rules:

- `<machine M>` binds a machine **symbol** at the spelling site, checked
  against its `where machine` signature and monomorphized per instance like
  every generic. After substitution, each use of `M` is a direct static
  call. No runtime value exists — the parameter is gone by codegen.
- A static-machine selection in `requires` or `ensures` instantiates a logical
  contract schema, not an executable call site. It is checked against the same
  callable requirement but does not by itself monomorphize the selected generic
  machine; universal quotient relations and their law witnesses therefore stay
  universal.
- Recursive proof-only data may use the same form to index a family:
  `data CauchySeq<machine S> where machine S(index: Nat) -> Rat; { ... }`.
  A concrete `CauchySeq<leibniz_term>` argument is checked against `S`'s full
  callable contract. This is schema identity only: finite-layout data rejects
  machine parameters, and `S` cannot be stored as a field type.
- A machine-parameter signature may itself declare machine parameters. Its
  nested requirements follow it in the same clause stream:

  ```omega
  machine forward<machine Schema, machine Selected>(value: Stream<Selected>) -> Stream<Selected>
  where machine Schema<machine Inner>(value: Stream<Inner>) -> Stream<Inner>
  where machine Inner(index: Nat) -> Rat;
  where machine Selected(index: Nat) -> Rat;
  {
      Schema<Selected>(value)
  }
  ```

  Refinement is binder-positional: a selected generic schema may call its
  nested parameter something other than `Inner`, but its complete nested
  parameter/result shape, service reach, suspension/blocking ceilings,
  termination guarantee, and contracts must conservatively refine the authored
  requirement. Forwarding a distinct
  machine parameter uses that same judgment. Specialization first replaces
  `Schema` and `Selected`, then continues to a fixed point until the nested
  call is direct and contains no runtime callable representation.
- Every machine parameter must have an authored `where machine M(...)`
  contract at its declaration. The compiler never infers that abstraction
  from `M(...)` uses or from the machines currently supplied by consumers,
  even in a whole-program build with only one instantiation. Missing the
  contract is a declaration-site error; a declared but unused parameter gets
  the ordinary unused-parameter warning. If exactly one implementation is
  intended, call that concrete machine instead of declaring a generic.
- Type and result parameters may be inferred from the selected machine and
  ordinary arguments. For example, `map<Card::power>(cards)` specializes
  `map<T, U, machine F>` with `T = Card`, `U = u64`, and every `F(value)` call
  becomes `Card::power(value)`.
- Generic bodies are checked modularly against the authored `where machine`
  contract: they prove the parameter machine's `requires`, assume its
  `ensures`, and include its published effects and other contract axes. At an
  instantiation, the selected machine must refine that requirement. The
  checker does not infer a stronger generic API from whichever implementation
  happens to be selected.
- The receiver mode in the required signature is the calling discipline:
  `&self` is freely repeatable, `&mut self` is a stateful callback (spell it
  as a type parameter whose machine is required, as below); a consuming
  mode arrives with the cleanup arc.
- There are **no runtime machine values and no capture inference**. A
  stateful callback is a machine *instance* — its fields are its declared
  captures, construction is the capture clause, and borrow modes are field
  types. A type-erased callable is a `dyn` trait (chapter 14). Concurrent task
  start moves the instance. Ownership determines whether the value may be
  transferred; its four-axis carry policy and the selected runtime contract
  determine whether that activation boundary is legal (chapters 7 and 18).
- A static machine parameter does not reify a machine into ordinary data. It
  cannot be stored, converted to an address, placed into a relocation field, or
  returned as a runtime callback reference. Those operations need separately
  designed sealed entry-reference and relocation machinery; compile-time
  substitution alone supplies only a direct call in the specialized body.
- Accepted generic axioms are granted once at the normalized template
  statement, including its machine-parameter contract. Each instantiation
  records that template receipt and the selected machine-contract identities
  for audit and cache invalidation, but spends no second grant. A project that
  trusts only particular instances must expose and grant non-generic accepted
  facts instead of granting the universal template.

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

`where` is one construct across the language: its facts hold at every
observation of the declared thing. On a compile-time-known operand (a const
parameter) that collapses to a single instantiation-time proof — this
section. On runtime fields of a `data` declaration it is the default
domain, maintained through invariant windows — see
[Dependent Types](chapter_12_dependent_types.md).

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
machine Buffer::first<T, const N: u64>(
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

Generic service and operational ceilings work the same way: a generic
requirement publishes the service reach and `suspends`/`blocks` possibilities
of calls through it, and a caller must admit every axis. Allocation capacity
and owned-resource cleanup are not service or operational clauses: they travel
through explicit capability contracts and the multiplicity/ownership rules.

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
