# Chapter 9: Proof obligations

An operation needs facts before it is safe to use. Checking establishes those
facts from types, live program state, contracts, and explicit proofs. It does
not normally insert a runtime check when proof search fails.

[Chapter 7](chapter_7_types_constraints_invariants.md) introduces contracts;
[Chapter 8](chapter_8_domains.md) introduces qualification. This chapter connects
them to the compiler's obligations. The reference is
[proof contracts](../spec/proofs/contracts.md).

## Prove an operation is valid

```omega
machine increment(value: u32) -> u32
requires value < 100
ensures result <= 100
{
    value + 1
}
```

The caller establishes `value < 100`. The body then proves both the Exact
addition's range and the result guarantee. The caller receives that guarantee
only for the successful returned value.

An annotation is a demand, not evidence. Giving an empty body the same
postcondition would not establish it. Stronger theorems can be supplied by
checked machines when automatic reasoning is insufficient; their exact premises
still need to hold at each use.

## Impossible paths

If a machine requires `count >= 10`, a branch that requires `count < 5` cannot
execute for that same value. The checker can prove the conditions contradict
each other and close that path's obligations. It cannot declare a path impossible
just because some other proof failed, or export facts from the impossible path
to a reachable one. The source must still be well-formed.

Such a source-level contradiction produces an unreachable-branch warning by
default, explaining the conflicting conditions. An alternative removed only by
generic specialization or compile-time selection normally does not warn. Neither
the warning nor removal of the dead instructions is needed for the proof to be
valid. See [unreachable branches](../spec/language/state_contracts.md#unreachable-branches).

## Restore a saved value

A copied value can supply a fact after a write:

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

The final assignment restores the saved value to the same slot. For a variable
index, checking must also establish that the index still selects that slot.

## Termination claims

Termination is separate from a postcondition. `ensures` describes a reached
return, not whether a return is eventually reached.

```omega
boundary trait FiniteReader {
    machine read_all(out: &mut Vec<u8>) -> ReadResult
        terminates
        crashes none;
}
```

A checked cycle can prove termination by decreasing a nonnegative counter:

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

The false arm proves the subtraction is valid. Each transfer decreases the
counter, so the cycle eventually reaches zero. More complex algorithms can
choose a ranking view, such as a slice's remaining length. See
[termination and progress](../spec/language/termination.md).

A server's event loop may intentionally run indefinitely. Leave off its
termination promise. A finite task can still need separate response-time or
work bounds; termination alone gives neither a deadline nor a fixed cost.

## Preserve a value constraint

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

At entry, health is established in range and the explicit mutable parameter
supplies exclusive access. A caller writes `take_damage(&mut player, damage)`;
the qualified name supplies no argument. The two requirements establish that
subtraction neither overflows nor takes health below zero. Assignment preserves
the field's domain.

If a multi-write update temporarily breaks a cross-field invariant, an
[invariant window](chapter_11_invariant_windows.md) may defer that invariant's
restoration until the next required consumption point. It cannot defer an
invalid arithmetic operation, out-of-bounds access, or missing loan authority.

Failure to prove a condition is neither its negation nor permission to assume
it. Supply a proof, establish it with an explicit checked branch/validator, use
a deliberately different valid operation contract, or reject the use. A debug
check is not the language's safety proof.

The same discipline applies to floats: proving `Finite` does not authorize
reassociation, signed-zero erasure, or approximate rewrites. Optimization needs
its own observation-preservation proof.
