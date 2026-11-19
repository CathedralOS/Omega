# Chapter 12: Dependent types

A contract can relate one runtime value to another: an index to a length, a
count to a capacity stored next to it. That is all "dependent" means here.

## Between parameters

```omega
machine get(items: &[u8], index: u64) -> u8
requires index < items.len
{
    items[index]
}
```

The bound is on `index`, in terms of `items`. The caller proves it, usually
with a branch, and the body relies on it.

## Between fields

```omega
data MemoryMap
requires
    embed(count) * embed(stride) <= embed(len),
    stride >= 40,
    len <= 4096,
{
    buf: [u8; 4096];
    len: u32;
    stride: u32;
    count: u32;
}
```

Every `MemoryMap` anyone can observe satisfies those three lines. `embed`
states the product in unbounded arithmetic, so the invariant itself cannot
overflow.

Zero is not a valid `MemoryMap`, since `stride >= 40` fails. Such a type has to
be constructed with real values; zeroed storage is not yet one. When you need
an "empty" slot, say so with a case:

```omega
data Player
requires health in 1..=100
{
    health: i32;
}

data PlayerSlot {
    case Empty;
    case Filled(player: Player);
}

data Team { roster: [PlayerSlot; 8]; }
```

A zeroed `Team` is eight `Empty` slots and contains no invalid `Player`.

## Dimensions known at compile time

```omega
data Matrix<const R: u64, const C: u64> {
    cells: [f64; R * C];
}
```

`Matrix<3, 4>` has twelve cells and stores neither number.
[Chapter 13](chapter_13_generics.md) covers `const` parameters.

## Dimensions known at run time

```omega
machine pixel(pixels: &[u8], width: u32, height: u32, x: u32, y: u32) -> u8
requires width in 1..=1024
requires height in 1..=1024
requires x < width
requires y < height
requires pixels.len == width * height
{
    pixels[y * width + x]
}
```

The size bounds keep `y * width + x` inside `u32`. The coordinate bounds put
it inside `pixels`.

## Facts across calls

A call invalidates what you knew about storage it may write, and nothing else.
The compiler works out which places a machine writes from its body:

```omega
public machine set_time(entry: &mut Entry, time: u64) {
    entry.at = time;
}

set_time(&mut e, 5);
use(e.owner);        // facts about e.owner survive the call
```

For a call with no body to inspect, such as a boundary operation or a machine
held in a field, the compiler assumes everything handed over may be written.
Two things keep that from costing you:

- Lend only what the callee needs: `&mut entry.at` instead of `&mut entry`.
- Have the requirement say what is unchanged:
  `ensures entry.owner == old(entry.owner)`.

## A loop that carries its bound

Each round must establish what the next round requires:

```omega
machine clear(buffer: &mut [u8], remaining: u64)
requires remaining <= buffer.len
terminates by remaining -> Nat::Descending
{
    transition remaining == 0 {
        true -> {}
        false -> step(buffer, remaining)
    }

    machine step(buffer: &mut [u8], remaining: u64)
    requires remaining > 0
    requires remaining <= buffer.len
    terminates by remaining -> Nat::Descending
    {
        buffer[remaining - 1] = 0;
        transition { _ -> clear(buffer, remaining - 1) }
    }
}
```

Start it with `clear(buffer, buffer.len)`. The `false` arm knows
`remaining > 0`, so `remaining - 1` is a valid index, and the next call's
`requires` follows.

## When the compiler says no

The error names the missing relationship, for example that `i < count` is not
known. Add a `requires`, branch on the condition and handle the other side,
call a validator that returns the value already checked, or cite a proof. The
compiler does not insert a runtime check for you.

## Where next

[Chapter 13](chapter_13_generics.md) covers generics. The complete rules are
in [dependent values](../spec/language/dependent_values.md).
