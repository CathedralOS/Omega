# Chapter 9: Proof obligations

Every operation that could go wrong, such as an addition that could overflow,
an index that could be out of range or a call with a precondition, creates an
obligation. The compiler must be able to discharge it from what is known at
that point. This chapter shows what that looks like in practice.

## An obligation and its proof

```omega
machine increment(value: u32) -> u32
requires value < 100
ensures result <= 100
{
    value + 1
}
```

`value + 1` must fit in a `u32`, and `result <= 100` must hold. Both follow
from `value < 100`. Remove the `requires` and the machine is rejected.

Writing a contract does not make it true. An `ensures` on an empty body is an
error, not an assumption.

## When the compiler cannot see it

If a fact is true but the compiler cannot work it out, you have four choices:

1. **Branch on it.** Test the condition and handle both outcomes.
2. **Require it.** Move the obligation to the caller with `requires`.
3. **Prove it.** Call a theorem machine that establishes it
   ([Chapter 10](chapter_10_compile_time_proofs.md)).
4. **Change the operation.** Use `+!` to trap, `+|` to clamp, and so on
   ([Chapter 5](chapter_5_expressions_evaluation.md#integer-arithmetic)).

Failing to prove something is never treated as proving its opposite.

## Impossible branches

```omega
machine clamp_small(count: u32) -> u32
requires count >= 10
{
    transition count < 5 {
        true -> 0            // cannot happen; the compiler warns
        false -> count
    }
}
```

When a branch contradicts what is already known, the compiler closes it and
warns that it is unreachable.

## Values you saved

```omega
machine restore(items: &mut [u8]) -> u8
requires items.len > 0
ensures result == 42
{
    items[0] = 42;
    let saved: u8 = items[0];
    items[0] = 0;
    items[0] = saved;
    items[0]
}
```

`saved` is a copy, so it still equals 42 after the slot is overwritten, and
writing it back restores the fact about `items[0]`.

## Keeping an invariant

```omega
data Player
requires health in 0..=100
{
    health: i32;
}

machine take_damage(player: &mut Player, damage: i32)
requires damage >= 0
requires damage <= player.health
{
    player.health -= damage;
}
```

On entry `health` is in range. The two requirements show the subtraction
neither overflows nor goes below zero, so the invariant still holds on return.
When an update needs several writes and the invariant is broken in between,
see [Chapter 11](chapter_11_invariant_windows.md).

## Termination

`ensures` describes the result if the machine returns. `terminates` promises
that it does:

```omega
machine countdown(remaining: u32)
terminates by remaining
{
    transition remaining == 0 {
        true -> {}
        false -> countdown(remaining - 1)
    }
}
```

`terminates by remaining` names the quantity that shrinks on every round. A
server loop that is meant to run forever simply leaves the promise off.
Termination says nothing about how long something takes.

## Where next

[Chapter 10](chapter_10_compile_time_proofs.md) covers writing proofs
yourself. The complete rules are in
[proof contracts](../spec/proofs/contracts.md) and
[termination](../spec/language/termination.md).
