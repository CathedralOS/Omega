# Chapter 8: Invariant Propagation

Omega should be able to weaken an invariant temporarily, then prove that each transition either restores the invariant or transfers a narrower proof obligation to the next state.

```omega
owns health: i32[range<1, 100>] = 100;

fn take_damage(amount: i32[range<1, 100>]) {
    relax self.health {
        self.health -= amount;

        let dead: bool = self.health <= 0;
        let bloodied_range: bool = self.health > 25 && amount <= 50;

        transition (dead, bloodied_range) {
            (true, _) -> revive()
            (false, true) -> bloodied(amount)
            (false, false) -> still_alive()
        }
    }
}

state bloodied(amount: i32[range<1, 50>]) {
}

state revive() {
    self.health = 100;
}
```

The useful idea is not that `relax` means "anything goes." It means the compiler has a proof debt. If `self.health` normally has `range<1, 100>`, then `self.health -= amount` may temporarily widen the known type to something like `i32[range<-98, 99>]`.

Each outgoing transition must account for that debt:

- `(true, _) -> revive()` is valid if `revive` re-establishes `self.health: i32[range<1, 100>]`.
- `(false, true) -> bloodied(amount)` is valid only if the matched facts plus the current proof context imply the target argument bounds.
- `(false, false) -> still_alive()` is valid only if the matched facts prove `self.health` is back inside `range<1, 100>` or `still_alive` accepts the weakened invariant.

Tuple transition dispatch makes the partition explicit. In this sketch,
`still_alive` sees `dead == false` and `bloodied_range == false` because those
facts are part of the selected arm.

This gives Omega a way to model controlled damage, recovery, saturation, clamping, retry state, and other real programs without pretending every intermediate instruction preserves every invariant.
