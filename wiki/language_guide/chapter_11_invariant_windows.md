# Chapter 11: Invariant windows

Some updates take more than one write, and the data is invalid in between.
Omega allows that, as long as nothing can see the invalid state.

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
    span.start = span.start + delta;     // start may now exceed end
    span.end = span.end + delta;         // valid again
}
```

With `{ start: 2, end: 5 }` and `delta = 10`, the span is briefly
`{ start: 12, end: 5 }`. That gap is an invariant window. It is permitted
because `shift` holds the span exclusively, and it must be closed before
anyone else relies on the span.

## Close the window before handing the value on

```omega
machine width(span: &Span) -> u32 {
    span.end - span.start
}

machine unfinished_shift(span: &mut Span, delta: u32)
requires span.end <= 1000
requires delta <= 1000
{
    span.start = span.start + delta;
    let width_now: u32 = width(span);    // Rejected: the span may be invalid here.
    span.end = span.end + delta;
}
```

`width` is entitled to a valid `Span`, so the call has to move below the
second write. Returning, including returning an error, also requires the
window to be closed.

A window only excuses the relationship between fields. Each write still has
to be a valid write: no overflow, no out-of-bounds index.

## Or build a new value

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

Nothing is ever invalid this way. Repair in place when the value is large or
must stay where it is.

## Split a repair across helpers

A type's own `requires` always holds when another machine receives the value.
There is no way to pass a `Span` with `start > end` to a helper, even one that
is about to fix it. So decide where a property lives when you declare the
type:

- In the type's `requires` if it holds whenever anyone else can see the value.
- In a named [domain](chapter_8_domains.md) if a helper needs to receive the
  value while the property is false.

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

`finish` receives a plain `Pair`, which any two numbers satisfy, and
establishes `Ordered`. A caller that stores a `Pair in Pair::Ordered` lends it
out as `&mut pair as -Pair::Ordered` and gets it back ordered.

| Guarantee | Must hold |
| --- | --- |
| The type's `requires` | Whenever another machine receives the value. |
| A named domain on stored data | When the data is used under that domain or returned to its owner. |
| A machine's `requires` | On entry. |
| A machine's `ensures` | On return. |

## Calling other code inside a window

A call is allowed while a window is open only if the callee provably cannot
see or disturb the broken data. A helper that takes an unrelated counter is
fine. A helper that takes the half-updated value is not, even if it promises
not to write to it.

## Failure

An error return hands the value back to the caller, so restore the invariant
on that path too. The simplest way is to do everything that can fail before
the first of the coupled writes. See
[Chapter 16](chapter_16_errors_traps_failure.md).

## Where next

[Chapter 12](chapter_12_dependent_types.md) covers fields whose validity
depends on other fields. The complete rules are in
[invariant windows](../spec/language/dependent_values.md#invariant-windows)
and [contract-based repair](../spec/language/dependent_values.md#contract-based-repair).
