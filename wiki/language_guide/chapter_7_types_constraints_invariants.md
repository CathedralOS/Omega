# Chapter 7: Contracts and flow facts

`requires` tells callers what to establish. `ensures` tells them what they get
back. The compiler checks both, using facts from values, branches, and other
calls.

## State a requirement and a guarantee

```omega
data Body {
    mass: i32;
}

machine set_mass(body: &mut Body, mass: i32)
    requires mass in 1..=100
    ensures body.mass in 1..=100
{
    body.mass = mass;
}
```

The caller supplies the range fact. The assignment transports it to `body.mass`,
which proves the postcondition. A missing proof normally produces a diagnostic;
an author does not get a runtime check merely by writing `requires`.
[Invocation contracts](../spec/language/state_contracts.md) owns fact transport and
[default domains](../spec/language/dependent_values.md#default-domains-and-zero-initialization)
owns establishment gates.

## Fact propagation

Branches refine the facts available on their own paths:

```omega
data Player
requires
    health in 0..=100,
{
    health: i32;
}

machine take_damage(player: &mut Player, amount: i32)
    requires amount in 0..=100
    ensures player.health in 0..=100
{
    let next: i32 = player.health - amount;
    transition next < 0 {
        true -> floored(player)
        false -> settle(player, next)
    }

    machine floored(player: &mut Player) {
        player.health = 0;
    }

    machine settle(player: &mut Player, next: i32)
    requires next in 0..=100
    {
        player.health = next;
    }
}
```

Both operands lie in `0..=100`, so subtraction fits `i32`. The false arm adds
`next >= 0`; the existing upper bound gives `next <= 100`. Each helper can
therefore establish the field's range and the machine's postcondition.
`next` is invocation-local scratch, passed only to the helper that needs it;
it is not a persistent `Player` field. Both transitions pass `player` explicitly.

A copied scalar keeps its value after its source changes. A reference still
points to live storage, so a write can invalidate facts about what it contains:

```omega
machine example() -> u32 {
    let mut current: u32 = 10;
    let saved: u32 = current;
    current = 20;
    saved // Still 10.
}
```

After a helper call, use its `ensures` and any facts preserved by its writes.
[Chapter 12](chapter_12_dependent_types.md#facts-across-calls) explains this for
borrowed data.

## Generic contracts

A contract may refer to symbolic parameters without enumerating their values:

```omega
machine interval_contains(value: i32, lower: i32, upper: i32) -> bool
    requires lower <= upper
    ensures result == (lower <= value && value <= upper)
{
    lower <= value && value <= upper
}
```

The same statement works for every admitted argument tuple. Compile-time type
and value parameters use this principle too; [generics](chapter_13_generics.md)
explains their binders. A proof does not substitute guessed concrete values for
an abstract parameter.

## Range forms

`a..b` excludes its end; `a..=b` includes it. For a slice of length `length`,
an exclusive end may equal `length`, whereas an inclusive end must be smaller.
An inclusive range normalizes to an exclusive end only after the required
increment and range-validity obligations are proved; normalization cannot hide
overflow.

Range membership in a contract describes values. A range used for subslicing
also creates a view and needs bounds, alignment where applicable, and a valid
loan. See [numeric bounds](../spec/language/numeric_values.md#collection-bounds).

## Window facts

A window fact states an element-wise condition over a sequence prefix:

```omega
data MapTable
requires
    loaded <= 8,
    maps[0..loaded] in MemoryMap,
{
    maps: [MemoryMap; 8];
    loaded: u32;
}
```

To extend the prefix, establish the next element and preserve the old prefix
before increasing `loaded`. To use an element fact, prove the index is inside
that prefix. A write to an earlier element must preserve or re-establish its
condition too.

For conditions relating several elements, such as sortedness, give helpers
contracts that state how each update preserves the condition. [Chapter 10](chapter_10_compile_time_proofs.md)
shows how to supply a proof when automatic checking needs help.

## Type properties

Properties describe checker laws for a type, rather than predicates about one
value. They use lowercase bracket lists:

```omega
data Point [copy] {
    x: i32;
    y: i32;
}

data Box<T [copy]> [copy] {
    value: T;
}
```

`copy` permits duplication and requires compatible fields. `linear` instead
requires each established obligation to be consumed or transferred exactly once.
They cannot both apply. Neither declaration generates a callable machine.

Some judgments, such as `sized`, are compiler-derived. Declared structural
properties must validate; opaque boundary properties need accepted evidence.
Ordinary packages cannot attach structural properties to foreign types.
[Property declarations](../spec/language/ownership.md#property-declarations)
owns the exact rules; [traits](chapter_14_traits.md) names behavior instead.

### Binding relevance

`[erased]` applies to one binding, not globally to its type:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

The checker uses `proof` during checking and emits no runtime field for it.
Runtime code therefore cannot branch on that field. Erasing a resource witness
still leaves its ownership obligations to be satisfied. See [explicit relevance](chapter_10_compile_time_proofs.md#keep-evidence-without-runtime-storage).

### Carry policy

Carry describes which execution transitions a live value can survive. It is
separate from ownership and copyability:

| Axis | Strict end | Relaxed end |
| --- | --- | --- |
| Suspension | forbidden | allowed |
| CPU affinity | same originating CPU | any CPU |
| Host-thread affinity | same originating thread | any thread |
| Address stability | stable storage | movable storage |

An authored policy names all four axes:

```omega
boundary data PerCpuLease [linear, carry(
    suspension: allowed,
    cpu: same,
    thread: any,
    address: movable,
)];
```

This admitted lease may survive suspension and thread changes, but the runtime
must preserve its CPU. The policy's truth needs boundary evidence; the spelling
alone does not force the host to behave that way.

Ordinary data derives its carry policy from its contents. At a suspension or
thread/CPU transfer, the compiler checks live values against the selected
runtime's guarantees. [Chapter 18](chapter_18_concurrency.md) shows the effect
on task code; the [carry specification](../spec/resources/carry.md) defines the
full composition rules.
