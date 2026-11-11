# Chapter 14: Traits and runtime machine values

A trait names machine requirements and declares the record that supplies them.
An implementation is a checked value of that schema. Generic code receives the
value explicitly; neither a type nor a nearby declaration selects it implicitly.

## A trait and an implementation

```omega
public data Card { power: u32; cost: u32; }

public trait Ranked<T> {
    machine before(card: &T, other: &T) -> bool;
    machine rank_value(card: &T) -> u32;
}

public PowerOrder satisfies Ranked<Card>
{
    machine before(card: &Card, other: &Card) -> bool {
        card.power < other.power
    }

    machine rank_value(card: &Card) -> u32 {
        card.power
    }
}

CostOrder satisfies Ranked<Card>
{
    machine before(card: &Card, other: &Card) -> bool {
        card.cost < other.cost
    }

    machine rank_value(card: &Card) -> u32 {
        card.cost
    }
}
```

An implementation has its own name. `PowerOrder` and `CostOrder` both
implement `Ranked<Card>`, and neither is "the" ordering for cards. There is no
`impl Trait for Type` and no `Self`: the type being worked on is an ordinary
trait parameter.

## Using an implementation

```omega
machine comes_first<T, const Order: Ranked<T>>(left: &T, right: &T) -> bool {
    Order::before(left, right)
}

let by_power: bool = comes_first<Card, PowerOrder>(&left, &right);
let by_cost: bool = comes_first<Card, CostOrder>(&left, &right);
```

The caller names the implementation. The call is compiled as a direct call to
`PowerOrder::before`.

## Building an implementation from existing machines

```omega
trait Codec<T> {
    machine encode(value: &T, out: &mut [u8]) -> u64;
    machine decode(input: &[u8], out: &mut T) -> bool;
}

WireCodec satisfies Codec<Message> {
    Codec::encode = lib_a::encode;
    Codec::decode = lib_b::decode;
}
```

A row, `Trait::member = machine;`, uses a machine that already exists instead
of wrapping it.

## Ordinary fields and identity

A trait schema can also declare ordinary typed fields and declaration-level
`requires` constraints, using the record forms. A `machine` member always stores
a selection bound to its requirement, even if it returns nothing and has
`ensures`. A field declared `evidence [erased]: P;` stores evidence of type `P`;
its type decides identity, and `[erased]` removes runtime storage only. A
constraint checks the assembled record; it is not another callable field.

For example, a resultless `machine positive(...) ensures ...;` stores the
chosen machine. An explicitly typed strict-evidence field instead carries the
established fact without retaining which machine was cited. Relevant witnesses
retain their value identity. These are ordinary types, not law/lemma categories.
Simple guarantees can live directly on the operation's `ensures` without a
separate evidence field. The [general citation rule](../spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use)
also applies to evidence initializers in ordinary records, checking all inputs,
premises and exact selected operations.

Two equal records of the same schema are one implementation, including empty
and generic implementations. If `Choice<N>` constructs an empty `Tag`, its
unused `N` does not distinguish `Choice<0>` from `Choice<8>`. If it constructs
`Tag<N>`, the actual schema arguments do distinguish them. To distinguish two
values of one schema deliberately, add an ordinary relevant field. There is no
special brand keyword or hidden constructor identity. Projecting the same
operations out of two differently tagged wrappers discards that distinction.

Actual schema indices, selected machine applications and carried lifetime or
ownership obligations remain significant even without runtime bytes. An inline
member declares a new machine, so identical-looking inline bodies can still
yield distinct selections. Names and proof/supply dependencies remain available
for diagnostics and admission without becoming extra value identity.

Ordinary fields do not become captured state. A callable using a policy's limit
still takes that limit explicitly; calling a field supplies no implicit receiver.
Construction establishes the constraints against the final selected operations,
and replacing an operation must repair invalidated evidence before reuse. See
[the complete record rules](../spec/language/conformances.md#implementations-are-operation-records).

## Default bodies

A trait machine with a body is a default. An implementation that leaves it
out gets the trait's version:

```omega
trait ResettableCounter<T> {
    machine set(counter: &mut T, value: i32);

    machine reset(counter: &mut T) {
        ResettableCounter::set(counter, 0);
    }
}
```

## Building on other traits

```omega
trait Resettable<T> {
    machine reset(counter: &mut T);
}

trait ManagedCounter<T> {
    requires Resettable<T>;
    machine increment(counter: &mut T);
}

boundary trait TimerInterrupt: InterruptService + Calling<X86InterruptPolicy> { }
```

`requires Other<T>;` in the body and `: Other<T>` in the header mean the same
thing.

Inheritance contributes requirements and fields, not automatically merged parent
implementation values. A diamond contributes one field for the same originating
member. Distinct origins stay distinct even when their short names match.
Construction supplies one final value for each field; use its originating
declaration's qualified name when needed, such as `Base::encode: left.encode`.
The projected selection must already have the required binding. Conflicting
defaults need an explicit initializer, never a left-parent-wins rule. Evidence
for an encoder discarded during construction does not certify the chosen one.

## Computed providers

Build's existing `Build::select_provider<Slot, Provider>(builder)` accepts a
fully evaluated static record as `Provider`, not only a named implementation.
An admitted compile-time factory can return it directly or through a `const`
binding. The same schema, operation, constraint, target-realization and admission
checks apply. Computing a record grants no authority; runtime-variable
descriptors are not provider selectors. See [provider selection](../spec/build/provider_selection.md#declaration-and-choice).

## Related types are parameters

Where another language would use an associated type, Omega uses a second
trait parameter:

```omega
trait WireEncodable<Value, Message> {
    machine to_wire(value: &Value, out: &mut Message);
}

PlayerWireEncoding satisfies WireEncodable<Player, PlayerMessage>
{
    machine to_wire(player: &Player, out: &mut PlayerMessage) {
        out.name = player.name;
        out.health = player.health;
    }
}
```

## Operator requirements

A trait can require an operator:

```omega
trait Ordered<T> {
    machine < before(left: T, right: T) -> bool;
}

Ascending satisfies Ordered<Card> {
    machine before(left: Card, right: Card) -> bool {
        left.power < right.power
    }
}

machine choose<Element, const Order: Ordered<Element>>(
    left: Element,
    right: Element
) -> bool {
    left < right
}

let selected: bool = choose<Card, Ascending>(first, second);
```

Inside `choose`, `<` means `Order`'s. Outside generic code, a type picks its
one meaning for an operator by declaring it:

```omega
machine < Card::less_by_power(left: Card, right: Card) -> bool {
    Ranked::compare<Card, PowerOrder>(left, right)
}
```

## Binding a machine to a requirement

A generic machine can ask for one machine instead of a whole implementation.
When the parameter's type is a trait requirement, the argument must be a
machine bound to that requirement, and binding is always written out:

```omega
trait Tidy<T> { machine clear(item: &mut T); }

machine tidy<T, const F: Tidy<T>::clear>(item: &mut T) { F(item); }

machine zero(item: &mut u32) { item = 0; }                        // plain
machine wipe(item: &mut u32) satisfies Tidy<u32>::clear { ... }   // bound where declared
const zero_tidy = (zero satisfies Tidy<u32>::clear);              // bound by expression

tidy<u32, wipe>(&mut x);          // accepted
tidy<u32, zero_tidy>(&mut x);     // accepted
tidy<u32, zero>(&mut x);          // Rejected: zero is not bound.
```

Having the right signature is not enough. The three ways to bind are the
`satisfies` on a declaration, a row in an implementation block, and the
`(machine satisfies Trait::member)` expression. See
[binding](../spec/language/conformances.md#binding-a-machine-to-a-requirement).

## Dispatch

Everything so far is resolved at compile time. To choose at run time, store
machines in a record:

```omega
data Counter { value: u64; }

data CounterOps {
    clear: machine(counter: &mut Counter) reaches none crashes none terminates;
    value: machine(counter: &Counter) -> u64 reaches none crashes none terminates;
}

machine clear(counter: &mut Counter) terminates { counter.value = 0; }
machine value(counter: &Counter) -> u64 terminates { counter.value }

machine inspect(operations: &CounterOps, counter: &Counter) -> u64 terminates {
    (operations.value)(counter)
}

let ops: CounterOps = CounterOps { clear: clear, value: value };
```

A machine-typed field always states `reaches` and `crashes`, so a stored
machine cannot do more than its type says. There is still no receiver: the
state is passed as an argument.

A trait works both ways without being declared twice:

```omega
machine save_static<T, const Format: Codec<T>>(value: &T, out: &mut [u8]) -> u64 {
    Format::encode(value, out)             // direct call
}

machine save_dynamic<T>(format: &Codec<T>, value: &T, out: &mut [u8]) -> u64 {
    (format.encode)(value, out)            // call through a field
}
```

### Different state types in one container

`some S` lets each value of a type carry its own state type, so an array can
hold readers over different things:

```omega
data Reader<'a, some S> {
    state: &'a mut S;
    read: machine(state: &mut S, output: &mut [u8]) -> u64
        requires output.len > 0
        reaches none
        crashes none;
}

machine read_one(reader: &mut Reader, output: &mut [u8]) -> u64
requires output.len > 0
{
    (reader.read)(reader.state, output)
}

let mut readers: [Reader; 2] = [
    Reader { state: &mut constant, read: read_constant },
    Reader { state: &mut toggle, read: read_toggle },
];
```

Each reader pairs a state with a machine for that state. Pairing
`&mut constant` with `read_toggle` is rejected. This is Omega's replacement
for trait objects and closures; see
[some packages](../spec/language/some_packages.md), which also shows how a
package can own its state.

## Narrowing a trait

A refinement names a stricter version of a trait's contract:

```omega
public trait LocalLogger<T> = Logger<T> {
    machine *
        reaches none
        suspends false
        blocks false
        terminates;
}

machine record<L, const Logging: LocalLogger<L>>(logger: &L) {
    Logging::write(logger, "record");
}
```

`record` accepts any `Logger<L>` implementation that reaches nothing and never
waits. Nobody implements `LocalLogger` as such; implementations of `Logger`
either fit it or do not.

## What a requirement promises

A requirement carries a contract like any machine, and an implementation must
meet it. For what the machine may touch, the defaults on a trait requirement
are:

| Clause left out | Means |
| --- | --- |
| `reaches` | Whatever the chosen implementation reaches. |
| `crashes` | Whatever the chosen implementation may crash with. |
| `suspends`, `blocks` | It does not. |

Write `reaches none` or `crashes none` to forbid them.
[Chapter 19](chapter_19_capabilities_effects_boundaries.md) covers these
clauses.

## Synthesized core traits

Equality on a record is requested with an empty implementation, which the
compiler fills in field by field:

```omega
trait Equatable<T> {
    machine equals(value: &T, other: &T) -> bool;
}

data Point { x: i32; y: i32; }
StructuralEquality satisfies Equatable<Point> { }
```

Scalars and payload-free sums have `==` built in. Records and sums with
payloads need this declaration, so adding a payload to a case makes you decide
what equality means.

## Where next

[Chapter 15](chapter_15_modules_imports_visibility.md) covers modules. The
complete rules are in [conformances](../spec/language/conformances.md) and
[machine values](../spec/language/machine_values.md).
