# Chapter 8: Invariant Propagation

Omega should be able to weaken an invariant temporarily, then prove the normal
invariant is restored before control leaves the local relax scope.

```omega
data Player {
    health: i32[range<1, 100>];
}

machine Player::take_damage(
    &mut self,
    amount: i32[range<1, 100>]
) {
    relax self.health {
        self.health -= amount;
        Player::restore_health_range(&mut relaxed self.health);
    }

    transition self.health <= 25 {
        true -> bloodied()
        false -> still_alive()
    }

    state bloodied(&mut self) {
    }

    state still_alive(&mut self) {
    }
}
```

The useful idea is not that `relax` means "anything goes." It means the compiler has a proof debt. If `self.health` normally has `range<1, 100>`, then `self.health -= amount` may temporarily widen the known type to something like `i32[range<-98, 99>]`.

The relax scope must account for that debt before any transition can run:

- `self.health -= amount` weakens the known range.
- `restore_health_range` explicitly accepts the relaxed health value and
  restores the declared range.
- Only after the block ends may control transition to `bloodied` or
  `still_alive`.

This gives Omega a way to model controlled in-place repair without allowing
weakened invariants to leak across the machine graph.
