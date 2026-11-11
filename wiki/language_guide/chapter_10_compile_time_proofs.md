# Chapter 10: Compile-time proofs

When the compiler cannot work out a fact by itself, you prove it. A proof is
an ordinary machine: `requires` is what it assumes and `ensures` is what
follows.

```omega
machine distinct_indices(i: u64, j: u64)
requires i < j
ensures i != j
{
}
```

The body is empty because the conclusion follows directly. Change the
conclusion to `i == j` and it is rejected.

## Using a proof

Call it. The caller shows the `requires` and then knows the `ensures`:

```omega
machine successor_bound(index: u64, length: u64)
requires index < length
ensures index + 1 <= length
{
}

machine next_index(index: u64, length: u64) -> u64
requires index < length
ensures result <= length
{
    successor_bound(index, length);
    index + 1
}
```

A call like this compiles to nothing. It only adds a fact. Break a hard proof
into small helpers like these, each one a step the compiler can follow.

## Proof by recursion

A proof may call itself on something smaller, which is induction:

```omega
machine countdown(n: u32) -> u32
terminates by n
ensures result == 0
{
    transition n {
        0 -> 0
        _ -> countdown(n - 1)
    }
}
```

`terminates by n` says what shrinks. Unlike runtime code, a proof may recurse
anywhere, not only as its last action.

## Mathematical numbers

Contracts use the same arithmetic as code, including its overflow rules. To
talk about the true mathematical value of a machine integer, `embed` it:

```omega
requires
    embed(left) + embed(right) <= embed(integer::max<i32>())
```

`Nat` and `Int` are unbounded. Subtraction on `Nat` still has to stay
non-negative:

```omega
machine remaining(total: Nat, used: Nat) -> Nat
requires used <= total
{
    total - used
}
```

## Referring to the value on entry

`old(place)` is the value a place had when the machine was entered:

```omega
machine increment(value: &mut u32)
requires value < 1000
ensures value == old(value) + 1
{
    value = value + 1;
}
```

It exists only in contracts and copies nothing at run time.

## Keep evidence without runtime storage

A field marked `[erased]` holds a proof alongside data and takes no space in
the compiled program:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

Building a `Certified<T>` requires the evidence. Runtime code cannot branch on
it, because it is not there at run time.

## Equality and identity

`==` is the type's comparison operation. `core::Identity<T>(a, b)` says the two
are the same value, which is what lets one be substituted for the other in a
proof:

```omega
machine add_comm(a: T, b: T)
ensures core::Identity<T>(add(a, b), add(b, a));
```

They differ for floats: `0.0 == -0.0` is true but the two are not identical,
and a NaN is identical to itself but not `==` to itself.

## When a proof fails

1. Check that the statement is true.
2. Check that what it depends on is in scope and was not invalidated by a
   write.
3. Split out a smaller helper and prove that first.

There are four ways a fact can come to be accepted, and they are not
interchangeable:

| Route | What you get |
| --- | --- |
| Checked proof | A fact that follows from its premises. |
| Evaluation | The result of running a terminating computation at compile time. |
| Deferral | A warning and a waiver at one site. Not reusable, and not allowed in a release. |
| Boundary admission | A claim about foreign code that the consumer chose to trust. |

Do not reach for an assumption to get past a failed proof or a borrow error.
Fix the program, the contract or the proof.
[Chapter 19](chapter_19_capabilities_effects_boundaries.md) explains where
trusted claims legitimately come from.

## Further

Mathematical libraries go beyond this chapter: predicates with no executable
test, existence claims, and quotients such as the real numbers built from
Cauchy sequences. See
[mathematical bindings](../spec/proofs/mathematical_bindings.md) and
[quotients](../spec/proofs/quotients.md). The complete rules for everything
here are in [proof contracts](../spec/proofs/contracts.md).
