# Chapter 8: Invariant Propagation

Omega should be able to weaken an invariant temporarily, then prove that each transition either restores the invariant or transfers a narrower proof obligation to the next state.

```omega
owns health: i32[range<1, 100>] = 100;

fn take_damage(amount: i32[range<1, 100>]) {
    relax self.health {
        self.health -= amount;

        -> revive() when self.health <= 0
        -> bloodied(amount) when self.health > 25 && amount <= 50
        -> still_alive()
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

- `-> revive() when self.health <= 0` is valid if `revive` re-establishes `self.health: i32[range<1, 100>]`.
- `-> bloodied(amount) when self.health > 25 && amount <= 50` is valid only if the guard plus the current proof context implies the target argument bounds.
- `-> still_alive()` is valid only if the remaining ordered-transition context proves `self.health` is back inside `range<1, 100>` or `still_alive` accepts the weakened invariant.

Because transitions are ordered, the final bare transition inherits the negation of earlier guards. In this sketch, `still_alive` sees `self.health > 0` and the negation of the `bloodied` guard if the earlier edges did not fire.

This gives Omega a way to model controlled damage, recovery, saturation, clamping, retry state, and other real programs without pretending every intermediate instruction preserves every invariant.
