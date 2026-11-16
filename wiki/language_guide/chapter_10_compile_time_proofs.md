# Chapter 10: Compile-time proofs

A proof is an ordinary machine: `requires` supplies assumptions and `ensures`
states what follows. Routine facts have a required automatic baseline; harder
facts may need a helper or cited evidence.

```omega
machine distinct_indices(i: u64, j: u64)
requires i < j
ensures i != j
{
}
```

The conclusion follows directly. Changing it to `i == j` breaks the promise.

## Required automatic reasoning

The proof floor includes integer order reasoning, range propagation and bounded
case splitting. For example:

```omega
machine get(items: &[u8], index: u64, limit: u64) -> u8
requires index < limit
requires limit <= items.len
{
    items[index]
}
```

The requirements establish `index < items.len`. No lemma or extra search budget
is needed for that implication. A conforming checker that fails this required
reasoning needs repair; a source workaround is not a language requirement.

The specified `--floor-only` mode identifies obligations relying on automation
beyond the floor. [Required automatic discharge](../spec/proofs/contracts.md#required-automatic-discharge)
defines the exact rules. `REQUIRED-AUTOMATIC-DISCHARGE` in
[TASKS.md](../../TASKS.md) owns delivery; this chapter states the contract, not
a claim that its implementation is complete.

## Using a proof

Call it. The caller establishes the requirements and then may use the guarantees:

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

A fact-only invocation emits no runtime call. This helper states a fact already
within the floor; harder proofs can be split into smaller helpers. A separately
compiled consumer uses published guarantees, not private implementation facts.

## Proof by recursion

A proof can call itself on something smaller, which is induction:

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

`terminates by n` states what shrinks. Proof and compile-time use may admit ranked
non-tail recursion, unlike runtime execution. Each recursive proof use still
owes its progress and invocation requirements.

## Mathematical numbers

Contracts use the same arithmetic as code, including overflow rules. `embed`
projects a machine integer to its mathematical value:

```omega
requires
    embed(left) + embed(right) <= embed(integer::max<i32>())
```

`Nat` and `Int` are mathematical integers. Subtraction on `Nat` still has to stay
nonnegative:

```omega
machine remaining(total: Nat, used: Nat) -> Nat
requires used <= total
{
    total - used
}
```

## Referring to the value on entry

`old(place)` is the value a place held when the machine was entered:

```omega
machine increment(value: &mut u32)
requires value < 1000
ensures value == old(value) + 1
{
    value = value + 1;
}
```

It exists only in contracts and copies nothing at runtime.

## Keep evidence without runtime storage

An `[erased]` field carries evidence without runtime storage:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

Constructing the value requires the evidence. Runtime code cannot branch on the
erased field. `Valid<T>` here denotes the library's evidence type.

## Equality and identity

`==` selects a comparison operation. `core::Identity<T>(a, b)` states that two
subjects are the same value, permitting substitution. For an in-scope carrier
`T` and operation `add`, a law can state:

```omega
machine add_comm(a: T, b: T)
ensures core::Identity<T>(add(a, b), add(b, a));
```

Equality and identity differ for floats: `0.0 == -0.0` is true although the
values are not identical, and a NaN is identical to itself but not `==` to itself.

## When a proof fails

First read the failure category. Unsupported source is an implementation gap,
not a proof that the program is invalid. A missing required floor discharge is
a checker defect. Beyond the floor, an unestablished goal may need a premise,
a branch or a lemma. Exhausted optional search proves neither the goal nor its
negation.

For an unestablished goal, check the statement and available premises, including
whether a write changed the storage the fact describes. Split out a smaller
helper when useful. A counterexample to the available premises does not by
itself establish a reachable execution of the whole program.

These routes are distinct:

| Route | Meaning |
| --- | --- |
| Checked proof | The statement follows from its exact premises. |
| Admitted evaluation | A checked computation produces a value under its evaluation contract. |
| Deferral | The proof contract describes a site-local development waiver, not a reusable fact or package-release acceptance. It is not an automatic response to failed search. |
| Boundary admission | The consumer accepts a named external claim as an assumption; this does not prove the provider obeys it. |

[Proof admission](../spec/proofs/contracts.md#axioms-and-receiving-policy)
owns the deferral and trust rules. Do not use an assumption to bypass a failed
proof or borrow check. Repair the program, contract, proof or checker.
[Chapter 19](chapter_19_capabilities_effects_boundaries.md) explains legitimate
trust boundaries.

## Further

Mathematical libraries include predicates without executable tests, existence
claims and quotients such as real numbers built from Cauchy sequences. See
[mathematical bindings](../spec/proofs/mathematical_bindings.md),
[quotients](../spec/proofs/quotients.md) and the complete
[proof contracts](../spec/proofs/contracts.md).
