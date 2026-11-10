# Chapter 11: Invariant windows

Moving a span forward changes both ends. After the first write, its start may
temporarily exceed its end:

```omega
data Span
requires start <= end
{
    start: u32;
    end: u32;
}

machine shift(span: &mut Span, delta: u32)
requires span.end <= 1000
requires delta <= 1000
{
    span.start = span.start + delta;
    span.end = span.end + delta;
}
```

For `Span { start: 2, end: 5 }` and `delta = 10`, the intermediate contents are
`{ start: 12, end: 5 }`. The second write produces `{ start: 12, end: 15 }`,
which satisfies the invariant again. The preconditions keep both additions
within `u32` range.

The interval between those writes is an **invariant window**. Exclusive access
lets you perform the update while the checker tracks the actual contents and
requires restoration before a use that relies on the invariant.

## Finish the update before handing it off

A machine receiving `&Span` may assume `start <= end`. Calling it halfway
through the update therefore fails:

```omega
machine width(span: &Span) -> u32 {
    span.end - span.start
}

machine unfinished_shift(span: &mut Span, delta: u32)
requires span.end <= 1000
requires delta <= 1000
{
    span.start = span.start + delta;
    let width_now: u32 = width(span); // Rejected: the span may be invalid here.
    span.end = span.end + delta;
}
```

Move the call below the second assignment to make it valid. Returning, handing
back a loan, or transferring the span to another machine also requires a valid
span. A diagnostic points to the write that opened the window and the use that
needs it closed.

Each write must still satisfy its own type, arithmetic and ownership rules.
An invariant window permits a temporary break in the relationship between
fields; it cannot justify an overflowing addition or a dangling reference.

## Split a repair across helpers

A data type's default domain is never left. No cast removes it and no
parameter type asks for the value without it, so every machine that receives a
`Pair` receives a valid one, and a promise to fix it later does not change
that. Where a property lives is therefore a rule, not a preference:

- It belongs in the default domain only if it holds whenever another machine
  can see the value.
- If a helper must be able to receive the value with the property false,
  because that helper is doing the repair, it belongs in a named domain.

Decide this when you declare the type, since moving a property out of the
default domain afterwards changes public signatures. Here `Pair` is
valid with any two `u32` fields, while `Pair::Ordered` adds a relationship
between them:

```omega
data Pair {
    low: u32;
    high: u32;
}

domain Pair::Ordered requires self.low <= self.high;

machine reset(pair: &mut Pair, value: u32)
ensures pair in Pair::Ordered
{
    pair.low = value;
    transition {
        _ -> finish(pair, value)
    }

    machine finish(pair: &mut Pair, value: u32)
    requires pair.low == value
    ensures pair in Pair::Ordered
    {
        pair.high = value;
    }
}
```

`reset` establishes `finish`'s precondition with its first write. `finish`
makes the fields equal and proves `Ordered`; its postcondition fulfills
`reset`'s promise. This remains an in-place update of the same pair.

If the lender stores `Pair in Pair::Ordered`, the loan must return ordered.
Passing a weaker view, written `&mut pair as -Pair::Ordered`, lets helpers perform
the repair, but retains that handback obligation: when the loan ends `Ordered`
is established again for whatever the pair then holds. If `Ordered` were instead part of `Pair`'s default
domain, `finish` would need it on entry as well. Its future postcondition
cannot justify receiving an invalid value.

Choose where a guarantee belongs according to when users may rely on it:

| Guarantee | When it must hold |
| --- | --- |
| Data's default domain | Whenever another machine receives a valid value of that type. |
| Stored named domain | When qualified storage is used under that domain or returned to its owner. |
| `requires` | On entry to that operation. |
| `ensures` | On the operation's matching completed outcome. |

An entry-only fact may expire after mutation. A promised result needs fresh
evidence about the current contents. For a larger repair, helper postconditions
provide that evidence one step at a time.

## Prefer a complete replacement when it is simple

You can also build the new value before assigning it:

```omega
machine shifted(span: &Span, delta: u32) -> Span
requires span.end <= 1000
requires delta <= 1000
{
    Span {
        start: span.start + delta,
        end: span.end + delta,
    }
}
```

This keeps every observable `Span` valid throughout. In-place repair is useful
for large values, stable locations, or updates that cannot cheaply replace
the whole value.

## Borrowing and unrelated work

Exclusive access prevents another observer from relying on the broken coupling.
A live borrow may also pin a field that determines another field's validity;
changing that witness while the borrow exists is a borrow error.

A helper can run during a window when its full contract proves it cannot
observe, expose or interfere with the affected state. A helper taking an
unrelated counter is a possible example. A helper that merely promises to
write nothing is insufficient: it might still read the broken value.

Write-only access must restore validity using supplied values and facts,
because it cannot inspect the old contents. Coupled fields may need to be
replaced together. See [borrowing](chapter_2_ownership_borrowing_moves.md).

## Handle failure before leaving

Returning an error still hands control and borrowed storage back to the caller.
Restore the invariant on that path too. Often the simplest approach is to
validate inputs and acquire resources before starting the coupled writes.

Crashing follows different rules: it abandons the activation without proving
its surviving resources safe. Recovery requires a separate isolation or
owner-death protocol. See [failure and recovery](chapter_16_errors_traps_failure.md).

## Initialize constrained storage

If zero fails a type's default domain, its zeroed storage has no usable value
yet. Construction must establish the invariant before handing out the value.
An unfinished whole cannot be passed to a successor expecting that type.
Carry the independent inputs or storage instead, then finish construction.

An exclusive storage cast can similarly begin initialization of a stricter
shape. Alignment, extent, write permission and exclusive access must already
hold; fill or validate the contents before reading it as the target type.
See the [byte-backed header example](chapter_20_memory_layout_abi.md#initializing-a-stricter-shape-in-byte-backing).

Failed construction cleans up any resource-bearing fields already established.
Repairing an existing value also owes that value's original cleanup and
handback obligations. The [invariant-window reference](../spec/language/dependent_values.md#invariant-windows)
and [contract-based repair rules](../spec/language/dependent_values.md#contract-based-repair)
give the details.
