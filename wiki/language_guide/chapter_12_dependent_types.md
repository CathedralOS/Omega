# Chapter 12: Dependent types

A contract can relate one runtime value to another. An index can be bounded by
a slice's length, or a counter by a capacity stored beside it.

## Dependent contracts

A parameter's contract may name another parameter:

```omega
machine get(items: &[u8], index: u64) -> u8
requires index < items.len
{
    items[index]
}
```

For this unsigned index, the contract supplies `index < items.len`. The caller
proves it, perhaps using a visible guard, and the callee may rely on it. A
runtime length is a witness already stored in the slice, not a const argument.
This `requires` spelling is canonical; dependent bounds are not embedded in a
parameter's scalar type. Results use `ensures` for the same reason.

Pre-state relationships must name a real binding or use `old(place)`. Equality
in `requires` does not invent a snapshot variable. For example, the caller can
pass a scalar snapshot explicitly:

```omega
data Counter { count: u32; cap: u32; }

machine bump(counter: &mut Counter, before: u32)
requires
    counter.count == before
    counter.count < counter.cap
ensures
    counter.count == before + 1
{
    counter.count = counter.count + 1;
}
```

The strict bound also
establishes representability of the increment. `old(place)` instead selects an
existing structural place's callable-entry revision without copying an owned
value; see [proof views](chapter_10_compile_time_proofs.md#refer-to-the-entry-value).

## Dependent data

Fields can witness one another. Their coupling belongs to the data's default
domain, expressed with `requires`:

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

Every observation must satisfy that domain. `embed` states the product bound in
unbounded proof arithmetic; actual runtime offset computations still prove their
own representability. The coupling does not execute a multiplication or exempt
a runtime intermediate from ordinary arithmetic obligations.

This type is gated because zero does not satisfy `stride >= 40`. Zeroed backing
may exist as storage but is not yet a `MemoryMap` value. Construction or checked
qualification establishes the complete domain. A decoder proves only its checked
conditions, not that firmware told the truth or that the bytes grant authority.

Gating propagates through contained values. An explicit empty case can make an
optional container zero-constructible without inventing an invalid payload:

```omega
data Player
requires health in 1..=100,
{
    health: i32;
}

data PlayerSlot {
    case Empty;
    case Filled(player: Player);
}

data Team { roster: [PlayerSlot; 8]; }
```

The zero case contains no `Player`; constructing `Filled` owes a valid one.
Local or provisioned backing may likewise begin zeroed while gated fields remain
inaccessible until established.

Mutating witnesses and dependents can open an
[invariant window](chapter_11_invariant_windows.md), which must close before
consumption. A live dependent borrow read-loans the witnesses required by its
validity, so those witnesses cannot change while the view remains live. Optional
facts about only some values belong in ordinary domains, not the default domain
of every instance.

## Fixed dimensions

When dimensions are static, ordinary generics select a fixed shape:

```omega
data Matrix<const R: u64, const C: u64> {
    cells: [f64; R * C];
}
```

A concrete `Matrix<3, 4>` has twelve cells after its layout and arithmetic
obligations are checked. Static arguments enter application identity and need
not be stored as runtime witnesses. [Generics](chapter_13_generics.md) explains
those applications; runtime dimensions do not become const arguments by analogy.

## Runtime dimensions

A slice stores its runtime length alongside a borrow of its elements. Use that
length in ordinary contracts. Runtime-capable generic value parameters can
retain similar relationships; see [Chapter 13](chapter_13_generics.md#runtime-capable-value-parameters).

For row-major indexing, prove both coordinates and the backing extent:

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

The dimension bounds keep the arithmetic inside `u32`. The coordinate bounds
put the selected element inside `pixels`. If automatic checking needs help
with a product relationship, cite a checked theorem as shown in [Chapter 10](chapter_10_compile_time_proofs.md).

Foreign records may have a runtime stride as well as a length. Use the supplied
stride to locate them, then establish alignment, representation, and valid
contents before creating a typed view. [Chapter 20](chapter_20_memory_layout_abi.md)
explains those additional requirements.

## Facts across calls

The practical frame rule is preserve-unless-written, at exact storage granularity:

- Unreachable places retain their facts.
- A shared loan excludes ordinary conflicting mutation. Synchronized mutation
  retains its own contract and fact-invalidation rules.
- Mutable or write-only access may invalidate facts about written places.
- Exact outcome guarantees can preserve or establish facts after the call.

The compiler works out which places a checked machine may write and publishes
that with its contract. Nothing is written in the header:

```omega
public machine set_time(entry: &mut Entry, time: u64) {
    entry.at = time;
}

set_time(&mut e, 5);
use(e.owner);        // facts about e.owner survive the call
```

Because the frame is published, editing `set_time` to write `entry.owner` too
is an interface change that dependents see, not a proof that breaks somewhere
else without explanation. A generic machine's frame follows the callback it
is given, so `apply<set_time>` writes what `set_time` writes.

A call with no body to inspect, through a runtime callable value or a boundary
operation, uses the ceiling: everything the callee could write. Unknown is not
an empty write set. Two things keep the loss small. Borrow only the needed
field, `&mut entry.at`, which is the first choice. Or have the requirement
state the end state, `ensures entry.owner == old(entry.owner)`, which gives
the caller its value facts back. Operand evaluation contributes writes too.
See [mutation frames](../spec/language/dependent_values.md#mutation-frames).
Default-domain facts must be restored before consumption; that is not a promise
that the value stayed unchanged.

For example, an insertion helper taking `&mut Entries` cannot mutate an unrelated
hasher merely because both are fields of a larger table. A helper receiving the
whole table needs a checked frame or an explicit preservation guarantee to retain
that precision. Entry/current comparisons use `old(place)` or a real pre-state
binding, not a name introduced by an equality expression.

The same requirements apply on every loop transfer. Here each step establishes
the bound needed by the next:

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

Start with `clear(buffer, buffer.len)`. The nonzero arm proves that the last
remaining element is in bounds. Each cycle writes it, decreases `remaining`,
and establishes the next invocation's requirements.

## When the checker says no

An unproved dependent use rejects with the missing relationship and its subject.
For a descriptor view, that might be the absence of `i < count`, or a missing
alignment or representation fact rather than a numeric bound.

Supply a contract, cite an appropriate proof, use an explicit guard with a handled
false outcome, or call a validator/decoder whose success establishes the required
fact. An `as` qualification uses established evidence; it is not a hidden
validation call. Omega does not insert residual runtime checks to make an
unproved refinement true.

## Indexed domains

A named domain can carry a static index too:

```omega
domain<T, const U: Unit> T::Quantity<U>;
```

A units library can use this to distinguish meters from seconds while retaining
the underlying numeric representation. Conversion between units remains a
library operation. [Indexed domains](../spec/language/domains.md#indexed-families)
and [generic indices](chapter_13_generics.md) give the declaration rules.
