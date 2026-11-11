# Chapter 7: Contracts and flow facts

`requires` says what a caller must establish. `ensures` says what it gets
back. The compiler checks both at compile time, using what it knows from
types, branches and earlier calls. A failed check is a compile error, not a
runtime check added for you.

## A requirement and a guarantee

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

The caller shows `mass` is in range. The assignment carries that fact to
`body.mass`, which proves the `ensures`.

## Invariants on data

A `requires` on a data declaration holds for every value of the type:

```omega
data Player
requires health in 0..=100
{
    health: i32;
}
```

Code that receives a `Player` may rely on it, and code that changes
`health` must leave it true.

## Fact propagation

Each branch knows what its condition established:

```omega
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

Both operands are in `0..=100`, so the subtraction fits. The `false` arm adds
`next >= 0`, which is exactly what `settle` requires.

A copy keeps its value when the original changes; a fact about storage does
not survive a write to that storage:

```omega
let mut current: u32 = 10;
let saved: u32 = current;
current = 20;
// saved is still 10; anything known about current must be re-established
```

After a call, you know the callee's `ensures`, plus whatever you knew about
storage it did not write. [Chapter 12](chapter_12_dependent_types.md#facts-across-calls)
goes into that.

## Contracts over all inputs

A contract is proved for every value the `requires` admits, not for examples:

```omega
machine interval_contains(value: i32, lower: i32, upper: i32) -> bool
    requires lower <= upper
    ensures result == (lower <= value && value <= upper)
{
    lower <= value && value <= upper
}
```

## Facts about part of an array

A data invariant can describe a prefix of an array:

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

The first `loaded` entries are valid maps. To grow the prefix, make the next
entry valid and then increase `loaded`.

## Type properties

A bracket list after a type's name states how its values may be handled:

```omega
data Point [copy] {
    x: i32;
    y: i32;
}

data Box<T [copy]> [copy] {
    value: T;
}
```

| Property | Meaning |
| --- | --- |
| `[copy]` | Values can be duplicated. Every field must be copyable too. |
| `[linear]` | Each value must be handed on or finished exactly once. |

A field can be marked `[erased]`, which keeps it for checking and removes it
from the compiled program:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

[Chapter 10](chapter_10_compile_time_proofs.md#keep-evidence-without-runtime-storage)
shows what erased fields are for. A third property, `carry(...)`, says whether
a value may be held across a wait or moved between threads; it matters for
locks and per-CPU data and is covered in
[Chapter 18](chapter_18_concurrency.md).

## Where next

[Chapter 8](chapter_8_domains.md) names sets of values so that a fact can
travel in a type. The complete rules are in
[contracts](../spec/language/state_contracts.md),
[dependent values](../spec/language/dependent_values.md) and
[property declarations](../spec/language/ownership.md#property-declarations).
