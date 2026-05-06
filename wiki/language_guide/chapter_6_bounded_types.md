# Chapter 6: Bounded Types

Omega should allow data types to carry proof-friendly refinements.

```omega
state clamp(
    value: i32,
    min: const i32,
    max: const i32
) -> i32[range<min, max>] {
}
```

Owned data may carry invariants:

```omega
owns mass: i32[range<1, 100>];
```

Working interpretation:

- `i32[range<1, 100>]` is an `i32` refined by a range invariant.
- `i32[range<min, max>]` is an `i32` refined by compile-time or proof-visible bounds.
- `const` parameters may be used in type-level constraints.
- The compiler emits proof obligations anywhere a value is assigned, produced, or transitioned into a bounded slot.

The clamp graph produces an obvious proof shape:

```omega
state Clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.ClampLow(min) when value < min;
    -> self.ClampHigh(max) when value > max;
    -> self.ClampDone(value);
}
```

The ordered transitions create a proof partition:

- If `value < min`, `ClampLow(min)` produces `min`.
- If `value > max`, `ClampHigh(max)` produces `max`.
- Otherwise, `ClampDone(value)` is only reachable when `min <= value <= max`.

The order matters. A later transition inherits the fact that earlier transitions did not fire.
